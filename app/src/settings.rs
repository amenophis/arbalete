//! The app's settings, the same for every profile, kept in a file of their own. A profile's own
//! settings, like its search engine, are kept with it in the sidebar.

use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

/// The settings' file name in the data directory.
pub const FILE_NAME: &str = "settings.json";

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// ⌘Q asks before quitting.
    pub warn_before_quit: bool,
    /// The "Today" tabs open at launch are those left open at the last quit; without, every
    /// Space starts with none, its pinned tabs and favorites kept.
    pub restore_session: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            warn_before_quit: false,
            restore_session: true,
        }
    }
}

/// Writes the settings atomically: a crash while saving leaves the previous file intact.
pub fn save(settings: &Settings, path: &Path) -> io::Result<()> {
    let json = serde_json::to_vec_pretty(settings).map_err(io::Error::other)?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, json)?;
    fs::rename(temporary, path)
}

/// Loads the settings, the default ones when there are none or they cannot be read.
pub fn load(path: &Path) -> Settings {
    fs::read(path)
        .ok()
        .and_then(|json| serde_json::from_slice(&json).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_settings_and_defaults_what_is_missing() {
        let dir = std::env::temp_dir().join(format!("arbalete-settings-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE_NAME);
        assert_eq!(load(&path), Settings::default());
        let settings = Settings {
            warn_before_quit: true,
            restore_session: false,
        };
        save(&settings, &path).unwrap();
        assert_eq!(load(&path), settings);
        fs::write(&path, r#"{ "warnBeforeQuit": true }"#).unwrap();
        assert!(load(&path).restore_session);
        fs::write(&path, "not json").unwrap();
        assert_eq!(load(&path), Settings::default());
        fs::remove_dir_all(dir).unwrap();
    }
}
