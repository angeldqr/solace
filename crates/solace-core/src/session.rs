use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::spaces::{Space, SpaceId, Tab, TabId};
use crate::terminals::{Direction, Layout, PaneId};

/// Everything the user has open. Use cases are methods; each returns `None`/`false`
/// when an id is unknown, and the panes it closed so the caller can stop their processes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub spaces: Vec<Space>,
    pub active_space: Option<SpaceId>,
    /// Ids are never reused, so stale references (API clients, late exit events) stay harmless.
    #[serde(default)]
    next_id: u32,
}

impl Session {
    fn next_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    fn space_mut(&mut self, id: SpaceId) -> Option<&mut Space> {
        self.spaces.iter_mut().find(|s| s.id == id)
    }

    /// (space index, tab index) of the tab holding `pane`.
    fn locate(&self, pane: PaneId) -> Option<(usize, usize)> {
        self.spaces.iter().enumerate().find_map(|(si, space)| {
            let ti = space.tabs.iter().position(|t| t.layout.contains(pane))?;
            Some((si, ti))
        })
    }

    fn locate_tab(&self, tab: TabId) -> Option<(usize, usize)> {
        self.spaces.iter().enumerate().find_map(|(si, space)| {
            let ti = space.tabs.iter().position(|t| t.id == tab)?;
            Some((si, ti))
        })
    }

    /// Opens `root` as a space with one terminal, or activates it if already open.
    pub fn open_space(&mut self, root: PathBuf) -> SpaceId {
        if let Some(space) = self.spaces.iter().find(|s| s.root == root) {
            self.active_space = Some(space.id);
            return space.id;
        }
        let id = SpaceId(self.next_id());
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.display().to_string());
        self.spaces.push(Space {
            id,
            name,
            root,
            tabs: Vec::new(),
            active_tab: None,
        });
        self.active_space = Some(id);
        self.new_tab(id);
        id
    }

    pub fn close_space(&mut self, id: SpaceId) -> Option<Vec<PaneId>> {
        let index = self.spaces.iter().position(|s| s.id == id)?;
        let space = self.spaces.remove(index);
        if self.active_space == Some(id) {
            let next = index.min(self.spaces.len().saturating_sub(1));
            self.active_space = self.spaces.get(next).map(|s| s.id);
        }
        Some(space.tabs.iter().flat_map(|t| t.layout.panes()).collect())
    }

    pub fn activate_space(&mut self, id: SpaceId) -> bool {
        let exists = self.spaces.iter().any(|s| s.id == id);
        if exists {
            self.active_space = Some(id);
        }
        exists
    }

    /// Adds a tab with a single new pane and makes it active.
    pub fn new_tab(&mut self, space: SpaceId) -> Option<PaneId> {
        self.space_mut(space)?;
        let tab = TabId(self.next_id());
        let pane = PaneId(self.next_id());
        let space = self.space_mut(space)?;
        space.tabs.push(Tab {
            id: tab,
            layout: Layout::leaf(pane),
            focused: pane,
        });
        space.active_tab = Some(tab);
        Some(pane)
    }

    pub fn activate_tab(&mut self, tab: TabId) -> bool {
        let Some((si, _)) = self.locate_tab(tab) else {
            return false;
        };
        self.spaces[si].active_tab = Some(tab);
        self.active_space = Some(self.spaces[si].id);
        true
    }

    pub fn close_tab(&mut self, tab: TabId) -> Option<Vec<PaneId>> {
        let (si, ti) = self.locate_tab(tab)?;
        let space = &mut self.spaces[si];
        let removed = space.tabs.remove(ti);
        if space.active_tab == Some(tab) {
            let next = ti.min(space.tabs.len().saturating_sub(1));
            space.active_tab = space.tabs.get(next).map(|t| t.id);
        }
        Some(removed.layout.panes())
    }

    /// Splits `pane` and focuses the new pane.
    pub fn split_pane(&mut self, pane: PaneId, direction: Direction) -> Option<PaneId> {
        let new = self.split_pane_unfocused(pane, direction)?;
        self.focus_pane(new);
        Some(new)
    }

    /// Splits `pane` without touching focus or the active tab and space: for panes agents open
    /// while the user may be typing somewhere else.
    pub fn split_pane_unfocused(&mut self, pane: PaneId, direction: Direction) -> Option<PaneId> {
        let (si, ti) = self.locate(pane)?;
        let new = PaneId(self.next_id());
        self.spaces[si].tabs[ti].layout.split(pane, direction, new);
        Some(new)
    }

    /// Removes `pane`; closes its tab when it was the last one.
    pub fn close_pane(&mut self, pane: PaneId) -> bool {
        let Some((si, ti)) = self.locate(pane) else {
            return false;
        };
        let tab = &mut self.spaces[si].tabs[ti];
        match tab.layout.clone().close(pane) {
            Some(layout) => {
                if tab.focused == pane {
                    tab.focused = layout.panes()[0];
                }
                tab.layout = layout;
            }
            None => {
                let id = tab.id;
                self.close_tab(id);
            }
        }
        true
    }

    /// Focuses `pane` and brings its tab and space to the front.
    pub fn focus_pane(&mut self, pane: PaneId) -> bool {
        let Some((si, ti)) = self.locate(pane) else {
            return false;
        };
        let space = &mut self.spaces[si];
        space.tabs[ti].focused = pane;
        space.active_tab = Some(space.tabs[ti].id);
        self.active_space = Some(space.id);
        true
    }

    pub fn resize_split(&mut self, tab: TabId, path: &[u8], ratio: f32) -> bool {
        let Some((si, ti)) = self.locate_tab(tab) else {
            return false;
        };
        self.spaces[si].tabs[ti].layout.set_ratio(path, ratio)
    }

    /// Working directory for a pane's shell.
    pub fn pane_root(&self, pane: PaneId) -> Option<&Path> {
        let (si, _) = self.locate(pane)?;
        Some(&self.spaces[si].root)
    }

    pub fn space_of(&self, pane: PaneId) -> Option<SpaceId> {
        self.locate(pane).map(|(si, _)| self.spaces[si].id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_tab(session: &Session) -> &Tab {
        let space = &session.spaces[0];
        space
            .tabs
            .iter()
            .find(|t| Some(t.id) == space.active_tab)
            .unwrap()
    }

    #[test]
    fn opening_the_same_folder_twice_reuses_the_space() {
        let mut s = Session::default();
        let a = s.open_space(PathBuf::from("/repo"));
        let b = s.open_space(PathBuf::from("/other"));
        assert_ne!(a, b);
        assert_eq!(s.open_space(PathBuf::from("/repo")), a);
        assert_eq!(s.spaces.len(), 2);
        assert_eq!(s.active_space, Some(a));
        assert_eq!(s.spaces[0].name, "repo");
        assert_eq!(s.spaces[0].tabs.len(), 1);
    }

    #[test]
    fn closing_the_last_pane_closes_its_tab() {
        let mut s = Session::default();
        let space = s.open_space(PathBuf::from("/repo"));
        let first = active_tab(&s).focused;
        let second = s.new_tab(space).unwrap();
        let split = s.split_pane(second, Direction::Row).unwrap();
        assert_eq!(active_tab(&s).focused, split);

        assert!(s.close_pane(split));
        assert_eq!(active_tab(&s).focused, second);
        assert!(s.close_pane(second));
        assert_eq!(s.spaces[0].tabs.len(), 1);
        assert_eq!(active_tab(&s).focused, first);
        assert!(!s.close_pane(second), "closed panes are unknown");
    }

    #[test]
    fn closing_a_space_returns_all_its_panes() {
        let mut s = Session::default();
        let a = s.open_space(PathBuf::from("/a"));
        let pane = s.new_tab(a).unwrap();
        let split = s.split_pane(pane, Direction::Column).unwrap();
        let b = s.open_space(PathBuf::from("/b"));

        let closed = s.close_space(a).unwrap();
        assert_eq!(closed.len(), 3);
        assert!(closed.contains(&split));
        assert_eq!(s.active_space, Some(b));
        assert_eq!(s.close_space(a), None);
    }

    #[test]
    fn focusing_a_pane_activates_its_tab_and_space() {
        let mut s = Session::default();
        let a = s.open_space(PathBuf::from("/a"));
        let pane = s.spaces[0].tabs[0].focused;
        s.new_tab(a);
        s.open_space(PathBuf::from("/b"));

        assert!(s.focus_pane(pane));
        assert_eq!(s.active_space, Some(a));
        assert_eq!(s.spaces[0].active_tab, Some(s.spaces[0].tabs[0].id));
        assert_eq!(s.pane_root(pane), Some(Path::new("/a")));
    }

    #[test]
    fn unfocused_split_leaves_the_users_place_alone() {
        let mut s = Session::default();
        let a = s.open_space(PathBuf::from("/a"));
        let agent_pane = s.spaces[0].tabs[0].focused;
        let user_pane = s.new_tab(a).unwrap();
        let before = s.clone();

        let new = s.split_pane_unfocused(agent_pane, Direction::Row).unwrap();
        assert!(s.spaces[0].tabs[0].layout.contains(new));
        assert_eq!(s.spaces[0].active_tab, before.spaces[0].active_tab);
        assert_eq!(s.spaces[0].tabs[1].focused, user_pane);
        assert_eq!(s.spaces[0].tabs[0].focused, agent_pane);
    }

    #[test]
    fn ids_are_not_reused_after_a_round_trip() {
        let mut s = Session::default();
        s.open_space(PathBuf::from("/a"));
        let json = serde_json::to_string(&s).unwrap();
        let mut restored: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, s);
        let pane = restored.spaces[0].tabs[0].focused;
        let new = restored.split_pane(pane, Direction::Row).unwrap();
        assert!(new.0 > pane.0);
    }
}
