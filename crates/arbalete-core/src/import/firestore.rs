//! Reads the copy Arc's Firestore client keeps of what it read from Arc's servers, where its
//! Boosts (arc_boosts) and its Easels (arc_easels) live.
//!
//! It is a LevelDB database in `~/Library/Application Support/firestore/<app>/bcny-arc-server/
//! main`, next to Arc's own directory. Each value is a cached document (its `MaybeDocument`
//! message, protocol buffers), whose field 2 is the document: its name (1), its path, and its
//! fields (2), entries of a name (1) and a value (2). A value is one of: a boolean (1), an integer
//! (2), a double (3), a reference (5), a map (6, entries as for fields), an array (9, values in
//! 1), a timestamp (10, seconds in 1), null (11), a string (17) or bytes (18).

use rusty_leveldb::{DB, LdbIterator, Options};
use serde_json::{Map, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::{env, fs, io, path::Path, process};

const PROJECT: &str = "bcny-arc-server";

/// A cached document: its path, and its fields as JSON.
#[derive(Debug, PartialEq)]
pub struct Document {
    pub name: String,
    pub fields: Map<String, Value>,
}

impl Document {
    /// The collection holding the document, and its id.
    pub fn place(&self) -> Option<(&str, &str)> {
        let mut segments = self.name.rsplit('/');
        let id = segments.next()?;
        Some((segments.next()?, id))
    }
}

/// The documents Arc's Firestore client cached, as Arc's data directory `dir`
/// (`…/Application Support/Arc`) sits next to its copy, in the order of the database; none when
/// there is no copy.
pub fn documents(dir: &Path) -> io::Result<Vec<Document>> {
    let Some(firestore) = dir.parent().map(|support| support.join("firestore")) else {
        return Ok(Vec::new());
    };
    let apps = match fs::read_dir(&firestore) {
        Ok(apps) => apps,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut documents = Vec::new();
    for app in apps {
        let database = app?.path().join(PROJECT).join("main");
        if database.is_dir() {
            documents.extend(read_database(&database)?);
        }
    }
    Ok(documents)
}

/// Reads a copy of the database: opening it writes to it, and Arc may have it open.
fn read_database(database: &Path) -> io::Result<Vec<Document>> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let copy = env::temp_dir().join(format!("arbalete-firestore-{}-{n}", process::id()));
    fs::create_dir_all(&copy)?;
    let read = copy_files(database, &copy).and_then(|()| read_documents(&copy));
    let _ = fs::remove_dir_all(&copy);
    read
}

fn copy_files(from: &Path, to: &Path) -> io::Result<()> {
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        if entry.file_type()?.is_file() && entry.file_name() != "LOCK" {
            fs::copy(entry.path(), to.join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn read_documents(database: &Path) -> io::Result<Vec<Document>> {
    let options = Options {
        create_if_missing: false,
        ..Options::default()
    };
    let mut db = DB::open(database, options).map_err(io::Error::other)?;
    let mut iter = db.new_iter().map_err(io::Error::other)?;
    let mut documents = Vec::new();
    while iter.advance() {
        if let Some(document) = iter.current().and_then(|(_, value)| document(&value)) {
            documents.push(document);
        }
    }
    Ok(documents)
}

/// A cached document; `None` for the database's other entries.
fn document(value: &[u8]) -> Option<Document> {
    let document = message(value)?
        .into_iter()
        .find_map(|(field, wire)| (field == 2).then_some(wire)?.bytes())?;
    let mut name = None;
    let mut fields = Map::new();
    for (field, wire) in message(document)? {
        match (field, wire.bytes()) {
            (1, Some(bytes)) => name = String::from_utf8(bytes.to_vec()).ok(),
            (2, Some(entry)) => {
                if let Some((key, value)) = map_entry(entry) {
                    fields.insert(key, value);
                }
            }
            _ => {}
        }
    }
    Some(Document {
        name: name?,
        fields,
    })
}

fn map_entry(entry: &[u8]) -> Option<(String, Value)> {
    let (mut key, mut value) = (None, Value::Null);
    for (field, wire) in message(entry)? {
        match (field, wire.bytes()) {
            (1, Some(bytes)) => key = String::from_utf8(bytes.to_vec()).ok(),
            (2, Some(bytes)) => value = to_json(bytes),
            _ => {}
        }
    }
    Some((key?, value))
}

/// A Firestore value as JSON: a timestamp as its seconds, bytes and the rest as null.
fn to_json(value: &[u8]) -> Value {
    let Some(fields) = message(value) else {
        return Value::Null;
    };
    let Some((field, wire)) = fields.into_iter().next() else {
        return Value::Null;
    };
    match (field, wire) {
        (1, Wire::Varint(n)) => Value::Bool(n != 0),
        (2, Wire::Varint(n)) => Value::from(n as i64),
        (3, Wire::Fixed64(bits)) => Value::from(f64::from_bits(bits)),
        (5 | 17, Wire::Bytes(bytes)) => {
            String::from_utf8(bytes.to_vec()).map_or(Value::Null, Value::String)
        }
        (6, Wire::Bytes(map)) => {
            let entries = message(map).into_iter().flatten();
            let entries = entries
                .filter_map(|(field, wire)| map_entry((field == 1).then_some(wire)?.bytes()?));
            Value::Object(entries.collect())
        }
        (9, Wire::Bytes(array)) => {
            let values = message(array).into_iter().flatten();
            let values = values
                .filter_map(|(field, wire)| Some(to_json((field == 1).then_some(wire)?.bytes()?)));
            Value::Array(values.collect())
        }
        (10, Wire::Bytes(timestamp)) => {
            let seconds =
                message(timestamp)
                    .into_iter()
                    .flatten()
                    .find_map(|(field, wire)| match (field, wire) {
                        (1, Wire::Varint(n)) => Some(n as i64),
                        _ => None,
                    });
            Value::from(seconds.unwrap_or_default())
        }
        _ => Value::Null,
    }
}

/// A protocol buffers field's value, as written.
enum Wire<'a> {
    Varint(u64),
    Fixed64(u64),
    Fixed32,
    Bytes(&'a [u8]),
}

impl<'a> Wire<'a> {
    fn bytes(self) -> Option<&'a [u8]> {
        match self {
            Wire::Bytes(bytes) => Some(bytes),
            _ => None,
        }
    }
}

/// The fields of a protocol buffers message, by number; `None` when it is not one.
fn message(mut bytes: &[u8]) -> Option<Vec<(u64, Wire<'_>)>> {
    let mut fields = Vec::new();
    while !bytes.is_empty() {
        let key = varint(&mut bytes)?;
        let wire = match key & 7 {
            0 => Wire::Varint(varint(&mut bytes)?),
            1 => {
                let (value, rest) = bytes.split_first_chunk::<8>()?;
                bytes = rest;
                Wire::Fixed64(u64::from_le_bytes(*value))
            }
            5 => {
                bytes = bytes.get(4..)?;
                Wire::Fixed32
            }
            2 => {
                let size = usize::try_from(varint(&mut bytes)?).ok()?;
                let value = bytes.get(..size)?;
                bytes = &bytes[size..];
                Wire::Bytes(value)
            }
            _ => return None,
        };
        fields.push((key >> 3, wire));
    }
    Some(fields)
}

fn varint(bytes: &mut &[u8]) -> Option<u64> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let (&byte, rest) = bytes.split_first()?;
        *bytes = rest;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

/// Writes cached documents as Arc's Firestore client does, for tests.
#[cfg(test)]
pub(crate) mod fixture {
    use rusty_leveldb::{DB, Options};
    use serde_json::Value;
    use std::path::Path;

    fn varint(mut n: u64, out: &mut Vec<u8>) {
        while n >= 0x80 {
            out.push(n as u8 | 0x80);
            n >>= 7;
        }
        out.push(n as u8);
    }

    /// A protocol buffers field holding bytes.
    pub fn field(number: u64, bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        varint(number << 3 | 2, &mut out);
        varint(bytes.len() as u64, &mut out);
        out.extend_from_slice(bytes);
        out
    }

    fn entry(key: &str, value: &Value) -> Vec<u8> {
        [field(1, key.as_bytes()), field(2, &self::value(value))].concat()
    }

    /// A Firestore value from JSON: integers as doubles but for whole ones.
    pub fn value(value: &Value) -> Vec<u8> {
        match value {
            Value::Null => vec![11 << 3, 0],
            Value::Bool(b) => vec![1 << 3, u8::from(*b)],
            Value::Number(n) => match n.as_i64() {
                Some(i) => {
                    let mut out = vec![2 << 3];
                    varint(i as u64, &mut out);
                    out
                }
                None => [vec![3 << 3 | 1], n.as_f64().unwrap().to_le_bytes().to_vec()].concat(),
            },
            Value::String(s) => field(17, s.as_bytes()),
            Value::Array(values) => {
                let values: Vec<_> = values.iter().map(|v| field(1, &self::value(v))).collect();
                field(9, &values.concat())
            }
            Value::Object(map) => {
                let entries: Vec<_> = map.iter().map(|(k, v)| field(1, &entry(k, v))).collect();
                field(6, &entries.concat())
            }
        }
    }

    /// A cached document, as its `MaybeDocument` message.
    pub fn document(name: &str, fields: &Value) -> Vec<u8> {
        let mut document = field(1, name.as_bytes());
        for (key, value) in fields.as_object().unwrap() {
            document.extend(field(2, &entry(key, value)));
        }
        // A number field, the update time, as Firestore writes one after the fields.
        document.extend([0x22, 0x02, 0x08, 0x01]);
        [field(2, &document), vec![0x20, 0x01]].concat()
    }

    /// Writes `documents` into the Firestore copy next to Arc's directory `support/Arc`.
    pub fn write(support: &Path, documents: &[(&str, Value)]) {
        let database = support.join("firestore/Arc/bcny-arc-server/main");
        std::fs::create_dir_all(&database).unwrap();
        let options = Options {
            compressor: 1,
            ..Options::default()
        };
        let mut db = DB::open(&database, options).unwrap();
        for (n, (name, fields)) in documents.iter().enumerate() {
            let key = format!("remote_document/{n:04}");
            db.put(key.as_bytes(), &document(name, fields)).unwrap();
        }
        db.put(b"target/1", b"\x0a\x03abc").unwrap();
        db.flush().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::TestDir;
    use serde_json::json;

    #[test]
    fn reads_every_kind_of_value() {
        let support = TestDir::new("arc-firestore");
        let fields = json!({
            "flag": true, "count": 3, "ratio": 0.25, "name": "Easel", "nothing": null,
            "list": [1, "two", { "x": 0.5 }], "nested": { "inner": { "deep": "yes" } },
        });
        let name = "projects/bcny-arc-server/databases/(default)/documents/easels/e1";
        fixture::write(&support.0, &[(name, fields.clone())]);
        let documents = documents(&support.0.join("Arc")).unwrap();
        assert_eq!(documents.len(), 1);
        assert_eq!(Value::Object(documents[0].fields.clone()), fields);
        assert_eq!(documents[0].place(), Some(("easels", "e1")));
    }

    #[test]
    fn reads_no_documents_without_a_copy() {
        let support = TestDir::new("arc-no-firestore");
        assert!(documents(&support.0.join("Arc")).unwrap().is_empty());
    }
}
