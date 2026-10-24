//! Bookmarks read from another browser, and how they enter a Space.
//!
//! Bookmarks are imported into an existing Space, which the import assistant picks or creates
//! (a new profile with its Space, or a Space of an existing profile). Each root folder of the
//! browser becomes a pinned folder of the Space, after its pinned tabs; the bookmarks bar becomes
//! one too, or the Favorites of the Space's profile.

use crate::sidebar::{self, NodeId, Sidebar, SpaceId};
use serde::Deserialize;

#[derive(Debug, Default, PartialEq)]
pub struct Bookmarks {
    pub bar: Folder,
    /// The browser's other root folders, such as "Other bookmarks".
    pub others: Vec<Folder>,
}

#[derive(Debug, Default, PartialEq)]
pub struct Folder {
    pub title: String,
    pub children: Vec<Bookmark>,
}

#[derive(Debug, PartialEq)]
pub enum Bookmark {
    Link { title: String, url: String },
    Folder(Folder),
}

/// Where the bookmarks bar goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Bar {
    Favorites,
    PinnedFolder,
}

/// Adds `bookmarks` to `space`. Empty root folders are left out.
pub fn add(
    sidebar: &mut Sidebar,
    space: SpaceId,
    bookmarks: &Bookmarks,
    bar: Bar,
) -> Result<(), sidebar::Error> {
    let favorites = sidebar
        .favorites_of(space)
        .ok_or(sidebar::Error::UnknownSpace)?;
    let pinned = sidebar.spaces().iter().find(|s| s.id == space);
    let pinned = pinned.ok_or(sidebar::Error::UnknownSpace)?.pinned;
    if bar == Bar::Favorites {
        add_links(sidebar, favorites, &bookmarks.bar.children)?;
    }
    let bar = (bar == Bar::PinnedFolder).then_some(&bookmarks.bar);
    for folder in bar.into_iter().chain(&bookmarks.others) {
        if !folder.children.is_empty() {
            add_folder(sidebar, pinned, folder)?;
        }
    }
    Ok(())
}

fn add_folder(
    sidebar: &mut Sidebar,
    parent: NodeId,
    folder: &Folder,
) -> Result<(), sidebar::Error> {
    let id = sidebar.add_folder(parent, title(&folder.title, "Folder"))?;
    for child in &folder.children {
        match child {
            Bookmark::Link { title: name, url } => {
                sidebar.add_tab(id, url, title(name, url))?;
            }
            Bookmark::Folder(folder) => add_folder(sidebar, id, folder)?,
        }
    }
    Ok(())
}

/// Favorites hold only tabs: a folder hands them its links.
fn add_links(
    sidebar: &mut Sidebar,
    parent: NodeId,
    bookmarks: &[Bookmark],
) -> Result<(), sidebar::Error> {
    for bookmark in bookmarks {
        match bookmark {
            Bookmark::Link { title: name, url } => {
                sidebar.add_tab(parent, url, title(name, url))?;
            }
            Bookmark::Folder(folder) => add_links(sidebar, parent, &folder.children)?,
        }
    }
    Ok(())
}

fn title<'a>(title: &'a str, fallback: &'a str) -> &'a str {
    if title.trim().is_empty() {
        fallback
    } else {
        title
    }
}

#[cfg(test)]
pub(crate) fn link(title: &str, url: &str) -> Bookmark {
    Bookmark::Link {
        title: title.to_string(),
        url: url.to_string(),
    }
}

#[cfg(test)]
pub(crate) fn folder(title: &str, children: Vec<Bookmark>) -> Folder {
    Folder {
        title: title.to_string(),
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sidebar::NodeKind;

    fn bookmarks() -> Bookmarks {
        Bookmarks {
            bar: folder(
                "Bookmarks bar",
                vec![
                    link("Mail", "https://mail.example.com/"),
                    Bookmark::Folder(folder("", vec![link("", "https://docs.example.com/")])),
                ],
            ),
            others: vec![
                folder(
                    "Other bookmarks",
                    vec![link("News", "https://news.example.net/")],
                ),
                folder("Mobile bookmarks", vec![]),
            ],
        }
    }

    /// A node's content, as titles and `folder[...]`, for comparisons that read like the sidebar.
    fn content(sidebar: &Sidebar, id: NodeId) -> Vec<String> {
        let node = sidebar.node(id).unwrap();
        node.children
            .iter()
            .map(|&child| match &sidebar.node(child).unwrap().kind {
                NodeKind::Tab(tab) => format!("{} {}", tab.title, tab.url),
                NodeKind::Folder { title, .. } => {
                    format!("{title}[{}]", content(sidebar, child).join(", "))
                }
                NodeKind::Container | NodeKind::Split => panic!("no container or split imported"),
            })
            .collect()
    }

    fn setup() -> (Sidebar, SpaceId) {
        let mut sidebar = Sidebar::default();
        let space = sidebar.spaces()[0].id;
        sidebar
            .open_tab(space, "https://today.example.com/")
            .unwrap();
        let pinned = sidebar.spaces()[0].pinned;
        sidebar
            .add_tab(pinned, "https://pinned.example.com/", "Pinned")
            .unwrap();
        (sidebar, space)
    }

    #[test]
    fn adds_the_bar_to_the_favorites_and_other_roots_as_pinned_folders() {
        let (mut sidebar, space) = setup();
        add(&mut sidebar, space, &bookmarks(), Bar::Favorites).unwrap();
        let favorites = sidebar.favorites_of(space).unwrap();
        assert_eq!(
            content(&sidebar, favorites),
            [
                "Mail https://mail.example.com/",
                "https://docs.example.com/ https://docs.example.com/",
            ]
        );
        let space = &sidebar.spaces()[0];
        assert_eq!(
            content(&sidebar, space.pinned),
            [
                "Pinned https://pinned.example.com/",
                "Other bookmarks[News https://news.example.net/]",
            ]
        );
        assert_eq!(content(&sidebar, space.today).len(), 1);
    }

    #[test]
    fn adds_the_bar_as_a_pinned_folder() {
        let (mut sidebar, space) = setup();
        add(&mut sidebar, space, &bookmarks(), Bar::PinnedFolder).unwrap();
        assert!(content(&sidebar, sidebar.favorites_of(space).unwrap()).is_empty());
        let pinned = sidebar.spaces()[0].pinned;
        assert_eq!(
            content(&sidebar, pinned),
            [
                "Pinned https://pinned.example.com/",
                "Bookmarks bar[Mail https://mail.example.com/, \
                 Folder[https://docs.example.com/ https://docs.example.com/]]",
                "Other bookmarks[News https://news.example.net/]",
            ]
        );
        let folder = sidebar.node(pinned).unwrap().children[1];
        let mail = sidebar.node(folder).unwrap().children[0];
        let saved_url = sidebar.tab_mut(mail).unwrap().saved_url.clone();
        assert_eq!(saved_url.as_deref(), Some("https://mail.example.com/"));
    }

    #[test]
    fn refuses_an_unknown_space() {
        let (mut sidebar, space) = setup();
        let mut other = Sidebar::default();
        other.create_space("Other", other.profiles()[0].id).unwrap();
        let unknown = other.spaces()[1].id;
        assert_ne!(unknown, space);
        assert_eq!(
            add(&mut sidebar, unknown, &bookmarks(), Bar::Favorites),
            Err(sidebar::Error::UnknownSpace)
        );
    }
}
