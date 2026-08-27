use super::{PaneId, TabId, WorkspaceId};
use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Branch {
    First,
    Second,
}

impl Branch {
    pub(super) fn as_wire(self) -> bool {
        match self {
            Self::First => false,
            Self::Second => true,
        }
    }
}

pub trait HerdrApi {
    fn tab_ids(&self, workspace_id: Option<&WorkspaceId>) -> Result<Vec<TabId>>;

    fn export_layout(&self, tab_id: &TabId) -> Result<Option<TabLayout>>;

    fn set_split_ratio(&self, tab_id: &TabId, path: &[Branch], ratio: f64) -> Result<bool>;
}

#[derive(Debug, Deserialize)]
pub struct TabLayout {
    pub tab_id: TabId,
    pub zoomed: bool,
    pub root: LayoutNode,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LayoutNode {
    Pane {
        pane_id: Option<PaneId>,
    },
    Split {
        ratio: f64,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

impl TabLayout {
    pub fn contains_pane(&self, pane_id: &PaneId) -> bool {
        self.root.contains_pane(pane_id)
    }
}

impl LayoutNode {
    fn contains_pane(&self, wanted: &PaneId) -> bool {
        match self {
            Self::Pane { pane_id } => pane_id.as_ref() == Some(wanted),
            Self::Split { first, second, .. } => {
                first.contains_pane(wanted) || second.contains_pane(wanted)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn pane(pane_id: &Value) -> Value {
        json!({ "type": "pane", "pane_id": pane_id })
    }

    fn layout(root: &Value) -> TabLayout {
        serde_json::from_value(json!({
            "workspace_id": "w1",
            "tab_id": "w1:t1",
            "zoomed": false,
            "focused_pane_id": "w1:p1",
            "root": root,
        }))
        .expect("the fixture matches LayoutDescription")
    }

    #[test]
    fn a_pane_is_found_at_any_depth() {
        let deep = layout(&json!({
            "type": "split", "direction": "right", "ratio": 0.5,
            "first": pane(&json!("w1:p1")),
            "second": {
                "type": "split", "direction": "down", "ratio": 0.5,
                "first": pane(&json!("w1:p2")),
                "second": pane(&json!("w1:p3")),
            },
        }));

        for pane_id in ["w1:p1", "w1:p2", "w1:p3"] {
            assert!(
                deep.contains_pane(&PaneId::from(pane_id)),
                "{pane_id} is there"
            );
        }
        assert!(!deep.contains_pane(&PaneId::from("w1:p9")));
    }

    #[test]
    fn a_lone_pane_is_found() {
        let alone = layout(&pane(&json!("w1:p1")));
        assert!(alone.contains_pane(&PaneId::from("w1:p1")));
        assert!(!alone.contains_pane(&PaneId::from("w1:p2")));
    }

    #[test]
    fn a_pane_without_an_id_matches_nothing() {
        assert!(!layout(&pane(&Value::Null)).contains_pane(&PaneId::from("w1:p1")));
        assert!(!layout(&json!({ "type": "pane" })).contains_pane(&PaneId::from("w1:p1")));
    }

    #[test]
    fn a_branch_encodes_which_child_the_split_path_takes() {
        assert!(!Branch::First.as_wire());
        assert!(Branch::Second.as_wire());
    }
}
