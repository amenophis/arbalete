//! Reads Safari's data, in `~/Library/Safari`.

use super::bookmarks::{Bookmark, Bookmarks, Folder};
use plist::{Dictionary, Value};
use std::{fs, io, io::Cursor, path::Path};

#[derive(Debug)]
pub enum Error {
    /// macOS lets an app read Safari's data only once the user grants it Full Disk Access.
    NeedsFullDiskAccess,
    Io(io::Error),
    Invalid(plist::Error),
}

/// Reads `Bookmarks.plist` in Safari's data directory. Safari without bookmarks has none.
pub fn read_bookmarks(dir: &Path) -> Result<Bookmarks, Error> {
    match fs::read(dir.join("Bookmarks.plist")) {
        Ok(plist) => bookmarks(&plist),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Bookmarks::default()),
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            Err(Error::NeedsFullDiskAccess)
        }
        Err(error) => Err(Error::Io(error)),
    }
}

const BAR: &str = "BookmarksBar";

/// Parses a `Bookmarks.plist`, binary or XML. Its root lists the bar, the bookmarks menu and the
/// reading list, which keep internal names as their titles, and a link to the history, skipped.
pub fn bookmarks(plist: &[u8]) -> Result<Bookmarks, Error> {
    let root = Value::from_reader(Cursor::new(plist)).map_err(Error::Invalid)?;
    let mut bookmarks = Bookmarks {
        bar: Folder {
            title: "Favorites Bar".to_string(),
            children: Vec::new(),
        },
        others: Vec::new(),
    };
    for list in children(root.as_dictionary()).filter(|d| kind(d) == "WebBookmarkTypeList") {
        let title = list
            .get("Title")
            .and_then(Value::as_string)
            .unwrap_or_default();
        let content = content(list);
        let title = match title {
            BAR => {
                bookmarks.bar.children = content;
                continue;
            }
            "BookmarksMenu" => "Bookmarks Menu",
            "com.apple.ReadingList" => "Reading List",
            title => title,
        };
        bookmarks.others.push(Folder {
            title: title.to_string(),
            children: content,
        });
    }
    Ok(bookmarks)
}

fn children(list: Option<&Dictionary>) -> impl Iterator<Item = &Dictionary> {
    let children = list.and_then(|l| l.get("Children")?.as_array());
    children
        .into_iter()
        .flatten()
        .filter_map(Value::as_dictionary)
}

fn kind(item: &Dictionary) -> &str {
    item.get("WebBookmarkType")
        .and_then(Value::as_string)
        .unwrap_or_default()
}

fn content(list: &Dictionary) -> Vec<Bookmark> {
    let text = |item: Option<&Value>| item.and_then(Value::as_string).map(str::to_string);
    children(Some(list))
        .filter_map(|item| match kind(item) {
            "WebBookmarkTypeLeaf" => Some(Bookmark::Link {
                title: text(
                    item.get("URIDictionary")
                        .and_then(|d| d.as_dictionary()?.get("title")),
                )
                .unwrap_or_default(),
                url: text(item.get("URLString"))?,
            }),
            "WebBookmarkTypeList" => Some(Bookmark::Folder(Folder {
                title: text(item.get("Title")).unwrap_or_default(),
                children: content(item),
            })),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::TestDir;
    use crate::import::bookmarks::{folder, link};

    fn list(title: &str, children: Vec<Value>) -> Value {
        let mut list = Dictionary::new();
        list.insert("Title".into(), title.into());
        list.insert("WebBookmarkType".into(), "WebBookmarkTypeList".into());
        list.insert("Children".into(), children.into());
        list.into()
    }

    fn leaf(title: &str, url: &str) -> Value {
        let mut uri = Dictionary::new();
        uri.insert("title".into(), title.into());
        let mut leaf = Dictionary::new();
        leaf.insert("URIDictionary".into(), uri.into());
        leaf.insert("URLString".into(), url.into());
        leaf.insert("WebBookmarkType".into(), "WebBookmarkTypeLeaf".into());
        leaf.into()
    }

    /// A binary `Bookmarks.plist` with fictitious bookmarks, as Safari writes it.
    fn plist() -> Vec<u8> {
        let mut history = Dictionary::new();
        history.insert("Title".into(), "History".into());
        history.insert("WebBookmarkType".into(), "WebBookmarkTypeProxy".into());
        let root = list(
            "",
            vec![
                history.into(),
                list(
                    "BookmarksBar",
                    vec![
                        leaf("Mail", "https://mail.example.com/"),
                        list(
                            "Work",
                            vec![leaf("Tracker", "https://tracker.example.com/")],
                        ),
                    ],
                ),
                list(
                    "BookmarksMenu",
                    vec![leaf("News", "https://news.example.net/")],
                ),
                list(
                    "com.apple.ReadingList",
                    vec![leaf("Post", "https://blog.example.org/post")],
                ),
            ],
        );
        let mut plist = Vec::new();
        root.to_writer_binary(&mut plist).unwrap();
        plist
    }

    #[test]
    fn parses_the_bar_the_menu_and_the_reading_list() {
        assert_eq!(
            bookmarks(&plist()).unwrap(),
            Bookmarks {
                bar: folder(
                    "Favorites Bar",
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
                        "Bookmarks Menu",
                        vec![link("News", "https://news.example.net/")]
                    ),
                    folder(
                        "Reading List",
                        vec![link("Post", "https://blog.example.org/post")]
                    ),
                ],
            }
        );
        assert!(matches!(bookmarks(b"not a plist"), Err(Error::Invalid(_))));
    }

    #[test]
    fn reads_the_bookmarks_file_in_safari_s_directory() {
        let dir = TestDir::new("safari");
        assert_eq!(read_bookmarks(&dir.0).unwrap(), Bookmarks::default());
        dir.write("Bookmarks.plist", plist());
        assert_eq!(
            read_bookmarks(&dir.0).unwrap(),
            bookmarks(&plist()).unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn asks_for_full_disk_access_when_the_file_cannot_be_read() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TestDir::new("safari-denied");
        dir.write("Bookmarks.plist", plist());
        let file = dir.0.join("Bookmarks.plist");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();
        assert!(matches!(
            read_bookmarks(&dir.0),
            Err(Error::NeedsFullDiskAccess)
        ));
    }
}
