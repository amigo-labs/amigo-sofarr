//! The web UI's server side (spec web-ui-shell): serving the build, the system page's
//! endpoint.

use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use reqwest::{StatusCode, header};
use serde_json::Value;

use crate::{
    api,
    app::{self, App},
    status::Job,
    testkit::{Library, Services, app},
    ui::Files,
};

struct Server {
    base: String,
    client: reqwest::Client,
    app: Arc<App>,
}

impl Server {
    async fn start(app: App, files: Files) -> Self {
        let app = Arc::new(app);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let router = api::app_router_with(Arc::clone(&app), files)
            .into_make_service_with_connect_info::<SocketAddr>();
        tokio::spawn(async move { axum::serve(listener, router).await });
        Self {
            base: format!("http://{addr}"),
            client: reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("client"),
            app,
        }
    }

    async fn get(&self, path: &str, gzip: bool) -> reqwest::Response {
        let mut request = self.client.get(format!("{}{path}", self.base));
        if gzip {
            request = request.header(header::ACCEPT_ENCODING, "gzip, br");
        }
        request.send().await.expect("get")
    }
}

const DOCUMENT: &str = "<!doctype html><title>sofarr</title><div id=app></div>";
const SCRIPT: &str = "export const answer = 42;";
const SCRIPT_GZ: &[u8] = b"\x1f\x8b pretend-gzip";

fn build() -> Files {
    let files: HashMap<String, Vec<u8>> = [
        ("index.html", DOCUMENT.as_bytes()),
        ("_app/immutable/entry/start.Ab12.js", SCRIPT.as_bytes()),
        ("_app/immutable/entry/start.Ab12.js.gz", SCRIPT_GZ),
        ("_app/version.json", b"{\"version\":\"1\"}"),
        ("favicon.svg", b"<svg/>"),
    ]
    .into_iter()
    .map(|(p, d)| (p.to_owned(), d.to_vec()))
    .collect();
    Files::Given(Arc::new(files))
}

fn header_of(response: &reqwest::Response, name: header::HeaderName) -> String {
    response
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

#[tokio::test]
async fn pages_get_the_app_document_and_assets_their_files() {
    let server = Server::start(app(Library::Movies, "").await, build()).await;
    // Every page address, also a deep one, starts from the app document (AC-02).
    for path in ["/", "/system", "/filme/12", "/anmelden?weiter=%2Fsystem"] {
        let response = server.get(path, true).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(
            header_of(&response, header::CACHE_CONTROL),
            "no-cache",
            "{path}"
        );
        assert!(header_of(&response, header::CONTENT_TYPE).starts_with("text/html"));
        assert_eq!(response.text().await.expect("text"), DOCUMENT, "{path}");
    }
    // An asset: a year in the cache (AC-17), gzip when the browser takes it.
    let asset = "/_app/immutable/entry/start.Ab12.js";
    let packed = server.get(asset, true).await;
    assert_eq!(packed.status(), StatusCode::OK);
    assert_eq!(
        header_of(&packed, header::CACHE_CONTROL),
        "public, max-age=31536000, immutable"
    );
    assert_eq!(header_of(&packed, header::CONTENT_ENCODING), "gzip");
    assert!(header_of(&packed, header::CONTENT_TYPE).starts_with("text/javascript"));
    assert_eq!(packed.bytes().await.expect("bytes").as_ref(), SCRIPT_GZ);
    let plain = server.get(asset, false).await;
    assert_eq!(header_of(&plain, header::CONTENT_ENCODING), "");
    assert_eq!(plain.text().await.expect("text"), SCRIPT);
    // Files outside the immutable folder are checked every time.
    let version = server.get("/_app/version.json", false).await;
    assert_eq!(header_of(&version, header::CACHE_CONTROL), "no-cache");
    let icon = server.get("/favicon.svg", false).await;
    assert_eq!(icon.status(), StatusCode::OK);
    assert_eq!(header_of(&icon, header::CONTENT_TYPE), "image/svg+xml");
}

#[tokio::test]
async fn unknown_assets_and_api_paths_are_not_found() {
    let server = Server::start(app(Library::Movies, "").await, build()).await;
    let asset = server
        .get("/_app/immutable/entry/start.Old9.js", true)
        .await;
    assert_eq!(asset.status(), StatusCode::NOT_FOUND);
    let raw = server
        .get("/_app/immutable/entry/start.Ab12.js.gz", true)
        .await;
    assert_eq!(raw.status(), StatusCode::NOT_FOUND);
    let api = server.get("/api/gibt-es-nicht", false).await;
    assert_eq!(api.status(), StatusCode::NOT_FOUND);
    assert!(header_of(&api, header::CONTENT_TYPE).starts_with("application/json"));
    let body: Value = api.json().await.expect("json");
    assert_eq!(body["error"], "Unbekannter API-Endpunkt");
    let post = server
        .client
        .post(format!("{}/system", server.base))
        .send()
        .await
        .expect("post");
    assert_eq!(post.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn with_auth_on_unknown_api_paths_ask_for_a_login_first() {
    let hash = crate::auth::hash_password(crate::test_secret::password()).expect("hash");
    let config =
        format!("[auth]\nenabled = true\nusername = \"daniel\"\npassword_hash = \"{hash}\"\n");
    let server = Server::start(app(Library::Movies, &config).await, build()).await;
    let api = server.get("/api/gibt-es-nicht", false).await;
    assert_eq!(api.status(), StatusCode::UNAUTHORIZED);
    // The UI itself is open: it asks for the login through the API.
    let page = server.get("/system", false).await;
    assert_eq!(page.status(), StatusCode::OK);
    assert_eq!(page.text().await.expect("text"), DOCUMENT);
}

#[tokio::test]
async fn without_a_build_every_page_explains_how_to_build_it() {
    let empty = Files::Given(Arc::new(HashMap::new()));
    let server = Server::start(app(Library::Movies, "").await, empty).await;
    for path in ["/", "/system"] {
        let response = server.get(path, false).await;
        assert_eq!(response.status(), StatusCode::OK);
        let text = response.text().await.expect("text");
        assert!(
            text.contains("Die Weboberfläche wurde nicht gebaut"),
            "{text}"
        );
        assert!(text.contains("bun run build"), "{text}");
    }
    // The API works without the UI.
    let health = server.get("/api/health", false).await;
    assert_eq!(health.status(), StatusCode::OK);
}

/// Waits until `job` has run once.
async fn ran(app: &App, job: Job) {
    for _ in 0..200 {
        if app.status.job(job).last_run.is_some() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("{} did not run", job.name());
}

#[tokio::test]
async fn the_system_page_shows_jobs_connections_and_masked_settings() {
    let s = Services::start().await;
    s.feed("good", &[]);
    s.lock().indexer_status.insert("bad".to_owned(), 502);
    let root = std::env::temp_dir().join(format!("sofarr-system-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let config = format!(
        "[automation]\nmode = \"live\"\n[backup]\ndir = '{}'\ninterval_hours = 24\n\
         [library]\nrefresh_interval_hours = 0\n[rss]\ninterval_minutes = 15\n{}{}{}",
        root.join("backups").display(),
        s.indexer_toml("good", &[]),
        s.indexer_toml("bad", &[]),
        s.nzbget_toml(),
    );
    let server = Server::start(app(Library::Movies, &config).await, build()).await;
    // The RSS job runs once right away, as at the start of the server.
    let job = Arc::clone(&server.app);
    let runner = Arc::clone(&server.app);
    let task = tokio::spawn(app::every(
        job,
        Job::Rss,
        Duration::from_secs(900),
        false,
        move || {
            let app = Arc::clone(&runner);
            async move { app.rss_sync().await.map(|_| ()) }
        },
    ));
    ran(&server.app, Job::Rss).await;
    task.abort();

    let response = server.get("/api/system", false).await;
    assert_eq!(response.status(), StatusCode::OK);
    let system: Value = response.json().await.expect("json");
    assert_eq!(system["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(system["mode"], "live");

    let job = |name: &str| {
        system["jobs"]
            .as_array()
            .and_then(|jobs| jobs.iter().find(|j| j["name"] == name))
            .cloned()
            .unwrap_or_else(|| panic!("job {name}"))
    };
    let rss = job("rss");
    assert_eq!(rss["label"], "RSS-Sync");
    assert_eq!(rss["interval_minutes"], 15);
    assert!(rss["last_run"].is_string(), "{rss}");
    assert_eq!(rss["result"]["state"], "ok", "{rss}");
    assert!(rss["next_run"].is_string(), "{rss}");
    // Scheduled but not run yet.
    let downloads = job("downloads");
    assert!(downloads["interval_minutes"].is_number(), "{downloads}");
    assert!(downloads["last_run"].is_null() && downloads["result"].is_null());
    // Switched off: no interval, no next run.
    for off in ["import-lists", "refresh"] {
        let off = job(off);
        assert!(off["interval_minutes"].is_null(), "{off}");
        assert!(off["next_run"].is_null(), "{off}");
    }
    let backup = job("backup");
    assert_eq!(backup["interval_minutes"], 24 * 60);
    assert!(backup["next_run"].is_string(), "{backup}");

    let connection = |name: &str| {
        system["connections"]
            .as_array()
            .and_then(|c| c.iter().find(|c| c["name"] == name))
            .cloned()
            .unwrap_or_else(|| panic!("connection {name}"))
    };
    assert_eq!(connection("good")["state"], "ok");
    let bad = connection("bad");
    assert_eq!(bad["state"], "error");
    assert!(
        bad["message"].as_str().is_some_and(|m| !m.is_empty()),
        "{bad}"
    );
    let nzbget = connection("nzbget");
    assert_eq!(nzbget["kind"], "download_client");
    assert_eq!(nzbget["state"], "no_contact");

    // Secrets are masked, the rest is shown.
    let config = system["config"].to_string();
    assert!(
        !config.contains("\"k\"") && !config.contains("\"p\""),
        "{config}"
    );
    assert_eq!(system["config"]["nzbget"]["password"], "***");
    assert_eq!(system["config"]["indexer"][0]["api_key"], "***");
    assert_eq!(system["config"]["indexer"][0]["name"], "good");
    assert_eq!(system["config"]["automation"]["mode"], "live");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn without_clients_the_system_page_says_not_configured() {
    let server = Server::start(app(Library::Movies, "").await, build()).await;
    let system: Value = server
        .get("/api/system", false)
        .await
        .json()
        .await
        .expect("json");
    assert_eq!(system["mode"], "shadow");
    let connections = system["connections"].as_array().expect("connections");
    assert_eq!(connections.len(), 2, "{connections:?}");
    for c in connections {
        assert_eq!(c["state"], "not_configured", "{c}");
    }
    assert!(
        system["jobs"]
            .as_array()
            .expect("jobs")
            .iter()
            .filter(|j| j["name"] == "rss" || j["name"] == "downloads")
            .all(|j| j["interval_minutes"].is_null())
    );
}
