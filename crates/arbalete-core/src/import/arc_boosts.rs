//! Reads Arc's Boosts.
//!
//! Arc keeps them in its Firebase project, whose copy its Firestore client keeps on the Mac
//! (firestore), as documents ending in `/boosts/<id>`, Arc's fields being all strings:
//!
//! - `hostPattern`: the site; `title`; `isActive`: `true` or `false`;
//! - `data`: JSON, with `css`, `js`, `font` (`family`), `textTransform` (`capitalize`,
//!   `uppercase`, `lowercase`), `textZoom` (`level`, a percentage: 90, 110, 125 or 150), `zaps`
//!   (selectors) and `colorScheme`, either `themeDriven` or `colorWheelDriven` (`_0`:
//!   `primaryHueDegrees`, `primarySaturation`, `invertLightness` and, in `advancedSettings`,
//!   `preservedSaturation`).
//!
//! The rest has no equivalent in Arbalete and is skipped: the colors following the Space's theme,
//! the second hue and the other advanced color settings, and what is only for sharing (author,
//! signature, the Boosts installed from a link).

use super::firestore;
use crate::boosts::{self, Boost, Colors, TextCase, TextSize};
use serde_json::Value;
use std::collections::HashMap;
use std::{io, path::Path};

/// Arc's Boosts, as Arc's data directory `dir` (`…/Application Support/Arc`) sits next to its
/// Firestore copy, in the order of the database; none when there is no copy. Their ids are left
/// to the profile they go into.
pub fn read_boosts(dir: &Path) -> io::Result<Vec<Boost>> {
    let documents = firestore::documents(dir)?;
    let boosts = documents
        .iter()
        .filter(|d| d.place().is_some_and(|(c, _)| c == "boosts"));
    let fields = boosts.map(|document| {
        let strings = document.fields.iter();
        let strings =
            strings.filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_string())));
        strings.collect::<HashMap<_, _>>()
    });
    Ok(fields.filter_map(|fields| boost(&fields)).collect())
}

/// A Boost from an Arc document's fields; `None` without a site.
fn boost(fields: &HashMap<String, String>) -> Option<Boost> {
    let pattern = fields.get("hostPattern")?.trim().trim_start_matches("*.");
    let pattern = pattern.split_once("://").map_or(pattern, |(_, host)| host);
    let site = boosts::site(&format!("https://{pattern}"))?;
    let data: Value = fields
        .get("data")
        .and_then(|data| serde_json::from_str(data).ok())
        .unwrap_or_default();
    let text = |key: &str| data[key].as_str().unwrap_or_default().to_string();
    let title = fields.get("title").map_or("", |title| title.trim());
    Some(Boost {
        id: 0,
        title: if title.is_empty() {
            site.clone()
        } else {
            title.to_string()
        },
        site,
        active: fields
            .get("isActive")
            .is_some_and(|active| active == "true"),
        zaps: data["zaps"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|zap| Some(zap.as_str()?.to_string()))
            .collect(),
        colors: colors(&data["colorScheme"]["colorWheelDriven"]["_0"]),
        font: data["font"]["family"].as_str().map(str::to_string),
        text_case: match data["textTransform"].as_str() {
            Some("capitalize") => Some(TextCase::Capitalize),
            Some("uppercase") => Some(TextCase::Uppercase),
            Some("lowercase") => Some(TextCase::Lowercase),
            _ => None,
        },
        text_size: data["textZoom"]["level"].as_f64().and_then(text_size),
        css: text("css"),
        js: text("js"),
    })
}

/// Arc's color wheel: the accent's hue and saturation, inverted or not, and how much of the
/// page's own colors it keeps.
fn colors(wheel: &Value) -> Option<Colors> {
    let original = wheel["advancedSettings"]["preservedSaturation"].as_f64();
    Some(Colors {
        hue: wheel["primaryHueDegrees"].as_f64()?,
        saturation: wheel["primarySaturation"].as_f64()?.clamp(0.0, 1.0),
        invert: wheel["invertLightness"].as_bool().unwrap_or_default(),
        contrast: 1.0,
        brightness: 1.0,
        original: original.map_or(0.3, |original| original.clamp(0.0, 1.0)),
    })
}

/// Arc's text zoom, a percentage, as the closest of Arbalete's text sizes; none at 100.
fn text_size(level: f64) -> Option<TextSize> {
    [
        (90.0, Some(TextSize::Small)),
        (100.0, None),
        (110.0, Some(TextSize::Medium)),
        (125.0, Some(TextSize::Large)),
        (150.0, Some(TextSize::Larger)),
    ]
    .into_iter()
    .min_by(|a, b| (a.0 - level).abs().total_cmp(&(b.0 - level).abs()))?
    .1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::TestDir;
    use rusty_leveldb::{DB, Options};
    use serde_json::json;

    /// A protocol buffers field holding bytes.
    fn field(number: u64, bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for mut n in [number << 3 | 2, bytes.len() as u64] {
            while n >= 0x80 {
                out.push(n as u8 | 0x80);
                n >>= 7;
            }
            out.push(n as u8);
        }
        out.extend_from_slice(bytes);
        out
    }

    /// A cached Firestore document, as its `MaybeDocument` message, with string fields.
    fn document(name: &str, fields: &[(&str, &str)]) -> Vec<u8> {
        let mut document = field(1, name.as_bytes());
        for (key, value) in fields {
            let entry = [
                field(1, key.as_bytes()),
                field(2, &field(17, value.as_bytes())),
            ];
            document.extend(field(2, &entry.concat()));
        }
        // A number field, the update time, as Firestore writes one after the fields.
        document.extend([0x22, 0x02, 0x08, 0x01]);
        [field(2, &document), vec![0x20, 0x01]].concat()
    }

    const NAME: &str = "projects/bcny-arc-server/databases/(default)/documents/users/u1/boosts/";

    #[test]
    fn reads_the_boosts_in_arcs_firestore_copy() {
        let support = TestDir::new("arc-boosts");
        support.dir("Arc");
        support.dir("firestore/Arc/bcny-arc-server/main");
        let database = support.0.join("firestore/Arc/bcny-arc-server/main");
        let data = json!({
            "css": "body { margin: 0 }", "js": "console.log(1)", "font": { "family": "Georgia" },
            "textTransform": "uppercase", "textZoom": { "level": 125 },
            "zaps": [".ad", "#banner"],
            "colorScheme": { "colorWheelDriven": { "_0": {
                "primaryHueDegrees": 210.0, "primarySaturation": 0.6, "secondaryHueOffset": 30.0,
                "invertLightness": true, "advancedSettings": { "preservedSaturation": 0.5 } } } },
        });
        let theme = json!({ "zaps": [], "colorScheme": { "themeDriven": {} } });
        {
            let options = Options {
                compressor: 1,
                ..Options::default()
            };
            let mut db = DB::open(&database, options).unwrap();
            let boost = document(
                &format!("{NAME}b1"),
                &[
                    ("hostPattern", "www.News.example"),
                    ("title", "Calm news"),
                    ("isActive", "true"),
                    ("author", "Someone"),
                    ("data", &data.to_string()),
                ],
            );
            db.put(b"remote_document/b1", &boost).unwrap();
            // Written to a table, then an older copy is replaced by a newer one in the log.
            db.flush().unwrap();
            let theme = document(
                &format!("{NAME}b2"),
                &[
                    ("hostPattern", "mail.example"),
                    ("data", &theme.to_string()),
                ],
            );
            db.put(b"remote_document/b2", &theme).unwrap();
            let snapshot = document(
                "projects/bcny-arc-server/databases/(default)/documents/boostSnapshots/s1",
                &[("hostPattern", "shared.example")],
            );
            db.put(b"remote_document/s1", &snapshot).unwrap();
            db.put(b"target/1", b"\x0a\x03abc").unwrap();
            db.flush().unwrap();
        }
        let boosts = read_boosts(&support.0.join("Arc")).unwrap();
        assert_eq!(boosts.len(), 2);
        assert_eq!(
            boosts[0],
            Boost {
                id: 0,
                site: "news.example".to_string(),
                title: "Calm news".to_string(),
                active: true,
                zaps: vec![".ad".to_string(), "#banner".to_string()],
                colors: Some(Colors {
                    hue: 210.0,
                    saturation: 0.6,
                    invert: true,
                    contrast: 1.0,
                    brightness: 1.0,
                    original: 0.5,
                }),
                font: Some("Georgia".to_string()),
                text_case: Some(TextCase::Uppercase),
                text_size: Some(TextSize::Large),
                css: "body { margin: 0 }".to_string(),
                js: "console.log(1)".to_string(),
            }
        );
        // The theme's colors have no equivalent; an untitled Boost is named after its site.
        let mail = &boosts[1];
        assert_eq!((mail.title.as_str(), mail.active), ("mail.example", false));
        assert_eq!(mail.colors, None);
    }

    #[test]
    fn reads_no_boosts_without_a_firestore_copy() {
        let support = TestDir::new("arc-no-boosts");
        assert_eq!(read_boosts(&support.0.join("Arc")).unwrap(), Vec::new());
    }

    #[test]
    fn rounds_text_zoom_to_the_closest_size() {
        assert_eq!(text_size(90.0), Some(TextSize::Small));
        assert_eq!(text_size(100.0), None);
        assert_eq!(text_size(112.0), Some(TextSize::Medium));
        assert_eq!(text_size(200.0), Some(TextSize::Larger));
    }
}
