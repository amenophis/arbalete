//! Reads the data of a Chromium browser's profile (Chrome, Edge, Brave…).

use super::bookmarks::{Bookmark, Bookmarks, Folder};
use serde_json::Value;
use std::{fs, io, path::Path};

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Invalid(serde_json::Error),
}

/// Reads the profile's `Bookmarks` file. A profile that has never saved a bookmark has none.
pub fn read_bookmarks(profile: &Path) -> Result<Bookmarks, Error> {
    match fs::read_to_string(profile.join("Bookmarks")) {
        Ok(json) => bookmarks(&json),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Bookmarks::default()),
        Err(error) => Err(Error::Io(error)),
    }
}

/// Parses a `Bookmarks` file: its roots are the bar, "Other bookmarks" and the bookmarks synced
/// from mobile devices. Nodes that are neither links nor folders are skipped.
pub fn bookmarks(json: &str) -> Result<Bookmarks, Error> {
    let json: Value = serde_json::from_str(json).map_err(Error::Invalid)?;
    let roots = &json["roots"];
    Ok(Bookmarks {
        bar: folder(&roots["bookmark_bar"], "Bookmarks bar"),
        others: [("other", "Other bookmarks"), ("synced", "Mobile bookmarks")]
            .iter()
            .map(|&(root, title)| folder(&roots[root], title))
            .collect(),
    })
}

fn folder(node: &Value, fallback: &str) -> Folder {
    let children = node["children"].as_array().into_iter().flatten();
    Folder {
        title: node["name"].as_str().unwrap_or(fallback).to_string(),
        children: children.filter_map(bookmark).collect(),
    }
}

fn bookmark(node: &Value) -> Option<Bookmark> {
    match node["type"].as_str()? {
        "url" => Some(Bookmark::Link {
            title: node["name"].as_str().unwrap_or_default().to_string(),
            url: node["url"].as_str()?.to_string(),
        }),
        "folder" => Some(Bookmark::Folder(folder(node, "Folder"))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::TestDir;
    use crate::import::bookmarks::{folder, link};

    const BOOKMARKS: &str = r#"{
        "checksum": "00000000000000000000000000000000",
        "roots": {
            "bookmark_bar": { "name": "Bookmarks bar", "type": "folder", "children": [
                { "name": "Mail", "type": "url", "url": "https://mail.example.com/" },
                { "name": "Work", "type": "folder", "children": [
                    { "name": "Tracker", "type": "url", "url": "https://tracker.example.com/" },
                    { "name": "Broken", "type": "url" }
                ] }
            ] },
            "other": { "name": "Other bookmarks", "type": "folder", "children": [
                { "name": "News", "type": "url", "url": "https://news.example.net/" },
                { "name": "Unknown", "type": "separator" }
            ] },
            "synced": { "name": "Mobile bookmarks", "type": "folder", "children": [] }
        },
        "version": 1
    }"#;

    #[test]
    fn parses_the_bar_and_the_other_roots() {
        assert_eq!(
            bookmarks(BOOKMARKS).unwrap(),
            Bookmarks {
                bar: folder(
                    "Bookmarks bar",
                    vec![
                        link("Mail", "https://mail.example.com/"),
                        Bookmark::Folder(folder(
                            "Work",
                            vec![link("Tracker", "https://tracker.example.com/")]
                        )),
                    ]
                ),
                others: vec![
                    folder(
                        "Other bookmarks",
                        vec![link("News", "https://news.example.net/")]
                    ),
                    folder("Mobile bookmarks", vec![]),
                ],
            }
        );
    }

    #[test]
    fn names_missing_roots_and_refuses_an_invalid_file() {
        let empty = bookmarks("{}").unwrap();
        assert_eq!(empty.bar, folder("Bookmarks bar", vec![]));
        assert_eq!(empty.others[1], folder("Mobile bookmarks", vec![]));
        assert!(matches!(bookmarks("not json"), Err(Error::Invalid(_))));
    }

    #[test]
    fn reads_the_bookmarks_file_of_a_profile() {
        let dir = TestDir::new("chromium");
        assert_eq!(read_bookmarks(&dir.0).unwrap(), Bookmarks::default());
        dir.write("Bookmarks", BOOKMARKS);
        let read = read_bookmarks(&dir.0).unwrap();
        assert_eq!(read, bookmarks(BOOKMARKS).unwrap());
    }
}
