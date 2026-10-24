//! The icons the sidebar shows, kept as PNG files in each profile's directory, so that a tab
//! shows its icon before its page loads: unloaded, after a restart or an import.
//!
//! A tab's `favicon` names where its icon comes from: the icon's url, or the imported item it was
//! found for. Its file is named after the md5 of that name: tabs showing the same icon share one
//! file, and a file never changes, as another icon has another name.

use crate::sidebar::{ProfileId, Sidebar};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::{fs, io};

/// The directory of the icons in a profile's directory. Not `Favicons`: Chromium's own database
/// has that name there.
pub const DIR: &str = "Sidebar Favicons";

/// The file of the icon `source`, in `DIR`.
pub fn file_name(source: &str) -> String {
    format!("{:x}.png", md5::compute(source))
}

/// Whether `name` is the name of an icon's file, so that a request names no other file.
pub fn is_file_name(name: &str) -> bool {
    name.strip_suffix(".png")
        .is_some_and(|key| key.len() == 32 && key.bytes().all(|b| b.is_ascii_hexdigit()))
}

pub fn path(profile_dir: &Path, source: &str) -> PathBuf {
    profile_dir.join(DIR).join(file_name(source))
}

pub fn write(profile_dir: &Path, source: &str, png: &[u8]) -> io::Result<()> {
    fs::create_dir_all(profile_dir.join(DIR))?;
    fs::write(path(profile_dir, source), png)
}

/// Forgets the icons of tabs whose file is missing, as an older session's icon urls, and deletes
/// the files no tab of their profile shows any more. Errors are left for the next launch.
pub fn sweep(sidebar: &mut Sidebar, profile_dir: impl Fn(ProfileId) -> PathBuf) {
    let profiles: Vec<_> = sidebar.profiles().iter().map(|p| p.id).collect();
    for profile in profiles {
        let dir = profile_dir(profile).join(DIR);
        let files: HashSet<String> = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .collect();
        let tabs: Vec<_> = sidebar
            .tabs()
            .filter(|&(id, _)| sidebar.profile_of(id) == Some(profile))
            .filter_map(|(id, tab)| Some((id, file_name(tab.favicon.as_deref()?))))
            .collect();
        let mut shown = HashSet::new();
        for (id, name) in tabs {
            if files.contains(&name) {
                shown.insert(name);
            } else if let Some(tab) = sidebar.tab_mut(id) {
                tab.favicon = None;
            }
        }
        for name in files.difference(&shown) {
            let _ = fs::remove_file(dir.join(name));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::TestDir;

    #[test]
    fn names_a_file_after_the_md5_of_its_source() {
        assert_eq!(file_name(""), "d41d8cd98f00b204e9800998ecf8427e.png");
        assert!(is_file_name(&file_name("https://example.com/favicon.ico")));
        for name in [
            "../session.json",
            "d41d8cd98f00b204e9800998ecf8427e",
            "a.png",
        ] {
            assert!(!is_file_name(name), "{name}");
        }
    }

    #[test]
    fn keeps_the_files_tabs_show_and_forgets_the_icons_without_one() {
        let dir = TestDir::new("favicon-sweep");
        let mut sidebar = Sidebar::default();
        let profile = sidebar.profiles()[0].id;
        let today = sidebar.spaces()[0].today;
        let [shown, missing] = ["https://a.example/", "https://b.example/"].map(|url| {
            let id = sidebar.add_tab(today, url, url).unwrap();
            sidebar.tab_mut(id).unwrap().favicon = Some(format!("{url}favicon.ico"));
            id
        });
        let profile_dir = dir.0.join("profile");
        write(&profile_dir, "https://a.example/favicon.ico", b"png").unwrap();
        write(&profile_dir, "https://gone.example/favicon.ico", b"png").unwrap();
        sweep(&mut sidebar, |p| {
            assert_eq!(p, profile);
            profile_dir.clone()
        });
        let favicon = |sidebar: &mut Sidebar, id| sidebar.tab_mut(id).unwrap().favicon.clone();
        assert_eq!(
            favicon(&mut sidebar, shown).as_deref(),
            Some("https://a.example/favicon.ico")
        );
        assert_eq!(favicon(&mut sidebar, missing), None);
        assert!(path(&profile_dir, "https://a.example/favicon.ico").exists());
        assert!(!path(&profile_dir, "https://gone.example/favicon.ico").exists());
    }
}
