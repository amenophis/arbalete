//! The sidebar: profiles, Spaces and the tree of tabs and folders each Space holds.
//!
//! Every profile has a root container for its favorites, shared by the Spaces attached to it. Every
//! Space has two root containers: its pinned tabs and its "Today" tabs. Tabs and folders are nodes of
//! a single tree, stored flat and linked by `parent` and `children`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProfileId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpaceId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(u64);

/// Isolated web data (cookies, history, extensions), shared by the Spaces attached to it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: ProfileId,
    pub name: String,
    #[serde(default)]
    pub color: Color,
    pub favorites: NodeId,
}

/// The colors a profile or a Space can take: a small palette, which the UI draws (ui/src/bridge.ts).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Color {
    #[default]
    Purple,
    Blue,
    Green,
    Yellow,
    Orange,
    Red,
    Pink,
    Gray,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Space {
    pub id: SpaceId,
    pub title: String,
    /// Shown in the Space switcher instead of the title's initial.
    #[serde(default)]
    pub emoji: Option<String>,
    #[serde(default)]
    pub color: Color,
    pub profile: ProfileId,
    pub pinned: NodeId,
    pub today: NodeId,
    /// The tab shown when the Space is displayed.
    pub selected: Option<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub kind: NodeKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    /// A root: a profile's favorites, or a Space's pinned tabs or "Today" tabs.
    Container,
    Folder {
        title: String,
        expanded: bool,
    },
    Tab(Tab),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tab {
    pub url: String,
    pub title: String,
    pub favicon: Option<String>,
    /// The tab selected when this one opened: closing this one returns to it.
    pub opener: Option<NodeId>,
    /// The url a pinned tab goes back to when it is closed.
    #[serde(default)]
    pub saved_url: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    UnknownProfile,
    UnknownSpace,
    UnknownNode,
    /// Only containers and folders hold other nodes.
    NotAContainer,
    NotATab,
    NotAFolder,
    /// Folders, profiles and Spaces keep a title to be told apart.
    EmptyTitle,
    /// A folder cannot move inside itself or one of its descendants.
    MoveIntoItself,
    /// Root containers cannot be moved or removed.
    RootNode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sidebar {
    profiles: Vec<Profile>,
    spaces: Vec<Space>,
    /// The Space the sidebar shows, `None` for the first one.
    #[serde(default)]
    displayed: Option<SpaceId>,
    nodes: HashMap<NodeId, Node>,
    next_id: u64,
}

impl Default for Sidebar {
    /// A first profile and a first Space, so the browser always has somewhere to open tabs.
    fn default() -> Self {
        let mut sidebar = Self {
            profiles: Vec::new(),
            spaces: Vec::new(),
            displayed: None,
            nodes: HashMap::new(),
            next_id: 1,
        };
        let profile = sidebar.add_profile("Default");
        sidebar
            .add_space("Home", profile)
            .expect("the profile exists");
        sidebar
    }
}

impl Sidebar {
    pub fn profiles(&self) -> &[Profile] {
        &self.profiles
    }

    /// Spaces in sidebar order.
    pub fn spaces(&self) -> &[Space] {
        &self.spaces
    }

    /// The Space the sidebar shows: the first one until another is displayed.
    pub fn displayed(&self) -> &Space {
        self.displayed
            .and_then(|id| self.space(id).ok())
            .unwrap_or(&self.spaces[0])
    }

    /// Shows a Space in the sidebar, with the tab it had selected.
    pub fn display(&mut self, space: SpaceId) -> Result<(), Error> {
        self.space(space)?;
        self.displayed = Some(space);
        Ok(())
    }

    /// The Space at `number` in sidebar order, counted from 1.
    pub fn space_at(&self, number: usize) -> Option<SpaceId> {
        self.spaces.get(number.checked_sub(1)?).map(|s| s.id)
    }

    /// The Space after the displayed one in sidebar order, or before it; none past the first or
    /// the last, as a swipe stops there.
    pub fn next_space(&self, forward: bool) -> Option<SpaceId> {
        let index = self
            .spaces
            .iter()
            .position(|s| s.id == self.displayed().id)?;
        let next = if forward {
            index + 1
        } else {
            index.checked_sub(1)?
        };
        self.spaces.get(next).map(|s| s.id)
    }

    /// The Space whose pinned or "Today" tabs hold a node; favorites belong to no Space.
    pub fn space_of(&self, id: NodeId) -> Option<SpaceId> {
        let root = self.ancestors(id).last()?;
        let space = self
            .spaces
            .iter()
            .find(|s| s.pinned == root || s.today == root);
        space.map(|s| s.id)
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn tab_mut(&mut self, id: NodeId) -> Option<&mut Tab> {
        match &mut self.nodes.get_mut(&id)?.kind {
            NodeKind::Tab(tab) => Some(tab),
            _ => None,
        }
    }

    pub fn add_profile(&mut self, name: &str) -> ProfileId {
        let id = ProfileId(self.next_id());
        let favorites = self.insert(None, NodeKind::Container);
        self.profiles.push(Profile {
            id,
            name: name.to_string(),
            color: Color::default(),
            favorites,
        });
        id
    }

    /// Creates a profile with a Space of the same name, so the profile has somewhere to open tabs.
    pub fn create_profile(&mut self, name: &str) -> Result<ProfileId, Error> {
        if name.trim().is_empty() {
            return Err(Error::EmptyTitle);
        }
        let profile = self.add_profile(name);
        self.add_space(name, profile)?;
        Ok(profile)
    }

    pub fn rename_profile(&mut self, id: ProfileId, name: &str) -> Result<(), Error> {
        if name.trim().is_empty() {
            return Err(Error::EmptyTitle);
        }
        self.profile_mut(id)?.name = name.to_string();
        Ok(())
    }

    pub fn set_profile_color(&mut self, id: ProfileId, color: Color) -> Result<(), Error> {
        self.profile_mut(id)?.color = color;
        Ok(())
    }

    pub fn add_space(&mut self, title: &str, profile: ProfileId) -> Result<SpaceId, Error> {
        if !self.profiles.iter().any(|p| p.id == profile) {
            return Err(Error::UnknownProfile);
        }
        let id = SpaceId(self.next_id());
        let pinned = self.insert(None, NodeKind::Container);
        let today = self.insert(None, NodeKind::Container);
        self.spaces.push(Space {
            id,
            title: title.to_string(),
            emoji: None,
            color: Color::default(),
            profile,
            pinned,
            today,
            selected: None,
        });
        Ok(id)
    }

    /// Creates a Space at the end of the switcher and displays it.
    pub fn create_space(&mut self, title: &str, profile: ProfileId) -> Result<SpaceId, Error> {
        if title.trim().is_empty() {
            return Err(Error::EmptyTitle);
        }
        let space = self.add_space(title, profile)?;
        self.display(space)?;
        Ok(space)
    }

    pub fn rename_space(&mut self, id: SpaceId, title: &str) -> Result<(), Error> {
        if title.trim().is_empty() {
            return Err(Error::EmptyTitle);
        }
        self.space_mut(id)?.title = title.to_string();
        Ok(())
    }

    /// Sets the Space's emoji; a blank one removes it, so the switcher shows the initial again.
    pub fn set_space_emoji(&mut self, id: SpaceId, emoji: &str) -> Result<(), Error> {
        let emoji = emoji.trim();
        self.space_mut(id)?.emoji = (!emoji.is_empty()).then(|| emoji.to_string());
        Ok(())
    }

    pub fn set_space_color(&mut self, id: SpaceId, color: Color) -> Result<(), Error> {
        self.space_mut(id)?.color = color;
        Ok(())
    }

    /// Opens a tab at the end of the Space's "Today" tabs, from the Space's selected tab.
    pub fn open_tab(&mut self, space: SpaceId, url: &str) -> Result<NodeId, Error> {
        let space = self.space(space)?;
        let today = space.today;
        let tab = Tab {
            url: url.to_string(),
            title: url.to_string(),
            favicon: None,
            opener: space.selected,
            saved_url: None,
        };
        Ok(self.insert(Some(today), NodeKind::Tab(tab)))
    }

    /// Shows `tab` in `space`. A favorite is shown in whichever Space it is selected from.
    pub fn select(&mut self, space: SpaceId, tab: NodeId) -> Result<(), Error> {
        match self.nodes.get(&tab).map(|n| &n.kind) {
            None => return Err(Error::UnknownNode),
            Some(NodeKind::Tab(_)) => {}
            Some(_) => return Err(Error::NotATab),
        }
        let space = self.spaces.iter_mut().find(|s| s.id == space);
        space.ok_or(Error::UnknownSpace)?.selected = Some(tab);
        Ok(())
    }

    pub fn add_folder(&mut self, parent: NodeId, title: &str) -> Result<NodeId, Error> {
        self.check_container(parent)?;
        let folder = NodeKind::Folder {
            title: title.to_string(),
            expanded: true,
        };
        Ok(self.insert(Some(parent), folder))
    }

    pub fn rename_folder(&mut self, id: NodeId, new_title: &str) -> Result<(), Error> {
        if new_title.trim().is_empty() {
            return Err(Error::EmptyTitle);
        }
        let (title, _) = self.folder_mut(id)?;
        *title = new_title.to_string();
        Ok(())
    }

    /// Collapses an expanded folder, or expands a collapsed one.
    pub fn toggle_folder(&mut self, id: NodeId) -> Result<(), Error> {
        let (_, expanded) = self.folder_mut(id)?;
        *expanded = !*expanded;
        Ok(())
    }

    /// Moves a tab or a folder, with its content, to `index` among `parent`'s children. Tabs that
    /// become pinned save their url, tabs that stop being pinned forget it. Favorites hold only
    /// tabs.
    pub fn move_node(&mut self, id: NodeId, parent: NodeId, index: usize) -> Result<(), Error> {
        let old_parent = self.movable(id)?;
        self.check_container(parent)?;
        if self.ancestors(parent).any(|ancestor| ancestor == id) {
            return Err(Error::MoveIntoItself);
        }
        if !self.is_tab(id) && self.profiles.iter().any(|p| p.favorites == parent) {
            return Err(Error::NotATab);
        }
        let was_pinned = self.is_pinned(id);
        self.children_mut(old_parent).retain(|child| *child != id);
        let children = self.children_mut(parent);
        children.insert(index.min(children.len()), id);
        self.nodes.get_mut(&id).expect("checked by movable").parent = Some(parent);
        let pinned = self.is_pinned(id);
        if pinned != was_pinned {
            self.save_urls(id, pinned);
        }
        Ok(())
    }

    /// The tab to show once `tab` is closed: the tab it was opened from if still open, else the
    /// next tab in its container, else the previous one.
    pub fn next_after_close(&self, tab: NodeId) -> Option<NodeId> {
        if let Some(opener) = self.open_opener(tab) {
            return Some(opener);
        }
        let siblings = &self.nodes.get(&self.nodes.get(&tab)?.parent?)?.children;
        let tabs: Vec<_> = siblings
            .iter()
            .copied()
            .filter(|&id| id == tab || self.is_tab(id))
            .collect();
        let index = tabs.iter().position(|&id| id == tab)?;
        tabs.get(index + 1)
            .or(index.checked_sub(1).and_then(|i| tabs.get(i)))
            .copied()
    }

    /// The tab after the Space's selected one, or before it, in the order the sidebar shows tabs:
    /// pinned tabs, those of expanded folders included, then "Today" tabs, wrapping around at the
    /// ends. Favorites and tabs hidden in collapsed folders are left out; from one of them, or
    /// from no selection, the first tab, or the last.
    pub fn next_tab(&self, space: SpaceId, forward: bool) -> Option<NodeId> {
        let space = self.space(space).ok()?;
        let mut tabs = Vec::new();
        self.push_visible_tabs(space.pinned, &mut tabs);
        self.push_visible_tabs(space.today, &mut tabs);
        if !forward {
            tabs.reverse();
        }
        let selected = space.selected;
        let next = tabs
            .iter()
            .position(|&id| Some(id) == selected)
            .map_or(0, |i| i + 1);
        tabs.get(next).or(tabs.first()).copied()
    }

    /// The favorite at `number`, counted from 1, of the Space's profile; 9 is the last one, as
    /// ⌘9 is in browsers.
    pub fn favorite(&self, space: SpaceId, number: usize) -> Option<NodeId> {
        let favorites = &self.nodes[&self.favorites_of(space)?].children;
        if number == 9 {
            favorites.last().copied()
        } else {
            favorites.get(number.checked_sub(1)?).copied()
        }
    }

    /// The favorites of the Space's profile, which its Space shows.
    pub fn favorites_of(&self, space: SpaceId) -> Option<NodeId> {
        let profile = self.space(space).ok()?.profile;
        let profile = self.profiles.iter().find(|p| p.id == profile)?;
        Some(profile.favorites)
    }

    /// The profile whose web data shows a node: the profile of its favorites, or of its Space.
    pub fn profile_of(&self, id: NodeId) -> Option<ProfileId> {
        let root = self.ancestors(id).last()?;
        if let Some(profile) = self.profiles.iter().find(|p| p.favorites == root) {
            return Some(profile.id);
        }
        Some(self.space(self.space_of(id)?).ok()?.profile)
    }

    pub fn is_favorite(&self, id: NodeId) -> bool {
        let parent = self.nodes.get(&id).and_then(|node| node.parent);
        self.profiles.iter().any(|p| Some(p.favorites) == parent)
    }

    /// Whether `id` is among a Space's pinned tabs, directly or in a folder.
    pub fn is_pinned(&self, id: NodeId) -> bool {
        self.spaces
            .iter()
            .any(|space| self.ancestors(id).any(|ancestor| ancestor == space.pinned))
    }

    /// Pins a "Today" tab at the end of the Space's pinned tabs, or moves a pinned tab back to the
    /// top of "Today". Favorites stay where they are.
    pub fn toggle_pin(&mut self, space: SpaceId, tab: NodeId) -> Result<(), Error> {
        let (pinned, today) = self.space(space).map(|s| (s.pinned, s.today))?;
        if !self.is_tab(tab) {
            return Err(Error::NotATab);
        }
        if self.ancestors(tab).any(|id| id == today) {
            self.move_node(tab, pinned, usize::MAX)
        } else if self.ancestors(tab).any(|id| id == pinned) {
            self.move_node(tab, today, 0)
        } else {
            Ok(())
        }
    }

    /// Favorites and pinned tabs stay when closed: closing one deselects it in `space` and takes
    /// it back to its saved url, if any. Returns the tab to show instead: the tab it was opened
    /// from if still open, else the Space's last "Today" tab, rather than a neighbor it stays with.
    pub fn close_in_place(&mut self, space: SpaceId, tab: NodeId) -> Result<Option<NodeId>, Error> {
        let opener = self.open_opener(tab);
        let space = self.spaces.iter_mut().find(|s| s.id == space);
        let space = space.ok_or(Error::UnknownSpace)?;
        if space.selected == Some(tab) {
            space.selected = None;
        }
        let today = space.today;
        if let Some(tab) = self.tab_mut(tab)
            && let Some(saved_url) = &tab.saved_url
        {
            tab.url = saved_url.clone();
            tab.title = saved_url.clone();
        }
        let today = &self.nodes[&today].children;
        Ok(opener.or_else(|| today.iter().rev().copied().find(|&id| self.is_tab(id))))
    }

    /// Removes a tab or a folder with everything it contains.
    pub fn remove(&mut self, id: NodeId) -> Result<(), Error> {
        let parent = self.movable(id)?;
        self.children_mut(parent).retain(|child| *child != id);
        let mut pending = vec![id];
        while let Some(id) = pending.pop() {
            let node = self.nodes.remove(&id).expect("children are known nodes");
            pending.extend(node.children);
        }
        for space in &mut self.spaces {
            if space
                .selected
                .is_some_and(|tab| !self.nodes.contains_key(&tab))
            {
                space.selected = None;
            }
        }
        Ok(())
    }

    fn next_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id - 1
    }

    fn insert(&mut self, parent: Option<NodeId>, kind: NodeKind) -> NodeId {
        let id = NodeId(self.next_id());
        self.nodes.insert(
            id,
            Node {
                id,
                parent,
                children: Vec::new(),
                kind,
            },
        );
        if let Some(parent) = parent {
            self.children_mut(parent).push(id);
        }
        id
    }

    /// Saves the url of every tab in `id`, or forgets it.
    fn save_urls(&mut self, id: NodeId, save: bool) {
        let mut pending = vec![id];
        while let Some(id) = pending.pop() {
            let node = self.nodes.get_mut(&id).expect("children are known nodes");
            if let NodeKind::Tab(tab) = &mut node.kind {
                tab.saved_url = save.then(|| tab.url.clone());
            }
            pending.extend(node.children.iter().copied());
        }
    }

    /// Appends the tabs inside `parent` the sidebar shows, depth first.
    fn push_visible_tabs(&self, parent: NodeId, tabs: &mut Vec<NodeId>) {
        for &id in &self.nodes[&parent].children {
            match &self.nodes[&id].kind {
                NodeKind::Tab(_) => tabs.push(id),
                NodeKind::Folder { expanded: true, .. } => self.push_visible_tabs(id, tabs),
                _ => {}
            }
        }
    }

    /// The tab `tab` was opened from, if it is still open.
    fn open_opener(&self, tab: NodeId) -> Option<NodeId> {
        match &self.nodes.get(&tab)?.kind {
            NodeKind::Tab(Tab { opener, .. }) => opener.filter(|&id| self.is_tab(id)),
            _ => None,
        }
    }

    fn folder_mut(&mut self, id: NodeId) -> Result<(&mut String, &mut bool), Error> {
        match &mut self.nodes.get_mut(&id).ok_or(Error::UnknownNode)?.kind {
            NodeKind::Folder { title, expanded } => Ok((title, expanded)),
            _ => Err(Error::NotAFolder),
        }
    }

    fn is_tab(&self, id: NodeId) -> bool {
        matches!(self.nodes.get(&id).map(|n| &n.kind), Some(NodeKind::Tab(_)))
    }

    fn profile_mut(&mut self, id: ProfileId) -> Result<&mut Profile, Error> {
        self.profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or(Error::UnknownProfile)
    }

    fn space_mut(&mut self, id: SpaceId) -> Result<&mut Space, Error> {
        self.spaces
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(Error::UnknownSpace)
    }

    fn space(&self, id: SpaceId) -> Result<&Space, Error> {
        self.spaces
            .iter()
            .find(|s| s.id == id)
            .ok_or(Error::UnknownSpace)
    }

    fn check_container(&self, id: NodeId) -> Result<(), Error> {
        match self.nodes.get(&id).map(|n| &n.kind) {
            None => Err(Error::UnknownNode),
            Some(NodeKind::Tab(_)) => Err(Error::NotAContainer),
            Some(_) => Ok(()),
        }
    }

    /// The parent of a node that may be moved or removed: any node but a root container.
    fn movable(&self, id: NodeId) -> Result<NodeId, Error> {
        self.nodes
            .get(&id)
            .ok_or(Error::UnknownNode)?
            .parent
            .ok_or(Error::RootNode)
    }

    /// `id` itself, then its parent, up to the root.
    fn ancestors(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        std::iter::successors(Some(id), |id| self.nodes.get(id).and_then(|n| n.parent))
    }

    fn children_mut(&mut self, id: NodeId) -> &mut Vec<NodeId> {
        &mut self
            .nodes
            .get_mut(&id)
            .expect("callers check the node exists")
            .children
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home(sidebar: &Sidebar) -> Space {
        sidebar.spaces()[0].clone()
    }

    fn children(sidebar: &Sidebar, id: NodeId) -> Vec<NodeId> {
        sidebar.node(id).unwrap().children.clone()
    }

    #[test]
    fn starts_with_a_profile_and_an_empty_space() {
        let sidebar = Sidebar::default();
        assert_eq!(sidebar.profiles().len(), 1);
        let home = home(&sidebar);
        assert_eq!(home.profile, sidebar.profiles()[0].id);
        assert!(children(&sidebar, home.pinned).is_empty());
        assert!(children(&sidebar, home.today).is_empty());
    }

    #[test]
    fn a_new_profile_has_no_favorites() {
        let mut sidebar = Sidebar::default();
        sidebar.add_profile("Work");
        for profile in sidebar.profiles() {
            assert!(children(&sidebar, profile.favorites).is_empty());
        }
        assert_ne!(
            sidebar.profiles()[0].favorites,
            sidebar.profiles()[1].favorites
        );
    }

    #[test]
    fn creates_a_profile_with_a_space_of_its_own() {
        let mut sidebar = Sidebar::default();
        let work = sidebar.create_profile("Work").unwrap();
        let profile = &sidebar.profiles()[1];
        assert_eq!((profile.id, profile.name.as_str()), (work, "Work"));
        assert_eq!(profile.color, Color::Purple);
        let space = &sidebar.spaces()[1];
        assert_eq!((space.profile, space.title.as_str()), (work, "Work"));
        assert_eq!(sidebar.create_profile(" "), Err(Error::EmptyTitle));
        assert_eq!(sidebar.profiles().len(), 2);
        assert_eq!(sidebar.spaces().len(), 2);
    }

    #[test]
    fn renames_and_recolors_a_profile() {
        let mut sidebar = Sidebar::default();
        let id = sidebar.profiles()[0].id;
        sidebar.rename_profile(id, "Personal").unwrap();
        sidebar.set_profile_color(id, Color::Green).unwrap();
        assert_eq!(sidebar.rename_profile(id, " "), Err(Error::EmptyTitle));
        let profile = &sidebar.profiles()[0];
        assert_eq!(
            (profile.name.as_str(), profile.color),
            ("Personal", Color::Green)
        );
        let unknown = ProfileId(999);
        assert_eq!(
            sidebar.rename_profile(unknown, "a"),
            Err(Error::UnknownProfile)
        );
        assert_eq!(
            sidebar.set_profile_color(unknown, Color::Red),
            Err(Error::UnknownProfile)
        );
    }

    #[test]
    fn a_profile_saved_without_a_color_is_purple() {
        let json = r#"{ "id": 1, "name": "Default", "favorites": 2 }"#;
        let profile: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.color, Color::Purple);
    }

    #[test]
    fn moves_a_tab_into_the_favorites() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let favorites = sidebar.profiles()[0].favorites;
        let tab = sidebar.open_tab(home.id, "https://example.com/").unwrap();
        sidebar.move_node(tab, favorites, 0).unwrap();
        assert!(children(&sidebar, home.today).is_empty());
        assert_eq!(children(&sidebar, favorites), [tab]);
        assert_eq!(sidebar.node(tab).unwrap().parent, Some(favorites));
    }

    #[test]
    fn keeps_the_favorites_root() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let favorites = sidebar.profiles()[0].favorites;
        assert_eq!(sidebar.remove(favorites), Err(Error::RootNode));
        assert_eq!(
            sidebar.move_node(favorites, home.pinned, 0),
            Err(Error::RootNode)
        );
    }

    #[test]
    fn selects_a_favorite_in_any_space_of_its_profile() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let work = sidebar.add_space("Work", home.profile).unwrap();
        let favorites = sidebar.profiles()[0].favorites;
        let a = sidebar.open_tab(home.id, "a").unwrap();
        let b = sidebar.open_tab(home.id, "b").unwrap();
        sidebar.move_node(a, favorites, usize::MAX).unwrap();
        sidebar.move_node(b, favorites, usize::MAX).unwrap();
        sidebar.select(home.id, a).unwrap();
        sidebar.select(work, a).unwrap();
        assert_eq!(sidebar.spaces()[1].selected, Some(a));
        sidebar.remove(a).unwrap();
        assert_eq!(sidebar.spaces()[0].selected, None);
        assert_eq!(sidebar.spaces()[1].selected, None);
    }

    #[test]
    fn closing_a_favorite_deselects_it_and_keeps_it() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let favorites = sidebar.profiles()[0].favorites;
        let a = sidebar.open_tab(home.id, "a").unwrap();
        let b = sidebar.open_tab(home.id, "b").unwrap();
        let favorite = sidebar.open_tab(home.id, "favorite").unwrap();
        let other = sidebar.open_tab(home.id, "other").unwrap();
        sidebar.move_node(favorite, favorites, usize::MAX).unwrap();
        sidebar.move_node(other, favorites, usize::MAX).unwrap();
        assert!(sidebar.is_favorite(favorite));
        assert!(!sidebar.is_favorite(a));
        sidebar.select(home.id, favorite).unwrap();
        assert_eq!(sidebar.close_in_place(home.id, favorite), Ok(Some(b)));
        assert_eq!(sidebar.spaces()[0].selected, None);
        assert_eq!(children(&sidebar, favorites), [favorite, other]);
        sidebar.remove(a).unwrap();
        sidebar.remove(b).unwrap();
        assert_eq!(sidebar.close_in_place(home.id, favorite), Ok(None));
    }

    #[test]
    fn closing_a_favorite_returns_to_its_opener() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let favorites = sidebar.profiles()[0].favorites;
        let opener = sidebar.open_tab(home.id, "opener").unwrap();
        sidebar.open_tab(home.id, "last").unwrap();
        sidebar.select(home.id, opener).unwrap();
        let favorite = sidebar.open_tab(home.id, "favorite").unwrap();
        sidebar.move_node(favorite, favorites, 0).unwrap();
        assert_eq!(sidebar.close_in_place(home.id, favorite), Ok(Some(opener)));
        assert_eq!(
            sidebar.close_in_place(SpaceId(999), favorite),
            Err(Error::UnknownSpace)
        );
    }

    #[test]
    fn pins_a_tab_with_its_url_and_unpins_it_to_the_top_of_today() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let first = sidebar.add_folder(home.pinned, "Docs").unwrap();
        let a = sidebar.open_tab(home.id, "a").unwrap();
        let b = sidebar.open_tab(home.id, "b").unwrap();
        sidebar.toggle_pin(home.id, b).unwrap();
        assert_eq!(children(&sidebar, home.pinned), [first, b]);
        assert!(sidebar.is_pinned(b));
        assert!(!sidebar.is_pinned(a));
        assert_eq!(sidebar.tab_mut(b).unwrap().saved_url.as_deref(), Some("b"));
        sidebar.toggle_pin(home.id, b).unwrap();
        assert_eq!(children(&sidebar, home.pinned), [first]);
        assert_eq!(children(&sidebar, home.today), [b, a]);
        assert_eq!(sidebar.tab_mut(b).unwrap().saved_url, None);
        assert_eq!(sidebar.toggle_pin(home.id, first), Err(Error::NotATab));
    }

    #[test]
    fn unpins_a_tab_from_a_pinned_folder() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let folder = sidebar.add_folder(home.pinned, "Docs").unwrap();
        let tab = sidebar.open_tab(home.id, "a").unwrap();
        sidebar.move_node(tab, folder, 0).unwrap();
        assert!(sidebar.is_pinned(tab));
        sidebar.toggle_pin(home.id, tab).unwrap();
        assert!(children(&sidebar, folder).is_empty());
        assert_eq!(children(&sidebar, home.today), [tab]);
    }

    #[test]
    fn leaves_favorites_where_they_are_on_pin() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let favorites = sidebar.profiles()[0].favorites;
        let tab = sidebar.open_tab(home.id, "a").unwrap();
        sidebar.move_node(tab, favorites, 0).unwrap();
        sidebar.toggle_pin(home.id, tab).unwrap();
        assert_eq!(children(&sidebar, favorites), [tab]);
        assert!(!sidebar.is_pinned(tab));
    }

    fn saved_url(sidebar: &mut Sidebar, tab: NodeId) -> Option<String> {
        sidebar.tab_mut(tab).unwrap().saved_url.clone()
    }

    #[test]
    fn moving_a_tab_into_or_out_of_the_pinned_tabs_pins_or_unpins_it() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let favorites = sidebar.profiles()[0].favorites;
        let folder = sidebar.add_folder(home.pinned, "Docs").unwrap();
        let tab = sidebar.open_tab(home.id, "a").unwrap();
        sidebar.move_node(tab, folder, 0).unwrap();
        assert_eq!(saved_url(&mut sidebar, tab).as_deref(), Some("a"));
        sidebar.tab_mut(tab).unwrap().url = "b".to_string();
        sidebar.move_node(tab, home.pinned, 0).unwrap();
        assert_eq!(saved_url(&mut sidebar, tab).as_deref(), Some("a"));
        sidebar.move_node(tab, home.today, 0).unwrap();
        assert_eq!(saved_url(&mut sidebar, tab), None);
        sidebar.move_node(tab, home.pinned, 0).unwrap();
        sidebar.move_node(tab, favorites, 0).unwrap();
        assert!(sidebar.is_favorite(tab));
        assert_eq!(saved_url(&mut sidebar, tab), None);
    }

    #[test]
    fn moving_a_folder_pins_or_unpins_its_tabs() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let outer = sidebar.add_folder(home.today, "Outer").unwrap();
        let inner = sidebar.add_folder(outer, "Inner").unwrap();
        let tab = sidebar.open_tab(home.id, "a").unwrap();
        sidebar.move_node(tab, inner, 0).unwrap();
        assert_eq!(saved_url(&mut sidebar, tab), None);
        sidebar.move_node(outer, home.pinned, 0).unwrap();
        assert_eq!(saved_url(&mut sidebar, tab).as_deref(), Some("a"));
        sidebar.move_node(outer, home.today, 0).unwrap();
        assert_eq!(saved_url(&mut sidebar, tab), None);
    }

    #[test]
    fn favorites_hold_only_tabs() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let favorites = sidebar.profiles()[0].favorites;
        let folder = sidebar.add_folder(home.pinned, "Docs").unwrap();
        assert_eq!(sidebar.move_node(folder, favorites, 0), Err(Error::NotATab));
        assert_eq!(children(&sidebar, home.pinned), [folder]);
    }

    #[test]
    fn closing_a_pinned_tab_keeps_it_and_takes_it_back_to_its_saved_url() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let today = sidebar.open_tab(home.id, "today").unwrap();
        let pinned = sidebar.open_tab(home.id, "saved").unwrap();
        sidebar.toggle_pin(home.id, pinned).unwrap();
        sidebar.select(home.id, pinned).unwrap();
        let tab = sidebar.tab_mut(pinned).unwrap();
        tab.url = "elsewhere".to_string();
        tab.title = "Elsewhere".to_string();
        assert_eq!(sidebar.close_in_place(home.id, pinned), Ok(Some(today)));
        assert_eq!(sidebar.spaces()[0].selected, None);
        assert_eq!(children(&sidebar, home.pinned), [pinned]);
        let tab = sidebar.tab_mut(pinned).unwrap();
        assert_eq!((tab.url.as_str(), tab.title.as_str()), ("saved", "saved"));
    }

    #[test]
    fn closing_an_unselected_pinned_tab_keeps_the_selection() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let today = sidebar.open_tab(home.id, "today").unwrap();
        let pinned = sidebar.open_tab(home.id, "saved").unwrap();
        sidebar.toggle_pin(home.id, pinned).unwrap();
        sidebar.select(home.id, today).unwrap();
        sidebar.close_in_place(home.id, pinned).unwrap();
        assert_eq!(sidebar.spaces()[0].selected, Some(today));
    }

    #[test]
    fn spaces_can_share_a_profile() {
        let mut sidebar = Sidebar::default();
        let profile = sidebar.profiles()[0].id;
        let work = sidebar.add_space("Work", profile).unwrap();
        assert_eq!(sidebar.spaces()[1].id, work);
        assert_eq!(sidebar.spaces()[1].profile, profile);
    }

    #[test]
    fn creates_a_space_and_displays_it() {
        let mut sidebar = Sidebar::default();
        let work = sidebar.create_profile("Work").unwrap();
        let space = sidebar.create_space("Mail", work).unwrap();
        let created = sidebar.displayed();
        assert_eq!(created.id, space);
        assert_eq!((created.title.as_str(), created.profile), ("Mail", work));
        assert_eq!(
            (created.emoji.as_deref(), created.color),
            (None, Color::Purple)
        );
        assert_eq!(sidebar.spaces().last().unwrap().id, space);
        assert_eq!(sidebar.create_space(" ", work), Err(Error::EmptyTitle));
        assert_eq!(
            sidebar.create_space("a", ProfileId(999)),
            Err(Error::UnknownProfile)
        );
        assert_eq!(sidebar.spaces().len(), 3);
        assert_eq!(sidebar.displayed().id, space);
    }

    #[test]
    fn renames_a_space_and_sets_its_emoji_and_color() {
        let mut sidebar = Sidebar::default();
        let id = home(&sidebar).id;
        sidebar.rename_space(id, "Personal").unwrap();
        sidebar.set_space_emoji(id, " 🏡 ").unwrap();
        sidebar.set_space_color(id, Color::Blue).unwrap();
        assert_eq!(sidebar.rename_space(id, " "), Err(Error::EmptyTitle));
        let space = home(&sidebar);
        assert_eq!(
            (space.title.as_str(), space.emoji.as_deref(), space.color),
            ("Personal", Some("🏡"), Color::Blue)
        );
        sidebar.set_space_emoji(id, " ").unwrap();
        assert_eq!(home(&sidebar).emoji, None);
        let unknown = SpaceId(999);
        assert_eq!(sidebar.rename_space(unknown, "a"), Err(Error::UnknownSpace));
        assert_eq!(
            sidebar.set_space_emoji(unknown, "a"),
            Err(Error::UnknownSpace)
        );
        assert_eq!(
            sidebar.set_space_color(unknown, Color::Red),
            Err(Error::UnknownSpace)
        );
    }

    #[test]
    fn a_space_saved_without_an_emoji_or_a_color_has_none_and_is_purple() {
        let json = r#"{ "id": 3, "title": "Home", "profile": 1, "pinned": 4, "today": 5,
            "selected": null }"#;
        let space: Space = serde_json::from_str(json).unwrap();
        assert_eq!((space.emoji, space.color), (None, Color::Purple));
    }

    #[test]
    fn displays_the_first_space_until_another_is_displayed() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let work = sidebar.add_space("Work", home.profile).unwrap();
        assert_eq!(sidebar.displayed().id, home.id);
        sidebar.display(work).unwrap();
        assert_eq!(sidebar.displayed().id, work);
        assert_eq!(sidebar.display(SpaceId(999)), Err(Error::UnknownSpace));
        assert_eq!(sidebar.displayed().id, work);
    }

    #[test]
    fn finds_a_space_by_number() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar).id;
        let work = sidebar.add_space("Work", sidebar.profiles()[0].id).unwrap();
        assert_eq!(sidebar.space_at(1), Some(home));
        assert_eq!(sidebar.space_at(2), Some(work));
        assert_eq!(sidebar.space_at(3), None);
        assert_eq!(sidebar.space_at(0), None);
    }

    #[test]
    fn finds_the_next_and_previous_space_without_wrapping_around() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar).id;
        let work = sidebar.add_space("Work", sidebar.profiles()[0].id).unwrap();
        let read = sidebar.add_space("Read", sidebar.profiles()[0].id).unwrap();
        assert_eq!(sidebar.next_space(true), Some(work));
        assert_eq!(sidebar.next_space(false), None);
        sidebar.display(work).unwrap();
        assert_eq!(sidebar.next_space(true), Some(read));
        assert_eq!(sidebar.next_space(false), Some(home));
        sidebar.display(read).unwrap();
        assert_eq!(sidebar.next_space(true), None);
    }

    #[test]
    fn each_space_keeps_its_selected_tab() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar).id;
        let work = sidebar.add_space("Work", sidebar.profiles()[0].id).unwrap();
        let read = sidebar.open_tab(home, "read").unwrap();
        let mail = sidebar.open_tab(work, "mail").unwrap();
        sidebar.select(home, read).unwrap();
        sidebar.select(work, mail).unwrap();
        sidebar.display(work).unwrap();
        sidebar.display(home).unwrap();
        assert_eq!(sidebar.displayed().selected, Some(read));
        assert_eq!(sidebar.spaces()[1].selected, Some(mail));
    }

    #[test]
    fn a_node_belongs_to_the_space_of_its_pinned_or_today_tabs() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let work = sidebar.add_space("Work", home.profile).unwrap();
        let folder = sidebar.add_folder(home.pinned, "Docs").unwrap();
        let pinned = sidebar.open_tab(home.id, "pinned").unwrap();
        sidebar.move_node(pinned, folder, 0).unwrap();
        let today = sidebar.open_tab(work, "today").unwrap();
        let favorite = sidebar.open_tab(work, "favorite").unwrap();
        let favorites = sidebar.profiles()[0].favorites;
        sidebar.move_node(favorite, favorites, 0).unwrap();
        assert_eq!(sidebar.space_of(pinned), Some(home.id));
        assert_eq!(sidebar.space_of(today), Some(work));
        assert_eq!(sidebar.space_of(favorite), None);
        assert_eq!(sidebar.space_of(NodeId(999)), None);
    }

    #[test]
    fn a_node_shows_in_the_profile_of_its_favorites_or_its_space() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let work = sidebar.add_profile("Work");
        let office = sidebar.add_space("Office", work).unwrap();
        let favorite = sidebar.open_tab(office, "favorite").unwrap();
        sidebar.move_node(favorite, home.today, 0).unwrap();
        let favorites = sidebar.profiles()[1].favorites;
        let folder = sidebar
            .add_folder(sidebar.spaces()[1].pinned, "Docs")
            .unwrap();
        let pinned = sidebar.open_tab(home.id, "pinned").unwrap();
        sidebar.move_node(pinned, folder, 0).unwrap();
        assert_eq!(sidebar.profile_of(favorite), Some(home.profile));
        sidebar.move_node(favorite, favorites, 0).unwrap();
        assert_eq!(sidebar.profile_of(favorite), Some(work));
        assert_eq!(sidebar.profile_of(pinned), Some(work));
        assert_eq!(sidebar.profile_of(home.today), Some(home.profile));
        assert_eq!(sidebar.profile_of(NodeId(999)), None);
    }

    #[test]
    fn a_space_needs_a_known_profile() {
        let mut sidebar = Sidebar::default();
        assert_eq!(
            sidebar.add_space("Work", ProfileId(999)),
            Err(Error::UnknownProfile)
        );
    }

    #[test]
    fn opens_tabs_at_the_end_of_today() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let first = sidebar.open_tab(home.id, "https://example.com/").unwrap();
        let second = sidebar.open_tab(home.id, "https://example.org/").unwrap();
        assert_eq!(children(&sidebar, home.today), [first, second]);
        let NodeKind::Tab(tab) = &sidebar.node(first).unwrap().kind else {
            panic!("not a tab")
        };
        assert_eq!(tab.url, "https://example.com/");
    }

    #[test]
    fn updates_a_tab() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let tab = sidebar.open_tab(home.id, "https://example.com/").unwrap();
        sidebar.tab_mut(tab).unwrap().title = "Example".to_string();
        let NodeKind::Tab(tab) = &sidebar.node(tab).unwrap().kind else {
            panic!("not a tab")
        };
        assert_eq!(tab.title, "Example");
        assert_eq!(sidebar.tab_mut(home.today), None);
    }

    #[test]
    fn selects_a_tab_until_it_is_removed() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let tab = sidebar.open_tab(home.id, "https://example.com/").unwrap();
        sidebar.select(home.id, tab).unwrap();
        assert_eq!(sidebar.spaces()[0].selected, Some(tab));
        assert_eq!(sidebar.select(home.id, home.today), Err(Error::NotATab));
        assert_eq!(
            sidebar.select(home.id, NodeId(999)),
            Err(Error::UnknownNode)
        );
        sidebar.remove(tab).unwrap();
        assert_eq!(sidebar.spaces()[0].selected, None);
    }

    #[test]
    fn shows_the_next_tab_after_closing_one() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let a = sidebar.open_tab(home.id, "a").unwrap();
        let b = sidebar.open_tab(home.id, "b").unwrap();
        let c = sidebar.open_tab(home.id, "c").unwrap();
        assert_eq!(sidebar.next_after_close(b), Some(c));
        assert_eq!(sidebar.next_after_close(c), Some(b));
        sidebar.remove(b).unwrap();
        sidebar.remove(c).unwrap();
        assert_eq!(sidebar.next_after_close(a), None);
    }

    #[test]
    fn returns_to_the_opener_while_it_is_open() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let first = sidebar.open_tab(home.id, "first").unwrap();
        let opener = sidebar.open_tab(home.id, "opener").unwrap();
        sidebar.select(home.id, opener).unwrap();
        let opened = sidebar.open_tab(home.id, "opened").unwrap();
        sidebar.move_node(opened, home.today, 0).unwrap();
        assert_eq!(sidebar.next_after_close(opened), Some(opener));
        sidebar.remove(opener).unwrap();
        assert_eq!(sidebar.next_after_close(opened), Some(first));
    }

    #[test]
    fn moves_a_tab_into_a_folder() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let tab = sidebar.open_tab(home.id, "https://example.com/").unwrap();
        let folder = sidebar.add_folder(home.pinned, "Docs").unwrap();
        sidebar.move_node(tab, folder, 0).unwrap();
        assert!(children(&sidebar, home.today).is_empty());
        assert_eq!(children(&sidebar, folder), [tab]);
        assert_eq!(sidebar.node(tab).unwrap().parent, Some(folder));
    }

    #[test]
    fn moves_to_a_position_clamped_to_the_end() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let a = sidebar.open_tab(home.id, "a").unwrap();
        let b = sidebar.open_tab(home.id, "b").unwrap();
        let c = sidebar.open_tab(home.id, "c").unwrap();
        sidebar.move_node(c, home.today, 0).unwrap();
        assert_eq!(children(&sidebar, home.today), [c, a, b]);
        sidebar.move_node(c, home.today, 99).unwrap();
        assert_eq!(children(&sidebar, home.today), [a, b, c]);
    }

    #[test]
    fn refuses_impossible_moves() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let outer = sidebar.add_folder(home.pinned, "Outer").unwrap();
        let inner = sidebar.add_folder(outer, "Inner").unwrap();
        let tab = sidebar.open_tab(home.id, "https://example.com/").unwrap();
        assert_eq!(
            sidebar.move_node(outer, inner, 0),
            Err(Error::MoveIntoItself)
        );
        assert_eq!(
            sidebar.move_node(outer, outer, 0),
            Err(Error::MoveIntoItself)
        );
        assert_eq!(sidebar.move_node(outer, tab, 0), Err(Error::NotAContainer));
        assert_eq!(
            sidebar.move_node(home.today, outer, 0),
            Err(Error::RootNode)
        );
    }

    fn folder_title(sidebar: &Sidebar, id: NodeId) -> String {
        match &sidebar.node(id).unwrap().kind {
            NodeKind::Folder { title, .. } => title.clone(),
            _ => panic!("not a folder"),
        }
    }

    fn folder_expanded(sidebar: &Sidebar, id: NodeId) -> bool {
        matches!(
            sidebar.node(id).unwrap().kind,
            NodeKind::Folder { expanded: true, .. }
        )
    }

    #[test]
    fn renames_a_folder_to_a_title_that_is_not_blank() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let folder = sidebar.add_folder(home.pinned, "New Folder").unwrap();
        sidebar.rename_folder(folder, "Reading").unwrap();
        assert_eq!(sidebar.rename_folder(folder, " "), Err(Error::EmptyTitle));
        assert_eq!(folder_title(&sidebar, folder), "Reading");
        let tab = sidebar.open_tab(home.id, "a").unwrap();
        assert_eq!(sidebar.rename_folder(tab, "b"), Err(Error::NotAFolder));
        assert_eq!(
            sidebar.rename_folder(home.pinned, "b"),
            Err(Error::NotAFolder)
        );
        assert_eq!(
            sidebar.rename_folder(NodeId(999), "b"),
            Err(Error::UnknownNode)
        );
    }

    #[test]
    fn collapses_and_expands_a_folder() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let folder = sidebar.add_folder(home.pinned, "Docs").unwrap();
        assert!(folder_expanded(&sidebar, folder));
        sidebar.toggle_folder(folder).unwrap();
        assert!(!folder_expanded(&sidebar, folder));
        sidebar.toggle_folder(folder).unwrap();
        assert!(folder_expanded(&sidebar, folder));
        let tab = sidebar.open_tab(home.id, "a").unwrap();
        assert_eq!(sidebar.toggle_folder(tab), Err(Error::NotAFolder));
    }

    #[test]
    fn removes_a_folder_with_its_content() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let folder = sidebar.add_folder(home.pinned, "Docs").unwrap();
        let tab = sidebar.open_tab(home.id, "https://example.com/").unwrap();
        sidebar.move_node(tab, folder, 0).unwrap();
        sidebar.remove(folder).unwrap();
        assert!(children(&sidebar, home.pinned).is_empty());
        assert_eq!(sidebar.node(folder), None);
        assert_eq!(sidebar.node(tab), None);
        assert_eq!(sidebar.remove(home.pinned), Err(Error::RootNode));
    }

    #[test]
    fn moves_between_the_tabs_the_sidebar_shows_and_wraps_around() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let favorites = sidebar.profiles()[0].favorites;
        let open = folder_with_tab(&mut sidebar, &home, "open");
        let shut = folder_with_tab(&mut sidebar, &home, "shut");
        sidebar.toggle_folder(shut.0).unwrap();
        let pinned = sidebar.open_tab(home.id, "pinned").unwrap();
        sidebar.move_node(pinned, home.pinned, usize::MAX).unwrap();
        let today = sidebar.open_tab(home.id, "today").unwrap();
        let favorite = sidebar.open_tab(home.id, "favorite").unwrap();
        sidebar.move_node(favorite, favorites, 0).unwrap();
        assert_eq!(sidebar.next_tab(home.id, true), Some(open.1));
        assert_eq!(sidebar.next_tab(home.id, false), Some(today));
        sidebar.select(home.id, open.1).unwrap();
        assert_eq!(sidebar.next_tab(home.id, true), Some(pinned));
        assert_eq!(sidebar.next_tab(home.id, false), Some(today));
        sidebar.select(home.id, today).unwrap();
        assert_eq!(sidebar.next_tab(home.id, true), Some(open.1));
        assert_eq!(sidebar.next_tab(home.id, false), Some(pinned));
        sidebar.select(home.id, favorite).unwrap();
        assert_eq!(sidebar.next_tab(home.id, true), Some(open.1));
    }

    #[test]
    fn moves_nowhere_without_tabs() {
        let sidebar = Sidebar::default();
        assert_eq!(sidebar.next_tab(home(&sidebar).id, true), None);
        assert_eq!(sidebar.next_tab(SpaceId(999), true), None);
    }

    /// A pinned folder holding one tab: (folder, tab).
    fn folder_with_tab(sidebar: &mut Sidebar, space: &Space, title: &str) -> (NodeId, NodeId) {
        let folder = sidebar.add_folder(space.pinned, title).unwrap();
        let tab = sidebar.open_tab(space.id, title).unwrap();
        sidebar.move_node(tab, folder, 0).unwrap();
        (folder, tab)
    }

    #[test]
    fn finds_a_favorite_by_number_and_the_last_one_with_9() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let favorites = sidebar.profiles()[0].favorites;
        assert_eq!(sidebar.favorite(home.id, 1), None);
        assert_eq!(sidebar.favorite(home.id, 9), None);
        let tabs: Vec<_> = (0..3)
            .map(|i| sidebar.open_tab(home.id, &i.to_string()).unwrap())
            .collect();
        for &tab in &tabs {
            sidebar.move_node(tab, favorites, usize::MAX).unwrap();
        }
        assert_eq!(sidebar.favorite(home.id, 1), Some(tabs[0]));
        assert_eq!(sidebar.favorite(home.id, 3), Some(tabs[2]));
        assert_eq!(sidebar.favorite(home.id, 4), None);
        assert_eq!(sidebar.favorite(home.id, 9), Some(tabs[2]));
        assert_eq!(sidebar.favorite(home.id, 0), None);
    }

    #[test]
    fn numbers_the_favorites_of_each_profile() {
        let mut sidebar = Sidebar::default();
        let home = home(&sidebar);
        let work = sidebar.create_profile("Work").unwrap();
        let office = sidebar.spaces()[1].id;
        let personal = sidebar.open_tab(home.id, "personal").unwrap();
        let job = sidebar.open_tab(office, "job").unwrap();
        let favorites = sidebar.favorites_of(home.id).unwrap();
        let work_favorites = sidebar.favorites_of(office).unwrap();
        assert_eq!(favorites, sidebar.profiles()[0].favorites);
        assert_eq!(work_favorites, sidebar.profiles()[1].favorites);
        sidebar.move_node(personal, favorites, 0).unwrap();
        sidebar.move_node(job, work_favorites, 0).unwrap();
        assert_eq!(sidebar.favorite(home.id, 1), Some(personal));
        assert_eq!(sidebar.favorite(office, 1), Some(job));
        assert_eq!(sidebar.profile_of(job), Some(work));
        assert_eq!(sidebar.favorites_of(SpaceId(999)), None);
    }
}
