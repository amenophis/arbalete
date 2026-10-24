//! The native menus of the sidebar's rows and Spaces, built from the sidebar: the app shows them
//! (app/src/mac.rs) and runs the item picked (shell::perform).

use crate::command::{Command, Target};
use arbalette_core::sidebar::{NodeId, NodeKind, Sidebar, Space};
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
    pub enabled: bool,
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Run(Command),
    /// Asks the sidebar to do what only it does, like editing a name: a DOM event and its detail.
    Ask(&'static str, serde_json::Value),
    CopyLink(String),
    /// Runs a command once the user confirms it, with a button named `button`.
    Confirm {
        question: String,
        detail: &'static str,
        button: &'static str,
        command: Command,
    },
}

/// The menu of what was right-clicked, if it has one.
pub fn build(sidebar: &Sidebar, target: Target) -> Option<Vec<Entry>> {
    match target {
        Target::Node(id) => match &sidebar.node(id)?.kind {
            NodeKind::Tab(tab) if sidebar.is_favorite(id) => Some(favorite(id, &tab.url)),
            NodeKind::Tab(tab) => Some(tab_menu(sidebar, id, &tab.url)),
            NodeKind::Folder { .. } => Some(folder(id)),
            // A split's menus are its tabs', from each half of its row.
            NodeKind::Container | NodeKind::Split => None,
        },
        Target::Space(id) => {
            let space = sidebar.spaces().iter().find(|space| space.id == id)?;
            Some(space_menu(sidebar, space))
        }
        Target::SpaceProfile(id) => {
            let space = sidebar.spaces().iter().find(|space| space.id == id)?;
            Some(profiles(sidebar, space))
        }
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
                enabled: true,
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

fn space_menu(sidebar: &Sidebar, space: &Space) -> Vec<Entry> {
    let id = space.id;
    let edit = |field| {
        Action::Ask(
            "arbalette:edit-space",
            json!({ "space": id, "field": field }),
        )
    };
    let delete = Action::Confirm {
        question: format!("Delete the Space “{}”?", space.title),
        detail: "Its tabs and folders will be closed.",
        button: "Delete",
        command: Command::DeleteSpace(id),
    };
    vec![
        item("icon", "Change Space Icon…", edit("emoji")),
        item("rename", "Rename Space…", edit("name")),
        item("color", "Edit Theme Color…", edit("color")),
        Entry::Submenu("Set Profile".to_string(), profiles(sidebar, space)),
        Entry::Separator,
        item(
            "new-folder",
            "New Folder",
            Action::Run(Command::NewFolder(Some(space.pinned))),
        ),
        Entry::Separator,
        item(
            "manage",
            "Manage Spaces…",
            Action::Ask("arbalette:library", json!(null)),
        ),
        Entry::Separator,
        Entry::Item(Item {
            id: "delete".to_string(),
            title: "Delete Space".to_string(),
            checked: false,
            // The browser always keeps a Space.
            enabled: sidebar.spaces().len() > 1,
            action: delete,
        }),
    ]
}

/// The profiles a Space may be attached to, its own checked, and a new one.
fn profiles(sidebar: &Sidebar, space: &Space) -> Vec<Entry> {
    let mut profiles: Vec<_> = sidebar
        .profiles()
        .iter()
        .map(|profile| {
            let profile_id = serde_json::to_string(&profile.id).expect("an id");
            Entry::Item(Item {
                id: format!("profile/{profile_id}"),
                title: profile.name.clone(),
                checked: profile.id == space.profile,
                enabled: true,
                action: Action::Run(Command::SetSpaceProfile(space.id, profile.id)),
            })
        })
        .collect();
    profiles.extend([
        Entry::Separator,
        item(
            "new-profile",
            "New Profile…",
            Action::Run(Command::NewSpaceProfile(space.id)),
        ),
    ]);
    profiles
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
        enabled: true,
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
                Entry::Item(item) if !item.enabled => vec![format!("({})", item.title)],
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
    fn a_space_edits_itself_changes_its_profile_and_deletes_itself_once_confirmed() {
        let mut sidebar = Sidebar::default();
        let work = sidebar.create_profile("Work").unwrap();
        let home = sidebar.spaces()[0].clone();
        let target = Target::Space(home.id);
        let menu = build(&sidebar, target).unwrap();
        assert_eq!(
            titles(&menu),
            [
                "Change Space Icon…",
                "Rename Space…",
                "Edit Theme Color…",
                "Set Profile ▸",
                "  ✓ Default",
                "  Work",
                "  -",
                "  New Profile…",
                "-",
                "New Folder",
                "-",
                "Manage Spaces…",
                "-",
                "Delete Space",
            ]
        );
        let choose = |item: &str| find(build(&sidebar, target).unwrap(), item);
        let edit = |field| {
            Action::Ask(
                "arbalette:edit-space",
                json!({ "space": home.id, "field": field }),
            )
        };
        assert_eq!(choose("icon"), Some(edit("emoji")));
        assert_eq!(choose("rename"), Some(edit("name")));
        assert_eq!(choose("color"), Some(edit("color")));
        let work_id = serde_json::to_string(&work).unwrap();
        assert_eq!(
            choose(&format!("profile/{work_id}")),
            Some(Action::Run(Command::SetSpaceProfile(home.id, work)))
        );
        assert_eq!(
            choose("new-profile"),
            Some(Action::Run(Command::NewSpaceProfile(home.id)))
        );
        assert_eq!(
            choose("new-folder"),
            Some(Action::Run(Command::NewFolder(Some(home.pinned))))
        );
        assert_eq!(
            choose("manage"),
            Some(Action::Ask("arbalette:library", json!(null)))
        );
        assert_eq!(
            choose("delete"),
            Some(Action::Confirm {
                question: "Delete the Space “Home”?".into(),
                detail: "Its tabs and folders will be closed.",
                button: "Delete",
                command: Command::DeleteSpace(home.id),
            })
        );
    }

    #[test]
    fn a_spaces_profile_pill_lists_the_profiles_its_own_checked() {
        let mut sidebar = Sidebar::default();
        let work = sidebar.create_profile("Work").unwrap();
        let home = sidebar.spaces()[0].id;
        let target = Target::SpaceProfile(home);
        let menu = build(&sidebar, target).unwrap();
        assert_eq!(titles(&menu), ["✓ Default", "Work", "-", "New Profile…"]);
        let work_id = serde_json::to_string(&work).unwrap();
        assert_eq!(
            find(menu, &format!("profile/{work_id}")),
            Some(Action::Run(Command::SetSpaceProfile(home, work)))
        );
    }

    #[test]
    fn the_last_space_cannot_be_deleted() {
        let sidebar = Sidebar::default();
        let menu = build(&sidebar, Target::Space(sidebar.spaces()[0].id)).unwrap();
        assert_eq!(titles(&menu).last().unwrap(), "(Delete Space)");
    }

    #[test]
    fn roots_and_unknown_nodes_and_spaces_have_no_menu() {
        let sidebar = Sidebar::default();
        let today = sidebar.spaces()[0].today;
        assert!(build(&sidebar, Target::Node(today)).is_none());
        let unknown = serde_json::from_str("999").unwrap();
        assert!(build(&sidebar, Target::Node(unknown)).is_none());
        let unknown = serde_json::from_str("999").unwrap();
        assert!(build(&sidebar, Target::Space(unknown)).is_none());
    }
}
