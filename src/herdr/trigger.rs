use super::{PaneId, TabId, WorkspaceId, env::PluginEnv};
use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, PartialEq, Eq)]
pub enum Target {
    Tabs(Vec<TabId>),
    Workspace(WorkspaceId),
    EveryWorkspace,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Settle {
    PanePresent(PaneId),
    PaneAbsent(PaneId),
    PaneRelocated { arrived: PaneId, departed: PaneId },
    Immediately,
}

impl Settle {
    pub fn is_met(&self, contains_pane: impl Fn(&PaneId) -> bool) -> bool {
        match self {
            Self::PanePresent(pane_id) => contains_pane(pane_id),
            Self::PaneAbsent(pane_id) => !contains_pane(pane_id),
            Self::PaneRelocated { arrived, departed } => {
                contains_pane(arrived) && !contains_pane(departed)
            }
            Self::Immediately => true,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Trigger {
    pub target: Target,
    pub settle: Settle,
}

#[derive(Deserialize)]
struct Envelope {
    data: Event,
}

#[derive(Deserialize)]
struct ActionContext {
    tab_id: Option<TabId>,
}

#[derive(Deserialize)]
struct EventPane {
    pane_id: PaneId,
    tab_id: TabId,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Event {
    PaneCreated {
        pane: EventPane,
    },
    PaneMoved {
        pane: EventPane,
        previous_pane_id: PaneId,
        previous_tab_id: TabId,
        closed_tab_id: Option<TabId>,
    },
    PaneClosed {
        pane_id: PaneId,
        workspace_id: WorkspaceId,
    },
    PaneExited {
        pane_id: PaneId,
        workspace_id: WorkspaceId,
    },
    #[serde(other)]
    Unsubscribed,
}

impl Trigger {
    pub fn from_env(env: &PluginEnv) -> Result<Self> {
        let Some(raw) = env.event_json.as_deref() else {
            return Ok(Self::from_action(env));
        };
        let envelope: Envelope = serde_json::from_str(raw)
            .with_context(|| format!("parsing HERDR_PLUGIN_EVENT_JSON: {raw}"))?;
        Ok(Self::from_event(envelope.data))
    }

    fn from_event(event: Event) -> Self {
        match event {
            Event::PaneCreated { pane } => Self {
                target: Target::Tabs(vec![pane.tab_id]),
                settle: Settle::PanePresent(pane.pane_id),
            },
            Event::PaneClosed {
                pane_id,
                workspace_id,
            }
            | Event::PaneExited {
                pane_id,
                workspace_id,
            } => Self {
                target: Target::Workspace(workspace_id),
                settle: Settle::PaneAbsent(pane_id),
            },
            Event::PaneMoved {
                pane,
                previous_pane_id,
                previous_tab_id,
                closed_tab_id,
            } => {
                let mut tab_ids = vec![pane.tab_id];
                let left_a_living_tab = previous_tab_id != tab_ids[0]
                    && Some(&previous_tab_id) != closed_tab_id.as_ref();
                if left_a_living_tab {
                    tab_ids.push(previous_tab_id);
                }

                let settle = if previous_pane_id == pane.pane_id {
                    Settle::PanePresent(pane.pane_id)
                } else {
                    Settle::PaneRelocated {
                        arrived: pane.pane_id,
                        departed: previous_pane_id,
                    }
                };
                Self {
                    target: Target::Tabs(tab_ids),
                    settle,
                }
            }
            Event::Unsubscribed => Self {
                target: Target::EveryWorkspace,
                settle: Settle::Immediately,
            },
        }
    }

    fn from_action(env: &PluginEnv) -> Self {
        let from_context = env
            .context_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<ActionContext>(raw).ok())
            .and_then(|context| context.tab_id);

        let target = match (
            from_context.or_else(|| env.tab_id.clone()),
            &env.workspace_id,
        ) {
            (Some(tab_id), _) => Target::Tabs(vec![tab_id]),
            (None, Some(workspace_id)) => Target::Workspace(workspace_id.clone()),
            (None, None) => Target::EveryWorkspace,
        };
        Self {
            target,
            settle: Settle::Immediately,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::path::PathBuf;

    fn bare_env() -> PluginEnv {
        PluginEnv {
            socket_path: PathBuf::new(),
            state_dir: PathBuf::new(),
            event_json: None,
            context_json: None,
            tab_id: None,
            workspace_id: None,
        }
    }

    fn on_event(event: &Value) -> Result<Trigger> {
        Trigger::from_env(&PluginEnv {
            event_json: Some(event.to_string()),
            ..bare_env()
        })
    }

    fn tabs(ids: &[&str]) -> Target {
        Target::Tabs(ids.iter().map(|id| TabId::from(*id)).collect())
    }

    #[test]
    fn a_created_pane_equalizes_the_tab_it_appeared_in() {
        let trigger = on_event(&json!({
            "event": "pane_created",
            "data": {
                "type": "pane_created",
                "pane": { "pane_id": "w1:p2", "tab_id": "w1:t1", "workspace_id": "w1" },
            },
        }))
        .unwrap();

        assert_eq!(trigger.target, tabs(&["w1:t1"]));
        assert_eq!(trigger.settle, Settle::PanePresent(PaneId::from("w1:p2")));
    }

    #[test]
    fn the_envelope_event_is_ignored_and_the_data_tag_decides() {
        let trigger = on_event(&json!({
            "event": "pane_created",
            "data": { "type": "pane_closed", "pane_id": "w1:p2", "workspace_id": "w1" },
        }))
        .unwrap();

        assert_eq!(trigger.target, Target::Workspace(WorkspaceId::from("w1")));
        assert_eq!(trigger.settle, Settle::PaneAbsent(PaneId::from("w1:p2")));
    }

    #[test]
    fn a_closed_pane_equalizes_the_whole_workspace() {
        let trigger = on_event(&json!({
            "event": "pane_closed",
            "data": { "type": "pane_closed", "pane_id": "w1:p2", "workspace_id": "w1" },
        }))
        .unwrap();

        assert_eq!(trigger.target, Target::Workspace(WorkspaceId::from("w1")));
        assert_eq!(trigger.settle, Settle::PaneAbsent(PaneId::from("w1:p2")));
    }

    #[test]
    fn an_exited_pane_is_no_different_from_a_closed_one() {
        let trigger = on_event(&json!({
            "event": "pane_exited",
            "data": { "type": "pane_exited", "pane_id": "w1:p2", "workspace_id": "w1" },
        }))
        .unwrap();

        assert_eq!(trigger.target, Target::Workspace(WorkspaceId::from("w1")));
        assert_eq!(trigger.settle, Settle::PaneAbsent(PaneId::from("w1:p2")));
    }

    fn on_move(extra: &Value) -> Trigger {
        let mut data = json!({
            "type": "pane_moved",
            "pane": { "pane_id": "w1:p2", "tab_id": "w1:t2" },
            "previous_pane_id": "w1:p2",
            "previous_tab_id": "w1:t1",
            "previous_workspace_id": "w1",
        });
        let (Value::Object(data_fields), Value::Object(extra_fields)) = (&mut data, extra.clone())
        else {
            panic!("the fixture is an object");
        };
        data_fields.extend(extra_fields);
        on_event(&json!({ "event": "pane_moved", "data": data })).unwrap()
    }

    #[test]
    fn a_moved_pane_equalizes_both_the_tab_it_left_and_the_one_it_reached() {
        assert_eq!(on_move(&json!({})).target, tabs(&["w1:t2", "w1:t1"]));
    }

    #[test]
    fn a_move_that_emptied_its_tab_leaves_the_gone_tab_alone() {
        let trigger = on_move(&json!({ "closed_tab_id": "w1:t1" }));
        assert_eq!(trigger.target, tabs(&["w1:t2"]));
    }

    #[test]
    fn a_move_within_one_tab_equalizes_it_once() {
        let trigger = on_move(&json!({ "pane": { "pane_id": "w1:p2", "tab_id": "w1:t1" } }));
        assert_eq!(trigger.target, tabs(&["w1:t1"]));
    }

    #[test]
    fn a_move_that_kept_the_pane_id_only_waits_for_the_pane() {
        assert_eq!(
            on_move(&json!({})).settle,
            Settle::PanePresent(PaneId::from("w1:p2"))
        );
    }

    #[test]
    fn a_move_across_workspaces_waits_for_both_sides() {
        let trigger = on_move(&json!({ "pane": { "pane_id": "w2:p5", "tab_id": "w2:t1" } }));
        assert_eq!(
            trigger.settle,
            Settle::PaneRelocated {
                arrived: PaneId::from("w2:p5"),
                departed: PaneId::from("w1:p2"),
            }
        );
    }

    #[test]
    fn an_event_we_did_not_subscribe_to_equalizes_everything_without_waiting() {
        let trigger = on_event(&json!({
            "event": "pane_focused",
            "data": { "type": "pane_focused", "pane_id": "w1:p2", "workspace_id": "w1" },
        }))
        .unwrap();

        assert_eq!(trigger.target, Target::EveryWorkspace);
        assert_eq!(trigger.settle, Settle::Immediately);
    }

    fn on_action(
        context_json: Option<&str>,
        tab_id: Option<&str>,
        workspace: Option<&str>,
    ) -> Trigger {
        Trigger::from_env(&PluginEnv {
            context_json: context_json.map(str::to_owned),
            tab_id: tab_id.map(TabId::from),
            workspace_id: workspace.map(WorkspaceId::from),
            ..bare_env()
        })
        .expect("an action never parses an event")
    }

    #[test]
    fn an_action_equalizes_the_tab_it_was_invoked_on() {
        let trigger = on_action(
            Some(r#"{"workspace_id":"w1","tab_id":"w1:t1"}"#),
            Some("w1:t9"),
            Some("w1"),
        );
        assert_eq!(trigger.target, tabs(&["w1:t1"]));
        assert_eq!(trigger.settle, Settle::Immediately);
    }

    #[test]
    fn an_action_without_a_usable_context_falls_back_to_the_tab_variable() {
        assert_eq!(
            on_action(Some("not json"), Some("w1:t9"), Some("w1")).target,
            tabs(&["w1:t9"])
        );
        assert_eq!(
            on_action(Some(r#"{"workspace_id":"w1"}"#), Some("w1:t9"), None).target,
            tabs(&["w1:t9"])
        );
    }

    #[test]
    fn an_action_that_cannot_name_a_tab_falls_back_to_the_workspace() {
        assert_eq!(
            on_action(None, None, Some("w1")).target,
            Target::Workspace(WorkspaceId::from("w1"))
        );
    }

    #[test]
    fn an_action_that_knows_nothing_equalizes_everything() {
        assert_eq!(on_action(None, None, None).target, Target::EveryWorkspace);
    }

    #[test]
    fn waiting_for_a_pane_ends_when_it_shows_up() {
        let settle = Settle::PanePresent(PaneId::from("w1:p2"));
        assert!(settle.is_met(|pane_id| *pane_id == PaneId::from("w1:p2")));
        assert!(!settle.is_met(|_| false));
    }

    #[test]
    fn waiting_for_a_pane_to_go_ends_when_it_is_gone() {
        let settle = Settle::PaneAbsent(PaneId::from("w1:p2"));
        assert!(settle.is_met(|_| false));
        assert!(!settle.is_met(|pane_id| *pane_id == PaneId::from("w1:p2")));
    }

    #[test]
    fn waiting_for_a_relocation_needs_both_sides_to_agree() {
        let settle = Settle::PaneRelocated {
            arrived: PaneId::from("w2:p5"),
            departed: PaneId::from("w1:p2"),
        };
        assert!(settle.is_met(|pane_id| *pane_id == PaneId::from("w2:p5")));
        assert!(!settle.is_met(|_| true), "the old pane is still there");
        assert!(!settle.is_met(|_| false), "the new pane has not landed");
    }

    #[test]
    fn not_waiting_is_always_met() {
        assert!(Settle::Immediately.is_met(|_| false));
        assert!(Settle::Immediately.is_met(|_| true));
    }

    #[test]
    fn an_event_missing_a_required_field_is_rejected() {
        let error = on_event(&json!({
            "event": "pane_closed",
            "data": { "type": "pane_closed", "workspace_id": "w1" },
        }))
        .unwrap_err();

        assert!(
            format!("{error:#}").contains("HERDR_PLUGIN_EVENT_JSON"),
            "the message should name the variable: {error:#}"
        );
    }
}
