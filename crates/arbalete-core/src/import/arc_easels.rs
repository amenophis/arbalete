//! Reads Arc's Easels into Arbalete's (decision 0009).
//!
//! Arc keeps its Easels in its cloud, not in its files, but its Firestore client keeps a copy of
//! what it last read (firestore): documents `easels/<id>/objects/<id>`, each with a `position`
//! (`rect`: `x`, `y`, `width`, `height`, fractions of the board's width; `zIndex`) and a
//! `content` holding one of:
//!
//! - `text` (`string`, `style`: `fontFamily`, `paragraphStyle`) or `richText` (`text`, plain);
//! - `shape`: its `type`, `isRectangle`, `isCircle`, an `arrow` (`start`, `end`) or `freehand`
//!   points, points as fractions of the frame, and a `color` of Arc's palette;
//! - `image`, `webCard`, a capture (`url`, `text`, its title, `image`, `favicon`), `link`, a
//!   link's preview (`source`, `title`, `image`), or `webBrowser`, a live page.
//!
//! An image's `content.url` is where Arc downloads it from; its cache of pictures,
//! `EaselImagesCache` in its data directory, a URL cache (`Cache.db`, larger files in
//! `fsCachedData`), may hold it.
//!
//! What Arbalete has no equivalent for is skipped: rich text keeps its text but not its formats, a
//! live page, a placeholder and a picture not cached are left out, and a link shows as a capture
//! of its preview. Only what Arc last read on this Mac is there.

use super::{Copy, firestore};
use crate::easel::{self, Color, Content, Easel, Font, Object, Point, Rect, Shape, TextStyle};
use crate::sidebar::{NodeId, Sidebar};
use rusqlite::{Connection, types::Value as Sql};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::{fs, path::Path, path::PathBuf};

/// An Easel of Arc's, read, with the pictures its objects show, by their name in the Easel.
#[derive(Debug, PartialEq)]
pub struct ArcEasel {
    pub easel: Easel,
    pub images: Vec<(String, Vec<u8>)>,
}

/// The Easels Arc's Firestore client cached, by their Arc id, as Arc's data directory `dir`
/// (`…/Application Support/Arc`) sits next to its copy. What cannot be read is skipped.
pub fn read(dir: &Path) -> HashMap<String, ArcEasel> {
    let Ok(documents) = firestore::documents(dir) else {
        return HashMap::new();
    };
    // A newer copy of an object comes later.
    let mut easels: HashMap<&str, BTreeMap<&str, &Map<String, Value>>> = HashMap::new();
    for document in &documents {
        let segments: Vec<&str> = document.name.split('/').collect();
        if let [.., "easels", easel, "objects", object] = segments[..] {
            easels
                .entry(easel)
                .or_default()
                .insert(object, &document.fields);
        }
    }
    let cache = PictureCache::open(&dir.join("EaselImagesCache"));
    let easels = easels.into_iter().map(|(id, objects)| {
        let mut images: Vec<(String, Vec<u8>)> = Vec::new();
        let mut urls: Vec<String> = Vec::new();
        let mut image = |value: &Value| {
            let url = url(&value["content"]["url"])?;
            if let Some(index) = urls.iter().position(|seen| *seen == url) {
                return Some(images[index].0.clone());
            }
            let data = cache.as_ref()?.get(&url)?;
            let name = format!("picture-{}", images.len() + 1);
            images.push((name.clone(), data));
            urls.push(url);
            Some(name)
        };
        let mut objects: Vec<_> = objects
            .into_values()
            .filter_map(|fields| object(fields, &mut image))
            .collect();
        objects.sort_by(|a, b| a.0.total_cmp(&b.0));
        let objects = objects.into_iter().enumerate();
        let objects = objects.map(|(index, (_, frame, content))| Object {
            id: index as u64 + 1,
            frame,
            content,
        });
        let easel = Easel {
            objects: objects.collect(),
        };
        (id.to_string(), ArcEasel { easel, images })
    });
    easels.collect()
}

/// An object's place, back to front, its frame in points and its content.
fn object(
    fields: &Map<String, Value>,
    image: &mut impl FnMut(&Value) -> Option<String>,
) -> Option<(f64, Rect, Content)> {
    let position = &fields["position"];
    let rect = &position["rect"];
    let point = |key: &str| rect[key].as_f64().map(|value| value * easel::WIDTH);
    let frame = Rect {
        x: point("x")?,
        y: point("y")?,
        width: point("width")?,
        height: point("height")?,
    };
    let z = position["zIndex"].as_f64().unwrap_or(0.0);
    Some((z, frame, content(&fields["content"], image)?))
}

fn content(content: &Value, image: &mut impl FnMut(&Value) -> Option<String>) -> Option<Content> {
    if content["isPlaceholder"] == true {
        return None;
    }
    let some = |key: &str| Some(&content[key]).filter(|value| value.is_object());
    let text = |value: &Value| value.as_str().map(str::to_string);
    if let Some(item) = some("text") {
        let style = &item["style"];
        Some(Content::Text {
            text: text(&item["string"])?,
            font: font(style["fontFamily"].as_str()),
            style: text_style(style["paragraphStyle"].as_str()),
        })
    } else if let Some(item) = some("richText") {
        Some(Content::Text {
            text: text(&item["text"])?,
            font: Font::Clean,
            style: TextStyle::Body,
        })
    } else if let Some(item) = some("shape") {
        shape(item)
    } else if let Some(item) = some("image") {
        Some(Content::Image {
            image: image(item)?,
        })
    } else if let Some(card) = some("webCard") {
        let url = url(&card["url"])?;
        Some(Content::Capture {
            image: image(&card["image"])?,
            title: text(&card["text"]).unwrap_or_else(|| url.clone()),
            favicon: image(&card["favicon"]),
            url,
            page: None,
            live: false,
        })
    } else {
        let link = some("link")?;
        let url = url(&link["source"])?;
        Some(Content::Capture {
            image: image(&link["image"])?,
            title: text(&link["title"]).unwrap_or_else(|| url.clone()),
            favicon: None,
            url,
            page: None,
            live: false,
        })
    }
}

fn shape(shape: &Value) -> Option<Content> {
    let kind = &shape["type"];
    let color = |default| color(shape["color"].as_str()).unwrap_or(default);
    Some(if kind["isRectangle"] == true {
        Content::Shape {
            shape: Shape::Rectangle,
            color: color(Color::LightGreen),
        }
    } else if kind["isCircle"] == true {
        Content::Shape {
            shape: Shape::Ellipse,
            color: color(Color::Red),
        }
    } else if kind["arrow"].is_object() {
        Content::Arrow {
            tail: point(&kind["arrow"]["start"])?,
            head: point(&kind["arrow"]["end"])?,
            color: color(Color::LightBlue),
        }
    } else {
        let points = kind["freehand"].as_array()?.iter().map(point);
        Content::Pen {
            points: points.collect::<Option<_>>()?,
            color: color(Color::Black),
        }
    })
}

/// A point, written as a pair or with `x` and `y`.
fn point(value: &Value) -> Option<Point> {
    let (x, y) = match value.as_array().map(Vec::as_slice) {
        Some([x, y]) => (x, y),
        _ => (&value["x"], &value["y"]),
    };
    Some(Point {
        x: x.as_f64()?,
        y: y.as_f64()?,
    })
}

/// An address, written as text, or as Swift writes a URL, its `relative` part.
fn url(value: &Value) -> Option<String> {
    let text = value.as_str().or(value["relative"].as_str())?;
    Some(text.to_string())
}

fn font(family: Option<&str>) -> Font {
    match family {
        Some("hip") => Font::Hip,
        Some("mono") => Font::Mono,
        Some("fancy") => Font::Fancy,
        _ => Font::Clean,
    }
}

fn text_style(style: Option<&str>) -> TextStyle {
    match style {
        Some("ultra") => TextStyle::Ultra,
        Some("h0") => TextStyle::H0,
        Some("h1") => TextStyle::H1,
        Some("h2") => TextStyle::H2,
        _ => TextStyle::Body,
    }
}

fn color(name: Option<&str>) -> Option<Color> {
    Some(match name? {
        "black" => Color::Black,
        "white" => Color::White,
        "gray" => Color::Grey,
        "pink" => Color::Pink,
        "red" => Color::Red,
        "yellow" => Color::Yellow,
        "lightGreen" => Color::LightGreen,
        "darkGreen" => Color::DarkGreen,
        "lightBlue" => Color::LightBlue,
        "darkBlue" => Color::DarkBlue,
        "purple" => Color::Purple,
        _ => return None,
    })
}

/// Arc's cache of pictures, a copy of its URL cache's database.
struct PictureCache {
    dir: PathBuf,
    db: Connection,
    _copy: Copy,
}

impl PictureCache {
    fn open(dir: &Path) -> Option<Self> {
        let copy = Copy::of(&dir.join("Cache.db")).ok()??;
        let db = Connection::open(copy.file()).ok()?;
        Some(Self {
            dir: dir.to_path_buf(),
            db,
            _copy: copy,
        })
    }

    /// The picture cached for `url`, or for the same address with another query: Firebase's
    /// links carry a token that may change.
    fn get(&self, url: &str) -> Option<Vec<u8>> {
        let path = url.split('?').next()?;
        let like = path
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
            + "?%";
        let (on_disk, data): (i64, Sql) = self
            .db
            .query_row(
                "SELECT d.isDataOnFS, d.receiver_data FROM cfurl_cache_response r
                 JOIN cfurl_cache_receiver_data d ON d.entry_ID = r.entry_ID
                 WHERE r.request_key = ?1 OR r.request_key = ?2 OR r.request_key LIKE ?3 ESCAPE '\\'
                 ORDER BY r.request_key = ?1 DESC, r.time_stamp DESC LIMIT 1",
                (url, path, like),
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok()?;
        let data = match data {
            Sql::Blob(data) => data,
            Sql::Text(text) => text.into_bytes(),
            _ => return None,
        };
        if on_disk == 0 {
            return Some(data);
        }
        let name = String::from_utf8(data).ok()?;
        let name = Path::new(&name).file_name()?;
        fs::read(self.dir.join("fsCachedData").join(name)).ok()
    }
}

/// Arc's Easels, as they take their place in the sidebar: each becomes a tab at the address of a
/// new Easel, numbered from `next`.
#[derive(Debug, Default)]
pub struct Import {
    /// Those not yet in the sidebar, by their Arc id.
    pub found: HashMap<String, ArcEasel>,
    pub next: u64,
    /// Those in the sidebar, by their number.
    pub added: Vec<(u64, ArcEasel)>,
}

impl Import {
    /// Adds the Easel Arc names `id`, if found, at the end of `parent`, titled `title`.
    pub fn add(&mut self, sidebar: &mut Sidebar, parent: NodeId, id: &str, title: Option<&str>) {
        let Some(easel) = self.found.remove(id) else {
            return;
        };
        let title = title.filter(|t| !t.trim().is_empty()).unwrap_or(UNTITLED);
        // Only full favorites refuse it.
        if sidebar
            .add_tab(parent, &easel::url(self.next), title)
            .is_ok()
        {
            self.added.push((self.next, easel));
            self.next += 1;
        }
    }
}

/// Like Arc's, an Easel never named.
const UNTITLED: &str = "Untitled Easel";

/// Saves `easel` as the Easel in `dir`, with its pictures.
pub fn write(easel: &ArcEasel, dir: &Path) -> Result<(), easel::Error> {
    let images = easel::images(dir);
    fs::create_dir_all(&images)?;
    for (name, data) in &easel.images {
        fs::write(images.join(name), data)?;
    }
    easel::save(&easel.easel, dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{TestDir, firestore::fixture};
    use serde_json::json;

    /// Arc's URL cache of pictures in `support/Arc/EaselImagesCache`: `url` held in the database,
    /// `other` in a file of its own.
    fn picture_cache(support: &TestDir, url: &str, other: &str) {
        support.write("Arc/EaselImagesCache/fsCachedData/F00D", "on disk");
        let db = Connection::open(support.0.join("Arc/EaselImagesCache/Cache.db")).unwrap();
        db.execute_batch(
            "CREATE TABLE cfurl_cache_response (entry_ID INTEGER PRIMARY KEY, version INTEGER,
                hash_value INTEGER, storage_policy INTEGER, request_key TEXT UNIQUE,
                time_stamp TEXT, partition TEXT);
             CREATE TABLE cfurl_cache_receiver_data (entry_ID INTEGER PRIMARY KEY,
                isDataOnFS INTEGER, receiver_data BLOB);",
        )
        .unwrap();
        let entries = [
            (1, url, 0, b"in db".to_vec()),
            (2, other, 1, b"F00D".to_vec()),
        ];
        for (id, key, on_disk, data) in entries {
            db.execute(
                "INSERT INTO cfurl_cache_response (entry_ID, request_key, time_stamp)
                 VALUES (?1, ?2, '2024-01-01')",
                (id, key),
            )
            .unwrap();
            db.execute(
                "INSERT INTO cfurl_cache_receiver_data VALUES (?1, ?2, ?3)",
                (id, on_disk, data),
            )
            .unwrap();
        }
    }

    #[test]
    fn reads_the_easels_arcs_firestore_client_cached() {
        let support = TestDir::new("arc-easels-cached");
        let shot =
            "https://firebasestorage.googleapis.com/v0/b/arc/o/easels%2Fshot?alt=media&token=1";
        let icon =
            "https://firebasestorage.googleapis.com/v0/b/arc/o/easels%2Ficon?alt=media&token=2";
        // The token changed since Arc cached the shot.
        picture_cache(&support, &shot.replace("token=1", "token=0"), icon);
        let picture = |url: &str| json!({ "id": { "value": "i" }, "content": { "url": url } });
        let object = |z: f64, content: Value| {
            let rect = json!({ "x": 0.1, "y": 0.1, "width": 0.2, "height": 0.1 });
            json!({ "position": { "rect": rect, "zIndex": z }, "content": content })
        };
        let name = |object: &str| {
            format!(
                "projects/bcny-arc-server/databases/(default)/documents/easels/e1/objects/{object}"
            )
        };
        let documents = [
            (
                name("a"),
                object(5.0, json!({ "text": { "string": "Old" } })),
            ),
            (
                name("b"),
                object(
                    1.0,
                    json!({ "shape": { "type": { "isRectangle": true }, "color": "gray" } }),
                ),
            ),
            (
                name("c"),
                object(
                    2.0,
                    json!({ "shape": { "type": { "arrow": { "start": [0.0, 1.0], "end": [1.0, 0.0] } } } }),
                ),
            ),
            (
                name("d"),
                object(
                    3.0,
                    json!({ "shape": { "type": { "freehand": [{ "x": 0.5, "y": 0.5 }] }, "color": "purple" } }),
                ),
            ),
            (
                name("e"),
                object(
                    4.0,
                    json!({ "webCard": { "url": "https://example.com/", "text": "Example",
                        "image": picture(shot), "favicon": picture(icon) } }),
                ),
            ),
            (name("f"), object(6.0, json!({ "image": picture(shot) }))),
            (
                name("g"),
                object(
                    7.0,
                    json!({ "link": { "source": { "relative": "https://video.example/" },
                        "image": picture(icon) } }),
                ),
            ),
            // Skipped: a live page, a picture not cached, a placeholder.
            (
                name("h"),
                object(
                    8.0,
                    json!({ "webBrowser": { "url": "https://example.com/" } }),
                ),
            ),
            (
                name("i"),
                object(9.0, json!({ "image": picture("https://lost.example/p") })),
            ),
            (
                name("j"),
                object(
                    10.0,
                    json!({ "richText": { "text": "Wait" }, "isPlaceholder": true }),
                ),
            ),
            (
                name("k"),
                object(11.0, json!({ "richText": { "text": "Notes" } })),
            ),
            // A newer copy of the text.
            (
                name("a"),
                object(
                    5.0,
                    json!({ "text": { "string": "Hello",
                        "style": { "fontFamily": "hip", "paragraphStyle": "h2" } } }),
                ),
            ),
            (
                "projects/bcny-arc-server/databases/(default)/documents/users/u/boosts/b".into(),
                json!({ "title": "Boost" }),
            ),
        ];
        let documents: Vec<_> = documents
            .iter()
            .map(|(name, fields)| (name.as_str(), fields.clone()))
            .collect();
        fixture::write(&support.0, &documents);
        let mut read = read(&support.0.join("Arc"));
        assert_eq!(read.len(), 1);
        let arc = read.remove("e1").unwrap();
        let contents: Vec<_> = arc
            .easel
            .objects
            .iter()
            .map(|o| o.content.clone())
            .collect();
        let capture =
            |image: &str, url: &str, title: &str, favicon: Option<&str>| Content::Capture {
                image: image.into(),
                url: url.into(),
                title: title.into(),
                favicon: favicon.map(Into::into),
                page: None,
                live: false,
            };
        let text = |text: &str, font, style| Content::Text {
            text: text.into(),
            font,
            style,
        };
        assert_eq!(
            contents,
            [
                Content::Shape {
                    shape: Shape::Rectangle,
                    color: Color::Grey
                },
                Content::Arrow {
                    tail: Point { x: 0.0, y: 1.0 },
                    head: Point { x: 1.0, y: 0.0 },
                    color: Color::LightBlue
                },
                Content::Pen {
                    points: vec![Point { x: 0.5, y: 0.5 }],
                    color: Color::Purple
                },
                capture(
                    "picture-1",
                    "https://example.com/",
                    "Example",
                    Some("picture-2")
                ),
                text("Hello", Font::Hip, TextStyle::H2),
                Content::Image {
                    image: "picture-1".into()
                },
                capture(
                    "picture-2",
                    "https://video.example/",
                    "https://video.example/",
                    None
                ),
                text("Notes", Font::Clean, TextStyle::Body),
            ]
        );
        let ids: Vec<_> = arc.easel.objects.iter().map(|o| o.id).collect();
        assert_eq!(ids, [1, 2, 3, 4, 5, 6, 7, 8]);
        let frame = Rect {
            x: 100.0,
            y: 100.0,
            width: 200.0,
            height: 100.0,
        };
        assert_eq!(arc.easel.objects[0].frame, frame);
        assert_eq!(
            arc.images,
            [
                ("picture-1".to_string(), b"in db".to_vec()),
                ("picture-2".to_string(), b"on disk".to_vec())
            ]
        );
    }

    #[test]
    fn reads_no_easels_without_a_firestore_copy() {
        let support = TestDir::new("arc-easels-none");
        assert!(read(&support.0.join("Arc")).is_empty());
    }

    #[test]
    fn writes_an_easel_with_its_pictures() {
        let dir = TestDir::new("arc-easels-write");
        let arc = ArcEasel {
            easel: Easel {
                objects: vec![Object {
                    id: 1,
                    frame: Rect::default(),
                    content: Content::Image {
                        image: "picture-1".into(),
                    },
                }],
            },
            images: vec![("picture-1".into(), b"png".to_vec())],
        };
        write(&arc, &dir.0).unwrap();
        assert_eq!(easel::load(&dir.0).unwrap(), arc.easel);
        let picture = fs::read(easel::images(&dir.0).join("picture-1")).unwrap();
        assert_eq!(picture, b"png");
    }
}
