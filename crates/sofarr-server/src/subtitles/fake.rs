//! Test support: a fake OpenSubtitles.com (axum router) and an app with one movie whose
//! video lies in a temporary folder.

use std::{
    collections::HashMap,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::{Path as UrlPath, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use jiff::Timestamp;
use rand::RngCore;
use serde_json::{Value, json};
use sofarr_db::{Db, FileRef, MovieFile, NewMovie, StoredMediaInfo};
use sofarr_mediainfo::{Audio, MediaInfo, ReadStatus, Subtitle};

use crate::{app::App, config::Config};

/// A random hex string made at run time: test credentials are never literals.
pub fn secret() -> String {
    let mut bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub const SRT: &str = "1\n00:00:01,000 --> 00:00:02,000\nHallo\n";

/// State of the fake provider; tests change it between runs.
#[derive(Debug, Default)]
pub struct Fake {
    pub addr: Option<SocketAddr>,
    pub password: String,
    /// Search results per `<language>` or `<language>-forced`.
    pub results: HashMap<String, Vec<Value>>,
    /// HTTP status of searches per the same key.
    pub search_status: HashMap<String, u16>,
    pub login_status: Option<u16>,
    /// Content per file id (default: a valid SRT).
    pub files: HashMap<i64, Vec<u8>>,
    /// HTTP status of `/download` per file id.
    pub download_status: HashMap<i64, u16>,
    /// Downloads left; at 0 `/download` answers 406.
    pub remaining: i64,
    pub reset: String,
    /// File ids asked for at `/download`, in order.
    pub downloads: Vec<i64>,
    /// Query strings of every search.
    pub queries: Vec<String>,
}

pub type Shared = Arc<Mutex<Fake>>;

/// One search result.
pub fn result(file_id: i64, language: &str, release: &str) -> Value {
    json!({ "id": file_id.to_string(), "type": "subtitle", "attributes": {
        "subtitle_id": file_id.to_string(), "language": language, "release": release,
        "hearing_impaired": false, "foreign_parts_only": false, "moviehash_match": false,
        "download_count": 0, "files": [{ "file_id": file_id, "file_name": "x.srt" }] } })
}

async fn login(State(fake): State<Shared>, Json(body): Json<Value>) -> Response {
    let fake = fake.lock().expect("lock");
    if let Some(status) = fake.login_status {
        return StatusCode::from_u16(status)
            .expect("status")
            .into_response();
    }
    if body["password"] != json!(fake.password) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(json!({ "token": "t", "status": 200 })).into_response()
}

async fn subtitles(
    State(fake): State<Shared>,
    Query(query): Query<HashMap<String, String>>,
    raw: axum::extract::RawQuery,
) -> Response {
    let mut fake = fake.lock().expect("lock");
    fake.queries.push(raw.0.unwrap_or_default());
    let language = query.get("languages").cloned().unwrap_or_default();
    let key = if query.get("foreign_parts_only").map(String::as_str) == Some("only") {
        format!("{language}-forced")
    } else {
        language
    };
    if let Some(status) = fake.search_status.get(&key) {
        return StatusCode::from_u16(*status)
            .expect("status")
            .into_response();
    }
    let data = fake.results.get(&key).cloned().unwrap_or_default();
    Json(json!({ "total_count": data.len(), "data": data })).into_response()
}

async fn download(State(fake): State<Shared>, Json(body): Json<Value>) -> Response {
    let mut fake = fake.lock().expect("lock");
    let id = body["file_id"].as_i64().expect("file id");
    fake.downloads.push(id);
    if let Some(status) = fake.download_status.get(&id) {
        return (
            StatusCode::from_u16(*status).expect("status"),
            Json(json!({})),
        )
            .into_response();
    }
    if fake.remaining <= 0 {
        return (
            StatusCode::NOT_ACCEPTABLE,
            Json(json!({ "remaining": -1, "reset_time_utc": fake.reset })),
        )
            .into_response();
    }
    fake.remaining -= 1;
    let addr = fake.addr.expect("addr");
    Json(
        json!({ "link": format!("http://{addr}/file/{id}"), "remaining": fake.remaining,
                 "reset_time_utc": fake.reset }),
    )
    .into_response()
}

async fn file(State(fake): State<Shared>, UrlPath(id): UrlPath<i64>) -> Response {
    let fake = fake.lock().expect("lock");
    fake.files
        .get(&id)
        .cloned()
        .unwrap_or_else(|| SRT.as_bytes().to_vec())
        .into_response()
}

/// Starts the fake; it accepts `password` at the login.
pub async fn start(password: &str) -> Shared {
    let fake: Shared = Arc::new(Mutex::new(Fake {
        password: password.to_owned(),
        remaining: 100,
        reset: "2026-11-13T00:00:00.000Z".to_owned(),
        ..Fake::default()
    }));
    let router = Router::new()
        .route("/api/v1/login", post(login))
        .route("/api/v1/subtitles", get(subtitles))
        .route("/api/v1/download", post(download))
        .route("/file/{id}", get(file))
        .with_state(Arc::clone(&fake));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    fake.lock().expect("lock").addr = Some(listener.local_addr().expect("addr"));
    tokio::spawn(async move { axum::serve(listener, router).await });
    fake
}

/// `[opensubtitles]` for the fake, with fresh credentials.
pub fn account_toml(fake: &Shared) -> String {
    let fake = fake.lock().expect("lock");
    format!(
        "[opensubtitles]\napi_key = \"{}\"\nusername = \"anna-k\"\npassword = \"{}\"\n\
         url = \"http://{}/api/v1\"\npause_ms = 0\n",
        secret(),
        fake.password,
        fake.addr.expect("addr")
    )
}

/// An app on `db` with a client that ignores system proxies.
pub fn app_on(toml: &str, db: Db) -> Arc<App> {
    let config = Config::parse(toml).expect("test config");
    let mut app = App::with_db(config, db).expect("app");
    let http = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client");
    app.opensubtitles = super::client(&app.config, &http);
    app.http = http;
    Arc::new(app)
}

pub fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sofarr-subs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

/// Media info with these audio languages and embedded subtitles `(language, forced)`.
pub fn media(audio: &[&str], subtitles: &[(&str, bool)]) -> MediaInfo {
    MediaInfo {
        status: ReadStatus::Complete,
        container: None,
        video: None,
        audio: audio
            .iter()
            .map(|l| Audio {
                codec: "AC3".to_owned(),
                channels: Some(6),
                object_audio: false,
                language: (*l).to_owned(),
            })
            .collect(),
        subtitles: subtitles
            .iter()
            .map(|(l, forced)| Subtitle {
                language: (*l).to_owned(),
                format: "SRT".to_owned(),
                forced: *forced,
                default: false,
                hearing_impaired: false,
            })
            .collect(),
    }
}

/// The video of a test movie: 200,000 zero bytes (movie hash `0000000000030d40`).
pub const VIDEO_HASH: &str = "0000000000030d40";

/// Adds a movie with a video in `root/<title>` imported at `added`; returns its id and
/// the video's path.
pub async fn add_movie(
    db: &Db,
    root: &Path,
    tmdb_id: i64,
    title: &str,
    added: Timestamp,
    media_info: Option<&MediaInfo>,
) -> (i64, PathBuf) {
    let folder = root.join(title);
    std::fs::create_dir_all(&folder).expect("folder");
    let file_name = format!("{title} Bluray-1080p.mkv");
    let video = folder.join(&file_name);
    std::fs::write(&video, vec![0u8; 200_000]).expect("video");
    let id = db
        .upsert_movie(
            &NewMovie {
                tmdb_id,
                imdb_id: Some(format!("tt{tmdb_id:07}")),
                title: title.to_owned(),
                original_title: None,
                title_de: None,
                year: Some(2019),
                runtime_minutes: 100,
                original_language: None,
                monitored: true,
                quality_profile_id: 1,
                path: folder.to_string_lossy().into_owned(),
                radarr_id: None,
                alternative_titles: vec![],
                availability: sofarr_db::Availability::default(),
                tags: vec![],
            },
            added,
        )
        .await
        .expect("movie");
    db.set_file(
        id,
        &MovieFile {
            relative_path: file_name,
            size: 200_000,
            quality: "Bluray-1080p".to_owned(),
            revision_version: 1,
            revision_real: 0,
            release_group: Some("GRP".to_owned()),
            scene_name: Some(format!(
                "{}.2019.German.1080p.BluRay.x264-GRP",
                title.replace(' ', ".")
            )),
            custom_format_score: 0,
        },
        added,
    )
    .await
    .expect("file");
    if let Some(info) = media_info {
        db.put_media_info(
            FileRef::Movie(id),
            &StoredMediaInfo {
                info: serde_json::to_string(info).expect("json"),
                size: 200_000,
                modified: String::new(),
            },
            added,
        )
        .await
        .expect("media info");
    }
    (id, video)
}

/// The names of the files in a folder, sorted.
pub fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}
