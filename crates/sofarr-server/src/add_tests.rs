//! Adding titles from the discover pages against a fake TMDB, TVDB and indexer (spec
//! discover AC-07..AC-12).

use std::path::{Path, PathBuf};

use axum::http::StatusCode;
use jiff::Timestamp;
use serde_json::{Value, json};

use crate::{
    metadata, series,
    testkit::{Http, Library, Services},
    tmdb_fake::{Providers, movie_detail, show_detail},
};

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("sofarr-add-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("root");
    root
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Waits until the fake indexer saw a request containing `needle`.
async fn searched(services: &Services, needle: &str) -> bool {
    for _ in 0..250 {
        if !services.requests_with(needle).is_empty() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    false
}

fn movie_body(tmdb_id: i64, root: &Path, search: bool) -> Value {
    json!({"tmdb_id": tmdb_id, "quality_profile_id": 1, "root_folder": text(root),
           "monitored": true, "minimum_availability": "released", "tags": [2], "search": search})
}

/// An app with movies, one root folder and a tag, wired to the fakes.
async fn movie_app(providers: &Providers, services: &Services, root: &Path) -> Http {
    let app = providers
        .app(Library::Movies, &services.indexer_toml("idx", &[]))
        .await;
    app.db
        .put_setting("rootfolder", &json!([{"path": text(root)}]))
        .await
        .expect("roots");
    app.db
        .put_setting("tag", &json!([{"id": 2, "label": "kids"}]))
        .await
        .expect("tags");
    Http::start(app).await
}

/// AC-07: a movie goes into the library like an import list adds it; an existing
/// folder is used and scanned; the search runs when chosen.
#[tokio::test]
async fn an_added_movie_is_put_like_the_import_lists_and_its_folder_scanned() {
    let providers = Providers::start().await;
    let services = Services::start().await;
    let root = temp_root("movie");
    let folder = root.join("Garten Eins (2001)");
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(
        folder.join("Garten.Eins.2001.German.1080p.BluRay.x264-GRP.mkv"),
        vec![1u8; 2048],
    )
    .expect("video");
    {
        let mut fake = providers.lock();
        fake.movies.insert(
            501,
            movie_detail(501, "Garten Eins", "2001-03-04", Some("tt0000501")),
        );
        fake.movies.insert(
            502,
            movie_detail(502, "Garten Zwei", "2002-03-04", Some("tt0000502")),
        );
    }
    let http = movie_app(&providers, &services, &root).await;

    let (status, added) = http
        .post("/api/discover/add/movie", movie_body(501, &root, false))
        .await;
    assert_eq!(status, StatusCode::OK, "{added}");
    assert_eq!(added["kind"], "movie");
    assert_eq!(added["title"], "Garten Eins");
    assert_eq!(added["path"], text(&folder));
    assert_eq!(added["files_found"], 1);
    assert_eq!(added["searching"], false);
    let movie = http
        .app
        .db
        .movie_by_tmdb(501)
        .await
        .expect("db")
        .expect("added");
    assert_eq!(movie.id, added["id"].as_i64().expect("id"));
    assert!(movie.monitored);
    assert_eq!(movie.quality_profile_id, 1);
    assert_eq!(movie.availability.minimum, "released");
    assert_eq!(movie.tags, [2]);
    assert_eq!(
        movie.file.as_ref().map(|f| f.quality.as_str()),
        Some("Bluray-1080p")
    );
    assert_eq!(
        http.app
            .db
            .poster_path(sofarr_db::MediaRef::Movie(movie.id))
            .await
            .expect("poster")
            .as_deref(),
        Some("/m501.jpg")
    );

    let (status, second) = http
        .post("/api/discover/add/movie", movie_body(502, &root, true))
        .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["files_found"], 0);
    assert_eq!(second["searching"], true);
    assert!(searched(&services, "imdbid=0000502").await);
    let _ = std::fs::remove_dir_all(&root);
}

/// AC-10: the add dialog offers the last choices per media kind; a choice that no
/// longer exists falls back; first-time values without stored defaults.
#[tokio::test]
async fn add_defaults_are_remembered_per_kind_and_fall_back() {
    let providers = Providers::start().await;
    let services = Services::start().await;
    let root = temp_root("defaults");
    providers.lock().movies.insert(
        502,
        movie_detail(502, "Garten Zwei", "2002-03-04", Some("tt0000502")),
    );
    let http = movie_app(&providers, &services, &root).await;

    let (status, first) = http.get("/api/discover/add/movie/options").await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["profiles"], json!([{"id": 1, "name": "HD"}]));
    assert_eq!(first["root_folders"], json!([text(&root)]));
    assert_eq!(
        first["minimum_availability"],
        json!(["announced", "inCinemas", "released"])
    );
    assert_eq!(
        first["defaults"],
        json!({"quality_profile_id": 1, "root_folder": text(&root), "monitored": true,
               "minimum_availability": "released", "tags": [], "search": true,
               "monitor_new_seasons": true})
    );

    let mut body = movie_body(502, &root, false);
    body["monitored"] = json!(false);
    body["minimum_availability"] = json!("inCinemas");
    assert_eq!(
        http.post("/api/discover/add/movie", body).await.0,
        StatusCode::OK
    );
    let (_, remembered) = http.get("/api/discover/add/movie/options").await;
    assert_eq!(remembered["defaults"]["monitored"], false);
    assert_eq!(remembered["defaults"]["minimum_availability"], "inCinemas");
    assert_eq!(remembered["defaults"]["tags"], json!([2]));
    assert_eq!(remembered["defaults"]["search"], false);
    // The series dialog has its own defaults.
    let (_, series) = http.get("/api/discover/add/series/options").await;
    assert_eq!(series["defaults"]["monitored"], true);
    assert_eq!(series["minimum_availability"], json!([]));

    // Gone tag, gone root folder (two others now), gone profile: first-time values.
    http.app
        .db
        .put_setting(
            "rootfolder",
            &json!([{"path": text(&root.join("andere"))}, {"path": text(&root.join("dritte"))}]),
        )
        .await
        .expect("roots");
    http.app
        .db
        .put_setting("tag", &json!([]))
        .await
        .expect("tags");
    let mut stored = http
        .app
        .db
        .setting("discover_add_movie")
        .await
        .expect("db")
        .expect("stored");
    stored["quality_profile_id"] = json!(42);
    http.app
        .db
        .put_setting("discover_add_movie", &stored)
        .await
        .expect("stored");
    let (_, fallen) = http.get("/api/discover/add/movie/options").await;
    assert_eq!(fallen["defaults"]["quality_profile_id"], 1);
    assert_eq!(fallen["defaults"]["root_folder"], Value::Null);
    assert_eq!(fallen["defaults"]["tags"], json!([]));
    assert_eq!(fallen["defaults"]["monitored"], false);
    let _ = std::fs::remove_dir_all(&root);
}

/// AC-11: a title already in the library or a root folder that is gone adds nothing;
/// an excluded title needs a confirmation and then leaves the exclusion list.
#[tokio::test]
async fn duplicates_missing_root_folders_and_excluded_titles_are_refused() {
    let providers = Providers::start().await;
    let services = Services::start().await;
    let root = temp_root("refused");
    {
        let mut fake = providers.lock();
        fake.movies
            .insert(101, movie_detail(101, "Erster Film", "2001-01-01", None));
        fake.movies.insert(
            503,
            movie_detail(503, "Ausgeschlossen", "2003-01-01", Some("tt0000503")),
        );
    }
    let http = movie_app(&providers, &services, &root).await;
    http.app
        .db
        .put_setting(
            "exclusions",
            &json!([{"tmdbId": 503, "movieTitle": "Ausgeschlossen"}, {"tmdbId": 7}]),
        )
        .await
        .expect("exclusions");
    let movies_before = http.app.db.movies().await.expect("movies").len();

    let (status, error) = http
        .post("/api/discover/add/movie", movie_body(101, &root, false))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["error"], "„Erster Film“ ist schon in der Bibliothek");

    let (status, error) = http
        .post("/api/discover/add/movie", movie_body(503, &root, false))
        .await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
    assert!(
        error["error"]
            .as_str()
            .expect("error")
            .contains("Ausschlussliste")
    );
    assert_eq!(
        http.app.db.movies().await.expect("movies").len(),
        movies_before
    );

    let mut confirmed = movie_body(503, &root, false);
    confirmed["confirm_excluded"] = json!(true);
    let (status, added) = http.post("/api/discover/add/movie", confirmed).await;
    assert_eq!(status, StatusCode::OK, "{added}");
    assert_eq!(
        http.app.db.setting("exclusions").await.expect("db"),
        Some(json!([{"tmdbId": 7}]))
    );

    // The root folder is gone from disk.
    std::fs::remove_dir_all(&root).expect("remove root");
    providers
        .lock()
        .movies
        .insert(504, movie_detail(504, "Neu", "2004-01-01", None));
    let (status, error) = http
        .post("/api/discover/add/movie", movie_body(504, &root, false))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        error["error"]
            .as_str()
            .expect("error")
            .contains("existiert nicht mehr"),
        "{error}"
    );
    assert!(http.app.db.movie_by_tmdb(504).await.expect("db").is_none());
}

/// An app with the Sonarr library, a series root folder, TVDB series 88 and its TMDB
/// entry 1400.
async fn series_app(providers: &Providers, services: &Services, root: &Path) -> Http {
    {
        let mut fake = providers.lock();
        fake.shows.insert(
            1400,
            show_detail(1400, "Großes Meer", Some(88), Some("tt0000888"), 2),
        );
        fake.tvdb_series.insert(
            88,
            json!({"id": 88, "name": "Big Sea", "year": "2020", "originalLanguage": "eng",
                   "status": {"name": "Continuing"}, "overview": "Water.",
                   "image": "https://artworks.thetvdb.com/banners/v4/series/88/posters/a.jpg",
                   "translations": {"nameTranslations": [{"language": "deu", "name": "Großes Meer"}]}}),
        );
        let episode = |id: i64, season: u16, number: u16| {
            json!({"id": id, "seasonNumber": season, "number": number,
                   "name": format!("Folge {number}"), "aired": "2020-01-01"})
        };
        fake.tvdb_episodes.insert(
            88,
            vec![
                episode(800, 0, 1),
                episode(801, 1, 1),
                episode(802, 1, 2),
                episode(803, 2, 1),
            ],
        );
    }
    let app = providers
        .app(Library::Series, &services.indexer_toml("idx", &[]))
        .await;
    app.db
        .put_setting(
            &format!("{}rootfolder", series::PREFIX),
            &json!([{"path": text(root)}]),
        )
        .await
        .expect("roots");
    Http::start(app).await
}

fn series_body(tmdb_id: i64, root: &Path) -> Value {
    json!({"tmdb_id": tmdb_id, "quality_profile_id": 1, "root_folder": text(root),
           "monitored": true, "seasons": [1], "monitor_new_seasons": false, "search": true})
}

/// AC-08: a series is found on TVDB through TMDB and put into the library with its
/// episodes; the chosen seasons are monitored, specials and later seasons not (switch
/// off), an existing folder is scanned and the missing episodes are searched.
#[tokio::test]
async fn an_added_series_monitors_the_chosen_seasons_and_new_ones_by_the_switch() {
    let providers = Providers::start().await;
    let services = Services::start().await;
    let root = temp_root("series");
    let folder = root.join("Big Sea");
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(
        folder.join("Big.Sea.S01E01.German.1080p.WEB-DL.h264-GRP.mkv"),
        vec![1u8; 2048],
    )
    .expect("video");
    let http = series_app(&providers, &services, &root).await;

    let (status, added) = http
        .post("/api/discover/add/series", series_body(1400, &root))
        .await;
    assert_eq!(status, StatusCode::OK, "{added}");
    assert_eq!(added["kind"], "series");
    assert_eq!(added["title"], "Big Sea");
    assert_eq!(added["path"], text(&folder));
    assert_eq!(added["files_found"], 1);
    assert_eq!(added["searching"], true);
    let id = added["id"].as_i64().expect("id");
    let show = http
        .app
        .db
        .series_by_tvdb(88)
        .await
        .expect("db")
        .expect("added");
    assert_eq!(show.id, id);
    assert_eq!(show.data.title_de.as_deref(), Some("Großes Meer"));
    assert_eq!(show.data.runtime_minutes, 45);
    assert!(show.data.monitored);
    assert_eq!(
        http.app.db.series_tmdb_id(id).await.expect("tmdb"),
        Some(1400)
    );
    assert!(!http.app.db.monitors_new_seasons(id).await.expect("switch"));
    let monitored = |episodes: &[sofarr_db::Episode]| -> Vec<(u16, u16, bool)> {
        episodes
            .iter()
            .map(|e| (e.data.season, e.data.number, e.data.monitored))
            .collect()
    };
    let episodes = http.app.db.episodes(Some(id)).await.expect("episodes");
    assert_eq!(
        monitored(&episodes),
        [(0, 1, false), (1, 1, true), (1, 2, true), (2, 1, false)]
    );
    assert!(episodes[1].file.is_some(), "S01E01 found on disk");
    assert!(
        searched(&services, "Big").await,
        "missing episodes searched"
    );

    // Later: a new episode of season 1 is monitored, a new season is not (switch off).
    {
        let mut fake = providers.lock();
        let episodes = fake.tvdb_episodes.get_mut(&88).expect("episodes");
        episodes.push(json!({"id": 804, "seasonNumber": 1, "number": 3, "aired": "2020-02-01"}));
        episodes.push(json!({"id": 805, "seasonNumber": 3, "number": 1, "aired": "2021-01-01"}));
    }
    let tvdb = metadata::tvdb(&http.app).expect("tvdb");
    series::refresh(&http.app.db, &tvdb, None, &show, Timestamp::now())
        .await
        .expect("refresh");
    let episodes = http.app.db.episodes(Some(id)).await.expect("episodes");
    let later: Vec<(u16, u16, bool)> = monitored(&episodes)
        .into_iter()
        .filter(|(s, n, _)| (*s, *n) == (1, 3) || *s == 3)
        .collect();
    assert_eq!(later, [(1, 3, true), (3, 1, false)]);

    // Added twice: refused.
    let (status, error) = http
        .post("/api/discover/add/series", series_body(1400, &root))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        error["error"]
            .as_str()
            .expect("error")
            .contains("schon in der Bibliothek")
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// AC-09: without a TVDB id at TMDB, unknown to TVDB, or with TVDB down, nothing is
/// added and a German message says why.
#[tokio::test]
async fn a_series_tvdb_cannot_give_is_not_added() {
    let providers = Providers::start().await;
    let services = Services::start().await;
    let root = temp_root("tvdb");
    let http = series_app(&providers, &services, &root).await;
    {
        let mut fake = providers.lock();
        fake.shows
            .insert(1401, show_detail(1401, "Ohne TVDB", None, None, 1));
        fake.shows
            .insert(1402, show_detail(1402, "Unbekannt", Some(99), None, 1));
    }
    let series_before = http.app.db.all_series().await.expect("series").len();
    let cases = [
        (1401, StatusCode::UNPROCESSABLE_ENTITY, "keine TVDB-ID"),
        (
            1402,
            StatusCode::UNPROCESSABLE_ENTITY,
            "TVDB kennt die Serie",
        ),
    ];
    for (tmdb_id, expected, words) in cases {
        let (status, error) = http
            .post("/api/discover/add/series", series_body(tmdb_id, &root))
            .await;
        assert_eq!(status, expected, "{tmdb_id}: {error}");
        assert!(
            error["error"].as_str().expect("error").contains(words),
            "{error}"
        );
    }
    providers.lock().tvdb_down = true;
    let (status, error) = http
        .post("/api/discover/add/series", series_body(1400, &root))
        .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(
        error["error"]
            .as_str()
            .expect("error")
            .contains("TVDB ist nicht erreichbar")
    );
    assert_eq!(
        http.app.db.all_series().await.expect("series").len(),
        series_before
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// AC-12: a TMDb person import list from a person page, once per person; the next sync
/// adds the person's movies.
#[tokio::test]
async fn a_person_import_list_is_created_once_and_synced_like_any_other() {
    let providers = Providers::start().await;
    let services = Services::start().await;
    let root = temp_root("list");
    {
        let mut fake = providers.lock();
        fake.people.insert(
            7,
            json!({"id": 7, "name": "Dora Director", "combined_credits": {"cast": [], "crew": []}}),
        );
        fake.movie_credits.insert(
            7,
            json!({"cast": [{"id": 601, "title": "Gespielt"}],
                   "crew": [{"id": 602, "title": "Gedreht", "department": "Directing"}]}),
        );
        fake.movies
            .insert(601, movie_detail(601, "Gespielt", "2001-01-01", None));
        fake.movies
            .insert(602, movie_detail(602, "Gedreht", "2002-01-01", None));
    }
    let http = movie_app(&providers, &services, &root).await;
    let body = json!({"cast": false, "departments": ["Directing"], "quality_profile_id": 1,
                      "root_folder": text(&root), "monitored": true,
                      "minimum_availability": "released", "tags": [2], "search_on_add": false});

    let (status, created) = http
        .post("/api/discover/people/7/import-list", body.clone())
        .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created, json!({"id": 1, "name": "Dora Director"}));
    let (status, error) = http
        .post("/api/discover/people/7/import-list", body.clone())
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        error["error"],
        "Für Dora Director gibt es schon eine Importliste („Dora Director“) – keine neue angelegt"
    );
    let mut wrong = body;
    wrong["departments"] = json!(["Camera"]);
    assert_eq!(
        http.post("/api/discover/people/8/import-list", wrong)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );

    let added = http.app.sync_import_lists().await.expect("sync");
    assert_eq!(added, ["Gedreht"]);
    let movie = http
        .app
        .db
        .movie_by_tmdb(602)
        .await
        .expect("db")
        .expect("added");
    assert_eq!(movie.tags, [2]);
    assert!(movie.path.starts_with(&text(&root)));
    let _ = std::fs::remove_dir_all(&root);
}

/// AC-10: without stored defaults the dialog offers the first quality profile by name,
/// not the one with the lowest id.
#[tokio::test]
async fn the_first_profile_by_name_is_the_first_time_default() {
    let providers = Providers::start().await;
    let services = Services::start().await;
    let root = temp_root("profile-by-name");
    let http = movie_app(&providers, &services, &root).await;
    let mut profiles = http
        .app
        .db
        .setting("qualityprofile")
        .await
        .expect("db")
        .expect("profiles");
    let mut alpha = profiles[0].clone();
    alpha["id"] = json!(2);
    alpha["name"] = json!("Alpha");
    profiles.as_array_mut().expect("list").push(alpha);
    http.app
        .db
        .put_setting("qualityprofile", &profiles)
        .await
        .expect("profiles");
    let (status, options) = http.get("/api/discover/add/movie/options").await;
    assert_eq!(status, StatusCode::OK, "{options}");
    assert_eq!(
        options["profiles"],
        json!([{"id": 2, "name": "Alpha"}, {"id": 1, "name": "HD"}])
    );
    assert_eq!(options["defaults"]["quality_profile_id"], 2);
    let _ = std::fs::remove_dir_all(&root);
}
