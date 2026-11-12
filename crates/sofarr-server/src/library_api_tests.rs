//! Library lists and details over HTTP (spec web-ui-library AC-02 to AC-06).

use std::time::{Duration, Instant};

use jiff::Timestamp;
use reqwest::StatusCode;
use serde_json::{Value, json};
use sofarr_db::{Availability, Db, MovieFile, NewMovie};

use crate::{
    app::App,
    config::Config,
    testkit::{Http, Library, app},
};

fn titles(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|i| i["title"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[tokio::test]
async fn movie_list_filters_searches_sorts_and_pages() {
    let app = app(Library::Movies, "").await;
    // Added on four different days, in an order neither title nor year gives.
    for (title, added) in [
        ("Vierter Film", "2020-01-01T00:00:00Z"),
        ("Zweiter Film", "2021-01-01T00:00:00Z"),
        ("Erster Film", "2022-01-01T00:00:00Z"),
        ("Dritter Film", "2023-01-01T00:00:00Z"),
    ] {
        sqlx::query("UPDATE movies SET added_at = ? WHERE title = ?")
            .bind(added)
            .bind(title)
            .execute(app.db.pool())
            .await
            .expect("added");
    }
    sqlx::query("UPDATE movies SET title_de = 'Der vierte Streifen' WHERE title = 'Vierter Film'")
        .execute(app.db.pool())
        .await
        .expect("German title");
    let http = Http::start(app).await;
    let (status, all) = http.get("/api/library/movies").await;
    assert_eq!(status, StatusCode::OK, "{all}");
    assert_eq!(all["total"], 4);
    assert_eq!(
        titles(&all),
        [
            "Dritter Film",
            "Erster Film",
            "Vierter Film",
            "Zweiter Film"
        ]
    );
    let first = &all["items"][1];
    assert_eq!(first["missing"], true);
    assert_eq!(first["quality"], Value::Null);
    let second = &all["items"][3];
    assert_eq!(second["cutoff_unmet"], true);
    assert_eq!(second["quality"], "WEBDL-1080p");

    let (_, missing) = http.get("/api/library/movies?missing=yes").await;
    assert_eq!(titles(&missing), ["Erster Film"]);
    let (_, cutoff) = http
        .get("/api/library/movies?cutoff_unmet=yes&has_file=yes&monitored=yes")
        .await;
    assert_eq!(titles(&cutoff), ["Zweiter Film"]);
    let (_, no_file) = http.get("/api/library/movies?has_file=no").await;
    assert_eq!(titles(&no_file), ["Erster Film", "Vierter Film"]);
    let (_, unmonitored) = http.get("/api/library/movies?monitored=no").await;
    assert_eq!(titles(&unmonitored), ["Vierter Film"]);
    // Alternative titles count, punctuation and case do not.
    let (_, found) = http.get("/api/library/movies?q=first%20FILM").await;
    assert_eq!(titles(&found), ["Erster Film"]);
    let (_, german) = http.get("/api/library/movies?q=vierte%20streifen").await;
    assert_eq!(titles(&german), ["Vierter Film"]);
    let (_, profile) = http.get("/api/library/movies?profile=2").await;
    assert_eq!(profile["total"], 0);
    let (_, by_year) = http
        .get("/api/library/movies?sort=year&order=desc&page=1&page_size=2")
        .await;
    assert_eq!(by_year["total"], 4);
    assert_eq!(titles(&by_year), ["Dritter Film", "Zweiter Film"]);
    let (_, page2) = http
        .get("/api/library/movies?sort=year&order=desc&page=2&page_size=2")
        .await;
    assert_eq!(titles(&page2), ["Erster Film", "Vierter Film"]);
    let (_, by_size) = http.get("/api/library/movies?sort=size&has_file=yes").await;
    assert_eq!(titles(&by_size), ["Dritter Film", "Zweiter Film"]);
    let (_, by_added) = http.get("/api/library/movies?sort=added").await;
    assert_eq!(
        titles(&by_added),
        [
            "Vierter Film",
            "Zweiter Film",
            "Erster Film",
            "Dritter Film"
        ]
    );
    let (_, newest) = http.get("/api/library/movies?sort=added&order=desc").await;
    assert_eq!(
        titles(&newest),
        [
            "Dritter Film",
            "Erster Film",
            "Zweiter Film",
            "Vierter Film"
        ]
    );

    for bad in [
        "/api/library/movies?missing=vielleicht",
        "/api/library/movies?page=0",
        "/api/library/movies?page_size=501",
        "/api/library/movies?sort=farbe",
        "/api/library/movies?unbekannt=1",
    ] {
        let (status, body) = http.get(bad).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
        assert!(
            body["error"]
                .as_str()
                .is_some_and(|e| e.starts_with("Ungültige Listenanfrage")),
            "{bad}: {body}"
        );
    }
}

#[tokio::test]
async fn series_list_counts_and_filters_by_the_series_rule() {
    let http = Http::start(app(Library::Series, "").await).await;
    let (status, all) = http.get("/api/library/series").await;
    assert_eq!(status, StatusCode::OK, "{all}");
    let show = &all["items"][0];
    assert_eq!(show["title"], "Kleine Insel");
    assert_eq!(show["episodes"], 3);
    assert_eq!(show["files"], 1);
    // S01E02 aired without a file; S01E03 airs in 2099.
    assert_eq!(show["missing"], 1);
    assert_eq!(show["cutoff_unmet"], 1);
    assert_eq!(show["complete"], false);
    let (_, complete) = http.get("/api/library/series?complete=yes").await;
    assert_eq!(complete["total"], 0);
    let (_, missing) = http.get("/api/library/series?missing=yes&q=small").await;
    assert_eq!(titles(&missing), ["Kleine Insel"]);
    let (status, _) = http.get("/api/library/series?has_file=yes").await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "no with-file filter for series"
    );
}

#[tokio::test]
async fn options_name_profiles_and_tags() {
    let http = Http::start(app(Library::Movies, "").await).await;
    http.app
        .db
        .put_setting("tag", &json!([{"id": 1, "label": "Kinder"}]))
        .await
        .expect("tags");
    let (status, options) = http.get("/api/library/options").await;
    assert_eq!(status, StatusCode::OK, "{options}");
    assert_eq!(options["movie_profiles"], json!([{"id": 1, "name": "HD"}]));
    assert_eq!(options["movie_tags"], json!([{"id": 1, "name": "Kinder"}]));
    assert_eq!(options["series_profiles"], json!([]));
    assert_eq!(options["file_actions"], false, "shadow mode");
}

/// A library of 2,500 movies with titles, files and availability like the owner's.
async fn large_library(n: i64) -> Db {
    let db = Db::in_memory().await.expect("db");
    crate::library::import_documents(&db, &crate::library::tests::documents(), Timestamp::now())
        .await
        .expect("settings");
    let now = Timestamp::now();
    for i in 0..n {
        let id = db
            .upsert_movie(
                &NewMovie {
                    tmdb_id: 10_000 + i,
                    imdb_id: None,
                    title: format!("Film Nummer {i}"),
                    original_title: Some(format!("Movie Number {i}")),
                    title_de: Some(format!("Film Nummer {i}")),
                    year: Some(1950 + u16::try_from(i % 75).unwrap_or(0)),
                    runtime_minutes: 100,
                    original_language: Some("English".to_owned()),
                    monitored: i % 50 == 0,
                    quality_profile_id: 1,
                    path: format!("/movies/Film Nummer {i}"),
                    radarr_id: None,
                    alternative_titles: vec![format!("Alt {i}"), format!("Autre {i}")],
                    availability: Availability {
                        digital_release: Some("2020-01-01".to_owned()),
                        ..Availability::default()
                    },
                    tags: vec![i % 3],
                },
                now,
            )
            .await
            .expect("movie");
        if i % 4 != 0 {
            db.set_file(
                id,
                &MovieFile {
                    relative_path: format!("Film Nummer {i}.mkv"),
                    size: 1_000_000 + u64::try_from(i).unwrap_or(0),
                    quality: "WEBDL-1080p".to_owned(),
                    revision_version: 1,
                    revision_real: 0,
                    release_group: Some("GRP".to_owned()),
                    scene_name: None,
                    custom_format_score: 0,
                },
                now,
            )
            .await
            .expect("file");
        }
    }
    db
}

#[tokio::test]
async fn a_large_library_answers_a_filtered_page_fast() {
    // AC-04 on the server side: the browser measurement needs the list endpoint to
    // filter, search and page 2,500 movies well within 200 ms (debug build included).
    // The imported documents bring four movies.
    let db = large_library(2_496).await;
    let app = App::with_db(Config::default(), db).expect("app");
    let http = Http::start(app).await;
    // The first page right after the start computes the list (AC-04: within 1 s).
    let started = Instant::now();
    let (status, first) = http.get("/api/library/movies?page_size=50").await;
    let cold = started.elapsed();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["total"], 2_500);
    eprintln!("first list answer for 2,500 movies: {cold:?}");
    assert!(cold < Duration::from_secs(1), "first answer took {cold:?}");
    let mut slowest = Duration::ZERO;
    for path in [
        "/api/library/movies?page_size=50",
        "/api/library/movies?q=nummer%2012&sort=year&order=desc",
        "/api/library/movies?missing=yes&monitored=yes&tag=0",
        "/api/library/movies?has_file=yes&cutoff_unmet=yes&sort=size&page=3",
    ] {
        let started = Instant::now();
        let (status, page) = http.get(path).await;
        slowest = slowest.max(started.elapsed());
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(page["total"].as_u64().is_some_and(|t| t > 0), "{path}");
    }
    eprintln!("slowest list answer for 2,500 movies: {slowest:?}");
    assert!(
        slowest < Duration::from_millis(200),
        "list answers took up to {slowest:?}"
    );
}

#[tokio::test]
async fn movie_page_shows_everything_about_one_movie() {
    let app = app(Library::Movies, "").await;
    let movie = app
        .db
        .movie_by_tmdb(102)
        .await
        .expect("db")
        .expect("Zweiter Film");
    app.db
        .put_setting(
            "config_naming",
            &json!({"movieFolderFormat": "{Movie Title} ({Release Year}) [tmdb-{TmdbId}]"}),
        )
        .await
        .expect("naming");
    app.db
        .put_setting("rootfolder", &json!([{"path": "/data/media/movies"}]))
        .await
        .expect("roots");
    app.db
        .put_setting("tag", &json!([{"id": 2, "label": "kinder"}]))
        .await
        .expect("tags");
    app.db
        .set_movie_presentation(movie.id, Some("/zweiter.jpg"), Some("Ein zweiter Film."))
        .await
        .expect("presentation");
    sqlx::query(
        "UPDATE movies SET tags = '2', in_cinemas = '2002-03-01', digital_release = '2002-07-15',
            physical_release = '2002-08-02' WHERE id = ?",
    )
    .bind(movie.id)
    .execute(app.db.pool())
    .await
    .expect("tags and dates");
    let grab = app
        .db
        .add_grab(
            &sofarr_db::NewGrab {
                movie_id: movie.id,
                title: "Zweiter.Film.2002.German.1080p.BluRay.x264-GRP".to_owned(),
                guid: "g1".to_owned(),
                indexer: "idx".to_owned(),
                download_url: None,
                quality: "Bluray-1080p".to_owned(),
                custom_format_score: 10,
                size: 8_000,
                status: sofarr_db::GrabStatus::Shadow,
                download_id: None,
            },
            Timestamp::now(),
        )
        .await
        .expect("grab");
    app.db
        .add_block(
            &sofarr_db::NewBlock {
                media: sofarr_db::MediaRef::Movie(movie.id),
                title: "Zweiter.Film.2002.1080p.WEB-DL-BAD".to_owned(),
                normalized_title: "zweiter film 2002 1080p web dl bad".to_owned(),
                guid: "bad".to_owned(),
                indexer: "idx".to_owned(),
                reason: "FAILURE/PAR".to_owned(),
            },
            Timestamp::now(),
        )
        .await
        .expect("block");
    let http = Http::start(app).await;
    let (status, detail) = http.get(&format!("/api/library/movies/{}", movie.id)).await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["title"], "Zweiter Film");
    assert_eq!(detail["overview"], "Ein zweiter Film.");
    assert_eq!(detail["has_poster"], true);
    assert_eq!(detail["quality_profile"], json!({"id": 1, "name": "HD"}));
    assert_eq!(detail["minimum_availability"], "released");
    assert_eq!(detail["tags"], json!([{"id": 2, "name": "kinder"}]));
    assert_eq!(
        detail["release_dates"],
        json!({"in_cinemas": "2002-03-01", "digital": "2002-07-15", "physical": "2002-08-02"})
    );
    assert_eq!(detail["file"]["quality"], "WEBDL-1080p");
    assert_eq!(detail["file"]["release_group"], Value::Null);
    assert_eq!(detail["file"]["size"], 1000);
    assert_eq!(detail["cutoff_unmet"], true);
    assert_eq!(detail["cutoff_reason"], "WEBDL-1080p statt Bluray-1080p");
    assert_eq!(detail["grabs"][0]["id"], grab);
    assert_eq!(detail["grabs"][0]["status"], "shadow");
    assert_eq!(detail["blocklist"][0]["reason"], "FAILURE/PAR");
    assert_eq!(
        detail["folder"],
        json!({
            "current": "/data/media/movies/Zweiter Film (2002)",
            "expected": "/data/media/movies/Zweiter Film (2002) [tmdb-102]",
            "differs": true
        })
    );
    // Shadow mode: no file actions (AC-17).
    assert_eq!(detail["file_actions"]["available"], false);
    assert!(
        detail["file_actions"]["reason"]
            .as_str()
            .is_some_and(|r| r.contains("Live-Modus")),
        "{detail}"
    );
    let (status, missing) = http.get("/api/library/movies/9999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(missing["error"], "Film 9999 ist nicht in der Bibliothek");
    let (status, _) = http.get("/api/library/movies/abc").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn series_page_shows_seasons_and_episodes() {
    let app = app(Library::Series, "[automation]\nmode = \"live\"\n").await;
    let show = app.db.all_series().await.expect("db").remove(0);
    let http = Http::start(app).await;
    let (status, detail) = http.get(&format!("/api/library/series/{}", show.id)).await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["title"], "Kleine Insel");
    assert_eq!(detail["missing"], 1);
    assert_eq!(
        detail["file_actions"],
        json!({"available": true, "reason": null})
    );
    let season = &detail["seasons"][0];
    assert_eq!(season["number"], 1);
    assert_eq!(season["episodes_total"], 3);
    assert_eq!(season["episodes_with_file"], 1);
    assert_eq!(season["episodes_monitored"], 3);
    let episodes = season["episodes"].as_array().expect("episodes");
    assert_eq!(episodes[0]["title"], "Ankunft");
    assert_eq!(episodes[0]["quality"], "WEBDL-1080p");
    assert_eq!(episodes[1]["missing"], true);
    assert_eq!(episodes[2]["missing"], false, "airs in 2099");
    assert_eq!(detail["folder"]["expected"], "/tv/Kleine Insel");
    assert_eq!(detail["folder"]["differs"], false);
    let (status, one) = http
        .get(&format!("/api/library/series/{}/seasons/1", show.id))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(one["episodes_total"], 3);
    let (status, none) = http
        .get(&format!("/api/library/series/{}/seasons/7", show.id))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        none["error"],
        "Staffel 7 von „Kleine Insel“ ist nicht in der Bibliothek"
    );
    let first = episodes[0]["id"].as_i64().expect("id");
    let (status, episode) = http
        .get(&format!("/api/library/series/{}/episodes/{first}", show.id))
        .await;
    assert_eq!(status, StatusCode::OK, "{episode}");
    assert_eq!(episode["series_title"], "Kleine Insel");
    assert_eq!(
        episode["file"]["relative_path"],
        "Staffel 01/Kleine Insel - S01E01.mkv"
    );
    assert_eq!(episode["cutoff_reason"], "WEBDL-1080p statt Bluray-1080p");
}
