//! Trailers (spec trailers) against a fake TMDB and an in-process fake downloader; the
//! real yt-dlp command line and its limits are tested in `ytdlp`.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::{Path as UrlPath, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use jiff::Timestamp;
use serde_json::{Value, json};
use sofarr_db::{Db, MediaRef, MovieFile, NewEpisode, NewMovie, NewSeries, TrailerItem};

use super::*;
use crate::{app::App, config::Config};

fn at(raw: &str) -> Timestamp {
    raw.parse().expect("timestamp")
}

fn record(state: TrailerState, attempts: u32, next: Option<&str>) -> TrailerRecord {
    TrailerRecord {
        state,
        attempts,
        next_attempt: next.map(at),
        ..TrailerRecord::default()
    }
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sofarr-trailers-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

// --- with a fake TMDB and a fake downloader ------------------------------------------

/// What the fake downloader does per video id: write a trailer of this many bytes, or
/// fail with a reason after leaving a partial file behind.
#[derive(Debug, Clone)]
enum Behavior {
    Write(usize),
    Fail(&'static str),
}

#[derive(Default)]
struct FakeDownloader {
    behavior: Mutex<HashMap<String, Behavior>>,
    calls: Mutex<Vec<String>>,
    /// Work folders handed to the downloader.
    folders: Mutex<Vec<PathBuf>>,
    tools: Mutex<Option<String>>,
}

impl Downloader for FakeDownloader {
    fn download(&self, id: &VideoId, dir: &Path) -> Result<PathBuf, String> {
        self.calls
            .lock()
            .expect("calls")
            .push(id.as_str().to_owned());
        self.folders
            .lock()
            .expect("folders")
            .push(dir.to_path_buf());
        let behavior = self
            .behavior
            .lock()
            .expect("behavior")
            .get(id.as_str())
            .cloned();
        match behavior.unwrap_or(Behavior::Fail("Video unavailable")) {
            Behavior::Write(bytes) => {
                let file = dir.join("trailer.mkv");
                fs::write(&file, vec![7u8; bytes]).map_err(|e| e.to_string())?;
                Ok(file)
            }
            Behavior::Fail(reason) => {
                let _ = fs::write(dir.join("trailer.f137.mp4.part"), b"partial");
                Err(format!("yt-dlp: {reason}"))
            }
        }
    }

    fn check(&self) -> Result<(), String> {
        self.tools
            .lock()
            .expect("tools")
            .clone()
            .map_or(Ok(()), Err)
    }
}

/// The fake TMDB: video lists by path (`movie/949`, `tv/1399`), requests seen.
#[derive(Default)]
struct FakeTmdb {
    videos: HashMap<String, Value>,
    requests: Vec<String>,
    down: bool,
}

type SharedTmdb = Arc<Mutex<FakeTmdb>>;

async fn videos(
    State(fake): State<SharedTmdb>,
    UrlPath((kind, id)): UrlPath<(String, i64)>,
) -> Response {
    let mut fake = fake.lock().expect("tmdb");
    let key = format!("{kind}/{id}");
    fake.requests.push(key.clone());
    if fake.down {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    match fake.videos.get(&key) {
        Some(v) => Json(v.clone()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn youtube(key: &str, kind: &str, language: Option<&str>) -> Value {
    json!({"key": key, "site": "YouTube", "type": kind, "official": true,
           "iso_639_1": language, "published_at": "2020-01-01T00:00:00.000Z"})
}

struct Setup {
    app: Arc<App>,
    tmdb: SharedTmdb,
    downloader: Arc<FakeDownloader>,
    root: PathBuf,
}

impl Setup {
    async fn new(name: &str, extra: &str) -> Self {
        Self::build(name, "live", extra).await
    }

    async fn build(name: &str, mode: &str, extra: &str) -> Self {
        let root = temp(name);
        let tmdb: SharedTmdb = Arc::default();
        let router = Router::new()
            .route("/3/{kind}/{id}/videos", get(videos))
            .with_state(Arc::clone(&tmdb));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move { axum::serve(listener, router).await });
        let toml = format!(
            "[automation]\nmode = \"{mode}\"\n[tmdb]\napi_key = \"{}\"\n\
             [database]\npath = '{}'\n{extra}",
            crate::test_secret::secret(),
            root.join("config/sofarr.db").display()
        );
        let config = Config::parse(&toml).expect("config");
        let mut app = App::with_db(config, Db::in_memory().await.expect("db")).expect("app");
        app.http = reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("client");
        app.tmdb_api = format!("http://{addr}/3");
        let downloader = Arc::new(FakeDownloader::default());
        app.trailers = Runtime::new(Arc::clone(&downloader) as Arc<dyn Downloader>);
        Self {
            app: Arc::new(app),
            tmdb,
            downloader,
            root,
        }
    }

    fn library(&self) -> PathBuf {
        self.root.join("data")
    }

    /// A movie added at `added`, with a video file unless `with_file` is false.
    async fn movie(&self, tmdb_id: i64, folder: &str, added: &str, with_file: bool) -> i64 {
        let path = self.library().join("movies").join(folder);
        fs::create_dir_all(&path).expect("folder");
        let movie = NewMovie {
            tmdb_id,
            imdb_id: None,
            title: folder.to_owned(),
            original_title: None,
            title_de: None,
            year: None,
            runtime_minutes: 100,
            original_language: None,
            monitored: true,
            quality_profile_id: 1,
            path: path.to_string_lossy().into_owned(),
            radarr_id: None,
            alternative_titles: Vec::new(),
            availability: sofarr_db::Availability::default(),
            tags: Vec::new(),
        };
        let id = self
            .app
            .db
            .upsert_movie(&movie, at(added))
            .await
            .expect("movie");
        if with_file {
            fs::write(path.join(format!("{folder}.mkv")), b"video").expect("video");
            self.app
                .db
                .set_file(id, &file(&format!("{folder}.mkv")), at(added))
                .await
                .expect("file");
        }
        id
    }

    fn videos(&self, path: &str, videos: Vec<Value>) {
        self.tmdb
            .lock()
            .expect("tmdb")
            .videos
            .insert(path.to_owned(), json!({"results": videos}));
    }

    fn behave(&self, key: &str, behavior: Behavior) {
        self.downloader
            .behavior
            .lock()
            .expect("behavior")
            .insert(key.to_owned(), behavior);
    }

    fn calls(&self) -> Vec<String> {
        self.downloader.calls.lock().expect("calls").clone()
    }

    async fn record(&self, media: MediaRef) -> Option<TrailerRecord> {
        self.app
            .db
            .trailer_item(media)
            .await
            .expect("read")
            .and_then(|i| i.trailer)
    }
}

impl Drop for Setup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn file(relative: &str) -> MovieFile {
    MovieFile {
        relative_path: relative.to_owned(),
        size: 5,
        quality: "Bluray-1080p".to_owned(),
        revision_version: 1,
        revision_real: 0,
        release_group: None,
        scene_name: None,
        custom_format_score: 0,
    }
}

fn now() -> Timestamp {
    at("2026-11-12T03:00:00Z")
}

#[tokio::test]
async fn the_daily_run_downloads_newest_first_within_the_budget() {
    let s = Setup::new("budget", "[trailers]\nper_day = 2\n").await;
    let heat = s
        .movie(949, "Heat (1995)", "2026-01-01T00:00:00Z", true)
        .await;
    let newer = s
        .movie(950, "Neu (2026)", "2026-05-01T00:00:00Z", true)
        .await;
    let newest = s
        .movie(951, "Neuer (2026)", "2026-09-01T00:00:00Z", true)
        .await;
    let empty = s
        .movie(952, "Leer (2026)", "2026-10-01T00:00:00Z", true)
        .await;
    s.videos(
        "movie/949",
        vec![youtube("heat_de_tr1", "Trailer", Some("de"))],
    );
    s.videos(
        "movie/950",
        vec![youtube("neu_en_tr01", "Trailer", Some("en"))],
    );
    s.videos("movie/951", vec![youtube("neuer_xx_01", "Teaser", None)]);
    // The newest has no candidate: it does not count against the budget.
    s.videos(
        "movie/952",
        vec![youtube("leer_clip01", "Clip", Some("de"))],
    );
    for key in ["heat_de_tr1", "neu_en_tr01", "neuer_xx_01"] {
        s.behave(key, Behavior::Write(100));
    }

    let report = run(&s.app, now()).await.expect("run");
    assert_eq!(report.downloaded, 2);
    assert_eq!(report.no_candidate, 1);
    assert_eq!(s.calls(), ["neuer_xx_01", "neu_en_tr01"]);
    let newest_folder = s.library().join("movies/Neuer (2026)");
    assert_eq!(
        fs::read(newest_folder.join("trailers/Neuer (2026) - Trailer (Original).mkv"))
            .expect("trailer")
            .len(),
        100
    );
    assert!(
        s.library()
            .join("movies/Neu (2026)/trailers/Neu (2026) - Trailer (Englisch).mkv")
            .is_file()
    );
    assert!(!s.library().join("movies/Heat (1995)/trailers").exists());
    let stored = s.record(MediaRef::Movie(newest)).await.expect("record");
    assert_eq!(stored.state, TrailerState::Downloaded);
    assert_eq!(stored.video_key.as_deref(), Some("neuer_xx_01"));
    assert_eq!(stored.language, None);
    assert_eq!(
        stored.file.as_deref(),
        Some("trailers/Neuer (2026) - Trailer (Original).mkv")
    );
    let none = s.record(MediaRef::Movie(empty)).await.expect("record");
    assert_eq!((none.state, none.attempts), (TrailerState::NoCandidate, 1));
    assert_eq!(s.record(MediaRef::Movie(heat)).await, None);
    assert_eq!(
        s.record(MediaRef::Movie(newer)).await.map(|r| r.state),
        Some(TrailerState::Downloaded)
    );
    // Nothing stays in the work folder.
    let work = work_folder(&s.app);
    assert!(fs::read_dir(&work).map_or(true, |mut d| d.next().is_none()));

    // The next run goes on with the rest; the movies that have a trailer are left alone.
    let report = run(&s.app, at("2026-11-13T03:00:00Z")).await.expect("run");
    assert_eq!(report.downloaded, 1);
    assert_eq!(s.calls(), ["neuer_xx_01", "neu_en_tr01", "heat_de_tr1"]);
    assert!(
        s.library()
            .join("movies/Heat (1995)/trailers/Heat (1995) - Trailer (Deutsch).mkv")
            .is_file()
    );
}

#[tokio::test]
async fn an_import_puts_an_item_first_in_line() {
    let s = Setup::new("import", "[trailers]\nper_day = 1\n").await;
    let old = s
        .movie(949, "Heat (1995)", "2025-01-01T00:00:00Z", true)
        .await;
    s.movie(950, "Neu (2026)", "2026-05-01T00:00:00Z", true)
        .await;
    s.videos(
        "movie/949",
        vec![youtube("heat_de_tr1", "Trailer", Some("de"))],
    );
    s.videos(
        "movie/950",
        vec![youtube("neu_en_tr01", "Trailer", Some("en"))],
    );
    s.behave("heat_de_tr1", Behavior::Write(10));
    s.behave("neu_en_tr01", Behavior::Write(10));
    s.app
        .db
        .put_trailer_first_in_line(MediaRef::Movie(old), now())
        .await
        .expect("first in line");
    run(&s.app, now()).await.expect("run");
    assert_eq!(s.calls(), ["heat_de_tr1"]);
}

#[tokio::test]
async fn a_failed_candidate_falls_back_and_leaves_nothing_behind() {
    let s = Setup::new("fallback", "").await;
    let heat = s
        .movie(949, "Heat (1995)", "2026-01-01T00:00:00Z", true)
        .await;
    s.videos(
        "movie/949",
        vec![
            youtube("en_trailer1", "Trailer", Some("en")),
            youtube("de_teaser01", "Teaser", Some("de")),
            // Not a YouTube id: never handed to yt-dlp (AC-13).
            youtube("--exec=rm -rf", "Trailer", Some("de")),
        ],
    );
    s.behave("de_teaser01", Behavior::Fail("Private video"));
    s.behave("en_trailer1", Behavior::Write(10));
    let report = run(&s.app, now()).await.expect("run");
    assert_eq!((report.downloaded, report.failed), (1, 0));
    assert_eq!(s.calls(), ["de_teaser01", "en_trailer1"]);
    for folder in s.downloader.folders.lock().expect("folders").iter() {
        assert!(!folder.exists(), "{} left behind", folder.display());
    }
    let stored = s.record(MediaRef::Movie(heat)).await.expect("record");
    assert_eq!(stored.language.as_deref(), Some("en"));

    // All candidates fail: one attempt, one budget unit, the reasons stored.
    let other = s
        .movie(950, "Neu (2026)", "2026-05-01T00:00:00Z", true)
        .await;
    s.videos(
        "movie/950",
        vec![
            youtube("neu_de_tr01", "Trailer", Some("de")),
            youtube("--exec=rm -rf", "Trailer", Some("de")),
        ],
    );
    s.behave(
        "neu_de_tr01",
        Behavior::Fail("Video unavailable in your country"),
    );
    let report = run(&s.app, now()).await.expect("run");
    assert_eq!(report.failed, 1);
    let stored = s.record(MediaRef::Movie(other)).await.expect("record");
    assert_eq!((stored.state, stored.attempts), (TrailerState::Failed, 1));
    let message = stored.message.expect("message");
    assert!(
        message.contains("neu_de_tr01: yt-dlp: Video unavailable in your country"),
        "{message}"
    );
    assert!(message.contains("keine gültige YouTube-ID"), "{message}");
    assert_eq!(stored.next_attempt, Some(at("2026-11-19T03:00:00Z")));
    assert!(!s.library().join("movies/Neu (2026)/trailers").exists());
}

#[tokio::test]
async fn items_that_are_not_eligible_are_skipped() {
    let s = Setup::new("eligible", "").await;
    // An own trailer next to the movie: sofarr does nothing (AC-06).
    s.movie(949, "Heat (1995)", "2026-01-01T00:00:00Z", true)
        .await;
    let own = s.library().join("movies/Heat (1995)/Film-trailer.mkv");
    fs::write(&own, b"mine").expect("own trailer");
    // No video file: not eligible.
    s.movie(950, "Gesucht (2026)", "2026-05-01T00:00:00Z", false)
        .await;
    // A series without TMDB id is not eligible, with one it is.
    let show_path = s.library().join("tv/Kleine Insel");
    fs::create_dir_all(&show_path).expect("series folder");
    let show = s
        .app
        .db
        .upsert_series(
            &NewSeries {
                tvdb_id: 81189,
                imdb_id: None,
                title: "Kleine Insel".to_owned(),
                title_de: None,
                year: Some(2020),
                runtime_minutes: 45,
                original_language: None,
                series_type: "standard".to_owned(),
                monitored: true,
                season_folder: true,
                quality_profile_id: 1,
                path: show_path.to_string_lossy().into_owned(),
                sonarr_id: None,
                alternative_titles: Vec::new(),
            },
            at("2026-06-01T00:00:00Z"),
        )
        .await
        .expect("series");
    let episode = s
        .app
        .db
        .upsert_episode(
            show,
            &NewEpisode {
                tvdb_id: None,
                season: 1,
                number: 1,
                absolute: None,
                title: None,
                title_de: None,
                air_date: None,
                monitored: true,
            },
        )
        .await
        .expect("episode");
    s.app
        .db
        .set_episode_file(show, &[episode], &file("S01E01.mkv"), now())
        .await
        .expect("episode file");
    for path in ["movie/949", "movie/950", "tv/1399"] {
        s.videos(path, vec![youtube("insel_de_01", "Trailer", Some("de"))]);
    }
    s.behave("insel_de_01", Behavior::Write(10));
    let report = run(&s.app, now()).await.expect("run");
    assert_eq!(report, RunReport::default());
    assert!(s.tmdb.lock().expect("tmdb").requests.is_empty());

    s.app
        .db
        .set_series_tmdb_id(show, 1399)
        .await
        .expect("tmdb id");
    let report = run(&s.app, now()).await.expect("run");
    assert_eq!(report.downloaded, 1);
    assert_eq!(s.tmdb.lock().expect("tmdb").requests, ["tv/1399"]);
    assert!(
        show_path
            .join("trailers/Kleine Insel - Trailer (Deutsch).mkv")
            .is_file()
    );
    assert_eq!(fs::read(&own).expect("own"), b"mine");
}

#[tokio::test]
async fn too_little_free_space_downloads_nothing_and_counts_nothing() {
    let s = Setup::new("space", "[trailers]\nper_day = 1\n").await;
    let heat = s
        .movie(949, "Heat (1995)", "2026-01-01T00:00:00Z", true)
        .await;
    s.videos(
        "movie/949",
        vec![youtube("heat_de_tr1", "Trailer", Some("de"))],
    );
    s.behave("heat_de_tr1", Behavior::Write(10));
    s.app
        .db
        .put_setting(
            "config_mediamanagement",
            &json!({"minimumFreeSpaceWhenImporting": u64::MAX / 4}),
        )
        .await
        .expect("setting");
    let report = run(&s.app, now()).await.expect("run");
    assert_eq!((report.low_space, report.downloaded), (1, 0));
    assert!(s.calls().is_empty());
    let stored = s.record(MediaRef::Movie(heat)).await.expect("record");
    assert_eq!((stored.state, stored.attempts), (TrailerState::LowSpace, 0));
    assert!(
        stored
            .message
            .expect("message")
            .starts_with("zu wenig Speicherplatz")
    );
    // Series have their own setting; with space the next run downloads.
    s.app
        .db
        .put_setting(
            "config_mediamanagement",
            &json!({"minimumFreeSpaceWhenImporting": 1}),
        )
        .await
        .expect("setting");
    let report = run(&s.app, now()).await.expect("run");
    assert_eq!(report.downloaded, 1);
}

#[tokio::test]
async fn shadow_mode_and_switched_off_download_nothing() {
    for (mode, extra, reason) in [
        ("shadow", "", "Schattenmodus"),
        ("live", "[trailers]\nenabled = false\n", "ausgeschaltet"),
    ] {
        let s = Setup::build(&format!("off-{mode}"), mode, extra).await;
        let heat = s
            .movie(949, "Heat (1995)", "2026-01-01T00:00:00Z", true)
            .await;
        s.videos(
            "movie/949",
            vec![youtube("heat_de_tr1", "Trailer", Some("de"))],
        );
        s.behave("heat_de_tr1", Behavior::Write(10));
        assert_eq!(crate::status::Job::Trailers.interval(&s.app), None);
        let error = run(&s.app, now()).await.expect_err("off");
        assert!(format!("{error:#}").contains(reason), "{error:#}");
        let support = api::support_answer(&s.app);
        assert!(!support.active);
        assert!(support.message.expect("why").contains(reason));
        assert!(s.calls().is_empty());
        assert!(s.tmdb.lock().expect("tmdb").requests.is_empty());
        assert_eq!(s.record(MediaRef::Movie(heat)).await, None);
        assert!(!s.library().join("movies/Heat (1995)/trailers").exists());
    }
}

#[tokio::test]
async fn missing_tools_download_nothing() {
    let s = Setup::new("tools", "").await;
    s.movie(949, "Heat (1995)", "2026-01-01T00:00:00Z", true)
        .await;
    s.videos(
        "movie/949",
        vec![youtube("heat_de_tr1", "Trailer", Some("de"))],
    );
    s.behave("heat_de_tr1", Behavior::Write(10));
    let missing = "yt-dlp (yt-dlp) nicht nutzbar: Programm nicht gefunden";
    *s.downloader.tools.lock().expect("tools") = Some(missing.to_owned());
    assert_eq!(
        api::support_answer(&s.app).message.as_deref(),
        Some("yt-dlp und ffmpeg werden noch geprüft")
    );
    let error = run(&s.app, now()).await.expect_err("tools missing");
    assert_eq!(format!("{error:#}"), missing);
    assert!(s.calls().is_empty());
    assert_eq!(
        api::support_answer(&s.app).message.as_deref(),
        Some(missing)
    );
    // Once the tools are there, the run downloads.
    *s.downloader.tools.lock().expect("tools") = None;
    assert_eq!(run(&s.app, now()).await.expect("run").downloaded, 1);
    assert!(api::support_answer(&s.app).active);
}

#[tokio::test]
async fn the_disk_scan_never_records_a_trailer() {
    use crate::rescan::{MovieAction, ScanSettings, inspect_movie};
    let s = Setup::new("scan", "").await;
    let id = s
        .movie(949, "Heat (1995)", "2026-01-01T00:00:00Z", false)
        .await;
    let folder = s.library().join("movies/Heat (1995)");
    fs::create_dir_all(folder.join("trailers")).expect("dir");
    fs::write(
        folder.join("trailers/Heat (1995) - Trailer (Deutsch).mkv"),
        b"t",
    )
    .expect("t");
    fs::write(folder.join("Heat (1995)-trailer.mkv"), b"t").expect("t");
    let movie = s.app.db.movie(id).await.expect("read").expect("movie");
    let settings = ScanSettings {
        delete_empty_folders: true,
        ..ScanSettings::default()
    };
    // Neither recorded as the movie's file nor removed as an empty folder.
    assert_eq!(inspect_movie(&movie, settings), []);
    fs::write(folder.join("Heat (1995).mkv"), b"video").expect("video");
    assert_eq!(
        inspect_movie(&movie, settings),
        [MovieAction::Found(folder.join("Heat (1995).mkv"))]
    );
}

mod api_tests;
mod rules;
