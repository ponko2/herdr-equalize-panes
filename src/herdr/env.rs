use super::{TabId, WorkspaceId};
use anyhow::{Context, Result};
use std::path::PathBuf;

#[derive(Debug)]
pub struct PluginEnv {
    pub socket_path: PathBuf,
    pub state_dir: PathBuf,
    pub event_json: Option<String>,
    pub context_json: Option<String>,
    pub tab_id: Option<TabId>,
    pub workspace_id: Option<WorkspaceId>,
}

impl PluginEnv {
    pub fn from_process() -> Result<Self> {
        Self::read(|key| std::env::var(key).ok())
    }

    fn read(var: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let socket_path = var("HERDR_SOCKET_PATH")
            .context("HERDR_SOCKET_PATH is not set; run this as a Herdr plugin")?;

        Ok(Self {
            socket_path: PathBuf::from(socket_path),
            state_dir: var("HERDR_PLUGIN_STATE_DIR").map_or_else(std::env::temp_dir, PathBuf::from),
            event_json: var("HERDR_PLUGIN_EVENT_JSON"),
            context_json: var("HERDR_PLUGIN_CONTEXT_JSON"),
            tab_id: var("HERDR_TAB_ID").map(TabId::from),
            workspace_id: var("HERDR_WORKSPACE_ID").map(WorkspaceId::from),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> Result<PluginEnv> {
        let owned: Vec<(String, String)> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect();
        PluginEnv::read(move |key| {
            owned
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.clone())
        })
    }

    #[test]
    fn the_socket_path_is_required() {
        let error = env(&[]).unwrap_err();
        assert!(
            format!("{error:#}").contains("HERDR_SOCKET_PATH"),
            "the message should name the variable: {error:#}"
        );
    }

    #[test]
    fn a_missing_state_directory_falls_back_to_a_temporary_one() {
        let env = env(&[("HERDR_SOCKET_PATH", "/tmp/herdr.sock")]).unwrap();
        assert_eq!(env.state_dir, std::env::temp_dir());
    }

    #[test]
    fn the_trigger_variables_are_read_as_they_are() {
        let env = env(&[
            ("HERDR_SOCKET_PATH", "/tmp/herdr.sock"),
            ("HERDR_PLUGIN_STATE_DIR", "/tmp/state"),
            ("HERDR_PLUGIN_EVENT_JSON", "{}"),
            ("HERDR_PLUGIN_CONTEXT_JSON", "{}"),
            ("HERDR_TAB_ID", "w1:t1"),
            ("HERDR_WORKSPACE_ID", "w1"),
        ])
        .unwrap();

        assert_eq!(env.socket_path, PathBuf::from("/tmp/herdr.sock"));
        assert_eq!(env.state_dir, PathBuf::from("/tmp/state"));
        assert_eq!(env.event_json.as_deref(), Some("{}"));
        assert_eq!(env.context_json.as_deref(), Some("{}"));
        assert_eq!(env.tab_id, Some(TabId::from("w1:t1")));
        assert_eq!(env.workspace_id, Some(WorkspaceId::from("w1")));
    }
}
