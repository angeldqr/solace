use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaneId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    /// Children side by side (vertical divider).
    Row,
    /// Children stacked (horizontal divider).
    Column,
}

/// Binary split tree of panes inside a tab.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Layout {
    Leaf {
        pane: PaneId,
    },
    Split {
        direction: Direction,
        /// Share of space given to `first`, in (0, 1).
        ratio: f32,
        first: Box<Layout>,
        second: Box<Layout>,
    },
}

impl Layout {
    pub fn leaf(pane: PaneId) -> Self {
        Layout::Leaf { pane }
    }

    /// Splits `target` in half, placing `new` after it.
    /// Returns false if `target` is absent or `new` is already in the layout.
    pub fn split(&mut self, target: PaneId, direction: Direction, new: PaneId) -> bool {
        if self.panes().contains(&new) {
            return false;
        }
        self.split_leaf(target, direction, new)
    }

    fn split_leaf(&mut self, target: PaneId, direction: Direction, new: PaneId) -> bool {
        match self {
            Layout::Leaf { pane } if *pane == target => {
                *self = Layout::Split {
                    direction,
                    ratio: 0.5,
                    first: Box::new(Layout::leaf(target)),
                    second: Box::new(Layout::leaf(new)),
                };
                true
            }
            Layout::Leaf { .. } => false,
            Layout::Split { first, second, .. } => {
                first.split_leaf(target, direction, new)
                    || second.split_leaf(target, direction, new)
            }
        }
    }

    /// Removes `target`; its sibling takes the freed space. `None` when the last pane closes.
    pub fn close(self, target: PaneId) -> Option<Layout> {
        match self {
            Layout::Leaf { pane } if pane == target => None,
            leaf @ Layout::Leaf { .. } => Some(leaf),
            Layout::Split {
                direction,
                ratio,
                first,
                second,
            } => match (first.close(target), second.close(target)) {
                (Some(first), Some(second)) => Some(Layout::Split {
                    direction,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(only), None) | (None, Some(only)) => Some(only),
                (None, None) => None,
            },
        }
    }

    /// Panes in visual order (left-to-right, top-to-bottom).
    pub fn panes(&self) -> Vec<PaneId> {
        match self {
            Layout::Leaf { pane } => vec![*pane],
            Layout::Split { first, second, .. } => {
                let mut panes = first.panes();
                panes.extend(second.panes());
                panes
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: PaneId = PaneId(1);
    const B: PaneId = PaneId(2);
    const C: PaneId = PaneId(3);

    #[test]
    fn split_then_close_restores_original() {
        let mut layout = Layout::leaf(A);
        assert!(layout.split(A, Direction::Row, B));
        assert!(layout.split(B, Direction::Column, C));
        assert_eq!(layout.panes(), vec![A, B, C]);
        assert!(!layout.split(PaneId(99), Direction::Row, PaneId(100)));
        assert!(
            !layout.split(A, Direction::Row, B),
            "duplicate pane id must be rejected"
        );

        let layout = layout.close(C).unwrap().close(B).unwrap();
        assert_eq!(layout, Layout::leaf(A));
        assert_eq!(layout.close(A), None);
    }

    #[test]
    fn sibling_takes_space_of_closed_pane() {
        let mut layout = Layout::leaf(A);
        layout.split(A, Direction::Row, B);
        layout.split(A, Direction::Column, C);
        // Row[ Column[A, C], B ] -> close A -> Row[C, B]
        let layout = layout.close(A).unwrap();
        assert_eq!(layout.panes(), vec![C, B]);
        assert!(matches!(
            layout,
            Layout::Split {
                direction: Direction::Row,
                ..
            }
        ));
    }

    #[test]
    fn serializes_with_type_tag_for_the_frontend() {
        let mut layout = Layout::leaf(A);
        layout.split(A, Direction::Row, B);
        let json = serde_json::to_string(&layout).unwrap();
        assert!(json.starts_with(r#"{"type":"split","direction":"row","ratio":0.5"#));
        assert_eq!(serde_json::from_str::<Layout>(&json).unwrap(), layout);
    }
}
