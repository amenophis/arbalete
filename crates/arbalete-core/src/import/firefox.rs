//! Reads the data of a Firefox browser's profile (Firefox, Zen, LibreWolf).

use super::Copy;
use super::bookmarks::{Bookmark, Bookmarks, Folder};
use crate::history::{self, Entry};
use rusqlite::Connection;
use std::collections::HashMap;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Sqlite(rusqlite::Error),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

/// Reads the bookmarks of the profile's `places.sqlite`. A profile Firefox has never opened has
/// none.
pub fn read_bookmarks(profile: &Path) -> Result<Bookmarks, Error> {
    let Some(copy) = Copy::of(&profile.join("places.sqlite"))? else {
        return Ok(Bookmarks::default());
    };
    bookmarks(&Connection::open(copy.file())?)
}

/// Reads the history of the profile's `places.sqlite`. A profile Firefox has never opened has
/// none.
pub fn read_history(profile: &Path) -> Result<Vec<Entry>, Error> {
    let Some(copy) = Copy::of(&profile.join("places.sqlite"))? else {
        return Ok(Vec::new());
    };
    Ok(history(&Connection::open(copy.file())?)?)
}

/// The visited pages in `moz_places`, whose times are microseconds since the Unix epoch. Places
/// only bookmarked have no visit; hidden ones are those of frames and redirect sources.
fn history(db: &Connection) -> rusqlite::Result<Vec<Entry>> {
    history::select(
        db,
        "SELECT url, IFNULL(title, ''), visit_count, last_visit_date / 1000 FROM moz_places
         WHERE visit_count > 0 AND last_visit_date IS NOT NULL AND hidden = 0",
    )
}

/// The root folders by their fixed `guid`. Firefox stores internal names as their titles and
/// shows translated ones, so they get English names here.
const BAR: (&str, &str) = ("toolbar_____", "Bookmarks Toolbar");
const OTHERS: [(&str, &str); 3] = [
    ("menu________", "Bookmarks Menu"),
    ("unfiled_____", "Other Bookmarks"),
    ("mobile______", "Mobile Bookmarks"),
];

struct Row {
    id: i64,
    kind: i64,
    title: String,
    url: Option<String>,
}

/// Reads the bookmarks of a `places.sqlite` database: `moz_bookmarks` is the tree, `moz_places`
/// holds the urls.
fn bookmarks(db: &Connection) -> Result<Bookmarks, Error> {
    let mut statement = db.prepare(
        "SELECT b.id, b.parent, b.type, IFNULL(b.title, ''), p.url, b.guid
         FROM moz_bookmarks b LEFT JOIN moz_places p ON p.id = b.fk
         ORDER BY b.parent, b.position",
    )?;
    let mut children: HashMap<i64, Vec<Row>> = HashMap::new();
    let mut roots = HashMap::new();
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let id = row.get(0)?;
        roots.insert(row.get::<_, String>(5)?, id);
        children.entry(row.get(1)?).or_default().push(Row {
            id,
            kind: row.get(2)?,
            title: row.get(3)?,
            url: row.get(4)?,
        });
    }
    let root = |(guid, title): (&str, &str)| Folder {
        title: title.to_string(),
        children: roots
            .get(guid)
            .map(|&id| content(&children, id))
            .unwrap_or_default(),
    };
    Ok(Bookmarks {
        bar: root(BAR),
        others: OTHERS.into_iter().map(root).collect(),
    })
}

/// The links and folders in the folder `id`. Separators, and `place:` queries such as "Most
/// Visited", which are not pages, are skipped.
fn content(children: &HashMap<i64, Vec<Row>>, id: i64) -> Vec<Bookmark> {
    let rows = children.get(&id).into_iter().flatten();
    rows.filter_map(|row| match (row.kind, &row.url) {
        (1, Some(url)) if !url.starts_with("place:") => Some(Bookmark::Link {
            title: row.title.clone(),
            url: url.clone(),
        }),
        (2, _) => Some(Bookmark::Folder(Folder {
            title: row.title.clone(),
            children: content(children, row.id),
        })),
        _ => None,
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::entry;
    use crate::import::TestDir;
    use crate::import::bookmarks::{folder, link};

    /// The part of Firefox's schema the import reads, with fictitious bookmarks: one in the
    /// toolbar, a folder in the menu and a separator and a query among the other bookmarks.
    const PLACES: &str = "
        CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url LONGVARCHAR, title LONGVARCHAR);
        CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER,
            parent INTEGER, position INTEGER, title LONGVARCHAR, guid TEXT);
        INSERT INTO moz_places (id, url) VALUES
            (1, 'https://mail.example.com/'), (2, 'https://docs.example.com/'),
            (3, 'https://news.example.net/'), (4, 'place:sort=8&maxResults=10');
        INSERT INTO moz_bookmarks (id, type, fk, parent, position, title, guid) VALUES
            (1, 2, NULL, 0, 0, '', 'root________'),
            (2, 2, NULL, 1, 0, 'menu', 'menu________'),
            (3, 2, NULL, 1, 1, 'toolbar', 'toolbar_____'),
            (4, 2, NULL, 1, 2, 'tags', 'tags________'),
            (5, 2, NULL, 1, 3, 'unfiled', 'unfiled_____'),
            (6, 2, NULL, 1, 4, 'mobile', 'mobile______'),
            (10, 1, 1, 3, 0, 'Mail', 'aaaaaaaaaaaa'),
            (11, 2, NULL, 2, 1, 'Reading', 'bbbbbbbbbbbb'),
            (12, 1, 2, 11, 0, NULL, 'cccccccccccc'),
            (13, 1, 4, 2, 0, 'Most Visited', 'dddddddddddd'),
            (14, 3, NULL, 5, 0, NULL, 'eeeeeeeeeeee'),
            (15, 1, 3, 5, 1, 'News', 'ffffffffffff');
    ";

    fn expected() -> Bookmarks {
        Bookmarks {
            bar: folder(
                "Bookmarks Toolbar",
                vec![link("Mail", "https://mail.example.com/")],
            ),
            others: vec![
                folder(
                    "Bookmarks Menu",
                    vec![Bookmark::Folder(folder(
                        "Reading",
                        vec![link("", "https://docs.example.com/")],
                    ))],
                ),
                folder(
                    "Other Bookmarks",
                    vec![link("News", "https://news.example.net/")],
                ),
                folder("Mobile Bookmarks", vec![]),
            ],
        }
    }

    #[test]
    fn reads_the_bookmarks_tree_with_english_root_names() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(PLACES).unwrap();
        assert_eq!(bookmarks(&db).unwrap(), expected());
    }

    #[test]
    fn reads_a_copy_of_a_database_locked_by_a_running_browser() {
        let dir = TestDir::new("firefox-locked");
        let browser = Connection::open(dir.0.join("places.sqlite")).unwrap();
        browser
            .execute_batch("PRAGMA locking_mode = EXCLUSIVE; PRAGMA journal_mode = WAL;")
            .unwrap();
        browser.execute_batch(PLACES).unwrap();
        // Unchecked, the changes are still in the write-ahead log only.
        assert_eq!(read_bookmarks(&dir.0).unwrap(), expected());
    }

    /// The part of `moz_places` the history import reads, with fictitious visits.
    const HISTORY: &str = "
        CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url LONGVARCHAR, title LONGVARCHAR,
            visit_count INTEGER DEFAULT 0, hidden INTEGER DEFAULT 0 NOT NULL,
            last_visit_date INTEGER);
        INSERT INTO moz_places (url, title, visit_count, hidden, last_visit_date) VALUES
            ('https://mail.example.com/', 'Mail', 3, 0, 1705526400000000),
            ('https://news.example.net/', NULL, 1, 0, 1705526401000000),
            ('https://redirect.example.org/', 'Redirect', 1, 1, 1705526402000000),
            ('https://bookmarked.example.org/', 'Bookmarked', 0, 0, NULL);
    ";

    #[test]
    fn reads_the_visited_pages_with_unix_times() {
        let dir = TestDir::new("firefox-history");
        assert_eq!(read_history(&dir.0).unwrap(), vec![]);
        let db = Connection::open(dir.0.join("places.sqlite")).unwrap();
        db.execute_batch(HISTORY).unwrap();
        assert_eq!(
            read_history(&dir.0).unwrap(),
            vec![
                entry("https://mail.example.com/", "Mail", 3, 1_705_526_400_000),
                entry("https://news.example.net/", "", 1, 1_705_526_401_000),
            ]
        );
    }

    #[test]
    fn reads_no_bookmarks_from_a_profile_without_database() {
        let dir = TestDir::new("firefox-empty");
        assert_eq!(read_bookmarks(&dir.0).unwrap(), Bookmarks::default());
        dir.write("places.sqlite", "not a database");
        assert!(matches!(read_bookmarks(&dir.0), Err(Error::Sqlite(_))));
    }
}
