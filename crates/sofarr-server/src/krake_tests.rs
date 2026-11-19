//! Krake as download client through its NZBGet-compatible JSON-RPC API
//! (spec krake-download-client). The fake answers with the shapes of Krake's
//! `crates/server/src/nzbget_api.rs` (amigo-labs/amigo-krake, MIT).

use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use serde_json::{Value, json};
use sofarr_db::GrabStatus;

use crate::testkit::{Library, Services, app};

const RELEASE: &str = "Erster.Film.2001.German.1080p.BluRay.x264-GRP";
const EPISODE: &str = "Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP";
/// Krake derives its NZBIDs from its own download ids: large positive integers.
const KRAKE_ID: i64 = 1_734_912_055;
const OTHER_ID: i64 = 902_114_377;

#[derive(Default)]
struct Krake {
    queue: Vec<Value>,
    history: Vec<Value>,
    /// `(filename, category)` of every `append`.
    appended: Vec<(String, String)>,
    /// Every call as `method params`.
    rpc: Vec<String>,
    /// Answer every call with this JSON-RPC error.
    error: Option<String>,
}

type Shared = Arc<Mutex<Krake>>;

/// Basic auth `u:p`, as Krake's `nzbget_api` credentials.
fn authorized(headers: &HeaderMap) -> bool {
    // `u:p` in Base64.
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == "Basic dTpw")
}

async fn jsonrpc(
    State(krake): State<Shared>,
    headers: HeaderMap,
    Json(req): Json<Value>,
) -> Response {
    if !authorized(&headers) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"id": null, "error": {"code": -32000, "message": "Unauthorized"}})),
        )
            .into_response();
    }
    let mut krake = krake.lock().expect("krake lock");
    let method = req["method"].as_str().unwrap_or_default().to_owned();
    krake.rpc.push(format!("{method} {}", req["params"]));
    if let Some(message) = krake.error.clone() {
        return Json(json!({"id": req["id"], "error": {"code": -32602, "message": message}}))
            .into_response();
    }
    let result = match method.as_str() {
        "version" => json!("krake-0.1.0"),
        "append" => {
            let p = &req["params"];
            krake.appended.push((
                p[0].as_str().unwrap_or_default().to_owned(),
                p[2].as_str().unwrap_or_default().to_owned(),
            ));
            json!(KRAKE_ID)
        }
        "listgroups" => json!(krake.queue),
        "history" => json!(krake.history),
        "status" => json!({"DownloadRate": 0, "DownloadPaused": false, "ServerStandBy": true}),
        _ => json!(true),
    };
    Json(json!({"id": req["id"], "result": result})).into_response()
}

/// A queue entry as Krake's `listgroups` lists it.
fn queue_entry(
    id: i64,
    name: &str,
    category: &str,
    status: &str,
    size_mb: u64,
    left_mb: u64,
) -> Value {
    json!({
        "NZBID": id, "NZBName": name, "NZBFilename": format!("{name}.nzb"),
        "NZBNicename": name, "Kind": "NZB", "URL": "nzb://upload",
        "DestDir": "/downloads", "FinalDir": "", "Category": category,
        "FileSizeLo": 0, "FileSizeHi": 0, "FileSizeMB": size_mb,
        "DownloadedSizeMB": size_mb - left_mb, "RemainingSizeMB": left_mb,
        "ActiveDownloads": 1, "Status": status, "Health": 1000, "CriticalHealth": 1000,
        "MaxPriority": 0, "MinPriority": 0, "Parameters": [], "ScriptStatuses": [],
        "ServerStats": [], "PostInfoText": "",
    })
}

/// A history entry as Krake's `history` lists it.
fn history_entry(id: i64, name: &str, category: &str, success: bool, dir: &str) -> Value {
    json!({
        "NZBID": id, "Name": name, "NZBName": name, "NZBFilename": format!("{name}.nzb"),
        "NZBNicename": name, "Kind": "NZB", "URL": "nzb://upload",
        "DestDir": dir, "FinalDir": dir, "Category": category,
        "FileSizeMB": 4000, "DownloadedSizeMB": 4000,
        "Status": if success { "SUCCESS/ALL" } else { "FAILURE/NONE" },
        "ParStatus": if success { "SUCCESS" } else { "NONE" },
        "UnpackStatus": if success { "SUCCESS" } else { "NONE" },
        "MoveStatus": if success { "SUCCESS" } else { "NONE" },
        "ScriptStatus": "NONE", "DeleteStatus": "NONE", "MarkStatus": "NONE",
        "Health": 1000, "Deleted": false, "Parameters": [],
    })
}

struct Setup {
    services: Services,
    krake: Shared,
    krake_addr: std::net::SocketAddr,
    root: std::path::PathBuf,
}

impl Setup {
    async fn start(name: &str, feed: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!("sofarr-krake-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("downloads")).expect("downloads");
        let services = Services::start().await;
        let items: Vec<String> = feed
            .iter()
            .map(|(title, guid)| services.item(title, guid, 4_000_000_000))
            .collect();
        services.feed("idx", &items);
        let krake: Shared = Arc::default();
        let router = Router::new()
            .route("/jsonrpc", post(jsonrpc))
            .with_state(Arc::clone(&krake));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let krake_addr = listener.local_addr().expect("addr");
        tokio::spawn(async move { axum::serve(listener, router).await });
        Self {
            services,
            krake,
            krake_addr,
            root,
        }
    }

    fn config(&self, password: &str) -> String {
        format!(
            "[automation]\nmode = \"live\"\n{}[nzbget]\nurl = \"http://{}\"\nusername = \"u\"\npassword = \"{password}\"\n\
             [[download.path_map]]\nremote = \"/downloads\"\nlocal = '{}'\n",
            self.services.indexer_toml("idx", &[]),
            self.krake_addr,
            self.root.join("downloads").display()
        )
    }

    fn krake(&self) -> std::sync::MutexGuard<'_, Krake> {
        self.krake.lock().expect("krake lock")
    }
}

impl Drop for Setup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[tokio::test]
async fn grab_sends_the_nzb_with_category_and_records_the_id() {
    // AC-01
    let s = Setup::start("grab", &[(RELEASE, "g1")]).await;
    let app = app(Library::Movies, &s.config("p")).await;
    let report = app.rss_sync().await.expect("rss");
    assert_eq!(report.approved, 1, "{report:?}");
    assert_eq!(
        s.krake().appended,
        [(format!("{RELEASE}.nzb"), "movies".to_owned())]
    );
    let grabs = app.db.grabs(None, 10).await.expect("grabs");
    assert_eq!(grabs[0].status, GrabStatus::Queued);
    assert_eq!(
        grabs[0].download_id.as_deref(),
        Some(&*KRAKE_ID.to_string())
    );

    // Episodes go to the series category.
    let t = Setup::start("grab-tv", &[(EPISODE, "e1")]).await;
    let series = app_series(&t).await;
    series.rss_sync().await.expect("rss");
    assert_eq!(
        t.krake().appended,
        [(format!("{EPISODE}.nzb"), "tv".to_owned())]
    );
}

async fn app_series(s: &Setup) -> crate::app::App {
    app(Library::Series, &s.config("p")).await
}

#[tokio::test]
async fn queue_entries_keep_the_grab_downloading_with_krakes_sizes() {
    // AC-02
    let s = Setup::start("queue", &[(RELEASE, "g1")]).await;
    let app = app(Library::Movies, &s.config("p")).await;
    app.rss_sync().await.expect("rss");
    s.krake().queue = vec![queue_entry(
        KRAKE_ID,
        RELEASE,
        "movies",
        "DOWNLOADING",
        4000,
        1000,
    )];
    assert_eq!(app.check_downloads().await.expect("check"), 0);
    let grab = &app.db.grabs(None, 10).await.expect("grabs")[0];
    assert_eq!(grab.status, GrabStatus::Downloading);

    let items = sofarr_download::DownloadClient::items(app.nzbget.as_ref().expect("client"))
        .await
        .expect("items");
    let item = items
        .iter()
        .find(|i| i.id == KRAKE_ID.to_string())
        .expect("listed");
    assert_eq!(
        (item.size_bytes, item.remaining_bytes),
        (4000 * 1024 * 1024, 1000 * 1024 * 1024)
    );
    assert_eq!(item.category, "movies");

    // Queued and paused stay sent, not downloading.
    let t = Setup::start("queue-paused", &[(RELEASE, "g1")]).await;
    let paused = app_movies(&t).await;
    paused.rss_sync().await.expect("rss");
    t.krake().queue = vec![queue_entry(
        KRAKE_ID, RELEASE, "movies", "PAUSED", 4000, 4000,
    )];
    paused.check_downloads().await.expect("check");
    assert_eq!(
        paused.db.grabs(None, 10).await.expect("grabs")[0].status,
        GrabStatus::Queued
    );
}

async fn app_movies(s: &Setup) -> crate::app::App {
    app(Library::Movies, &s.config("p")).await
}

#[tokio::test]
async fn success_imports_from_krakes_folder_and_removes_without_deleting() {
    // AC-03, AC-05
    let s = Setup::start("import", &[(RELEASE, "g1")]).await;
    let folder = s.root.join("downloads").join(RELEASE);
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("erster.mkv"), vec![1u8; 4096]).expect("video");
    let other = s.root.join("downloads").join("Fremder.Download");
    std::fs::create_dir_all(&other).expect("other");
    std::fs::write(other.join("fremd.mkv"), vec![2u8; 64]).expect("other video");

    let app = app_movies(&s).await;
    // The library folder lies in the test's directory (the documents' `/data` is not
    // writable outside a container).
    let library = s.root.join("movies").join("Erster Film (2001)");
    let id = crate::media_tests::place(&app, 101, &library, "unused.mkv").await;
    app.db.remove_file(id).await.expect("no file yet");
    app.rss_sync().await.expect("rss");
    s.krake().history = vec![
        history_entry(
            OTHER_ID,
            "Fremder.Download",
            "music",
            true,
            "/downloads/Fremder.Download",
        ),
        history_entry(
            KRAKE_ID,
            RELEASE,
            "movies",
            true,
            &format!("/downloads/{RELEASE}"),
        ),
    ];
    assert_eq!(app.check_downloads().await.expect("check"), 1);
    let grab = &app.db.grabs(None, 10).await.expect("grabs")[0];
    assert_eq!(grab.status, GrabStatus::Imported);
    let movie = app.db.movie_by_tmdb(101).await.expect("db").expect("movie");
    assert!(movie.file.is_some(), "file recorded");
    let placed = std::fs::read_dir(&library).expect("library folder").count();
    assert_eq!(placed, 1, "the video lies in the library folder");

    let rpc = s.krake().rpc.clone();
    let edits: Vec<&String> = rpc.iter().filter(|c| c.starts_with("editqueue")).collect();
    assert!(!edits.is_empty(), "the finished entry is removed: {rpc:?}");
    assert!(
        edits.iter().all(|c| c.contains(&KRAKE_ID.to_string())),
        "only sofarr's own entry is touched: {edits:?}"
    );
    // NZBGet's commands that keep the files; Krake keeps them as well (amigo-krake
    // `fix/nzbget-compat-for-sofarr`).
    assert!(
        edits
            .iter()
            .all(|c| c.starts_with("editqueue [\"GroupFinalDelete\"")
                || c.starts_with("editqueue [\"HistoryDelete\"")),
        "only commands that keep the files: {edits:?}"
    );
    assert!(other.join("fremd.mkv").exists(), "other downloads stay");
}

#[tokio::test]
async fn failure_marks_the_grab_failed_without_blocklist() {
    // AC-04
    let s = Setup::start("failure", &[(RELEASE, "g1")]).await;
    let app = app_movies(&s).await;
    app.rss_sync().await.expect("rss");
    s.krake().history = vec![history_entry(
        KRAKE_ID,
        RELEASE,
        "movies",
        false,
        "/downloads/x",
    )];
    app.check_downloads().await.expect("check");
    let grab = &app.db.grabs(None, 10).await.expect("grabs")[0];
    assert_eq!(grab.status, GrabStatus::Failed);
    assert!(
        app.db.blocks().await.expect("blocklist").is_empty(),
        "a local failure is not blocklisted"
    );
}

#[tokio::test]
async fn refused_credentials_and_rpc_errors_fail_the_grab() {
    // AC-06
    let s = Setup::start("auth", &[(RELEASE, "g1")]).await;
    let wrong = app(Library::Movies, &s.config("falsch")).await;
    wrong.rss_sync().await.expect("rss");
    let grab = &wrong.db.grabs(None, 10).await.expect("grabs")[0];
    assert_eq!(grab.status, GrabStatus::Failed);
    assert!(grab.download_id.is_none());
    let message = grab.message.clone().unwrap_or_default();
    assert!(
        message.contains("nzbget: Anfrage fehlgeschlagen"),
        "{message}"
    );

    let t = Setup::start("rpc-error", &[(RELEASE, "g1")]).await;
    t.krake().error = Some("Invalid NZB: no files".to_owned());
    let refused = app_movies(&t).await;
    refused.rss_sync().await.expect("rss");
    let grab = &refused.db.grabs(None, 10).await.expect("grabs")[0];
    assert_eq!(grab.status, GrabStatus::Failed);
    assert!(grab.download_id.is_none());
    let message = grab.message.clone().unwrap_or_default();
    assert!(message.contains("nzbget: Fehler Invalid NZB"), "{message}");
}
