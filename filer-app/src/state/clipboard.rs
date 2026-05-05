use filer_core::model::node::NodeId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardOp {
    Copy,
    Cut,
}

#[derive(Debug, Clone)]
pub struct ClipboardState {
    pub op: ClipboardOp,
    pub nodes: Vec<NodeId>,
}

impl ClipboardState {
    pub fn copy(nodes: Vec<NodeId>) -> Self {
        Self {
            op: ClipboardOp::Copy,
            nodes,
        }
    }

    pub fn cut(nodes: Vec<NodeId>) -> Self {
        Self {
            op: ClipboardOp::Cut,
            nodes,
        }
    }

    pub fn is_cut(&self) -> bool {
        self.op == ClipboardOp::Cut
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_copy_paste_retains_clipboard() {
        let nodes = vec![NodeId(1), NodeId(2)];
        let cb = ClipboardState::copy(nodes.clone());
        assert_eq!(cb.op, ClipboardOp::Copy);
        assert_eq!(cb.nodes, nodes);
        assert!(!cb.is_cut());
    }

    #[test]
    fn test_clipboard_cut() {
        let cb = ClipboardState::cut(vec![NodeId(5)]);
        assert!(cb.is_cut());
    }
}
