use crate::{
    herdr::{
        TabId,
        api::{Branch, HerdrApi, LayoutNode, TabLayout},
        trigger::{Settle, Target, Trigger},
    },
    pane_tree::{PaneTree, plan::Side, ratio::Ratio},
};
use anyhow::{Context, Result};
use std::{num::NonZeroU32, thread::sleep, time::Duration};

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub attempts: NonZeroU32,
    pub interval: Duration,
}

pub struct Equalizer<A> {
    api: A,
    retry: RetryPolicy,
}

impl<A: HerdrApi> Equalizer<A> {
    pub fn new(api: A, retry: RetryPolicy) -> Self {
        Self { api, retry }
    }

    pub fn run(&self, trigger: &Trigger) -> Result<()> {
        let tab_ids = self.tabs_in(&trigger.target)?;
        for layout in self.read_settled(&tab_ids, &trigger.settle)? {
            self.equalize(&layout)?;
        }
        Ok(())
    }

    fn tabs_in(&self, target: &Target) -> Result<Vec<TabId>> {
        match target {
            Target::Tabs(tab_ids) => Ok(tab_ids.clone()),
            Target::Workspace(workspace_id) => self.api.tab_ids(Some(workspace_id)),
            Target::EveryWorkspace => self.api.tab_ids(None),
        }
    }

    fn read_settled(&self, tab_ids: &[TabId], settle: &Settle) -> Result<Vec<TabLayout>> {
        let mut layouts = self.read(tab_ids)?;
        for _ in 1..self.retry.attempts.get() {
            if layouts.is_empty() {
                break;
            }
            let settled =
                settle.is_met(|pane_id| layouts.iter().any(|layout| layout.contains_pane(pane_id)));
            if settled {
                break;
            }
            sleep(self.retry.interval);
            layouts = self.read(tab_ids)?;
        }
        Ok(layouts)
    }

    fn read(&self, tab_ids: &[TabId]) -> Result<Vec<TabLayout>> {
        tab_ids
            .iter()
            .filter_map(|tab_id| {
                self.api
                    .export_layout(tab_id)
                    .with_context(|| format!("exporting {tab_id}"))
                    .transpose()
            })
            .collect()
    }

    fn equalize(&self, layout: &TabLayout) -> Result<()> {
        if layout.zoomed {
            log::debug!("{} is zoomed; leaving it alone", layout.tab_id);
            return Ok(());
        }

        let tree = pane_tree(&layout.root)
            .with_context(|| format!("reading the layout of {}", layout.tab_id))?;

        for adjustment in tree.equalization_plan() {
            let path: Vec<Branch> = adjustment.path.iter().copied().map(branch_of).collect();
            let applied = self
                .api
                .set_split_ratio(&layout.tab_id, &path, adjustment.ratio.as_f64())
                .with_context(|| format!("equalizing {}", layout.tab_id))?;

            if !applied {
                log::debug!("{} changed under us; leaving the rest", layout.tab_id);
                break;
            }
        }
        Ok(())
    }
}

fn branch_of(side: Side) -> Branch {
    match side {
        Side::First => Branch::First,
        Side::Second => Branch::Second,
    }
}

fn pane_tree(node: &LayoutNode) -> Result<PaneTree> {
    match node {
        LayoutNode::Pane { .. } => Ok(PaneTree::Pane),
        LayoutNode::Split {
            ratio,
            first,
            second,
        } => Ok(PaneTree::Split {
            ratio: Ratio::try_from(*ratio)?,
            first: Box::new(pane_tree(first)?),
            second: Box::new(pane_tree(second)?),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::herdr::{PaneId, WorkspaceId};
    use serde_json::{Value, json};
    use std::cell::{Cell, RefCell};

    #[derive(Debug, PartialEq)]
    struct Applied {
        tab_id: TabId,
        path: Vec<Branch>,
        ratio: f64,
    }

    struct FakeApi<E> {
        tab_ids: Vec<TabId>,
        export: E,
        reads: Cell<u32>,
        applied: RefCell<Vec<Applied>>,
        path_is_stale: bool,
    }

    impl<E> FakeApi<E>
    where
        E: Fn(u32, &TabId) -> Result<Option<TabLayout>>,
    {
        fn new(export: E) -> Self {
            Self {
                tab_ids: vec![TabId::from("w1:t1")],
                export,
                reads: Cell::new(0),
                applied: RefCell::new(Vec::new()),
                path_is_stale: false,
            }
        }
    }

    impl<E> HerdrApi for FakeApi<E>
    where
        E: Fn(u32, &TabId) -> Result<Option<TabLayout>>,
    {
        fn tab_ids(&self, _workspace_id: Option<&WorkspaceId>) -> Result<Vec<TabId>> {
            Ok(self.tab_ids.clone())
        }

        fn export_layout(&self, tab_id: &TabId) -> Result<Option<TabLayout>> {
            self.reads.set(self.reads.get() + 1);
            (self.export)(self.reads.get(), tab_id)
        }

        fn set_split_ratio(&self, tab_id: &TabId, path: &[Branch], ratio: f64) -> Result<bool> {
            self.applied.borrow_mut().push(Applied {
                tab_id: tab_id.clone(),
                path: path.to_vec(),
                ratio,
            });
            Ok(!self.path_is_stale)
        }
    }

    fn tree_of(pane_ids: &[&str]) -> Value {
        let mut panes = pane_ids
            .iter()
            .map(|pane_id| json!({ "type": "pane", "pane_id": pane_id }));
        let first = panes.next().expect("a tab has at least one pane");
        panes.fold(first, |nested, pane| {
            json!({
                "type": "split", "direction": "right", "ratio": 0.5,
                "first": nested, "second": pane,
            })
        })
    }

    fn layout(tab_id: &str, pane_ids: &[&str]) -> TabLayout {
        zoomed_layout(tab_id, pane_ids, false)
    }

    fn zoomed_layout(tab_id: &str, pane_ids: &[&str], zoomed: bool) -> TabLayout {
        serde_json::from_value(json!({
            "workspace_id": "w1",
            "tab_id": tab_id,
            "zoomed": zoomed,
            "focused_pane_id": pane_ids[0],
            "root": tree_of(pane_ids),
        }))
        .expect("the fixture matches LayoutDescription")
    }

    fn eagerly<E>(api: FakeApi<E>) -> Equalizer<FakeApi<E>>
    where
        E: Fn(u32, &TabId) -> Result<Option<TabLayout>>,
    {
        Equalizer::new(
            api,
            RetryPolicy {
                attempts: NonZeroU32::new(5).unwrap(),
                interval: Duration::ZERO,
            },
        )
    }

    fn on_one_tab(settle: Settle) -> Trigger {
        Trigger {
            target: Target::Tabs(vec![TabId::from("w1:t1")]),
            settle,
        }
    }

    #[test]
    fn a_layout_that_already_settled_is_read_once() {
        let equalizer = eagerly(FakeApi::new(|_, tab_id| {
            Ok(Some(layout(&tab_id.to_string(), &["w1:p1", "w1:p2"])))
        }));
        equalizer
            .run(&on_one_tab(Settle::PanePresent(PaneId::from("w1:p2"))))
            .unwrap();

        assert_eq!(equalizer.api.reads.get(), 1);
    }

    #[test]
    fn reading_repeats_until_the_pane_lands() {
        let equalizer = eagerly(FakeApi::new(|read, tab_id| {
            let panes: &[&str] = if read < 3 {
                &["w1:p1"]
            } else {
                &["w1:p1", "w1:p2"]
            };
            Ok(Some(layout(&tab_id.to_string(), panes)))
        }));
        equalizer
            .run(&on_one_tab(Settle::PanePresent(PaneId::from("w1:p2"))))
            .unwrap();

        assert_eq!(equalizer.api.reads.get(), 3);
    }

    #[test]
    fn a_layout_that_never_settles_is_equalized_as_last_read() {
        let equalizer = eagerly(FakeApi::new(|_, tab_id| {
            Ok(Some(layout(
                &tab_id.to_string(),
                &["w1:p1", "w1:p2", "w1:p3"],
            )))
        }));
        equalizer
            .run(&on_one_tab(Settle::PanePresent(PaneId::from("w1:p9"))))
            .unwrap();

        assert_eq!(
            equalizer.api.reads.get(),
            5,
            "it gave up after the attempts"
        );
        assert_eq!(
            equalizer.api.applied.borrow().len(),
            1,
            "the last layout it could read was equalized anyway"
        );
    }

    #[test]
    fn a_tab_that_is_gone_is_not_waited_for() {
        let equalizer = eagerly(FakeApi::new(|_, _| Ok(None)));
        equalizer
            .run(&on_one_tab(Settle::PanePresent(PaneId::from("w1:p2"))))
            .unwrap();

        assert_eq!(equalizer.api.reads.get(), 1);
        assert!(equalizer.api.applied.borrow().is_empty());
    }

    #[test]
    fn a_tab_that_is_gone_is_skipped_and_the_others_are_still_equalized() {
        let equalizer = eagerly(FakeApi::new(|_, tab_id| {
            if *tab_id == TabId::from("w1:t1") {
                Ok(None)
            } else {
                Ok(Some(layout(
                    &tab_id.to_string(),
                    &["w1:p1", "w1:p2", "w1:p3"],
                )))
            }
        }));
        let trigger = Trigger {
            target: Target::Tabs(vec![TabId::from("w1:t1"), TabId::from("w1:t2")]),
            settle: Settle::Immediately,
        };
        equalizer.run(&trigger).unwrap();

        let applied = equalizer.api.applied.borrow();
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].tab_id, TabId::from("w1:t2"));
    }

    #[test]
    fn a_zoomed_tab_is_left_alone() {
        let equalizer = eagerly(FakeApi::new(|_, tab_id| {
            Ok(Some(zoomed_layout(
                &tab_id.to_string(),
                &["w1:p1", "w1:p2", "w1:p3"],
                true,
            )))
        }));
        equalizer.run(&on_one_tab(Settle::Immediately)).unwrap();

        assert!(equalizer.api.applied.borrow().is_empty());
    }

    #[test]
    fn descendants_are_applied_before_ancestors() {
        let equalizer = eagerly(FakeApi::new(|_, tab_id| {
            Ok(Some(layout(
                &tab_id.to_string(),
                &["w1:p1", "w1:p2", "w1:p3", "w1:p4"],
            )))
        }));
        equalizer.run(&on_one_tab(Settle::Immediately)).unwrap();

        let applied = equalizer.api.applied.borrow();
        assert_eq!(
            *applied,
            [
                Applied {
                    tab_id: TabId::from("w1:t1"),
                    path: vec![Branch::First],
                    ratio: 2.0 / 3.0,
                },
                Applied {
                    tab_id: TabId::from("w1:t1"),
                    path: vec![],
                    ratio: 0.75,
                },
            ]
        );
    }

    #[test]
    fn a_stale_path_abandons_the_rest_of_the_tab() {
        let mut api = FakeApi::new(|_, tab_id| {
            Ok(Some(layout(
                &tab_id.to_string(),
                &["w1:p1", "w1:p2", "w1:p3", "w1:p4"],
            )))
        });
        api.path_is_stale = true;
        let equalizer = eagerly(api);
        equalizer.run(&on_one_tab(Settle::Immediately)).unwrap();

        assert_eq!(
            equalizer.api.applied.borrow().len(),
            1,
            "it stopped after the first refusal"
        );
    }

    #[test]
    fn an_unexpected_failure_is_propagated() {
        let equalizer = eagerly(FakeApi::new(|_, _| {
            Err(anyhow::anyhow!("the socket is on fire"))
        }));
        let error = equalizer.run(&on_one_tab(Settle::Immediately)).unwrap_err();

        assert!(
            format!("{error:#}").contains("the socket is on fire"),
            "the cause should survive: {error:#}"
        );
    }
}
