//! Download-client abstraction (`DownloadClient` trait) and implementations.
//! First implementation: nzbget (spec D6). amigo-downloader follows once sofarr
//! replaces Radarr.

use std::future::Future;

use base64::Engine as _;
use serde_json::{Value, json};

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("{client}: Anfrage fehlgeschlagen: {source}")]
    Http {
        client: &'static str,
        #[source]
        source: reqwest::Error,
    },
    #[error("{client}: Fehler {message}")]
    Rpc {
        client: &'static str,
        message: String,
    },
    /// The client refused the NZB itself (invalid file), not a connection problem.
    #[error("{client}: NZB abgelehnt: {message}")]
    Rejected {
        client: &'static str,
        message: String,
    },
}

pub type Result<T> = std::result::Result<T, DownloadError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadState {
    Queued,
    Downloading,
    Paused,
    /// Download, repair and unpack finished.
    Completed,
    Failed(Failure),
}

/// Why a download ended without success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// Caused by the release itself: health or par check, unpacking, deleted by hand.
    Release,
    /// Anything else: moving, disk space, scripts, rejected as a duplicate, …
    Local,
}

/// nzbget history states that prove the release itself unusable.
const RELEASE_FAILURES: &[&str] = &[
    "FAILURE/HEALTH",
    "FAILURE/PAR",
    "FAILURE/UNPACK",
    "DELETED/HEALTH",
    "DELETED/MANUAL",
];

/// One download as the client reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadItem {
    pub id: String,
    pub name: String,
    pub category: String,
    pub state: DownloadState,
    pub size_bytes: u64,
    pub remaining_bytes: u64,
    /// Folder with the finished files, as the client sees it.
    pub output_path: Option<String>,
    /// Client status text, e.g. `FAILURE/PAR`.
    pub message: Option<String>,
}

/// What sofarr needs from a download client.
pub trait DownloadClient {
    /// Hands an NZB to the client; returns the client's id of the download.
    fn add_nzb(
        &self,
        name: &str,
        nzb: &[u8],
        category: &str,
    ) -> impl Future<Output = Result<String>> + Send;

    /// Queue and history of the client.
    fn items(&self) -> impl Future<Output = Result<Vec<DownloadItem>>> + Send;

    /// Removes a download from queue or history (files are left alone).
    fn remove(&self, id: &str) -> impl Future<Output = Result<()>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NzbgetConfig {
    /// Base URL, e.g. `http://nzbget:6789`.
    pub url: String,
    pub username: String,
    pub password: String,
}

pub struct Nzbget {
    client: reqwest::Client,
    config: NzbgetConfig,
}

const NZBGET: &str = "nzbget";
const MIB: u64 = 1024 * 1024;

impl Nzbget {
    pub fn new(client: reqwest::Client, config: NzbgetConfig) -> Self {
        Self { client, config }
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let http = |source: reqwest::Error| DownloadError::Http {
            client: NZBGET,
            source: source.without_url(),
        };
        let body: Value = self
            .client
            .post(format!("{}/jsonrpc", self.config.url.trim_end_matches('/')))
            .basic_auth(&self.config.username, Some(&self.config.password))
            .json(&json!({ "version": "1.1", "id": 1, "method": method, "params": params }))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(http)?
            .json()
            .await
            .map_err(http)?;
        if !body["error"].is_null() {
            return Err(DownloadError::Rpc {
                client: NZBGET,
                message: body["error"]["message"]
                    .as_str()
                    .map_or_else(|| body["error"].to_string(), str::to_owned),
            });
        }
        Ok(body["result"].clone())
    }
}

impl Nzbget {
    /// The current download speed in bytes per second (0 while paused or idle).
    pub async fn download_rate(&self) -> Result<u64> {
        let status = self.call("status", json!([])).await?;
        Ok(status["DownloadRate"].as_u64().unwrap_or(0))
    }
}

fn mib(value: &Value) -> u64 {
    value.as_u64().unwrap_or(0) * MIB
}

fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

/// Reads one entry of `listgroups`.
pub fn queue_item(group: &Value) -> DownloadItem {
    let status = text(&group["Status"]);
    let state = match status.as_str() {
        "PAUSED" => DownloadState::Paused,
        "QUEUED" => DownloadState::Queued,
        _ => DownloadState::Downloading,
    };
    DownloadItem {
        id: group["NZBID"].as_i64().unwrap_or(0).to_string(),
        name: text(&group["NZBName"]),
        category: text(&group["Category"]),
        state,
        size_bytes: mib(&group["FileSizeMB"]),
        remaining_bytes: mib(&group["RemainingSizeMB"]),
        output_path: Some(text(&group["DestDir"])).filter(|p| !p.is_empty()),
        message: Some(status),
    }
}

/// Reads one entry of `history`.
pub fn history_item(entry: &Value) -> DownloadItem {
    let status = text(&entry["Status"]);
    let state = if status.starts_with("SUCCESS") {
        DownloadState::Completed
    } else if RELEASE_FAILURES.iter().any(|f| status.starts_with(f)) {
        DownloadState::Failed(Failure::Release)
    } else {
        DownloadState::Failed(Failure::Local)
    };
    let final_dir = text(&entry["FinalDir"]);
    let dest_dir = text(&entry["DestDir"]);
    DownloadItem {
        id: entry["NZBID"].as_i64().unwrap_or(0).to_string(),
        name: text(&entry["Name"]),
        category: text(&entry["Category"]),
        state,
        size_bytes: mib(&entry["FileSizeMB"]),
        remaining_bytes: 0,
        output_path: Some(if final_dir.is_empty() {
            dest_dir
        } else {
            final_dir
        })
        .filter(|p| !p.is_empty()),
        message: Some(status),
    }
}

impl DownloadClient for Nzbget {
    async fn add_nzb(&self, name: &str, nzb: &[u8], category: &str) -> Result<String> {
        let content = base64::engine::general_purpose::STANDARD.encode(nzb);
        let filename = if name.ends_with(".nzb") {
            name.to_owned()
        } else {
            format!("{name}.nzb")
        };
        let result = self
            .call(
                "append",
                json!([
                    filename,
                    content,
                    category,
                    0,
                    false,
                    false,
                    "",
                    0,
                    "SCORE",
                    []
                ]),
            )
            .await?;
        match result.as_i64() {
            Some(id) if id > 0 => Ok(id.to_string()),
            _ => Err(DownloadError::Rejected {
                client: NZBGET,
                message: format!("append lieferte {result}"),
            }),
        }
    }

    async fn items(&self) -> Result<Vec<DownloadItem>> {
        let queue = self.call("listgroups", json!([0])).await?;
        let history = self.call("history", json!([false])).await?;
        Ok(queue
            .as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .map(queue_item)
            .chain(
                history
                    .as_array()
                    .map_or(&[][..], Vec::as_slice)
                    .iter()
                    .map(history_item),
            )
            .collect())
    }

    async fn remove(&self, id: &str) -> Result<()> {
        let id: i64 = id.parse().map_err(|_| DownloadError::Rpc {
            client: NZBGET,
            message: format!("ungültige Download-ID {id}"),
        })?;
        // Queue entries are deleted, history entries hidden; both keep the files.
        self.call("editqueue", json!(["GroupFinalDelete", "", [id]]))
            .await?;
        let _ = self
            .call("editqueue", json!(["HistoryDelete", "", [id]]))
            .await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::{Json, Router, extract::State, http::HeaderMap, routing::post};

    use super::*;

    #[test]
    fn maps_queue_and_history() {
        let q = queue_item(
            &json!({"NZBID": 7, "NZBName": "Film.2020.1080p", "Category": "movies",
            "FileSizeMB": 4000, "RemainingSizeMB": 1000, "Status": "DOWNLOADING", "DestDir": "/downloads/inter/Film"}),
        );
        assert_eq!(q.state, DownloadState::Downloading);
        assert_eq!(q.remaining_bytes, 1000 * MIB);
        let h = history_item(
            &json!({"NZBID": 8, "Name": "Film.2020.1080p", "Category": "movies",
            "Status": "SUCCESS/ALL", "DestDir": "/downloads/completed/movies/Film", "FinalDir": ""}),
        );
        assert_eq!(h.state, DownloadState::Completed);
        assert_eq!(
            h.output_path.as_deref(),
            Some("/downloads/completed/movies/Film")
        );
        let f = history_item(&json!({"NZBID": 9, "Status": "FAILURE/PAR"}));
        assert_eq!(f.state, DownloadState::Failed(Failure::Release));
        assert_eq!(f.message.as_deref(), Some("FAILURE/PAR"));
    }

    #[test]
    fn classifies_failed_downloads() {
        let state = |status: &str| history_item(&json!({"NZBID": 1, "Status": status})).state;
        for status in [
            "FAILURE/HEALTH",
            "FAILURE/PAR",
            "FAILURE/UNPACK",
            "DELETED/HEALTH",
            "DELETED/MANUAL",
        ] {
            assert_eq!(
                state(status),
                DownloadState::Failed(Failure::Release),
                "{status}"
            );
        }
        for status in [
            "DELETED/DUPE",
            "FAILURE/MOVE",
            "WARNING/SPACE",
            "WARNING/SCRIPT",
            "DELETED/COPY",
        ] {
            assert_eq!(
                state(status),
                DownloadState::Failed(Failure::Local),
                "{status}"
            );
        }
        assert_eq!(state("SUCCESS/UNPACK"), DownloadState::Completed);
    }

    #[tokio::test]
    async fn a_rejected_nzb_is_its_own_error() {
        let app = Router::new().route("/jsonrpc", post(|| async { Json(json!({"result": 0})) }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move { axum::serve(listener, app).await });
        let nzbget = Nzbget::new(
            reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("client"),
            NzbgetConfig {
                url: format!("http://{addr}"),
                username: "u".to_owned(),
                password: "p".to_owned(),
            },
        );
        let err = nzbget
            .add_nzb("Film", b"kaputt", "movies")
            .await
            .expect_err("rejected");
        assert!(matches!(err, DownloadError::Rejected { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn speaks_json_rpc() {
        type Calls = Arc<Mutex<Vec<(String, Value)>>>;
        async fn rpc(
            State(calls): State<Calls>,
            headers: HeaderMap,
            Json(body): Json<Value>,
        ) -> Json<Value> {
            assert!(headers.get("authorization").is_some());
            let method = body["method"].as_str().unwrap_or_default().to_owned();
            calls
                .lock()
                .expect("lock")
                .push((method.clone(), body["params"].clone()));
            Json(match method.as_str() {
                "append" => json!({"result": 42}),
                "listgroups" => {
                    json!({"result": [{"NZBID": 42, "NZBName": "a", "Status": "QUEUED"}]})
                }
                "history" => {
                    json!({"result": [{"NZBID": 41, "Name": "b", "Status": "SUCCESS/ALL", "DestDir": "/d/b"}]})
                }
                "status" => json!({"result": {"DownloadRate": 2_097_152, "DownloadPaused": false}}),
                _ => json!({"result": true}),
            })
        }
        let calls: Calls = Arc::default();
        let app = Router::new()
            .route("/jsonrpc", post(rpc))
            .with_state(Arc::clone(&calls));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move { axum::serve(listener, app).await });
        let nzbget = Nzbget::new(
            reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("client"),
            NzbgetConfig {
                url: format!("http://{addr}"),
                username: "nzbget".to_owned(),
                password: "geheim".to_owned(),
            },
        );
        assert_eq!(
            nzbget
                .add_nzb("Film.2020", b"<nzb/>", "movies")
                .await
                .expect("add"),
            "42"
        );
        let items = nzbget.items().await.expect("items");
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].state, DownloadState::Completed);
        nzbget.remove("41").await.expect("remove");
        assert_eq!(nzbget.download_rate().await.expect("rate"), 2_097_152);
        let calls = calls.lock().expect("lock");
        let (_, append) = &calls[0];
        assert_eq!(append[0], "Film.2020.nzb");
        assert_eq!(
            append[1],
            base64::engine::general_purpose::STANDARD.encode(b"<nzb/>")
        );
        assert_eq!(append[2], "movies");
    }
}
