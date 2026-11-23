//! Undoing and redoing the user's sidebar edits, like Arc's ⌘Z and ⇧⌘Z: the sidebar before and
//! after each edit, the last [`MAX_EDITS`] ones.
//!
//! An edit is whatever a command of the user changed that `Sidebar::same_edits` tells: moving,
//! renaming, closing or pinning tabs, folders, Spaces and their themes; not what a page does to
//! its tab, nor which tab shows. Undoing puts the sidebar before it back, keeping what pages did
//! since (`Sidebar::back_over`). The sidebar may also change out of the user's commands, a page
//! opening a tab or tabs archived: the edits before can no longer be undone, as putting the
//! sidebar back would undo that too.

use crate::sidebar::Sidebar;

/// How many edits may be undone.
pub const MAX_EDITS: usize = 50;

#[derive(Default)]
pub struct Edits {
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    /// The sidebar before a Space's form opened (`Sidebar::create_draft_space`): the Space it
    /// creates, once named, is one edit from it.
    before_draft: Option<Sidebar>,
}

struct Edit {
    before: Sidebar,
    after: Sidebar,
    /// When, in milliseconds since the Unix epoch.
    at: u64,
}

impl Edits {
    /// Keeps the edit a command made at `at`, from `before` to `after`, if it made one; with
    /// `continued`, as part of the last one, like the moves of one drag. An edit changing what
    /// lasts (`Sidebar::lasting`), a profile or a blank window, cannot be undone, nor the ones
    /// before.
    pub fn record(&mut self, before: Sidebar, after: &Sidebar, at: u64, continued: bool) {
        if after.has_draft() {
            if !before.has_draft() {
                self.before_draft = Some(before);
            }
            return;
        }
        let before = match self.before_draft.take() {
            Some(draft) if before.has_draft() => draft,
            _ => before,
        };
        if before.same_edits(after) {
            return;
        }
        self.redo.clear();
        if before.lasting() != after.lasting() {
            self.undo.clear();
            return;
        }
        if let Some(last) = self
            .undo
            .last_mut()
            .filter(|last| continued && last.after.same_edits(&before))
        {
            last.after = after.clone();
            return;
        }
        self.undo.push(Edit {
            before,
            after: after.clone(),
            at,
        });
        let over = self.undo.len().saturating_sub(MAX_EDITS);
        self.undo.drain(..over);
    }

    /// Undoes the last edit on `sidebar`, and tells when it was made; none when there is no edit
    /// left to undo, or the sidebar changed since out of the user's edits.
    pub fn undo(&mut self, sidebar: &mut Sidebar) -> Option<u64> {
        let edit = self.undo.pop()?;
        if !edit.after.same_edits(sidebar) {
            (self.undo, self.redo) = Default::default();
            return None;
        }
        *sidebar = edit.before.back_over(&edit.after, sidebar);
        let at = edit.at;
        self.redo.push(edit);
        Some(at)
    }

    /// Makes the last edit undone again on `sidebar`, if the sidebar did not change since out of
    /// the user's edits; whether it did.
    pub fn redo(&mut self, sidebar: &mut Sidebar) -> bool {
        let Some(edit) = self.redo.pop() else {
            return false;
        };
        if !edit.before.same_edits(sidebar) {
            self.redo.clear();
            return false;
        }
        *sidebar = edit.after.back_over(&edit.before, sidebar);
        self.undo.push(edit);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sidebar::NodeKind;

    fn titles(sidebar: &Sidebar) -> Vec<String> {
        let today = sidebar.displayed().today;
        let rows = &sidebar.node(today).expect("a root").children;
        rows.iter()
            .map(|&id| match &sidebar.node(id).expect("a row").kind {
                NodeKind::Tab(tab) => tab.label().to_string(),
                _ => String::new(),
            })
            .collect()
    }

    /// Runs `command` on `sidebar` as the user's, keeping its edit.
    fn edit(edits: &mut Edits, sidebar: &mut Sidebar, command: impl FnOnce(&mut Sidebar)) {
        let before = sidebar.clone();
        command(sidebar);
        edits.record(before, sidebar, 0, false);
    }

    #[test]
    fn undoes_and_redoes_closing_a_tab_selected_again() {
        let mut sidebar = Sidebar::default();
        let space = sidebar.displayed().id;
        let a = sidebar.open_tab(space, "https://a.test").unwrap();
        sidebar.open_tab(space, "https://b.test").unwrap();
        sidebar.select(space, a).unwrap();
        let mut edits = Edits::default();
        edit(&mut edits, &mut sidebar, |s| s.remove(a).unwrap());
        assert_eq!(titles(&sidebar), ["https://b.test"]);
        assert!(edits.undo(&mut sidebar).is_some());
        assert_eq!(titles(&sidebar), ["https://a.test", "https://b.test"]);
        assert_eq!(sidebar.displayed().selected, Some(a));
        assert!(edits.redo(&mut sidebar));
        assert_eq!(titles(&sidebar), ["https://b.test"]);
        assert!(edits.undo(&mut sidebar).is_some());
        assert!(edits.undo(&mut sidebar).is_none());
    }

    #[test]
    fn keeps_what_pages_did_and_which_tab_shows_out_of_edits() {
        let mut sidebar = Sidebar::default();
        let space = sidebar.displayed().id;
        let a = sidebar.open_tab(space, "https://a.test").unwrap();
        let b = sidebar.open_tab(space, "https://b.test").unwrap();
        let mut edits = Edits::default();
        // Selecting a tab, or its page loading, is no edit.
        edit(&mut edits, &mut sidebar, |s| s.select(space, b).unwrap());
        assert!(edits.undo.is_empty());
        edit(&mut edits, &mut sidebar, |s| s.rename(a, "Mine").unwrap());
        sidebar.tab_mut(a).unwrap().title = "A, loaded".into();
        assert!(edits.undo(&mut sidebar).is_some());
        let tab = sidebar.tab_mut(a).unwrap();
        assert_eq!(
            (tab.name.as_deref(), tab.title.as_str()),
            (None, "A, loaded")
        );
    }

    #[test]
    fn cannot_undo_past_a_change_out_of_the_users_edits() {
        let mut sidebar = Sidebar::default();
        let space = sidebar.displayed().id;
        let a = sidebar.open_tab(space, "https://a.test").unwrap();
        let mut edits = Edits::default();
        edit(&mut edits, &mut sidebar, |s| s.remove(a).unwrap());
        // A page opens a tab.
        sidebar.open_tab(space, "https://popup.test").unwrap();
        assert!(edits.undo(&mut sidebar).is_none());
        assert_eq!(titles(&sidebar), ["https://popup.test"]);
    }

    #[test]
    fn a_drag_undoes_as_one_edit() {
        let mut sidebar = Sidebar::default();
        let space = sidebar.displayed().id;
        let mut edits = Edits::default();
        for title in ["One", "Two", "Three"] {
            let before = sidebar.clone();
            sidebar.rename_space(space, title).unwrap();
            edits.record(before, &sidebar, 0, true);
        }
        assert!(edits.undo(&mut sidebar).is_some());
        assert_eq!(sidebar.displayed().title, "Home");
        assert!(edits.undo(&mut sidebar).is_none());
    }

    #[test]
    fn cannot_undo_adding_a_profile_nor_the_edits_before() {
        let mut sidebar = Sidebar::default();
        let space = sidebar.displayed().id;
        let mut edits = Edits::default();
        edit(&mut edits, &mut sidebar, |s| {
            s.rename_space(space, "Mine").unwrap()
        });
        edit(&mut edits, &mut sidebar, |s| {
            s.add_profile("Work");
        });
        assert!(edits.undo(&mut sidebar).is_none());
        assert_eq!(sidebar.displayed().title, "Mine");
    }

    #[test]
    fn a_space_named_in_its_form_undoes_as_one_edit() {
        let mut sidebar = Sidebar::default();
        let profile = sidebar.displayed().profile;
        let mut edits = Edits::default();
        let mut draft = None;
        edit(&mut edits, &mut sidebar, |s| {
            draft = Some(s.create_draft_space("Space 2", profile).unwrap());
        });
        let draft = draft.unwrap();
        edit(&mut edits, &mut sidebar, |s| {
            s.rename_space(draft, "Work").unwrap()
        });
        assert_eq!(sidebar.spaces().len(), 2);
        assert!(edits.undo(&mut sidebar).is_some());
        assert_eq!(sidebar.spaces().len(), 1);
    }
}
