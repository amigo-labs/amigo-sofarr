//! Test support: fake Newznab indexers and a fake nzbget as one axum router, and an
//! [`App`] wired to them.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::{Path, RawQuery, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use jiff::Timestamp;
use serde_json::{Value, json};
use sofarr_db::Db;

use crate::{app::App, config::Config};

/// State of the fake services; tests change it between calls.
#[derive(Debug)]
pub struct Fake {
    /// `<item>` elements per indexer name, served for every listing request.
    pub feeds: HashMap<String, String>,
    /// `t=caps` answer per indexer name (default: TV search by title only).
    pub caps: HashMap<String, String>,
    /// Query strings of every indexer request, as `<indexer>?<query>`.
    pub requests: Vec<String>,
    /// HTTP status an indexer answers with instead of its feed.
    pub indexer_status: HashMap<String, u16>,
    /// HTTP status of `/nzb/<guid>` (default 200 with an NZB).
    pub nzb_status: HashMap<String, u16>,
    /// Body of `/nzb/<guid>` instead of an NZB.
    pub nzb_body: HashMap<String, String>,
    /// GUIDs whose NZB was downloaded.
    pub nzb_fetched: Vec<String>,
    pub queue: Vec<Value>,
    pub history: Vec<Value>,
    /// NZB names handed to nzbget.
    pub appended: Vec<String>,
    /// Every JSON-RPC call as `method params`.
    pub rpc: Vec<String>,
    /// nzbget answers HTTP 503 to everything.
    pub nzbget_down: bool,
    /// Result of the next `append` calls (default: ids from 77 upwards).
    pub append_result: Option<i64>,
    next_id: i64,
}

impl Default for Fake {
    fn default() -> Self {
        Self {
            feeds: HashMap::new(),
            caps: HashMap::new(),
            requests: Vec::new(),
            indexer_status: HashMap::new(),
            nzb_status: HashMap::new(),
            nzb_body: HashMap::new(),
            nzb_fetched: Vec::new(),
            queue: Vec::new(),
            history: Vec::new(),
            appended: Vec::new(),
            rpc: Vec::new(),
            nzbget_down: false,
            append_result: None,
            next_id: 77,
        }
    }
}

pub type Shared = Arc<Mutex<Fake>>;

/// A running fake with its address.
pub struct Services {
    pub fake: Shared,
    pub addr: SocketAddr,
}

impl Services {
    pub async fn start() -> Self {
        let fake: Shared = Arc::default();
        let router = Router::new()
            .route("/{indexer}/api", get(indexer))
            .route("/nzb/{guid}", get(nzb))
            .route("/jsonrpc", post(rpc))
            .with_state(Arc::clone(&fake));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move { axum::serve(listener, router).await });
        Self { fake, addr }
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, Fake> {
        self.fake.lock().expect("fake lock")
    }

    /// One RSS item whose NZB is served by this fake.
    pub fn item(&self, title: &str, guid: &str, size: u64) -> String {
        format!(
            r#"<item><title>{title}</title><guid>{guid}</guid>
               <enclosure url="http://{}/nzb/{guid}" length="{size}"/></item>"#,
            self.addr
        )
    }

    pub fn feed(&self, indexer: &str, items: &[String]) {
        self.lock().feeds.insert(indexer.to_owned(), items.concat());
    }

    /// Indexer requests whose query contains `needle`.
    pub fn requests_with(&self, needle: &str) -> Vec<String> {
        self.lock()
            .requests
            .iter()
            .filter(|r| r.contains(needle))
            .cloned()
            .collect()
    }

    /// nzbget history entry of a finished download.
    pub fn finish(&self, id: i64, name: &str, status: &str, dest: &str) {
        self.lock().history.push(json!({
            "NZBID": id, "Name": name, "Status": status, "DestDir": dest
        }));
    }

    /// `[[indexer]]` section for one of the fake's indexers.
    pub fn indexer_toml(&self, name: &str, tags: &[&str]) -> String {
        let tags: Vec<String> = tags.iter().map(|t| format!("\"{t}\"")).collect();
        format!(
            "[[indexer]]\nname = \"{name}\"\nurl = \"http://{}/{name}/api\"\napi_key = \"k\"\n\
             categories = [2000]\ntv_categories = [5000]\ntags = [{}]\n",
            self.addr,
            tags.join(", ")
        )
    }

    /// `[nzbget]` section pointing at the fake.
    pub fn nzbget_toml(&self) -> String {
        format!(
            "[nzbget]\nurl = \"http://{}\"\nusername = \"u\"\npassword = \"p\"\n",
            self.addr
        )
    }
}

async fn indexer(
    State(fake): State<Shared>,
    Path(name): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    let query = query.unwrap_or_default();
    let mut fake = fake.lock().expect("fake lock");
    fake.requests.push(format!("{name}?{query}"));
    if let Some(status) = fake.indexer_status.get(&name) {
        return StatusCode::from_u16(*status)
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            .into_response();
    }
    if query.contains("t=caps") {
        return fake.caps.get(&name).cloned().unwrap_or_else(|| {
            r#"<caps><searching><tv-search available="yes" supportedParams="q"/></searching></caps>"#
                .to_owned()
        })
        .into_response();
    }
    let items = fake.feeds.get(&name).cloned().unwrap_or_default();
    format!("<rss><channel>{items}</channel></rss>").into_response()
}

async fn nzb(State(fake): State<Shared>, Path(guid): Path<String>) -> Response {
    let mut fake = fake.lock().expect("fake lock");
    fake.nzb_fetched.push(guid.clone());
    if let Some(status) = fake.nzb_status.get(&guid) {
        return StatusCode::from_u16(*status)
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            .into_response();
    }
    fake.nzb_body
        .get(&guid)
        .cloned()
        .unwrap_or_else(|| "<nzb/>".to_owned())
        .into_response()
}

async fn rpc(State(fake): State<Shared>, Json(body): Json<Value>) -> Response {
    let mut fake = fake.lock().expect("fake lock");
    let method = body["method"].as_str().unwrap_or_default().to_owned();
    fake.rpc.push(format!("{method} {}", body["params"]));
    if fake.nzbget_down {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let result = match method.as_str() {
        "append" => {
            fake.appended
                .push(body["params"][0].as_str().unwrap_or_default().to_owned());
            let id = fake.append_result.unwrap_or(fake.next_id);
            fake.next_id += 1;
            json!(id)
        }
        "listgroups" => json!(fake.queue),
        "history" => json!(fake.history),
        _ => json!(true),
    };
    Json(json!({ "result": result })).into_response()
}

/// Which library the app starts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Library {
    /// `library::tests::documents()`: Erster Film missing, Zweiter Film below the cutoff, …
    Movies,
    /// `series::tests::sonarr_documents()`: Kleine Insel, S01E02 missing.
    Series,
}

/// An app on an in-memory database with the given library and config (TOML).
pub async fn app(library: Library, toml: &str) -> App {
    let config = Config::parse(toml).expect("test config");
    let db = Db::in_memory().await.expect("db");
    let now = Timestamp::now();
    match library {
        Library::Movies => {
            crate::library::import_documents(&db, &crate::library::tests::documents(), now)
                .await
                .expect("movies");
        }
        Library::Series => {
            let (docs, episodes) = crate::series::tests::sonarr_documents();
            crate::series::import_documents(&db, &docs, &episodes, now)
                .await
                .expect("series");
        }
    }
    let mut app = App::with_db(config, db).expect("app");
    let http = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client");
    let (indexers, nzbget) = App::build_clients(&app.config, &http);
    app.http = http;
    app.indexers = indexers;
    app.nzbget = nzbget;
    app
}

/// sofarr's API routes (without the login guard) on a local port.
pub struct Http {
    pub base: String,
    pub client: reqwest::Client,
    pub app: Arc<App>,
}

impl Http {
    pub async fn start(app: App) -> Self {
        let app = Arc::new(app);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let routes = crate::api::routes(Arc::clone(&app));
        tokio::spawn(async move { axum::serve(listener, routes).await });
        Self {
            base: format!("http://{addr}"),
            client: reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("client"),
            app,
        }
    }

    /// Status and JSON body (`null` when the body is no JSON).
    pub async fn send(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = self.client.request(method, format!("{}{path}", self.base));
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.expect("request");
        let status = StatusCode::from_u16(response.status().as_u16()).expect("status");
        let text = response.text().await.expect("body");
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    pub async fn get(&self, path: &str) -> (StatusCode, Value) {
        self.send(reqwest::Method::GET, path, None).await
    }

    pub async fn post(&self, path: &str, body: Value) -> (StatusCode, Value) {
        self.send(reqwest::Method::POST, path, Some(body)).await
    }
}
