//! Reads Arc's sidebar, `StorableSidebar.json`, into Spaces and tabs.
//!
//! The format is described in docs/PLAN.md, section 4. Items Arbalette has no equivalent for, or
//! that are malformed, are skipped: one odd item must not stop the import of the others.

use crate::sidebar::{Color, NodeId, ProfileId, Sidebar};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::{fs, io, path::Path};

/// Reads the sidebar in Arc's data directory `dir`, `StorableSidebar.json`. `None` when Arc saved
/// none there.
pub fn read(dir: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(dir.join("StorableSidebar.json")) {
        Ok(json) => Ok(Some(json)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Adds the Spaces and the favorites of one Arc profile, the one in `User Data/<directory>`, to
/// `profile`, from the content of `StorableSidebar.json`.
pub fn add(
    sidebar: &mut Sidebar,
    storable_sidebar: &str,
    directory: &str,
    profile: ProfileId,
) -> Result<(), serde_json::Error> {
    let json: Value = serde_json::from_str(storable_sidebar)?;
    let containers = json["sidebar"]["containers"].as_array();
    let Some(container) = containers
        .into_iter()
        .flatten()
        .find(|c| c.get("spaces").is_some())
    else {
        return Ok(());
    };
    let mut import = Import {
        sidebar,
        items: pairs(&container["items"])
            .filter_map(|(id, item)| Some((id.as_str()?, item)))
            .collect(),
        directory,
        profile,
        imported: HashSet::new(),
    };
    for (_, space) in pairs(&container["spaces"]) {
        import.space(space);
    }
    for (arc_profile, favorites) in pairs(&container["topAppsContainerIDs"]) {
        import.favorites(arc_profile, favorites);
    }
    Ok(())
}

struct Import<'a> {
    sidebar: &'a mut Sidebar,
    items: HashMap<&'a str, &'a Value>,
    /// The directory of the Arc profile imported, and the profile it goes into.
    directory: &'a str,
    profile: ProfileId,
    /// Guards against an item listed twice, or inside itself.
    imported: HashSet<&'a str>,
}

impl<'a> Import<'a> {
    /// The Spaces of other Arc profiles are left out.
    fn space(&mut self, space: &'a Value) -> Option<()> {
        if profile_directory(&space["profile"])? != self.directory {
            return None;
        }
        let containers: HashMap<_, _> = pairs(&space["containerIDs"])
            .filter_map(|(kind, id)| Some((kind.as_str()?, id.as_str()?)))
            .collect();
        let (arc_pinned, arc_today) = (*containers.get("pinned")?, *containers.get("unpinned")?);
        let title = text(&space["title"]).unwrap_or("Space");
        let id = self.sidebar.add_space(title, self.profile).ok()?;
        let custom = &space["customInfo"];
        if let Some(emoji) = custom["iconType"]["emoji_v2"].as_str() {
            self.sidebar.set_space_emoji(id, emoji).ok()?;
        }
        if let Some(color) = nearest_color(&custom["windowTheme"]["primaryColorPalette"]["midTone"])
        {
            self.sidebar.set_space_color(id, color).ok()?;
        }
        let space = self.sidebar.spaces().last()?;
        let (pinned, today) = (space.pinned, space.today);
        self.children(arc_pinned, pinned);
        self.children(arc_today, today);
        Some(())
    }

    fn favorites(&mut self, arc_profile: &Value, container: &'a Value) -> Option<()> {
        if profile_directory(arc_profile)? != self.directory {
            return None;
        }
        let favorites = self
            .sidebar
            .profiles()
            .iter()
            .find(|p| p.id == self.profile)?;
        let favorites = favorites.favorites;
        self.children(container.as_str()?, favorites);
        Some(())
    }

    /// Imports the children of Arc's item `id` at the end of `parent`.
    fn children(&mut self, id: &str, parent: NodeId) {
        let Some(item) = self.items.get(id).copied() else {
            return;
        };
        let children = item["childrenIds"].as_array().into_iter().flatten();
        for child in children.filter_map(Value::as_str) {
            self.item(child, parent);
        }
    }

    fn item(&mut self, id: &'a str, parent: NodeId) {
        let Some(item) = self.items.get(id).copied() else {
            return;
        };
        if !self.imported.insert(id) {
            return;
        }
        let data = &item["data"];
        if let Some(url) = data["tab"]["savedURL"].as_str() {
            // The title the user gave the tab, else the page's.
            let title = text(&item["title"]).or(text(&data["tab"]["savedTitle"]));
            // The parent is a container or a folder: only full favorites refuse the tab, which is
            // then left out.
            let _ = self.sidebar.add_tab(parent, url, title.unwrap_or(url));
        } else if data.get("list").is_some() {
            let title = text(&item["title"]).unwrap_or("Folder");
            // Favorites hold only tabs: a folder among them hands them its tabs.
            let folder = self.sidebar.add_folder(parent, title).unwrap_or(parent);
            self.children(id, folder);
        } else if data.get("splitView").is_some() {
            // Arbalette has no split views yet: their tabs take their place.
            self.children(id, parent);
        }
    }
}

/// Arc stores its maps as arrays alternating keys and values.
fn pairs(array: &Value) -> impl Iterator<Item = (&Value, &Value)> {
    let array = array.as_array().map(Vec::as_slice).unwrap_or_default();
    array
        .as_chunks::<2>()
        .0
        .iter()
        .map(|[key, value]| (key, value))
}

/// The directory of an Arc profile in `User Data`, as Chromium names it.
fn profile_directory(profile: &Value) -> Option<String> {
    if profile["default"] == true {
        return Some("Default".to_string());
    }
    let directory = profile["custom"]["_0"]["directoryBasename"].as_str()?;
    Some(directory.to_string())
}

fn text(value: &Value) -> Option<&str> {
    value.as_str().filter(|text| !text.trim().is_empty())
}

/// The colors of ui/src/bridge.ts.
const PALETTE: [(Color, [u8; 3]); 8] = [
    (Color::Purple, [0x6e, 0x56, 0xcf]),
    (Color::Blue, [0x3e, 0x7b, 0xfa]),
    (Color::Green, [0x30, 0xa4, 0x6c]),
    (Color::Yellow, [0xe5, 0xb8, 0x00]),
    (Color::Orange, [0xf7, 0x6b, 0x15]),
    (Color::Red, [0xe5, 0x48, 0x4d]),
    (Color::Pink, [0xd6, 0x40, 0x9f]),
    (Color::Gray, [0x8b, 0x8d, 0x98]),
];

/// The palette's color closest to an Arc color, whose components go from 0 to 1.
fn nearest_color(color: &Value) -> Option<Color> {
    let rgb = ["red", "green", "blue"].map(|c| color[c].as_f64());
    let [Some(red), Some(green), Some(blue)] = rgb else {
        return None;
    };
    let distance = |palette: [u8; 3]| -> f64 {
        let [r, g, b] = palette.map(|c| f64::from(c) / 255.0);
        (r - red).powi(2) + (g - green).powi(2) + (b - blue).powi(2)
    };
    PALETTE
        .iter()
        .min_by(|a, b| distance(a.1).total_cmp(&distance(b.1)))
        .map(|(color, _)| *color)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sidebar::NodeKind;
    use serde_json::json;

    const SIDEBAR: &str = include_str!("../../tests/fixtures/arc/StorableSidebar.json");
    /// The fixture's two Arc profiles, each into a profile of its own.
    fn fixture() -> Sidebar {
        let mut sidebar = Sidebar::empty();
        for (directory, name) in [("Default", "Personal"), ("Profile 1", "Work")] {
            let profile = sidebar.add_profile(name);
            add(&mut sidebar, SIDEBAR, directory, profile).unwrap();
        }
        sidebar
    }

    /// A node's content, as urls and `folder[...]`, for comparisons that read like the sidebar.
    fn content(sidebar: &Sidebar, id: NodeId) -> Vec<String> {
        let node = sidebar.node(id).unwrap();
        node.children
            .iter()
            .map(|&child| match &sidebar.node(child).unwrap().kind {
                NodeKind::Tab(tab) => tab.url.clone(),
                NodeKind::Folder { title, .. } => {
                    format!("{title}[{}]", content(sidebar, child).join(", "))
                }
                NodeKind::Container | NodeKind::Split => panic!("no container or split imported"),
            })
            .collect()
    }

    #[test]
    fn imports_only_the_spaces_and_favorites_of_one_arc_profile_into_another() {
        let mut sidebar = Sidebar::default();
        let profile = sidebar.profiles()[0].id;
        add(&mut sidebar, SIDEBAR, "Profile 1", profile).unwrap();
        let titles: Vec<_> = sidebar.spaces().iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["Home", "Office"]);
        assert_eq!(sidebar.spaces()[1].profile, profile);
        let favorites = sidebar.profiles()[0].favorites;
        assert_eq!(
            content(&sidebar, favorites),
            ["https://tracker.example.com/"]
        );
    }

    #[test]
    fn imports_spaces_in_order_with_their_profile_emoji_and_nearest_color() {
        let sidebar = fixture();
        let [personal, work] = [0, 1].map(|i| sidebar.profiles()[i].id);
        let spaces: Vec<_> = sidebar
            .spaces()
            .iter()
            .map(|s| (s.title.as_str(), s.emoji.as_deref(), s.color, s.profile))
            .collect();
        assert_eq!(
            spaces,
            [
                ("Home", Some("🏡"), Color::Red, personal),
                ("Space", None, Color::Purple, personal),
                ("Office", Some("💼"), Color::Blue, work),
            ]
        );
        assert_eq!(sidebar.displayed().title, "Home");
    }

    #[test]
    fn imports_the_favorites_of_each_profile() {
        let sidebar = fixture();
        let [personal, work] = [0, 1].map(|i| sidebar.profiles()[i].favorites);
        assert_eq!(
            content(&sidebar, personal),
            ["https://mail.example.com/", "https://calendar.example.com/"]
        );
        assert_eq!(content(&sidebar, work), ["https://tracker.example.com/"]);
        let tracker = sidebar.node(work).unwrap().children[0];
        let NodeKind::Tab(tab) = &sidebar.node(tracker).unwrap().kind else {
            panic!("not a tab")
        };
        assert_eq!(tab.title, "Tracker");
    }

    #[test]
    fn imports_pinned_tabs_and_nested_folders_with_their_saved_urls() {
        let mut sidebar = fixture();
        let home = sidebar.spaces()[0].clone();
        assert_eq!(
            content(&sidebar, home.pinned),
            [
                "Docs[https://docs.example.com/guide, Recipes[https://recipes.example.org/bread]]",
                "https://news.example.net/",
            ]
        );
        let news = sidebar.node(home.pinned).unwrap().children[1];
        let tab = sidebar.tab_mut(news).unwrap();
        assert_eq!(tab.title, "https://news.example.net/");
        assert_eq!(tab.saved_url.as_deref(), Some("https://news.example.net/"));
        let office = sidebar.spaces()[2].clone();
        assert_eq!(
            content(&sidebar, office.pinned),
            ["https://wiki.example.com/"]
        );
    }

    #[test]
    fn imports_today_tabs_with_split_views_as_separate_tabs() {
        let sidebar = fixture();
        let [home, reading] = [0, 1].map(|i| sidebar.spaces()[i].today);
        assert_eq!(
            content(&sidebar, home),
            [
                "https://search.example.com/?q=arbalette",
                "https://left.example.com/",
                "https://right.example.com/",
            ]
        );
        assert_eq!(
            content(&sidebar, reading),
            ["https://blog.example.org/post"]
        );
    }

    #[test]
    fn imports_a_folder_listed_inside_itself_once() {
        let json = json!({ "sidebar": { "containers": [{
            "topAppsContainerIDs": [{ "default": true }, "favorites"],
            "spaces": ["s", { "profile": { "default": true },
                "containerIDs": ["pinned", "pinned", "unpinned", "today"] }],
            "items": [
                "pinned", { "childrenIds": ["loop"] },
                "loop", { "title": "Loop", "childrenIds": ["loop", "pinned"], "data": { "list": {} } },
                "favorites", { "childrenIds": ["folder"] },
                "folder", { "childrenIds": ["tab"], "data": { "list": {} } },
                "tab", { "data": { "tab": { "savedURL": "https://example.com/" } } },
            ],
        }]}});
        let mut sidebar = Sidebar::empty();
        let profile = sidebar.add_profile("Default");
        add(&mut sidebar, &json.to_string(), "Default", profile).unwrap();
        let space = &sidebar.spaces()[0];
        assert_eq!(content(&sidebar, space.pinned), ["Loop[]"]);
        assert!(content(&sidebar, space.today).is_empty());
        let favorites = sidebar.profiles()[0].favorites;
        assert_eq!(content(&sidebar, favorites), ["https://example.com/"]);
    }

    #[test]
    fn reads_the_sidebar_in_arc_s_data_directory() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/arc");
        assert_eq!(read(&dir).unwrap().as_deref(), Some(SIDEBAR));
        assert_eq!(read(&dir.join("missing")).unwrap(), None);
    }

    #[test]
    fn imports_nothing_from_a_file_without_spaces() {
        let mut sidebar = Sidebar::default();
        let profile = sidebar.profiles()[0].id;
        let invalid = add(&mut sidebar, "not json", "Default", profile);
        assert!(invalid.is_err());
        let empty = json!({ "sidebar": { "containers": [{ "global": {} }, { "spaces": [] }] } });
        for json in ["{}".to_string(), empty.to_string()] {
            add(&mut sidebar, &json, "Default", profile).unwrap();
        }
        assert_eq!(sidebar, Sidebar::default());
    }

    #[test]
    fn maps_a_color_to_the_nearest_of_the_palette() {
        let color =
            |red, green, blue| nearest_color(&json!({ "red": red, "green": green, "blue": blue }));
        assert_eq!(color(0.2, 0.65, 0.4), Some(Color::Green));
        assert_eq!(color(0.55, 0.55, 0.58), Some(Color::Gray));
        assert_eq!(color(1.0, 0.75, 0.0), Some(Color::Yellow));
        assert_eq!(nearest_color(&json!({ "red": 1.0 })), None);
    }
}
