//! End-to-end test of `collect` against a minimal in-process HTTP mock that plays both
//! a Newznab indexer and Radarr. All data is invented.

use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use serde_json::Value;
use sofarr_parity::{
    collect::{self, HISTORY_FILE, PUBLIC_FILE, Sources},
    config::ParityConfig,
    http::HttpClient,
    releases,
};

const FEED: &str = r#"<?xml version="1.0"?>
<rss><channel>
  <item><title>Erfundener.Film.2021.1080p.WEB-DL-TEST</title></item>
  <item><title>Noch.Einer.1999.720p.BluRay.x264-TEST</title></item>
  <item><title>Erfundener.Film.2021.1080p.WEB-DL-TEST</title></item>
</channel></rss>"#;

/// Starts the mock server and returns its base URL plus the log of requested paths.
fn start_mock() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("addr"));
    let log = Arc::new(Mutex::new(Vec::new()));
    let log_thread = Arc::clone(&log);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_err() {
                continue;
            }
            let mut headers = HashMap::new();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some((k, v)) = line.trim_end().split_once(": ") {
                    headers.insert(k.to_ascii_lowercase(), v.to_owned());
                }
            }
            let target = request_line.split(' ').nth(1).unwrap_or("/").to_owned();
            log_thread.lock().expect("lock").push(target.clone());
            let (path, query) = target.split_once('?').unwrap_or((&target, ""));
            let authorized = headers.get("x-api-key").map(String::as_str) == Some("radarr-key");

            let (status, body) = match path {
                "/newznab/api"
                    if query.contains("apikey=indexer-key") && query.contains("offset=0") =>
                {
                    ("200 OK", FEED.to_owned())
                }
                "/newznab/api" if query.contains("apikey=indexer-key") => {
                    ("200 OK", "<rss><channel/></rss>".to_owned())
                }
                "/newznab/api" => (
                    "200 OK",
                    r#"<error code="100" description="Falscher Schlüssel"/>"#.to_owned(),
                ),
                _ if !authorized => ("401 Unauthorized", "{}".to_owned()),
                "/api/v3/system/status" => ("200 OK", r#"{"version":"9.8.7"}"#.to_owned()),
                "/api/v3/history" if query.contains("page=1&") => (
                    "200 OK",
                    r#"{"page":1,"totalRecords":3,"records":[
                        {"sourceTitle":"Privater.Titel.2010.1080p.BluRay-HIST"},
                        {"sourceTitle":"Privater.Titel.2010.1080p.BluRay-HIST"},
                        {"sourceTitle":"Zweiter.Privater.2012.720p.HDTV-HIST"}]}"#
                        .to_owned(),
                ),
                "/api/v3/history" => ("200 OK", r#"{"records":[]}"#.to_owned()),
                "/api/v3/parse" => (
                    "200 OK",
                    serde_json::json!({
                        "title": "echo",
                        "parsedMovieInfo": {"movieTitles": ["Erfunden"], "year": 2021},
                        "languages": [],
                        "movie": {"path": "/data/media/movies/Geheim"}
                    })
                    .to_string(),
                ),
                "/api/v3/movie" => (
                    "200 OK",
                    serde_json::json!([
                        {"id": 1, "title": "Erfunden", "monitored": true, "runtime": 90,
                         "qualityProfileId": 4, "hasFile": false,
                         "path": "/data/media/movies/Geheim"},
                        {"id": 2, "title": "Unbeobachtet", "monitored": false},
                        {"id": 3, "title": "Zweiter", "monitored": true, "runtime": 100}
                    ])
                    .to_string(),
                ),
                "/api/v3/release" => (
                    "200 OK",
                    serde_json::json!([
                        {"title": format!("Erfunden.2021.1080p.WEB-DL-{query}"),
                         "indexer": "Geheimer Indexer", "guid": "geheime-guid",
                         "downloadUrl": "https://geheim.example/get?apikey=x",
                         "approved": false, "rejections": ["Geheimer Indexer sagt nein"]}
                    ])
                    .to_string(),
                ),
                _ => ("404 Not Found", "{}".to_owned()),
            };
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (base, log)
}

fn fixtures_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn config(base: &str, indexer_key: &str) -> ParityConfig {
    ParityConfig::parse(&format!(
        "rate_limit_per_sec = 1000\n\
         [radarr]\nurl = \"{base}/\"\napi_key = \"radarr-key\"\n\
         [[indexer]]\nurl = \"{base}/newznab/api\"\napi_key = \"{indexer_key}\"\n"
    ))
    .expect("config")
}

fn lines(path: &PathBuf) -> Vec<Value> {
    fs::read_to_string(path)
        .expect("fixture file")
        .lines()
        .map(|l| serde_json::from_str(l).expect("json line"))
        .collect()
}

#[test]
fn collects_dedupes_and_redacts() {
    let (base, log) = start_mock();
    let dir = fixtures_dir("collect-e2e");
    let http = HttpClient::new(1000.0, false);
    let config = config(&base, "indexer-key");

    let summary = collect::run(&config, &http, &dir, Sources::All).expect("first run");
    assert_eq!(summary.rss_new, 2);
    assert_eq!(summary.history_new, 2);

    let public = lines(&dir.join(PUBLIC_FILE));
    assert_eq!(public.len(), 2);
    assert_eq!(public[0]["title"], "Erfundener.Film.2021.1080p.WEB-DL-TEST");
    assert_eq!(public[0]["origin"], "rss");
    assert_eq!(public[0]["oracle"]["app"], "radarr");
    assert_eq!(public[0]["oracle"]["version"], "9.8.7");
    assert!(
        public[0]["collected_at"]
            .as_str()
            .expect("ts")
            .ends_with('Z')
    );
    assert_eq!(public[0]["response"]["parsedMovieInfo"]["year"], 2021);
    let public_raw = fs::read_to_string(dir.join(PUBLIC_FILE)).expect("read");
    assert!(!public_raw.contains("/data/media"), "library data leaked");
    assert!(!public_raw.contains("127.0.0.1"), "indexer URL leaked");

    let history = lines(&dir.join(HISTORY_FILE));
    assert_eq!(history.len(), 2);
    assert_eq!(history[0]["origin"], "history");
    // Private fixtures keep the raw answer.
    assert_eq!(
        history[0]["response"]["movie"]["path"],
        "/data/media/movies/Geheim"
    );

    // The parse request carried the title, URL-encoded.
    assert!(
        log.lock()
            .expect("lock")
            .iter()
            .any(|p| p.starts_with("/api/v3/parse?title=Noch.Einer.1999"))
    );

    // Second run: nothing new, files unchanged.
    let summary = collect::run(&config, &http, &dir, Sources::All).expect("second run");
    assert_eq!(summary, collect::Summary::default());
    assert_eq!(lines(&dir.join(PUBLIC_FILE)).len(), 2);
    assert_eq!(lines(&dir.join(HISTORY_FILE)).len(), 2);
}

#[test]
fn paging_continues_at_offset_and_stops_on_empty_page() {
    let (base, log) = start_mock();
    let dir = fixtures_dir("collect-paging");
    let http = HttpClient::new(1000.0, false);
    let mut config = config(&base, "indexer-key");
    config.indexers[0].pages = 5;

    let summary = collect::run(&config, &http, &dir, Sources::Rss).expect("run");
    assert_eq!(summary.rss_new, 2);
    let pages: Vec<String> = log
        .lock()
        .expect("lock")
        .iter()
        .filter(|p| p.starts_with("/newznab/api"))
        .cloned()
        .collect();
    // First page at offset 0 (3 items), second at offset 3 is empty: stop there.
    assert_eq!(pages.len(), 2);
    assert!(pages[1].contains("offset=3"));
}

#[test]
fn indexer_error_aborts_without_partial_lines() {
    let (base, _log) = start_mock();
    let dir = fixtures_dir("collect-indexer-error");
    let http = HttpClient::new(1000.0, false);

    let err = collect::run(&config(&base, "falsch"), &http, &dir, Sources::Rss)
        .expect_err("indexer rejects key");
    let message = format!("{err:#}");
    assert!(message.contains("Falscher Schlüssel"), "{message}");
    assert!(!message.contains("falsch&"), "query leaked: {message}");
    let raw = fs::read_to_string(dir.join(PUBLIC_FILE)).unwrap_or_default();
    assert!(raw.is_empty());
}

#[test]
fn wrong_radarr_key_is_reported() {
    let (base, _log) = start_mock();
    let dir = fixtures_dir("collect-radarr-error");
    let http = HttpClient::new(1000.0, false);
    let mut config = config(&base, "indexer-key");
    config.radarr.api_key = "falsch".to_owned();

    let err = collect::run(&config, &http, &dir, Sources::All).expect_err("401");
    assert!(format!("{err:#}").contains("Orakel"));
}

#[test]
fn collects_releases_for_monitored_films_and_redacts() {
    let (base, log) = start_mock();
    let dir = fixtures_dir("collect-releases");
    let http = HttpClient::new(1000.0, false);
    let config = config(&base, "indexer-key");

    let summary =
        collect::run(&config, &http, &dir, Sources::Releases { movies: 1 }).expect("first run");
    assert_eq!(summary.releases_new, 1);
    assert_eq!(summary.rss_new, 0);
    // The second run queries the film not covered yet.
    collect::run(&config, &http, &dir, Sources::Releases { movies: 1 }).expect("second run");
    let searched: Vec<String> = log
        .lock()
        .expect("lock")
        .iter()
        .filter(|p| p.starts_with("/api/v3/release"))
        .cloned()
        .collect();
    assert_eq!(
        searched,
        ["/api/v3/release?movieId=1", "/api/v3/release?movieId=3"]
    );

    let public = lines(&dir.join(releases::PUBLIC_FILE));
    assert_eq!(public.len(), 2);
    assert_eq!(public[0]["origin"], "release");
    assert_eq!(public[0]["movie"]["runtime"], 90);
    assert_eq!(
        public[0]["response"]["rejections"][0],
        "<indexer> sagt nein"
    );
    let text = fs::read_to_string(dir.join(releases::PUBLIC_FILE)).expect("public");
    for secret in [
        "Geheimer Indexer",
        "geheime-guid",
        "geheim.example",
        "/data/media",
    ] {
        assert!(!text.contains(secret), "{secret} leaked");
    }

    let private = lines(&dir.join(releases::PRIVATE_FILE));
    assert_eq!(private.len(), 2);
    assert_eq!(private[0]["movie"]["id"], 1);
    assert_eq!(private[0]["response"]["guid"], "geheime-guid");
}
