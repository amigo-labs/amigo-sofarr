//! Taking over a Radarr or Sonarr library through their API (spec library-import).

use std::{
    collections::BTreeMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::{Path, RawQuery, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use jiff::Timestamp;
use serde_json::{Value, json};
use sofarr_db::Db;

use crate::{
    config::RadarrConfig,
    library::{self, tests::documents},
    radarr::Radarr,
    series::{self, tests::sonarr_documents},
    test_secret,
};

/// A fake Radarr/Sonarr API: answers `api/v3/<path>` from `answers`, records each
/// request as `<path>?<query>` and refuses a wrong API key with 401.
#[derive(Default)]
struct FakeArr {
    key: String,
    answers: BTreeMap<String, Value>,
    requests: Vec<String>,
}

type Shared = Arc<Mutex<FakeArr>>;

async fn answer(
    State(fake): State<Shared>,
    Path(path): Path<String>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Response {
    let mut fake = fake.lock().expect("lock");
    let query = query.unwrap_or_default();
    fake.requests.push(format!("{path}?{query}"));
    if headers.get("X-Api-Key").and_then(|v| v.to_str().ok()) != Some(fake.key.as_str()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let key = if query.is_empty() {
        path
    } else {
        format!("{path}?{query}")
    };
    Json(fake.answers.get(&key).cloned().unwrap_or_else(|| json!([]))).into_response()
}

async fn start(answers: BTreeMap<String, Value>, key: &str) -> (Shared, SocketAddr) {
    let fake: Shared = Arc::new(Mutex::new(FakeArr {
        key: key.to_owned(),
        answers,
        requests: Vec::new(),
    }));
    let router = Router::new()
        .route("/api/v3/{*path}", get(answer))
        .with_state(Arc::clone(&fake));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move { axum::serve(listener, router).await });
    (fake, addr)
}

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client")
}

fn arr_config(addr: SocketAddr, key: &str) -> RadarrConfig {
    RadarrConfig {
        url: format!("http://{addr}/"),
        api_key: key.to_owned(),
    }
}

/// The API answers of a Radarr from the test library (setting keys as API paths).
fn radarr_answers() -> BTreeMap<String, Value> {
    documents()
        .into_iter()
        .map(|(key, value)| (key.replacen("config_", "config/", 1), value))
        .collect()
}

#[tokio::test]
async fn fetches_settings_and_movies_from_radarr() {
    let key = test_secret::secret();
    let (fake, addr) = start(radarr_answers(), &key).await;
    let radarr = Radarr::new(http(), &arr_config(addr, &key));
    let docs = library::fetch_from_radarr(&radarr).await.expect("fetch");
    let requested = fake.lock().expect("lock").requests.clone();
    assert_eq!(
        requested,
        [
            "qualityprofile?",
            "qualitydefinition?",
            "customformat?",
            "config/mediamanagement?",
            "config/indexer?",
            "config/naming?",
            "delayprofile?",
            "rootfolder?",
            "config/downloadclient?",
            "importlist?",
            "exclusions?",
            "tag?",
            "movie?",
        ]
    );
    let db = Db::in_memory().await.expect("db");
    let summary = library::import_documents(&db, &docs, Timestamp::now())
        .await
        .expect("import");
    assert_eq!(
        summary,
        library::ImportSummary {
            settings: 12,
            movies: 4,
            files: 2,
            skipped: 1
        }
    );
    assert_eq!(db.movies().await.expect("movies").len(), 4);
    assert!(db.setting("importlist").await.expect("db").is_some());
    let rules = library::load_rules(&db).await.expect("rules");
    assert_eq!(rules.config.profiles.len(), 1);
}

#[tokio::test]
async fn a_rejected_key_or_an_unreachable_app_stops_the_import() {
    let key = test_secret::secret();
    let (_fake, addr) = start(radarr_answers(), &key).await;
    let wrong = test_secret::wrong();
    let err = library::fetch_from_radarr(&Radarr::new(http(), &arr_config(addr, wrong)))
        .await
        .expect_err("key refused");
    assert!(
        format!("{err:#}").contains("Radarr: API-Key abgelehnt (HTTP 401)"),
        "{err:#}"
    );
    let err = series::fetch_from_sonarr(&Radarr::sonarr(http(), &arr_config(addr, wrong)))
        .await
        .expect_err("key refused");
    assert!(
        format!("{err:#}").contains("Sonarr: API-Key abgelehnt (HTTP 401)"),
        "{err:#}"
    );
    // Nothing listens on a port that was just freed.
    let closed = {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        listener.local_addr().expect("addr")
    };
    let err = library::fetch_from_radarr(&Radarr::new(http(), &arr_config(closed, &key)))
        .await
        .expect_err("unreachable");
    assert!(
        format!("{err:#}").contains("Radarr: qualityprofile nicht abrufbar"),
        "{err:#}"
    );
}

#[tokio::test]
async fn reimport_updates_movies_by_tmdb_id_and_keeps_the_rest() {
    let db = Db::in_memory().await.expect("db");
    let t0: Timestamp = "2026-11-01T00:00:00Z".parse().expect("ts");
    library::import_documents(&db, &documents(), t0)
        .await
        .expect("first import");
    let ids: BTreeMap<i64, i64> = db
        .movies()
        .await
        .expect("movies")
        .iter()
        .map(|m| (m.tmdb_id, m.id))
        .collect();

    // Radarr renamed the first movie and stopped monitoring it, the second lost its
    // file, the third is no longer listed, and the custom formats were not exported.
    let mut docs = documents();
    docs.remove("customformat");
    let movies = docs.get_mut("movie").expect("movies");
    movies[0]["title"] = json!("Erster Film (neu)");
    movies[0]["monitored"] = json!(false);
    movies[1]
        .as_object_mut()
        .expect("object")
        .remove("movieFile");
    movies.as_array_mut().expect("array").remove(2);
    let summary = library::import_documents(&db, &docs, t0)
        .await
        .expect("second import");
    assert_eq!((summary.movies, summary.files), (3, 0));

    let after = db.movies().await.expect("movies");
    assert_eq!(after.len(), 4, "no movie is added twice or dropped");
    for movie in &after {
        assert_eq!(ids[&movie.tmdb_id], movie.id, "same record");
    }
    let by_tmdb = |tmdb: i64| after.iter().find(|m| m.tmdb_id == tmdb).expect("movie");
    assert_eq!(by_tmdb(101).title, "Erster Film (neu)");
    assert!(!by_tmdb(101).monitored);
    assert_eq!(by_tmdb(102).file, None);
    assert!(
        by_tmdb(103).file.is_some(),
        "an unlisted movie stays as it was"
    );
    // Settings missing from the new import stay as stored.
    assert!(db.setting("customformat").await.expect("db").is_some());
}

#[tokio::test]
async fn movies_keep_radarrs_fields() {
    let db = Db::in_memory().await.expect("db");
    let mut docs = documents();
    docs.insert("tag".to_owned(), json!([{"id": 4, "label": "Kids"}]));
    let movies = docs.get_mut("movie").expect("movies");
    movies[0] = json!({
        "id": 11, "tmdbId": 101, "imdbId": "tt0000101", "title": "Erster Film",
        "originalTitle": "First Film", "year": 2001, "runtime": 95, "monitored": true,
        "qualityProfileId": 7, "path": "/media/filme/Erster Film (2001)",
        "originalLanguage": {"id": 1, "name": "English"},
        "minimumAvailability": "inCinemas", "status": "inCinemas",
        "inCinemas": "2001-05-03T00:00:00Z", "digitalRelease": "2001-09-01T00:00:00Z",
        "physicalRelease": "2001-11-20T00:00:00Z",
        "tags": [4], "alternateTitles": [{"title": "Der erste Film"}],
        "movieFile": {"relativePath": "Erster Film (2001).mkv", "size": 4096,
            "releaseGroup": "GRP", "sceneName": "Erster.Film.2001.1080p.BluRay.x264-GRP",
            "customFormatScore": 25,
            "quality": {"quality": {"name": "Bluray-1080p"}, "revision": {"version": 2, "real": 1}}}
    });
    // Without a minimum availability Radarr's default `released` applies.
    movies[1]
        .as_object_mut()
        .expect("object")
        .remove("minimumAvailability");
    library::import_documents(&db, &docs, Timestamp::now())
        .await
        .expect("import");
    let movie = db.movie_by_tmdb(101).await.expect("db").expect("movie");
    assert_eq!(movie.imdb_id.as_deref(), Some("tt0000101"));
    assert_eq!(movie.original_title.as_deref(), Some("First Film"));
    assert_eq!((movie.year, movie.runtime_minutes), (Some(2001), 95));
    assert_eq!(movie.original_language.as_deref(), Some("English"));
    assert_eq!(movie.radarr_id, Some(11));
    assert_eq!(movie.quality_profile_id, 7);
    assert_eq!(movie.path, "/media/filme/Erster Film (2001)");
    assert_eq!(movie.availability.status.as_deref(), Some("inCinemas"));
    assert_eq!(
        movie.availability.physical_release.as_deref(),
        Some("2001-11-20")
    );
    assert_eq!(movie.alternative_titles, ["Der erste Film"]);
    assert_eq!(movie.availability.minimum, "inCinemas");
    assert_eq!(movie.availability.in_cinemas.as_deref(), Some("2001-05-03"));
    assert_eq!(
        movie.availability.digital_release.as_deref(),
        Some("2001-09-01")
    );
    assert_eq!(movie.tags, [4]);
    let file = movie.file.expect("file");
    assert_eq!(file.relative_path, "Erster Film (2001).mkv");
    assert_eq!(file.size, 4096);
    assert_eq!(file.quality, "Bluray-1080p");
    assert_eq!((file.revision_version, file.revision_real), (2, 1));
    assert_eq!(file.release_group.as_deref(), Some("GRP"));
    assert_eq!(
        file.scene_name.as_deref(),
        Some("Erster.Film.2001.1080p.BluRay.x264-GRP")
    );
    // Radarr's own score wins over one computed from the scene name.
    assert_eq!(file.custom_format_score, 25);
    let second = db.movie_by_tmdb(102).await.expect("db").expect("movie");
    assert_eq!(second.availability.minimum, "released");
    // No score from Radarr: computed from the scene name (German +10).
    assert_eq!(second.file.expect("file").custom_format_score, 10);
}

#[tokio::test]
async fn entries_without_tmdb_id_or_title_are_skipped() {
    let db = Db::in_memory().await.expect("db");
    let mut docs = documents();
    let movies = docs
        .get_mut("movie")
        .expect("movies")
        .as_array_mut()
        .expect("array");
    // A TMDB id but no title, and a TMDB id with an empty title.
    movies.push(
        json!({"id": 6, "tmdbId": 106, "monitored": true, "qualityProfileId": 1,
        "path": "/x/ohne Titel"}),
    );
    movies.push(
        json!({"id": 7, "tmdbId": 107, "title": "", "monitored": true,
        "qualityProfileId": 1, "path": "/x/leer"}),
    );
    let summary = library::import_documents(&db, &docs, Timestamp::now())
        .await
        .expect("import");
    // The fixture's entry with TMDB id 0 plus the two without a title.
    assert_eq!((summary.movies, summary.skipped), (4, 3));
    assert_eq!(db.movie_by_tmdb(106).await.expect("db"), None);
    assert_eq!(db.movie_by_tmdb(107).await.expect("db"), None);
    assert_eq!(db.movies().await.expect("movies").len(), 4);
}

/// Sonarr's API answers for the test series: settings, the series list, and episodes
/// and files per series id.
fn sonarr_answers() -> BTreeMap<String, Value> {
    let (docs, episodes) = sonarr_documents();
    let mut answers: BTreeMap<String, Value> = docs
        .into_iter()
        .map(|(key, value)| (key.replacen("config_", "config/", 1), value))
        .collect();
    let (mut eps, files) = episodes[&3].clone();
    // One file holds E01 and E02.
    eps[1]["episodeFileId"] = json!(300);
    answers.insert("episode?seriesId=3".to_owned(), Value::Array(eps));
    answers.insert("episodefile?seriesId=3".to_owned(), Value::Array(files));
    answers
}

#[tokio::test]
async fn fetches_series_episodes_and_files_from_sonarr() {
    let key = test_secret::secret();
    let mut answers = sonarr_answers();
    // A series without TVDB id cannot be taken over.
    answers
        .get_mut("series")
        .expect("series")
        .as_array_mut()
        .expect("array")
        .push(json!({"id": 4, "title": "Ohne TVDB", "path": "/tv/x"}));
    let (fake, addr) = start(answers, &key).await;
    let sonarr = Radarr::sonarr(http(), &arr_config(addr, &key));
    let (docs, episodes) = series::fetch_from_sonarr(&sonarr).await.expect("fetch");
    let requested = fake.lock().expect("lock").requests.clone();
    for path in [
        "qualityprofile?",
        "config/naming?",
        "rootfolder?",
        "tag?",
        "series?",
        "episode?seriesId=3",
        "episodefile?seriesId=3",
    ] {
        assert!(requested.iter().any(|r| r == path), "{path}: {requested:?}");
    }

    let db = Db::in_memory().await.expect("db");
    let now = Timestamp::now();
    let summary = series::import_documents(&db, &docs, &episodes, now)
        .await
        .expect("import");
    assert_eq!(
        summary,
        series::ImportSummary {
            settings: 9,
            series: 1,
            episodes: 3,
            files: 1
        }
    );
    let shows = db.all_series().await.expect("series");
    assert_eq!(shows.len(), 1);
    assert_eq!(shows[0].data.tvdb_id, 77);
    assert_eq!(shows[0].data.alternative_titles, ["Small Island"]);
    let eps = db.episodes(None).await.expect("episodes");
    let path = |n: u16| {
        eps.iter()
            .find(|e| e.data.number == n)
            .and_then(|e| e.file.as_ref())
            .map(|f| f.relative_path.clone())
    };
    assert_eq!(
        path(1).as_deref(),
        Some("Staffel 01/Kleine Insel - S01E01.mkv")
    );
    assert_eq!(path(1), path(2), "one file for both episodes");
    assert_eq!(path(3), None);

    // Again: the same series and episodes, no second record.
    series::import_documents(&db, &docs, &episodes, now)
        .await
        .expect("again");
    assert_eq!(db.all_series().await.expect("series").len(), 1);
    assert_eq!(db.episodes(None).await.expect("episodes").len(), 3);
}

#[tokio::test]
async fn movie_and_series_settings_stay_apart() {
    let db = Db::in_memory().await.expect("db");
    let now = Timestamp::now();
    library::import_documents(&db, &documents(), now)
        .await
        .expect("movies");
    let (mut docs, episodes) = sonarr_documents();
    docs.insert(
        "qualityprofile".to_owned(),
        json!([{"id": 1, "name": "Serien", "upgradeAllowed": false, "cutoff": 3,
            "items": [{"quality": {"id": 3, "name": "WEBDL-1080p"}, "allowed": true}]}]),
    );
    series::import_documents(&db, &docs, &episodes, now)
        .await
        .expect("series");
    let movie_rules = library::load_rules(&db).await.expect("movie rules");
    let series_rules = series::load_rules(&db).await.expect("series rules");
    assert_eq!(movie_rules.profile(1).expect("profile").name, "HD");
    assert_eq!(series_rules.profile(1).expect("profile").name, "Serien");
    assert_eq!(movie_rules.config.custom_formats.len(), 1);
    assert!(series_rules.config.custom_formats.is_empty());
    assert!(
        db.setting("sonarr_config_naming")
            .await
            .expect("db")
            .is_some()
    );
    assert!(db.setting("config_naming").await.expect("db").is_none());
    // The series import leaves the movies alone.
    assert_eq!(db.movies().await.expect("movies").len(), 4);
}
