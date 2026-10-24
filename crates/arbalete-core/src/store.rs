//! Saves the sidebar as a JSON file and loads it back.
//!
//! The file carries a format version, so a later version of Arbalete can migrate older files.

use crate::sidebar::Sidebar;
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

const VERSION: u32 = 1;

/// The session's file name in the data directory.
pub const FILE_NAME: &str = "session.json";

#[derive(Serialize, Deserialize)]
struct Saved {
    version: u32,
    sidebar: Sidebar,
}

#[derive(Debug)]
pub enum LoadError {
    Io(io::Error),
    Invalid(serde_json::Error),
    UnsupportedVersion(u32),
}

/// Writes the sidebar atomically: a crash while saving leaves the previous file intact.
pub fn save(sidebar: &Sidebar, path: &Path) -> io::Result<()> {
    let saved = Saved {
        version: VERSION,
        sidebar: sidebar.clone(),
    };
    let json = serde_json::to_vec_pretty(&saved).map_err(io::Error::other)?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, json)?;
    fs::rename(temporary, path)
}

/// Loads the sidebar, or `None` when nothing was saved yet.
pub fn load(path: &Path) -> Result<Option<Sidebar>, LoadError> {
    let json = match fs::read(path) {
        Ok(json) => json,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(LoadError::Io(error)),
    };
    let saved: Saved = serde_json::from_slice(&json).map_err(LoadError::Invalid)?;
    if saved.version != VERSION {
        return Err(LoadError::UnsupportedVersion(saved.version));
    }
    Ok(Some(saved.sidebar))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sidebar::Color;
    use std::path::PathBuf;

    /// A fresh directory per test, so tests can run in parallel.
    fn directory(test: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("arbalete-store-{}-{test}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn saves_and_loads_the_sidebar() {
        let path = directory("round-trip").join("sidebar.json");
        let mut sidebar = Sidebar::default();
        let home = sidebar.spaces()[0].clone();
        let folder = sidebar.add_folder(home.pinned, "Docs").unwrap();
        let tab = sidebar.open_tab(home.id, "https://example.com/").unwrap();
        sidebar.move_node(tab, folder, 0).unwrap();
        let work = sidebar.add_space("Work", home.profile).unwrap();
        sidebar.display(work).unwrap();
        sidebar.set_space_emoji(work, "💼").unwrap();
        sidebar.set_space_color(work, Color::Green).unwrap();

        save(&sidebar, &path).unwrap();
        assert_eq!(load(&path).unwrap(), Some(sidebar));
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn loads_nothing_before_the_first_save() {
        let path = directory("missing").join("sidebar.json");
        assert!(matches!(load(&path), Ok(None)));
    }

    #[test]
    fn rejects_an_unknown_version() {
        let path = directory("version").join("sidebar.json");
        save(&Sidebar::default(), &path).unwrap();
        let json = fs::read_to_string(&path)
            .unwrap()
            .replace("\"version\": 1", "\"version\": 99");
        fs::write(&path, json).unwrap();
        assert!(matches!(
            load(&path),
            Err(LoadError::UnsupportedVersion(99))
        ));
    }

    #[test]
    fn rejects_an_invalid_file() {
        let path = directory("invalid").join("sidebar.json");
        fs::write(&path, "not json").unwrap();
        assert!(matches!(load(&path), Err(LoadError::Invalid(_))));
    }
}
