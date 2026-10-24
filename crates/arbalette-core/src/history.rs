//! A profile's browsing history, in a SQLite database of its own.
//!
//! One row per url, with what ranking it needs: how often and how recently it was visited.

use rusqlite::{Connection, params};
use std::path::Path;

/// The history's file name in a profile's directory.
pub const FILE_NAME: &str = "History.sqlite";

/// A visited url. Times are milliseconds since the Unix epoch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub url: String,
    pub title: String,
    pub visit_count: i64,
    pub last_visit: i64,
}

pub struct History(Connection);

impl History {
    /// Opens the history at `path`, created empty the first time.
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let db = Connection::open(path)?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS urls (
                url TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                visit_count INTEGER NOT NULL,
                last_visit INTEGER NOT NULL
            )",
        )?;
        Ok(Self(db))
    }

    /// Records a visit of `url` at `time`. Its title comes later, once the page has one. Only web
    /// pages are history: blank and internal pages are not.
    pub fn visit(&self, url: &str, time: i64) -> rusqlite::Result<()> {
        if !url.starts_with("https://") && !url.starts_with("http://") {
            return Ok(());
        }
        self.0.execute(
            "INSERT INTO urls (url, title, visit_count, last_visit) VALUES (?1, '', 1, ?2)
             ON CONFLICT (url) DO UPDATE SET
                visit_count = visit_count + 1, last_visit = MAX(last_visit, ?2)",
            params![url, time],
        )?;
        Ok(())
    }

    /// Sets the title of a visited url.
    pub fn set_title(&self, url: &str, title: &str) -> rusqlite::Result<()> {
        self.0.execute(
            "UPDATE urls SET title = ?2 WHERE url = ?1",
            params![url, title],
        )?;
        Ok(())
    }

    /// Every visited url, the most recent first.
    pub fn entries(&self) -> rusqlite::Result<Vec<Entry>> {
        let mut statement = self.0.prepare(
            "SELECT url, title, visit_count, last_visit FROM urls ORDER BY last_visit DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(Entry {
                url: row.get(0)?,
                title: row.get(1)?,
                visit_count: row.get(2)?,
                last_visit: row.get(3)?,
            })
        })?;
        rows.collect()
    }
}

#[cfg(test)]
pub(crate) fn entry(url: &str, title: &str, visit_count: i64, last_visit: i64) -> Entry {
    Entry {
        url: url.to_string(),
        title: title.to_string(),
        visit_count,
        last_visit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::TestDir;

    #[test]
    fn counts_the_visits_of_each_url_and_keeps_the_latest() {
        let dir = TestDir::new("history-visits");
        let history = History::open(&dir.0.join(FILE_NAME)).unwrap();
        history.visit("https://mail.example.com/", 1_000).unwrap();
        history.visit("https://news.example.net/", 2_000).unwrap();
        history.visit("https://mail.example.com/", 3_000).unwrap();
        history.visit("about:blank", 4_000).unwrap();
        assert_eq!(
            history.entries().unwrap(),
            vec![
                entry("https://mail.example.com/", "", 2, 3_000),
                entry("https://news.example.net/", "", 1, 2_000),
            ]
        );
    }

    #[test]
    fn keeps_the_title_across_visits() {
        let dir = TestDir::new("history-title");
        let history = History::open(&dir.0.join(FILE_NAME)).unwrap();
        history.visit("https://mail.example.com/", 1_000).unwrap();
        history
            .set_title("https://mail.example.com/", "Mail")
            .unwrap();
        history.visit("https://mail.example.com/", 2_000).unwrap();
        history
            .set_title("https://never.example.org/", "Never visited")
            .unwrap();
        assert_eq!(
            history.entries().unwrap(),
            vec![entry("https://mail.example.com/", "Mail", 2, 2_000)]
        );
    }

    #[test]
    fn keeps_the_history_in_its_file() {
        let dir = TestDir::new("history-file");
        let path = dir.0.join(FILE_NAME);
        History::open(&path)
            .unwrap()
            .visit("https://mail.example.com/", 1_000)
            .unwrap();
        assert_eq!(History::open(&path).unwrap().entries().unwrap().len(), 1);
    }
}
