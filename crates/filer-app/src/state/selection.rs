use std::collections::HashSet;

use filer_core::model::node::NodeId;

/// How a click should modify the selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectMode {
    /// Replace selection with just this node.
    Single,
    /// Extend to a contiguous range from the anchor.
    Range,
    /// Toggle membership without changing others.
    Toggle,
}

#[derive(Debug, Default, Clone)]
pub struct SelectionState {
    selected: HashSet<NodeId>,
    /// Anchor for range selection (last single-click target).
    anchor: Option<NodeId>,
}

impl SelectionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn select_one(&mut self, id: NodeId) {
        self.selected.clear();
        self.selected.insert(id);
        self.anchor = Some(id);
    }

    pub fn toggle(&mut self, id: NodeId) {
        if !self.selected.remove(&id) {
            self.selected.insert(id);
        }
        self.anchor = Some(id);
    }

    /// Select a contiguous range from the anchor to `id` within `all_ids`.
    ///
    /// If there is no anchor, falls back to single selection.
    pub fn range_to(&mut self, id: NodeId, all_ids: &[NodeId]) {
        let Some(anchor) = self.anchor else {
            self.select_one(id);
            return;
        };

        let pos_anchor = all_ids.iter().position(|n| *n == anchor);
        let pos_id = all_ids.iter().position(|n| *n == id);

        match (pos_anchor, pos_id) {
            (Some(a), Some(b)) => {
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                self.selected.clear();
                for n in &all_ids[lo..=hi] {
                    self.selected.insert(*n);
                }
            }
            _ => self.select_one(id),
        }
    }

    pub fn select_all(&mut self, ids: &[NodeId]) {
        self.selected = ids.iter().copied().collect();
    }

    pub fn clear(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    pub fn retain(&mut self, ids: &[NodeId]) {
        let visible: HashSet<NodeId> = ids.iter().copied().collect();
        self.selected.retain(|id| visible.contains(id));
        if self.anchor.is_some_and(|id| !visible.contains(&id)) {
            self.anchor = self.selected.iter().next().copied();
        }
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.selected.contains(&id)
    }

    pub fn ids(&self) -> Vec<NodeId> {
        self.selected.iter().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.selected.len()
    }

    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(xs: &[u64]) -> Vec<NodeId> {
        xs.iter().map(|x| NodeId(*x)).collect()
    }

    #[test]
    fn test_selection_single_click() {
        let mut sel = SelectionState::new();
        sel.select_one(NodeId(1));
        sel.select_one(NodeId(2));
        assert_eq!(sel.len(), 1);
        assert!(sel.contains(NodeId(2)));
        assert!(!sel.contains(NodeId(1)));
    }

    #[test]
    fn test_selection_shift_click_range() {
        let all = ids(&[1, 2, 3, 4, 5]);
        let mut sel = SelectionState::new();
        sel.select_one(NodeId(2));
        sel.range_to(NodeId(4), &all);
        assert_eq!(sel.len(), 3);
        assert!(sel.contains(NodeId(2)));
        assert!(sel.contains(NodeId(3)));
        assert!(sel.contains(NodeId(4)));
        assert!(!sel.contains(NodeId(1)));
        assert!(!sel.contains(NodeId(5)));
    }

    #[test]
    fn test_selection_ctrl_click_toggle() {
        let mut sel = SelectionState::new();
        sel.select_one(NodeId(1));
        sel.toggle(NodeId(2));
        assert_eq!(sel.len(), 2);
        sel.toggle(NodeId(1));
        assert_eq!(sel.len(), 1);
        assert!(!sel.contains(NodeId(1)));
    }

    #[test]
    fn test_selection_clear() {
        let all = ids(&[1, 2, 3]);
        let mut sel = SelectionState::new();
        sel.select_all(&all);
        assert_eq!(sel.len(), 3);
        sel.clear();
        assert!(sel.is_empty());
    }
}
