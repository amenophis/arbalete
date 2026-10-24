//! Turns other browsers' data into Arbalette's.

pub mod arc;
pub mod bookmarks;
pub mod browsers;
pub mod chromium;
pub mod firefox;
pub mod safari;

use crate::history::Entry;
use crate::sidebar::{ProfileId, Sidebar, SpaceId};
use bookmarks::{Bar, Bookmarks};
use browsers::{Browser, Family};
use serde::Deserialize;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::{env, fs, io, process};

/// A browser profile to import, as picked in the import assistant: `browser` and `profile` are
/// indexes in the browsers detected. It goes into the existing profile `into`, or a new one.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Choice {
    pub browser: usize,
    pub profile: usize,
    #[serde(default)]
    pub into: Option<ProfileId>,
    /// Ignored for Arc, which has no bookmarks bar.
    pub bar: Bar,
}

/// What is left to the app once the sidebar has its imports.
#[derive(Debug, Default)]
pub struct Imported {
    /// The visits to add to each profile's history.
    pub histories: Vec<(ProfileId, Vec<Entry>)>,
    /// One message per browser profile that could not be read, fully or in part.
    pub errors: Vec<String>,
}

/// Imports each chosen browser profile into `sidebar`: Arc's Spaces and favorites, other browsers'
/// bookmarks into the profile's first Space. Every profile imported into has a Space, even when
/// its browser's data cannot be read, so that it shows. Choices that name no known browser
/// profile or Arbalette profile are skipped.
pub fn import(sidebar: &mut Sidebar, browsers: &[Browser], choices: &[Choice]) -> Imported {
    let mut imported = Imported::default();
    for choice in choices {
        let Some(browser) = browsers.get(choice.browser) else {
            continue;
        };
        let Some(source) = browser.profiles.get(choice.profile) else {
            continue;
        };
        let name = profile_name(browser, choice.profile);
        let profile = match choice.into {
            None => sidebar.add_profile(&name),
            Some(id) if sidebar.profiles().iter().any(|p| p.id == id) => id,
            Some(_) => continue,
        };
        let target = Target {
            profile,
            name: &name,
        };
        match add(sidebar, browser.family, &source.path, target, choice.bar) {
            Ok(history) => imported.histories.push((profile, history)),
            Err(error) => imported
                .errors
                .push(format!("{} {}: {error}", browser.name, source.name)),
        }
        target.space(sidebar);
    }
    imported
}

/// A new profile is named after its browser, and its browser profile when there are several.
fn profile_name(browser: &Browser, profile: usize) -> String {
    match browser.profiles.as_slice() {
        [_] => browser.name.to_string(),
        profiles => format!("{} ({})", profiles[profile].name, browser.name),
    }
}

#[derive(Clone, Copy)]
struct Target<'a> {
    profile: ProfileId,
    /// The name of a Space created for the profile.
    name: &'a str,
}

impl Target<'_> {
    /// The profile's first Space, created when it has none.
    fn space(self, sidebar: &mut Sidebar) -> SpaceId {
        match sidebar.spaces().iter().find(|s| s.profile == self.profile) {
            Some(space) => space.id,
            None => sidebar
                .add_space(self.name, self.profile)
                .expect("a profile of the sidebar"),
        }
    }
}

/// Adds a browser profile's sidebar or bookmarks to `target` and returns its history.
fn add(
    sidebar: &mut Sidebar,
    family: Family,
    path: &Path,
    target: Target,
    bar: Bar,
) -> Result<Vec<Entry>, String> {
    let add_bookmarks = |sidebar: &mut Sidebar, bookmarks: Bookmarks| {
        let space = target.space(sidebar);
        bookmarks::add(sidebar, space, &bookmarks, bar).map_err(debug)
    };
    match family {
        Family::Arc => {
            // Arc's data directory holds `User Data`, which holds its profiles.
            let dir = path.parent().and_then(Path::parent).unwrap_or(path);
            let directory = path.file_name().and_then(|name| name.to_str());
            arc::add(sidebar, dir, directory.unwrap_or_default(), target.profile).map_err(debug)?;
            chromium::read_history(path).map_err(debug)
        }
        Family::Chromium => {
            add_bookmarks(sidebar, chromium::read_bookmarks(path).map_err(debug)?)?;
            chromium::read_history(path).map_err(debug)
        }
        Family::Firefox => {
            add_bookmarks(sidebar, firefox::read_bookmarks(path).map_err(debug)?)?;
            firefox::read_history(path).map_err(debug)
        }
        Family::Safari => {
            add_bookmarks(sidebar, safari::read_bookmarks(path).map_err(debug)?)?;
            safari::read_history(path).map_err(debug)
        }
    }
}

fn debug(error: impl Debug) -> String {
    format!("{error:?}")
}

/// A copy of a SQLite database, with its write-ahead log, in a temporary directory removed when
/// dropped. The browser keeps its database locked while it runs, and the log holds its latest
/// changes.
pub(crate) struct Copy(PathBuf);

impl Copy {
    /// `None` when there is no database to copy.
    pub fn of(database: &Path) -> io::Result<Option<Self>> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = env::temp_dir().join(format!("arbalette-import-{}-{n}", process::id()));
        fs::create_dir_all(&dir)?;
        let copy = Self(dir);
        match fs::copy(database, copy.file()) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            result => result?,
        };
        let mut wal = database.as_os_str().to_owned();
        wal.push("-wal");
        match fs::copy(wal, copy.0.join("database-wal")) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            result => _ = result?,
        }
        Ok(Some(copy))
    }

    pub fn file(&self) -> PathBuf {
        self.0.join("database")
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A directory under the system's temporary one for a test's files, removed when dropped.
#[cfg(test)]
pub(crate) struct TestDir(pub std::path::PathBuf);

#[cfg(test)]
impl TestDir {
    /// `name` keeps tests running in parallel apart.
    pub fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("arbalette-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub fn write(&self, path: &str, content: impl AsRef<[u8]>) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    pub fn dir(&self, path: &str) {
        std::fs::create_dir_all(self.0.join(path)).unwrap();
    }
}

#[cfg(test)]
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sidebar::NodeKind;
    use browsers::Profile;
    use rusqlite::Connection;

    fn profile(name: &str, path: PathBuf) -> Profile {
        Profile {
            name: name.to_string(),
            path,
        }
    }

    /// Arc's fixture, with two profiles, and a Chrome profile in `chrome` with a bookmark in its
    /// bar and a visit.
    fn browsers(chrome: &TestDir) -> Vec<Browser> {
        let arc = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/arc/User Data");
        chrome.write(
            "Bookmarks",
            r#"{ "roots": { "bookmark_bar": { "children": [
                { "name": "Mail", "type": "url", "url": "https://mail.example.com/" }
            ] } } }"#,
        );
        let db = Connection::open(chrome.0.join("History")).unwrap();
        db.execute_batch(
            "CREATE TABLE urls (url TEXT, title TEXT, visit_count INTEGER,
                last_visit_time INTEGER, hidden INTEGER);
             INSERT INTO urls VALUES ('https://news.example.net/', 'News', 2, 13350000000000000, 0);",
        )
        .unwrap();
        vec![
            Browser {
                name: "Arc",
                family: Family::Arc,
                profiles: vec![
                    profile("Personal", arc.join("Default")),
                    profile("Work", arc.join("Profile 1")),
                ],
            },
            Browser {
                name: "Chrome",
                family: Family::Chromium,
                profiles: vec![profile("Default", chrome.0.clone())],
            },
        ]
    }

    fn choice(browser: usize, profile: usize, into: Option<ProfileId>, bar: Bar) -> Choice {
        Choice {
            browser,
            profile,
            into,
            bar,
        }
    }

    fn urls(sidebar: &Sidebar, container: crate::sidebar::NodeId) -> Vec<String> {
        let children = &sidebar.node(container).unwrap().children;
        let nodes = children.iter().map(|&id| &sidebar.node(id).unwrap().kind);
        nodes
            .map(|kind| match kind {
                NodeKind::Tab(tab) => tab.url.clone(),
                NodeKind::Folder { title, .. } => title.clone(),
                NodeKind::Container => panic!("a container inside a container"),
            })
            .collect()
    }

    #[test]
    fn imports_each_browser_profile_into_a_new_profile() {
        let chrome = TestDir::new("import-new");
        let browsers = browsers(&chrome);
        let mut sidebar = Sidebar::empty();
        let choices = [
            choice(0, 0, None, Bar::Favorites),
            choice(0, 1, None, Bar::Favorites),
            choice(1, 0, None, Bar::Favorites),
        ];
        let imported = import(&mut sidebar, &browsers, &choices);
        assert!(imported.errors.is_empty(), "{:?}", imported.errors);
        let names: Vec<_> = sidebar.profiles().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Personal (Arc)", "Work (Arc)", "Chrome"]);
        let spaces: Vec<_> = sidebar.spaces().iter().map(|s| s.title.as_str()).collect();
        assert_eq!(spaces, ["Home", "Space", "Office", "Chrome"]);
        let favorites = |i: usize| urls(&sidebar, sidebar.profiles()[i].favorites);
        assert_eq!(favorites(1), ["https://tracker.example.com/"]);
        assert_eq!(favorites(2), ["https://mail.example.com/"]);
        let chrome_profile = sidebar.profiles()[2].id;
        let histories: Vec<_> = imported
            .histories
            .iter()
            .map(|(profile, entries)| (*profile, entries.len()))
            .collect();
        let profiles: Vec<_> = sidebar.profiles().iter().map(|p| p.id).collect();
        assert_eq!(
            histories,
            [(profiles[0], 0), (profiles[1], 0), (chrome_profile, 1)]
        );
    }

    #[test]
    fn merges_a_browser_profile_into_an_existing_one() {
        let chrome = TestDir::new("import-merge");
        let browsers = browsers(&chrome);
        let mut sidebar = Sidebar::default();
        let default = sidebar.profiles()[0].id;
        let choices = [
            choice(1, 0, Some(default), Bar::PinnedFolder),
            choice(0, 1, Some(default), Bar::Favorites),
        ];
        let imported = import(&mut sidebar, &browsers, &choices);
        assert_eq!(sidebar.profiles().len(), 1);
        let spaces: Vec<_> = sidebar.spaces().iter().map(|s| s.title.as_str()).collect();
        assert_eq!(spaces, ["Home", "Office"]);
        assert_eq!(
            urls(&sidebar, sidebar.spaces()[0].pinned),
            ["Bookmarks bar"]
        );
        let favorites = urls(&sidebar, sidebar.profiles()[0].favorites);
        assert_eq!(favorites, ["https://tracker.example.com/"]);
        assert_eq!(imported.histories[0].0, default);
    }

    #[test]
    fn gives_a_space_to_a_profile_whose_data_cannot_be_read() {
        let chrome = TestDir::new("import-invalid");
        let browsers = browsers(&chrome);
        chrome.write("Bookmarks", "not json");
        let mut sidebar = Sidebar::empty();
        let imported = import(
            &mut sidebar,
            &browsers,
            &[choice(1, 0, None, Bar::Favorites)],
        );
        assert_eq!(imported.errors.len(), 1);
        assert!(imported.errors[0].starts_with("Chrome Default: Invalid"));
        assert_eq!(sidebar.spaces()[0].title, "Chrome");
        assert!(imported.histories.is_empty());
    }

    #[test]
    fn skips_unknown_browser_profiles_and_arbalette_profiles() {
        let chrome = TestDir::new("import-unknown");
        let browsers = browsers(&chrome);
        let mut sidebar = Sidebar::default();
        let unknown = Sidebar::default().add_profile("Elsewhere");
        let choices = [
            choice(2, 0, None, Bar::Favorites),
            choice(1, 1, None, Bar::Favorites),
            choice(1, 0, Some(unknown), Bar::Favorites),
        ];
        let imported = import(&mut sidebar, &browsers, &choices);
        assert_eq!(sidebar, Sidebar::default());
        assert!(imported.histories.is_empty() && imported.errors.is_empty());
    }

    #[test]
    fn reads_choices_from_the_assistant() {
        let choice: Choice = serde_json::from_str(
            r#"{ "browser": 1, "profile": 0, "into": null, "bar": "pinnedFolder" }"#,
        )
        .unwrap();
        assert_eq!(choice, self::choice(1, 0, None, Bar::PinnedFolder));
    }
}
