//! The client against a fake OpenSubtitles.com (axum router on a local port).

use std::{
    collections::HashMap,
    hash::BuildHasher,
    net::SocketAddr,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{Path, RawQuery, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::{Value, json};

use crate::{DownloadError, Feature, OpenSubtitles, ProviderConfig, ProviderError, SearchQuery};

/// A random hex string made at run time: test credentials are never literals.
fn secret(salt: u64) -> String {
    format!(
        "{:016x}",
        std::collections::hash_map::RandomState::new().hash_one(salt)
    )
}

/// The test account's API key and password, the same for the whole test run.
fn credentials() -> &'static (String, String) {
    static CREDENTIALS: OnceLock<(String, String)> = OnceLock::new();
    CREDENTIALS.get_or_init(|| (secret(1), secret(2)))
}

#[derive(Default)]
struct Fake {
    login_status: Option<u16>,
    logins: u32,
    /// Token the fake accepts; a download with another one gets 401.
    valid_token: String,
    search_status: Option<u16>,
    results: Vec<Value>,
    /// `(status, body)` of `/download` per file id; default: a link to `/file/<id>`.
    download_answers: HashMap<i64, (u16, Value)>,
    files: HashMap<i64, Vec<u8>>,
    queries: Vec<String>,
    headers: Vec<(String, String, String)>,
}

type Shared = Arc<Mutex<Fake>>;

fn note(fake: &mut Fake, headers: &HeaderMap) {
    let get = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned()
    };
    fake.headers
        .push((get("api-key"), get("user-agent"), get("authorization")));
}

async fn login(
    State(fake): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let mut fake = fake.lock().expect("lock");
    note(&mut fake, &headers);
    fake.logins += 1;
    if let Some(status) = fake.login_status {
        return StatusCode::from_u16(status)
            .expect("status")
            .into_response();
    }
    assert_eq!(body["username"], "anna-k");
    assert_eq!(body["password"], credentials().1.as_str());
    let token = format!("token-{}", fake.logins);
    fake.valid_token = token.clone();
    Json(json!({ "token": token, "status": 200, "user": { "allowed_downloads": 20 } }))
        .into_response()
}

async fn subtitles(
    State(fake): State<Shared>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    let mut fake = fake.lock().expect("lock");
    note(&mut fake, &headers);
    fake.queries.push(query.unwrap_or_default());
    if let Some(status) = fake.search_status {
        return StatusCode::from_u16(status)
            .expect("status")
            .into_response();
    }
    Json(json!({ "total_count": fake.results.len(), "page": 1, "data": fake.results }))
        .into_response()
}

async fn download(
    State(fake): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let mut fake = fake.lock().expect("lock");
    note(&mut fake, &headers);
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if auth != format!("Bearer {}", fake.valid_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    assert_eq!(body["sub_format"], "srt");
    let id = body["file_id"].as_i64().expect("file id");
    let (status, answer) = fake.download_answers.get(&id).cloned().unwrap_or((
        200,
        json!({ "link": format!("FILE/{id}"), "remaining": 5,
                "reset_time_utc": "2026-11-13T00:00:00.000Z" }),
    ));
    (StatusCode::from_u16(status).expect("status"), Json(answer)).into_response()
}

async fn file(State(fake): State<Shared>, Path(id): Path<i64>) -> Response {
    match fake.lock().expect("lock").files.get(&id) {
        Some(bytes) => bytes.clone().into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn start() -> (Shared, SocketAddr) {
    let fake: Shared = Arc::default();
    let router = Router::new()
        .route("/api/v1/login", post(login))
        .route("/api/v1/subtitles", get(subtitles))
        .route("/api/v1/download", post(download))
        .route("/file/{id}", get(file))
        .with_state(Arc::clone(&fake));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move { axum::serve(listener, router).await });
    (fake, addr)
}

fn client(addr: SocketAddr) -> OpenSubtitles {
    let http = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client");
    OpenSubtitles::new(
        http,
        ProviderConfig {
            url: format!("http://{addr}/api/v1/"),
            api_key: credentials().0.clone(),
            username: "anna-k".to_owned(),
            password: credentials().1.clone(),
            user_agent: "amigo-sofarr v0.1.0".to_owned(),
            pause: Duration::ZERO,
        },
    )
}

/// Rewrites the fake's relative `FILE/<id>` links into full URLs.
fn absolute_links(fake: &Shared, addr: SocketAddr, ids: &[i64]) {
    let mut fake = fake.lock().expect("lock");
    for id in ids {
        fake.download_answers.insert(
            *id,
            (
                200,
                json!({ "link": format!("http://{addr}/file/{id}"), "remaining": 4,
                        "reset_time_utc": "2026-11-13T00:00:00.000Z" }),
            ),
        );
    }
}

fn movie_query() -> SearchQuery {
    SearchQuery {
        feature: Feature::Movie {
            imdb_id: Some("tt0000042".to_owned()),
            tmdb_id: Some(42),
        },
        moviehash: Some("0102030405091449".to_owned()),
        language: "de".to_owned(),
        forced: false,
    }
}

#[tokio::test]
async fn searches_with_key_and_agent_and_reads_candidates() {
    let (fake, addr) = start().await;
    fake.lock().expect("lock").results = vec![
        json!({ "id": "9", "type": "subtitle", "attributes": {
            "subtitle_id": "9", "language": "de", "release": "Film.2019.1080p.BluRay.x264-GRP",
            "hearing_impaired": false, "foreign_parts_only": false, "moviehash_match": true,
            "download_count": 12, "files": [{ "file_id": 900, "file_name": "a.srt" }] } }),
        json!({ "id": "10", "type": "subtitle", "attributes": { "language": "de", "files": [] } }),
    ];
    let candidates = client(addr).search(&movie_query()).await.expect("search");
    assert_eq!(candidates.len(), 1, "a subtitle without a file is skipped");
    let found = &candidates[0];
    assert_eq!((found.subtitle_id.as_str(), found.file_id), ("9", 900));
    assert!(found.moviehash_match && !found.hearing_impaired);
    assert_eq!(found.download_count, 12);
    let fake = fake.lock().expect("lock");
    assert_eq!(
        fake.queries,
        [
            "foreign_parts_only=exclude&hearing_impaired=include&imdb_id=42&languages=de\
          &moviehash=0102030405091449&type=movie"
        ]
    );
    assert_eq!(
        fake.headers[0],
        (
            credentials().0.clone(),
            "amigo-sofarr v0.1.0".to_owned(),
            String::new()
        ),
        "searches need no login"
    );
    assert_eq!(fake.logins, 0);
}

#[tokio::test]
async fn search_failures_end_the_run() {
    let (fake, addr) = start().await;
    let client = client(addr);
    for (status, expected) in [
        (503, ProviderError::Server(503)),
        (403, ProviderError::KeyRefused(403)),
        (429, ProviderError::RateLimited),
    ] {
        fake.lock().expect("lock").search_status = Some(status);
        assert_eq!(client.search(&movie_query()).await, Err(expected));
    }
    let gone = self::client("127.0.0.1:9".parse().expect("addr"));
    assert!(matches!(
        gone.search(&movie_query()).await,
        Err(ProviderError::Unreachable(_))
    ));
}

#[tokio::test]
async fn downloads_after_login_and_reports_the_quota() {
    let (fake, addr) = start().await;
    absolute_links(&fake, addr, &[900]);
    fake.lock()
        .expect("lock")
        .files
        .insert(900, b"1\n00:00:01,000 --> 00:00:02,000\nHallo\n".to_vec());
    let client = client(addr);
    let got = client.download(900).await.expect("download");
    assert!(got.content.starts_with(b"1\n00:00:01"));
    assert_eq!(got.remaining, Some(4));
    assert_eq!(
        got.reset,
        Some("2026-11-13T00:00:00Z".parse().expect("reset"))
    );
    // The token is reused; an expired one leads to one new login.
    client.download(900).await.expect("again");
    assert_eq!(fake.lock().expect("lock").logins, 1);
    fake.lock().expect("lock").valid_token = "rotated".to_owned();
    // The fake hands out "token-2" at the next login, which it then accepts.
    client.download(900).await.expect("after relogin");
    let fake = fake.lock().expect("lock");
    assert_eq!(fake.logins, 2);
    assert!(
        fake.headers
            .iter()
            .all(|(key, agent, _)| *key == credentials().0 && agent == "amigo-sofarr v0.1.0")
    );
}

#[tokio::test]
async fn quota_refused_login_and_broken_candidates_are_told_apart() {
    let (fake, addr) = start().await;
    let client = client(addr);
    {
        let mut fake = fake.lock().expect("lock");
        fake.download_answers.insert(
            1,
            (
                406,
                json!({ "remaining": -1, "message": "quota",
                        "reset_time_utc": "2026-11-13T05:00:00.000Z" }),
            ),
        );
        fake.download_answers
            .insert(2, (404, json!({ "message": "not found" })));
        fake.download_answers.insert(3, (502, json!({})));
    }
    assert_eq!(
        client.download(1).await,
        Err(DownloadError::Quota {
            reset: Some("2026-11-13T05:00:00Z".parse().expect("reset"))
        })
    );
    assert!(matches!(
        client.download(2).await,
        Err(DownloadError::Candidate(_))
    ));
    assert_eq!(
        client.download(3).await,
        Err(DownloadError::Provider(ProviderError::Server(502)))
    );
    // A link whose file is gone fails only this candidate.
    absolute_links(&fake, addr, &[4]);
    assert!(matches!(
        client.download(4).await,
        Err(DownloadError::Candidate(_))
    ));

    let (fake, addr) = start().await;
    fake.lock().expect("lock").login_status = Some(401);
    let refused = self::client(addr).download(1).await;
    assert_eq!(
        refused,
        Err(DownloadError::Provider(ProviderError::LoginRefused(401)))
    );
    let message = refused.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(
        !message.contains(credentials().1.as_str()) && !message.contains("anna-k"),
        "{message}"
    );
}
