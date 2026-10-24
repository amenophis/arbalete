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
    pub favorites: NodeId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Space {
    pub id: SpaceId,
    pub title: String,
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
    /// A folder cannot move inside itself or one of its descendants.
    MoveIntoItself,
    /// Root containers cannot be moved or removed.
    RootNode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sidebar {
    profiles: Vec<Profile>,
    spaces: Vec<Space>,
    nodes: HashMap<NodeId, Node>,
    next_id: u64,
}

impl Default for Sidebar {
    /// A first profile and a first Space, so the browser always has somewhere to open tabs.
    fn default() -> Self {
        let mut sidebar = Self {
            profiles: Vec::new(),
            spaces: Vec::new(),
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
            favorites,
        });
        id
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
            profile,
            pinned,
            today,
            selected: None,
        });
        Ok(id)
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

    /// Moves a tab or a folder, with its content, to `index` among `parent`'s children.
    pub fn move_node(&mut self, id: NodeId, parent: NodeId, index: usize) -> Result<(), Error> {
        let old_parent = self.movable(id)?;
        self.check_container(parent)?;
        if self.ancestors(parent).any(|ancestor| ancestor == id) {
            return Err(Error::MoveIntoItself);
        }
        self.children_mut(old_parent).retain(|child| *child != id);
        let children = self.children_mut(parent);
        children.insert(index.min(children.len()), id);
        self.nodes.get_mut(&id).expect("checked by movable").parent = Some(parent);
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

    /// Pins a "Today" tab at the end of the Space's pinned tabs, saving its url, or moves a pinned
    /// tab back to the top of "Today", forgetting its saved url. Favorites stay where they are.
    pub fn toggle_pin(&mut self, space: SpaceId, tab: NodeId) -> Result<(), Error> {
        let (pinned, today) = self.space(space).map(|s| (s.pinned, s.today))?;
        let url = self.tab_mut(tab).ok_or(Error::NotATab)?.url.clone();
        let saved_url = if self.ancestors(tab).any(|id| id == today) {
            self.move_node(tab, pinned, usize::MAX)?;
            Some(url)
        } else if self.ancestors(tab).any(|id| id == pinned) {
            self.move_node(tab, today, 0)?;
            None
        } else {
            return Ok(());
        };
        self.tab_mut(tab).expect("checked above").saved_url = saved_url;
        Ok(())
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

    /// The tab `tab` was opened from, if it is still open.
    fn open_opener(&self, tab: NodeId) -> Option<NodeId> {
        match &self.nodes.get(&tab)?.kind {
            NodeKind::Tab(Tab { opener, .. }) => opener.filter(|&id| self.is_tab(id)),
            _ => None,
        }
    }

    fn is_tab(&self, id: NodeId) -> bool {
        matches!(self.nodes.get(&id).map(|n| &n.kind), Some(NodeKind::Tab(_)))
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
}
