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
    /// Opens the history at `path`, created empty the first time. Several connections can write
    /// to it, such as a merge on a thread of its own and the visits of the browsing: rusqlite's
    /// connections wait up to 5 seconds for another's write to end.
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
        if !is_web(url) {
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

    /// Adds another browser's history: the visits of a url already here add up, and the title of
    /// its latest visit wins.
    pub fn merge(&mut self, entries: &[Entry]) -> rusqlite::Result<()> {
        let transaction = self.0.transaction()?;
        let mut statement = transaction.prepare(
            "INSERT INTO urls (url, title, visit_count, last_visit) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (url) DO UPDATE SET
                title = CASE WHEN title = '' OR (?2 != '' AND ?4 > last_visit) THEN ?2 ELSE title END,
                visit_count = visit_count + ?3,
                last_visit = MAX(last_visit, ?4)",
        )?;
        for entry in entries.iter().filter(|entry| is_web(&entry.url)) {
            statement.execute(params![
                entry.url,
                entry.title,
                entry.visit_count,
                entry.last_visit
            ])?;
        }
        drop(statement);
        transaction.commit()
    }

    /// Every visited url, the most recent first.
    pub fn entries(&self) -> rusqlite::Result<Vec<Entry>> {
        select(
            &self.0,
            "SELECT url, title, visit_count, last_visit FROM urls ORDER BY last_visit DESC",
        )
    }

    /// The `limit` most recently visited urls whose address or title holds `text`, ignoring case;
    /// all of them for an empty `text`.
    pub fn search(&self, text: &str, limit: usize) -> rusqlite::Result<Vec<Entry>> {
        self.matching(text, "last_visit DESC", limit)
    }

    /// The `limit` urls that best answer `text` typed in the Command Bar at `now`: those whose
    /// address or title holds it, ranked by how they match it, how often and how recently they
    /// were visited (`score`).
    pub fn suggest(&self, text: &str, limit: usize, now: i64) -> rusqlite::Result<Vec<Entry>> {
        let mut entries = self.matching(text, "visit_count DESC, last_visit DESC", CANDIDATES)?;
        let text = text.to_lowercase();
        let scores: Vec<_> = entries
            .iter()
            .map(|entry| score(entry, &text, now))
            .collect();
        let mut ranked: Vec<_> = scores.into_iter().zip(entries.drain(..)).collect();
        // The higher score first; between equals, the shorter address, closer to a site's home.
        ranked.sort_by(|(a, x), (b, y)| b.total_cmp(a).then(x.url.len().cmp(&y.url.len())));
        Ok(ranked
            .into_iter()
            .take(limit)
            .map(|(_, entry)| entry)
            .collect())
    }

    /// The `limit` first urls in `order` whose address or title holds `text`, ignoring case.
    fn matching(&self, text: &str, order: &str, limit: usize) -> rusqlite::Result<Vec<Entry>> {
        // `%`, `_` and the escape character itself match literally.
        let pattern = format!(
            "%{}%",
            text.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let mut statement = self.0.prepare(&format!(
            "SELECT url, title, visit_count, last_visit FROM urls
             WHERE url LIKE ?1 ESCAPE '\\' OR title LIKE ?1 ESCAPE '\\'
             ORDER BY {order} LIMIT ?2"
        ))?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = statement.query_map(params![pattern, limit], |row| {
            Ok(Entry {
                url: row.get(0)?,
                title: row.get(1)?,
                visit_count: row.get(2)?,
                last_visit: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    /// Forgets a url.
    pub fn remove(&self, url: &str) -> rusqlite::Result<()> {
        self.0
            .execute("DELETE FROM urls WHERE url = ?1", params![url])?;
        Ok(())
    }
}

/// The most visited matches `suggest` ranks: enough to find the best among many.
const CANDIDATES: usize = 200;

const DAY: i64 = 24 * 60 * 60 * 1000;

/// How well a page answers `text`, in lowercase, at `now`: its address starting with it counts
/// most, then a word of its address or title starting with it, then holding it anywhere; times
/// how often it was visited, which counts less and less, and how recently.
fn score(entry: &Entry, text: &str, now: i64) -> f64 {
    let url = entry.url.to_lowercase();
    let address = address(&url);
    let title = entry.title.to_lowercase();
    let words = |words: &str| {
        words
            .split(|c: char| !c.is_alphanumeric())
            .any(|word| word.starts_with(text))
    };
    let matched = if address.starts_with(text) {
        4.0
    } else if words(address) || words(&title) {
        2.0
    } else {
        1.0
    };
    let frequency = 1.0 + (1.0 + entry.visit_count.max(0) as f64).ln();
    let recency = match (now - entry.last_visit) / DAY {
        0 => 1.0,
        1..7 => 0.8,
        7..30 => 0.5,
        30..90 => 0.3,
        _ => 0.15,
    };
    matched * frequency * recency
}

/// A url as typed: without its scheme and "www.".
fn address(url: &str) -> &str {
    let url = url.split_once("://").map_or(url, |(_, rest)| rest);
    url.strip_prefix("www.").unwrap_or(url)
}

fn is_web(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// The entries a query selects as url, title, visit count and last visit, in that order.
pub(crate) fn select(db: &Connection, query: &str) -> rusqlite::Result<Vec<Entry>> {
    let mut statement = db.prepare(query)?;
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
    fn merges_another_browser_s_history() {
        let dir = TestDir::new("history-merge");
        let mut history = History::open(&dir.0.join(FILE_NAME)).unwrap();
        history.visit("https://mail.example.com/", 5_000).unwrap();
        history
            .set_title("https://mail.example.com/", "Mail")
            .unwrap();
        history.visit("https://news.example.net/", 1_000).unwrap();
        history
            .merge(&[
                entry("https://mail.example.com/", "Old mail", 3, 2_000),
                entry("https://news.example.net/", "News", 2, 4_000),
                entry("https://docs.example.org/", "Docs", 1, 3_000),
                entry("file:///tmp/notes.txt", "Notes", 1, 6_000),
            ])
            .unwrap();
        assert_eq!(
            history.entries().unwrap(),
            vec![
                entry("https://mail.example.com/", "Mail", 4, 5_000),
                entry("https://news.example.net/", "News", 3, 4_000),
                entry("https://docs.example.org/", "Docs", 1, 3_000),
            ]
        );
    }

    #[test]
    fn records_visits_while_another_connection_merges() {
        let dir = TestDir::new("history-concurrent");
        let path = dir.0.join(FILE_NAME);
        let browsing = History::open(&path).unwrap();
        let entries: Vec<_> = (0..20_000)
            .map(|i| entry(&format!("https://{i}.example.com/"), "", 1, i))
            .collect();
        let merge = std::thread::spawn({
            let path = path.clone();
            move || History::open(&path).unwrap().merge(&entries)
        });
        for i in 0..100 {
            browsing.visit("https://mail.example.com/", i).unwrap();
        }
        merge.join().unwrap().unwrap();
        let entries = browsing.entries().unwrap();
        assert_eq!(entries.len(), 20_001);
        assert!(entries.contains(&entry("https://mail.example.com/", "", 100, 99)));
    }

    #[test]
    fn searches_addresses_and_titles_the_most_recent_first() {
        let dir = TestDir::new("history-search");
        let mut history = History::open(&dir.0.join(FILE_NAME)).unwrap();
        history
            .merge(&[
                entry("https://mail.example.com/", "Inbox", 1, 1_000),
                entry("https://news.example.net/", "Mail delivery news", 1, 3_000),
                entry("https://docs.example.org/100%_done", "Docs", 1, 2_000),
            ])
            .unwrap();
        let urls = |text: &str, limit: usize| -> Vec<String> {
            let entries = history.search(text, limit).unwrap();
            entries.into_iter().map(|entry| entry.url).collect()
        };
        assert_eq!(
            urls("MAIL", 10),
            ["https://news.example.net/", "https://mail.example.com/"]
        );
        assert_eq!(urls("", 2).len(), 2);
        assert_eq!(urls("%_", 10), ["https://docs.example.org/100%_done"]);
        assert!(urls("_x", 10).is_empty());
    }

    #[test]
    fn suggests_the_best_match_the_most_visited_and_recent_first() {
        let dir = TestDir::new("history-suggest");
        let mut history = History::open(&dir.0.join(FILE_NAME)).unwrap();
        let now = 100 * DAY;
        history
            .merge(&[
                // Holds "git" in a word of its path only.
                entry("https://blog.example.com/git-tips", "Tips", 50, now),
                // Starts with it, often and lately visited.
                entry("https://github.com/", "GitHub", 40, now - DAY),
                // Starts with it too, but once and long ago.
                entry("https://gitlab.com/", "GitLab", 1, now - 60 * DAY),
                entry("https://www.digital.example/", "Digital", 90, now),
            ])
            .unwrap();
        let urls: Vec<_> = history
            .suggest("GIT", 10, now)
            .unwrap()
            .into_iter()
            .map(|entry| entry.url)
            .collect();
        assert_eq!(
            urls,
            [
                "https://github.com/",
                "https://blog.example.com/git-tips",
                "https://www.digital.example/",
                "https://gitlab.com/",
            ]
        );
        assert_eq!(history.suggest("git", 1, now).unwrap().len(), 1);
    }

    #[test]
    fn reads_an_address_as_typed() {
        assert_eq!(address("https://www.example.com/a"), "example.com/a");
        assert_eq!(address("http://docs.example.com/"), "docs.example.com/");
        assert_eq!(address("example.com"), "example.com");
    }

    #[test]
    fn forgets_a_url() {
        let dir = TestDir::new("history-remove");
        let history = History::open(&dir.0.join(FILE_NAME)).unwrap();
        history.visit("https://mail.example.com/", 1_000).unwrap();
        history.visit("https://news.example.net/", 2_000).unwrap();
        history.remove("https://mail.example.com/").unwrap();
        assert_eq!(
            history.entries().unwrap(),
            vec![entry("https://news.example.net/", "", 1, 2_000)]
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
