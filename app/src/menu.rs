//! The native menus of the sidebar's rows, built from the sidebar: the app shows them (app/src/mac.rs)
//! and runs the item picked (shell::perform).

use crate::command::{Command, Target};
use arbalette_core::sidebar::{NodeId, NodeKind, Sidebar};
use serde_json::json;

pub enum Entry {
    Item(Item),
    Submenu(String, Vec<Entry>),
    Separator,
}

pub struct Item {
    /// Names the item for the end-to-end checks (Command::ChooseMenu).
    pub id: String,
    pub title: String,
    pub checked: bool,
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Run(Command),
    /// Asks the sidebar to do what only it does, like editing a name: a DOM event and its detail.
    Ask(&'static str, serde_json::Value),
    CopyLink(String),
}

/// The menu of what was right-clicked, if it has one.
pub fn build(sidebar: &Sidebar, target: Target) -> Option<Vec<Entry>> {
    match target {
        Target::Node(id) => match &sidebar.node(id)?.kind {
            NodeKind::Tab(tab) if sidebar.is_favorite(id) => Some(favorite(id, &tab.url)),
            NodeKind::Tab(tab) => Some(tab_menu(sidebar, id, &tab.url)),
            NodeKind::Folder { .. } => Some(folder(id)),
            NodeKind::Container => None,
        },
    }
}

/// The action of the item named `id`, in submenus too.
pub fn find(entries: Vec<Entry>, id: &str) -> Option<Action> {
    entries.into_iter().find_map(|entry| match entry {
        Entry::Item(item) if item.id == id => Some(item.action),
        Entry::Submenu(_, entries) => find(entries, id),
        _ => None,
    })
}

fn tab_menu(sidebar: &Sidebar, tab: NodeId, url: &str) -> Vec<Entry> {
    let pin = if sidebar.is_pinned(tab) {
        "Unpin Tab"
    } else {
        "Pin Tab"
    };
    let current = sidebar.space_of(tab);
    let mut spaces: Vec<_> = sidebar
        .spaces()
        .iter()
        .map(|space| {
            let id = serde_json::to_string(&space.id).expect("an id");
            Entry::Item(Item {
                id: format!("move/{id}"),
                title: match &space.emoji {
                    Some(emoji) => format!("{emoji}  {}", space.title),
                    None => space.title.clone(),
                },
                checked: Some(space.id) == current,
                action: Action::Run(Command::MoveToSpace(tab, space.id)),
            })
        })
        .collect();
    spaces.extend([
        Entry::Separator,
        item(
            "new-space",
            "New Space…",
            Action::Ask("arbalette:new-space", json!(tab)),
        ),
    ]);
    vec![
        copy_link(url),
        item("pin", pin, Action::Run(Command::Pin(Some(tab)))),
        Entry::Submenu("Move to".to_string(), spaces),
        Entry::Separator,
        item("close", "Close Tab", Action::Run(Command::Close(Some(tab)))),
    ]
}

fn folder(folder: NodeId) -> Vec<Entry> {
    vec![
        item(
            "rename",
            "Rename Folder",
            Action::Ask("arbalette:rename", json!(folder)),
        ),
        item(
            "new-folder",
            "New Folder",
            Action::Run(Command::NewFolder(Some(folder))),
        ),
        Entry::Separator,
        item(
            "close-tabs",
            "Close Tabs in Folder",
            Action::Run(Command::CloseFolderTabs(folder)),
        ),
    ]
}

fn favorite(tab: NodeId, url: &str) -> Vec<Entry> {
    vec![
        copy_link(url),
        Entry::Separator,
        item(
            "remove",
            "Remove from Favorites",
            Action::Run(Command::RemoveFavorite(tab)),
        ),
    ]
}

fn copy_link(url: &str) -> Entry {
    item("copy-link", "Copy Link", Action::CopyLink(url.to_string()))
}

fn item(id: &str, title: &str, action: Action) -> Entry {
    Entry::Item(Item {
        id: id.to_string(),
        title: title.to_string(),
        checked: false,
        action,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The titles of a menu, a submenu's items after its title, `-` for a separator, `✓` before
    /// a checked item.
    fn titles(entries: &[Entry]) -> Vec<String> {
        entries
            .iter()
            .flat_map(|entry| match entry {
                Entry::Item(item) if item.checked => vec![format!("✓ {}", item.title)],
                Entry::Item(item) => vec![item.title.clone()],
                Entry::Submenu(title, entries) => {
                    let mut titles = vec![format!("{title} ▸")];
                    titles.extend(titles_of(entries));
                    titles
                }
                Entry::Separator => vec!["-".to_string()],
            })
            .collect()
    }

    fn titles_of(entries: &[Entry]) -> Vec<String> {
        titles(entries)
            .into_iter()
            .map(|t| format!("  {t}"))
            .collect()
    }

    fn choose(sidebar: &Sidebar, id: NodeId, item: &str) -> Option<Action> {
        find(build(sidebar, Target::Node(id))?, item)
    }

    #[test]
    fn a_tab_moves_to_any_space_its_own_checked() {
        let mut sidebar = Sidebar::default();
        let home = sidebar.spaces()[0].id;
        let work = sidebar.add_space("Work", sidebar.profiles()[0].id).unwrap();
        sidebar.set_space_emoji(work, "💼").unwrap();
        let tab = sidebar.open_tab(home, "https://example.com/").unwrap();
        let menu = build(&sidebar, Target::Node(tab)).unwrap();
        assert_eq!(
            titles(&menu),
            [
                "Copy Link",
                "Pin Tab",
                "Move to ▸",
                "  ✓ Home",
                "  💼  Work",
                "  -",
                "  New Space…",
                "-",
                "Close Tab",
            ]
        );
        let work_id = serde_json::to_string(&work).unwrap();
        assert_eq!(
            choose(&sidebar, tab, &format!("move/{work_id}")),
            Some(Action::Run(Command::MoveToSpace(tab, work)))
        );
        assert_eq!(
            choose(&sidebar, tab, "copy-link"),
            Some(Action::CopyLink("https://example.com/".into()))
        );
        assert_eq!(
            choose(&sidebar, tab, "new-space"),
            Some(Action::Ask("arbalette:new-space", json!(tab)))
        );
        assert_eq!(
            choose(&sidebar, tab, "close"),
            Some(Action::Run(Command::Close(Some(tab))))
        );
        assert_eq!(choose(&sidebar, tab, "unknown"), None);
    }

    #[test]
    fn a_pinned_tab_unpins() {
        let mut sidebar = Sidebar::default();
        let pinned = sidebar.spaces()[0].pinned;
        let tab = sidebar.add_tab(pinned, "a", "a").unwrap();
        let menu = build(&sidebar, Target::Node(tab)).unwrap();
        assert_eq!(titles(&menu)[1], "Unpin Tab");
        assert_eq!(
            choose(&sidebar, tab, "pin"),
            Some(Action::Run(Command::Pin(Some(tab))))
        );
    }

    #[test]
    fn a_folder_renames_holds_a_new_folder_and_closes_its_tabs() {
        let mut sidebar = Sidebar::default();
        let pinned = sidebar.spaces()[0].pinned;
        let folder = sidebar.add_folder(pinned, "Folder").unwrap();
        let menu = build(&sidebar, Target::Node(folder)).unwrap();
        assert_eq!(
            titles(&menu),
            ["Rename Folder", "New Folder", "-", "Close Tabs in Folder"]
        );
        assert_eq!(
            choose(&sidebar, folder, "rename"),
            Some(Action::Ask("arbalette:rename", json!(folder)))
        );
        assert_eq!(
            choose(&sidebar, folder, "new-folder"),
            Some(Action::Run(Command::NewFolder(Some(folder))))
        );
        assert_eq!(
            choose(&sidebar, folder, "close-tabs"),
            Some(Action::Run(Command::CloseFolderTabs(folder)))
        );
    }

    #[test]
    fn a_favorite_copies_its_link_or_leaves_the_favorites() {
        let mut sidebar = Sidebar::default();
        let favorites = sidebar.profiles()[0].favorites;
        let tab = sidebar.add_tab(favorites, "https://a.test/", "a").unwrap();
        let menu = build(&sidebar, Target::Node(tab)).unwrap();
        assert_eq!(titles(&menu), ["Copy Link", "-", "Remove from Favorites"]);
        assert_eq!(
            choose(&sidebar, tab, "remove"),
            Some(Action::Run(Command::RemoveFavorite(tab)))
        );
    }

    #[test]
    fn roots_and_unknown_nodes_have_no_menu() {
        let sidebar = Sidebar::default();
        let today = sidebar.spaces()[0].today;
        assert!(build(&sidebar, Target::Node(today)).is_none());
        let unknown = serde_json::from_str("999").unwrap();
        assert!(build(&sidebar, Target::Node(unknown)).is_none());
    }
}
