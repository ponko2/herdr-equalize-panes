use super::{
    TabId, WorkspaceId,
    api::{Branch, HerdrApi, TabLayout},
};
use anyhow::{Context, Result, bail};
use serde::{
    Deserialize, Serialize,
    de::{DeserializeOwned, IgnoredAny},
};
use std::{
    io::{BufRead, BufReader, BufWriter, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
};

const CLIENT_ID: &str = "equalize-panes";

pub struct SocketClient {
    socket_path: PathBuf,
}

#[derive(Serialize)]
struct Request<'a, P> {
    id: &'a str,
    method: &'a str,
    params: P,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Response<R> {
    Ok { result: R },
    Err { error: ResponseError },
}

#[derive(Deserialize)]
struct ResponseError {
    code: String,
    message: String,
}

impl ResponseError {
    fn target_is_gone(&self) -> bool {
        matches!(
            self.code.as_str(),
            "tab_not_found" | "workspace_not_found" | "layout_not_found" | "split_not_found"
        )
    }
}

impl SocketClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }

    fn call<P: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<Option<R>> {
        let stream = UnixStream::connect(&self.socket_path)
            .with_context(|| format!("connecting {}", self.socket_path.display()))?;

        let mut writer = BufWriter::new(&stream);
        serde_json::to_writer(
            &mut writer,
            &Request {
                id: CLIENT_ID,
                method,
                params,
            },
        )
        .with_context(|| format!("serializing {method}"))?;
        writer
            .write_all(b"\n")
            .and_then(|()| writer.flush())
            .with_context(|| format!("sending {method}"))?;

        let mut line = String::new();
        BufReader::new(&stream)
            .read_line(&mut line)
            .with_context(|| format!("reading the {method} response"))?;

        match serde_json::from_str(&line)
            .with_context(|| format!("parsing the {method} response: {line}"))?
        {
            Response::Ok { result } => Ok(Some(result)),
            Response::Err { error } if error.target_is_gone() => {
                log::debug!("{method}: {} ({})", error.message, error.code);
                Ok(None)
            }
            Response::Err { error } => bail!("{method}: {}: {}", error.code, error.message),
        }
    }
}

impl HerdrApi for SocketClient {
    fn tab_ids(&self, workspace_id: Option<&WorkspaceId>) -> Result<Vec<TabId>> {
        #[derive(Serialize)]
        struct Params<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            workspace_id: Option<&'a WorkspaceId>,
        }
        #[derive(Deserialize)]
        struct TabList {
            tabs: Vec<Tab>,
        }
        #[derive(Deserialize)]
        struct Tab {
            tab_id: TabId,
        }

        let list: Option<TabList> = self.call("tab.list", Params { workspace_id })?;
        Ok(list
            .into_iter()
            .flat_map(|list| list.tabs)
            .map(|tab| tab.tab_id)
            .collect())
    }

    fn export_layout(&self, tab_id: &TabId) -> Result<Option<TabLayout>> {
        #[derive(Serialize)]
        struct Params<'a> {
            tab_id: &'a TabId,
        }
        #[derive(Deserialize)]
        struct Export {
            layout: TabLayout,
        }

        let export: Option<Export> = self.call("layout.export", Params { tab_id })?;
        Ok(export.map(|export| export.layout))
    }

    fn set_split_ratio(&self, tab_id: &TabId, path: &[Branch], ratio: f64) -> Result<bool> {
        #[derive(Serialize)]
        struct Params<'a> {
            tab_id: &'a TabId,
            path: Vec<bool>,
            ratio: f64,
        }

        let applied: Option<IgnoredAny> = self.call(
            "layout.set_split_ratio",
            Params {
                tab_id,
                path: path.iter().copied().map(Branch::as_wire).collect(),
                ratio,
            },
        )?;
        Ok(applied.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::{super::PaneId, *};
    use std::{
        fs,
        os::unix::net::UnixListener,
        process,
        sync::{
            Arc, Mutex,
            atomic::{AtomicU32, Ordering},
        },
        thread,
    };

    fn unique_socket_path() -> PathBuf {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let name = format!(
            "eqp-{}-{}.sock",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        std::env::temp_dir().join(name)
    }

    #[test]
    fn a_socket_path_fits_in_the_104_byte_sun_path_limit() {
        // NOTE: macOS caps sockaddr_un.sun_path at 104 bytes
        assert!(unique_socket_path().as_os_str().len() <= 104);
    }

    struct FakeServer {
        path: PathBuf,
        requests: Arc<Mutex<Vec<String>>>,
    }

    impl FakeServer {
        fn replying_with(replies: &[&str]) -> Self {
            let path = unique_socket_path();
            let listener = UnixListener::bind(&path).expect("binding a fresh socket path");
            let requests = Arc::new(Mutex::new(Vec::new()));

            let recorded = Arc::clone(&requests);
            let replies: Vec<String> = replies.iter().map(|reply| (*reply).to_owned()).collect();
            thread::spawn(move || {
                for reply in replies {
                    let Ok((stream, _)) = listener.accept() else {
                        return;
                    };
                    let mut request = String::new();
                    BufReader::new(&stream)
                        .read_line(&mut request)
                        .expect("the client sends one line");
                    recorded.lock().expect("no test panics here").push(request);
                    writeln!(&stream, "{reply}").expect("the client is still reading");
                }
            });

            Self { path, requests }
        }

        fn client(&self) -> SocketClient {
            SocketClient::new(&self.path)
        }

        fn requests(&self) -> Vec<String> {
            self.requests.lock().expect("no test panics here").clone()
        }
    }

    impl Drop for FakeServer {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    const REAL_LAYOUT_EXPORT: &str = r#"{"id":"ro","result":{"type":"layout_export","layout":{"workspace_id":"w8N","tab_id":"w8N:t2","zoomed":false,"focused_pane_id":"w8N:p5","root":{"type":"split","direction":"right","ratio":0.5,"first":{"type":"pane","pane_id":"w8N:p2","cwd":"/Users/x"},"second":{"type":"pane","pane_id":"w8N:p5","cwd":"/Users/x"}}}}}"#;
    const REAL_TAB_LIST: &str = r#"{"id":"ro","result":{"type":"tab_list","tabs":[{"tab_id":"w8N:t1","workspace_id":"w8N","number":1,"label":"1","focused":false,"pane_count":2,"agent_status":"done"},{"tab_id":"w8N:t2","workspace_id":"w8N","number":2,"label":"2","focused":true,"pane_count":2,"agent_status":"working"}]}}"#;

    const APPLIED: &str = r#"{"id":"ro","result":{"type":"layout_split_ratio_set"}}"#;

    fn error(code: &str) -> String {
        format!(r#"{{"id":"ro","error":{{"code":"{code}","message":"gone"}}}}"#)
    }

    #[test]
    fn a_request_goes_out_as_one_line_of_ndjson() {
        let server = FakeServer::replying_with(&[REAL_LAYOUT_EXPORT]);
        server
            .client()
            .export_layout(&TabId::from("w8N:t2"))
            .unwrap();

        assert_eq!(
            server.requests(),
            [concat!(
                r#"{"id":"equalize-panes","method":"layout.export","#,
                r#""params":{"tab_id":"w8N:t2"}}"#,
                "\n"
            )]
        );
    }

    #[test]
    fn a_layout_export_becomes_a_tab_layout_despite_fields_we_do_not_declare() {
        let server = FakeServer::replying_with(&[REAL_LAYOUT_EXPORT]);
        let layout = server
            .client()
            .export_layout(&TabId::from("w8N:t2"))
            .unwrap()
            .expect("the tab is there");

        assert_eq!(layout.tab_id, TabId::from("w8N:t2"));
        assert!(!layout.zoomed);
        assert!(layout.contains_pane(&PaneId::from("w8N:p5")));
    }

    #[test]
    fn a_real_tab_list_becomes_tab_ids() {
        let server = FakeServer::replying_with(&[REAL_TAB_LIST]);
        assert_eq!(
            server.client().tab_ids(None).unwrap(),
            [TabId::from("w8N:t1"), TabId::from("w8N:t2")]
        );
        assert_eq!(
            server.requests(),
            [concat!(
                r#"{"id":"equalize-panes","method":"tab.list","params":{}}"#,
                "\n"
            )]
        );
    }

    #[test]
    fn a_split_path_goes_out_as_an_array_of_bools() {
        let server = FakeServer::replying_with(&[APPLIED]);
        let applied = server
            .client()
            .set_split_ratio(
                &TabId::from("w8N:t2"),
                &[Branch::First, Branch::Second],
                0.75,
            )
            .unwrap();

        assert!(applied);
        assert_eq!(
            server.requests(),
            [concat!(
                r#"{"id":"equalize-panes","method":"layout.set_split_ratio","#,
                r#""params":{"tab_id":"w8N:t2","path":[false,true],"ratio":0.75}}"#,
                "\n"
            )]
        );
    }

    #[test]
    fn a_target_that_is_gone_is_not_an_error() {
        for code in [
            "tab_not_found",
            "workspace_not_found",
            "layout_not_found",
            "split_not_found",
        ] {
            let server = FakeServer::replying_with(&[&error(code)]);
            assert!(
                server
                    .client()
                    .export_layout(&TabId::from("w8N:t2"))
                    .unwrap()
                    .is_none(),
                "{code} should read as a gone target"
            );
        }
    }

    #[test]
    fn a_stale_split_path_reads_as_not_applied() {
        let server = FakeServer::replying_with(&[&error("split_not_found")]);
        let applied = server
            .client()
            .set_split_ratio(&TabId::from("w8N:t2"), &[], 0.5)
            .unwrap();
        assert!(!applied);
    }

    #[test]
    fn an_unexpected_error_code_is_propagated() {
        let server = FakeServer::replying_with(&[&error("invalid_ratio")]);
        let error = server
            .client()
            .set_split_ratio(&TabId::from("w8N:t2"), &[], 0.5)
            .unwrap_err();

        assert!(
            format!("{error:#}").contains("invalid_ratio"),
            "the code should survive: {error:#}"
        );
    }

    #[test]
    fn every_call_opens_a_fresh_connection() {
        let server = FakeServer::replying_with(&[REAL_TAB_LIST, REAL_LAYOUT_EXPORT]);
        let client = server.client();

        client.tab_ids(None).unwrap();
        client.export_layout(&TabId::from("w8N:t2")).unwrap();

        assert_eq!(
            server.requests().len(),
            2,
            "the server accepted both calls separately"
        );
    }
}
