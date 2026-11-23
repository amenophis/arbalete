//! Reads a Chromium browser profile's cookies, decrypted, to keep its sites signed in
//! (decision 0012). The app writes them through CEF's cookie manager.

use super::Copy;
use super::safe_storage::Key;
use rusqlite::{Connection, Row};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;

/// A cookie as the browser kept it. Times are microseconds since 1601, as in Chromium and CEF.
#[derive(Clone, PartialEq, Eq)]
pub struct Cookie {
    /// With a leading `.` for a domain cookie, without for a host-only one.
    pub host: String,
    pub name: String,
    pub value: String,
    pub path: String,
    pub creation: i64,
    pub last_access: i64,
    /// `None` for a session cookie.
    pub expires: Option<i64>,
    pub secure: bool,
    pub http_only: bool,
    /// -1 unspecified, 0 none, 1 lax, 2 strict.
    pub same_site: i64,
    /// 0 low, 1 medium, 2 high.
    pub priority: i64,
}

/// Never the name or the value: they are sign-ins.
impl fmt::Debug for Cookie {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Cookie({}{})", self.host, self.path)
    }
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    /// Not one cookie could be decrypted with the key given: it is not the browser's.
    Key,
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Error::Sqlite(error)
    }
}

/// The cookies of the profile that have not expired at `now`, but partitioned ones, which CEF
/// cannot set as such. A profile that has never been opened has none.
pub fn read(profile: &Path, key: &Key, now: i64) -> Result<Vec<Cookie>, Error> {
    let file = ["Network/Cookies", "Cookies"]
        .map(|file| profile.join(file))
        .into_iter()
        .find(|file| file.exists());
    let Some(file) = file else {
        return Ok(Vec::new());
    };
    let Some(copy) = Copy::of(&file).map_err(Error::Io)? else {
        return Ok(Vec::new());
    };
    let db = Connection::open(copy.file()).map_err(Error::Sqlite)?;
    cookies(&db, key, now)
}

fn cookies(db: &Connection, key: &Key, now: i64) -> Result<Vec<Cookie>, Error> {
    let version: i64 = db
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'version'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_default();
    let mut query = db.prepare("SELECT * FROM cookies").map_err(Error::Sqlite)?;
    let mut rows = query.query([]).map_err(Error::Sqlite)?;
    let (mut cookies, mut undecrypted) = (Vec::new(), 0);
    while let Some(row) = rows.next().map_err(Error::Sqlite)? {
        let partitioned = !row
            .get::<_, String>("top_frame_site_key")
            .unwrap_or_default()
            .is_empty();
        match cookie(row, key, version).map_err(Error::Sqlite)? {
            Some(cookie) if cookie.expires.is_none_or(|e| e > now) && !partitioned => {
                cookies.push(cookie)
            }
            Some(_) => {}
            None => undecrypted += 1,
        }
    }
    if cookies.is_empty() && undecrypted > 0 {
        return Err(Error::Key);
    }
    Ok(cookies)
}

/// A row of `cookies`, `None` when its value cannot be decrypted. Older versions of the table lack
/// some columns: only those read must exist.
fn cookie(row: &Row, key: &Key, version: i64) -> rusqlite::Result<Option<Cookie>> {
    let host: String = row.get("host_key")?;
    let mut value: String = row.get("value")?;
    let encrypted: Vec<u8> = row.get("encrypted_value")?;
    if !encrypted.is_empty() {
        let Some(clear) = key.decrypt(&encrypted) else {
            return Ok(None);
        };
        // From version 24, the value follows the SHA-256 of its host.
        let clear = match version >= 24 {
            true => match clear.split_at_checked(32) {
                Some((hash, clear)) if hash == Sha256::digest(host.as_bytes()).as_slice() => {
                    clear.to_vec()
                }
                _ => return Ok(None),
            },
            false => clear,
        };
        let Ok(clear) = String::from_utf8(clear) else {
            return Ok(None);
        };
        value = clear;
    }
    let expires: i64 = row.get("expires_utc")?;
    let has_expires: bool = row.get("has_expires").unwrap_or(expires != 0);
    Ok(Some(Cookie {
        host,
        name: row.get("name")?,
        value,
        path: row.get("path")?,
        creation: row.get("creation_utc")?,
        last_access: row.get("last_access_utc").unwrap_or_default(),
        expires: has_expires.then_some(expires),
        secure: row.get("is_secure")?,
        http_only: row.get("is_httponly")?,
        same_site: row.get("samesite").unwrap_or(-1),
        priority: row.get("priority").unwrap_or(1),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::TestDir;

    /// The part of Chromium's `Cookies` schema the import reads, version 24.
    const SCHEMA: &str = "
        CREATE TABLE meta (key LONGVARCHAR NOT NULL UNIQUE PRIMARY KEY, value LONGVARCHAR);
        INSERT INTO meta VALUES ('version', '24');
        CREATE TABLE cookies (creation_utc INTEGER NOT NULL, host_key TEXT NOT NULL,
            top_frame_site_key TEXT NOT NULL, name TEXT NOT NULL, value TEXT NOT NULL,
            encrypted_value BLOB NOT NULL, path TEXT NOT NULL, expires_utc INTEGER NOT NULL,
            is_secure INTEGER NOT NULL, is_httponly INTEGER NOT NULL,
            last_access_utc INTEGER NOT NULL, has_expires INTEGER NOT NULL DEFAULT 1,
            is_persistent INTEGER NOT NULL DEFAULT 1, priority INTEGER NOT NULL DEFAULT 1,
            samesite INTEGER NOT NULL DEFAULT -1);
    ";

    const NOW: i64 = 13_400_000_000_000_000;

    fn insert(db: &Connection, key: &Key, host: &str, name: &str, expires: i64, partition: &str) {
        let clear = [Sha256::digest(host.as_bytes()).as_slice(), b"secret"].concat();
        db.execute(
            "INSERT INTO cookies VALUES (1, ?1, ?2, ?3, '', ?4, '/', ?5, 1, 1, 2, ?6, 1, 2, 1)",
            rusqlite::params![
                host,
                partition,
                name,
                key.encrypt(&clear),
                expires,
                expires != 0
            ],
        )
        .unwrap();
    }

    #[test]
    fn reads_the_cookies_that_have_not_expired() {
        let dir = TestDir::new("cookies");
        let key = Key::new(b"password");
        assert!(read(&dir.0, &key, NOW).unwrap().is_empty());
        dir.dir("Network");
        let db = Connection::open(dir.0.join("Network/Cookies")).unwrap();
        db.execute_batch(SCHEMA).unwrap();
        insert(&db, &key, ".example.com", "sid", NOW + 1, "");
        insert(&db, &key, "mail.example.com", "session", 0, "");
        insert(&db, &key, ".old.example", "gone", NOW - 1, "");
        insert(
            &db,
            &key,
            ".embed.example",
            "chips",
            NOW + 1,
            "https://site.example",
        );
        let cookies = read(&dir.0, &key, NOW).unwrap();
        assert_eq!(
            cookies,
            [
                Cookie {
                    host: ".example.com".to_string(),
                    name: "sid".to_string(),
                    value: "secret".to_string(),
                    path: "/".to_string(),
                    creation: 1,
                    last_access: 2,
                    expires: Some(NOW + 1),
                    secure: true,
                    http_only: true,
                    same_site: 1,
                    priority: 2,
                },
                Cookie {
                    host: "mail.example.com".to_string(),
                    name: "session".to_string(),
                    value: "secret".to_string(),
                    path: "/".to_string(),
                    creation: 1,
                    last_access: 2,
                    expires: None,
                    secure: true,
                    http_only: true,
                    same_site: 1,
                    priority: 2,
                },
            ]
        );
        assert_eq!(format!("{:?}", cookies[0]), "Cookie(.example.com/)");
        assert!(matches!(
            read(&dir.0, &Key::new(b"other"), NOW),
            Err(Error::Key)
        ));
    }

    #[test]
    fn reads_an_older_table_without_the_host_hash() {
        let dir = TestDir::new("cookies-old");
        let key = Key::new(b"password");
        let db = Connection::open(dir.0.join("Cookies")).unwrap();
        db.execute_batch(
            "CREATE TABLE meta (key TEXT, value TEXT);
             INSERT INTO meta VALUES ('version', '20');
             CREATE TABLE cookies (creation_utc INTEGER, host_key TEXT, name TEXT, value TEXT,
                encrypted_value BLOB, path TEXT, expires_utc INTEGER, is_secure INTEGER,
                is_httponly INTEGER);",
        )
        .unwrap();
        db.execute(
            "INSERT INTO cookies VALUES (1, 'a.example', 'n', '', ?1, '/', 0, 0, 0),
                (1, 'b.example', 'clear', 'text', x'', '/', 0, 0, 0)",
            [key.encrypt(b"v")],
        )
        .unwrap();
        let cookies = read(&dir.0, &key, NOW).unwrap();
        let values: Vec<_> = cookies
            .iter()
            .map(|c| (c.value.as_str(), c.same_site, c.expires))
            .collect();
        assert_eq!(values, [("v", -1, None), ("text", -1, None)]);
    }
}
