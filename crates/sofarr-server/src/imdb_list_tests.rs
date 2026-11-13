//! IMDb ratings in the library lists and pages (spec imdb-ratings AC-04, AC-06, AC-07,
//! AC-09, AC-11).

use std::time::{Duration, Instant};

use jiff::Timestamp;
use reqwest::StatusCode;
use serde_json::Value;
use sofarr_db::{Availability, Db, NewMovie, NewSeries, RatingRow};

use crate::{app::App, config::Config, testkit::Http};

fn movie(tmdb: i64, imdb: Option<&str>, title: &str) -> NewMovie {
    NewMovie {
        tmdb_id: tmdb,
        imdb_id: imdb.map(str::to_owned),
        title: title.to_owned(),
        original_title: None,
        title_de: None,
        year: Some(2001),
        runtime_minutes: 100,
        original_language: Some("English".to_owned()),
        monitored: true,
        quality_profile_id: 1,
        path: format!("/movies/{title}"),
        radarr_id: None,
        alternative_titles: Vec::new(),
        availability: Availability::default(),
        tags: Vec::new(),
    }
}

async fn load(db: &Db, rows: &[(&str, f32, u32)]) {
    let mut load = db.begin_imdb_load().await.expect("begin");
    let rows: Vec<RatingRow> = rows
        .iter()
        .map(|(id, r, v)| ((*id).to_owned(), *r, *v))
        .collect();
    load.add(&rows).await.expect("add");
    load.finish(Timestamp::now()).await.expect("finish");
}

/// Four movies (one without an IMDb id) and a series, with ratings for three movies.
async fn rated_library() -> Db {
    let db = Db::in_memory().await.expect("db");
    crate::library::import_documents(&db, &crate::library::tests::documents(), Timestamp::now())
        .await
        .expect("settings");
    let now = Timestamp::now();
    for m in [
        movie(9001, Some("tt9000001"), "Alpha"),
        movie(9002, Some("tt9000002"), "Beta"),
        movie(9003, Some("tt9000003"), "Gamma"),
        movie(9004, None, "Delta"),
        movie(9005, Some("tt9000005"), "Epsilon"),
    ] {
        db.upsert_movie(&m, now).await.expect("movie");
    }
    db.upsert_series(
        &NewSeries {
            tvdb_id: 7001,
            imdb_id: Some("tt9100001".to_owned()),
            title: "Serie".to_owned(),
            title_de: None,
            year: Some(2010),
            runtime_minutes: 45,
            original_language: None,
            series_type: "standard".to_owned(),
            monitored: true,
            season_folder: true,
            quality_profile_id: 1,
            path: "/tv/Serie".to_owned(),
            sonarr_id: None,
            alternative_titles: Vec::new(),
        },
        now,
    )
    .await
    .expect("series");
    load(
        &db,
        &[
            ("tt9000001", 8.0, 100),
            ("tt9000002", 8.0, 500),
            ("tt9000003", 6.5, 1000),
            ("tt9100001", 7.7, 4242),
        ],
    )
    .await;
    db
}

async fn server(config: &str) -> Http {
    let config = Config::parse(config).expect("config");
    Http::start(App::with_db(config, rated_library().await).expect("app")).await
}

fn titles(page: &Value) -> Vec<&str> {
    page["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|i| i["title"].as_str().unwrap_or_default())
        .filter(|t| ["Alpha", "Beta", "Gamma", "Delta", "Epsilon"].contains(t))
        .collect()
}

fn item<'a>(page: &'a Value, title: &str) -> &'a Value {
    page["items"]
        .as_array()
        .and_then(|items| items.iter().find(|i| i["title"] == title))
        .unwrap_or_else(|| panic!("{title} in {page}"))
}

#[tokio::test]
async fn lists_and_pages_show_rating_and_votes_or_nothing() {
    let http = server("").await;
    let (status, page) = http.get("/api/library/movies?page_size=100").await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(item(&page, "Beta")["imdb_rating"], 8.0);
    assert_eq!(item(&page, "Beta")["imdb_votes"], 500);
    // No IMDb id, or an id without a rating: no rating (the UI shows "–").
    for title in ["Delta", "Epsilon"] {
        assert!(item(&page, title)["imdb_rating"].is_null(), "{title}");
        assert!(item(&page, title)["imdb_votes"].is_null(), "{title}");
    }
    let (_, shows) = http.get("/api/library/series").await;
    let show = item(&shows, "Serie");
    assert!(
        (show["imdb_rating"].as_f64().unwrap_or(0.0) - 7.7).abs() < 0.01,
        "{show}"
    );
    assert_eq!(show["imdb_votes"], 4242);
    let beta = http
        .app
        .db
        .movie_by_tmdb(9002)
        .await
        .expect("db")
        .expect("beta")
        .id;
    let (_, detail) = http.get(&format!("/api/library/movies/{beta}")).await;
    assert_eq!(detail["imdb_rating"], 8.0, "{detail}");
    assert_eq!(detail["imdb_votes"], 500);
}

#[tokio::test]
async fn sorting_by_rating_puts_unrated_titles_last() {
    let http = server("").await;
    let (_, high_first) = http
        .get("/api/library/movies?sort=rating&page_size=100")
        .await;
    // Higher first, votes break the tie, titles without a rating last (AC-06).
    let order = titles(&high_first);
    assert_eq!(&order[..3], ["Beta", "Alpha", "Gamma"]);
    let mut unrated: Vec<&str> = order[3..].to_vec();
    unrated.sort_unstable();
    assert_eq!(unrated, ["Delta", "Epsilon"]);
    let (_, low_first) = http
        .get("/api/library/movies?sort=rating&order=desc&page_size=100")
        .await;
    assert_eq!(&titles(&low_first)[..3], ["Gamma", "Alpha", "Beta"]);
}

#[tokio::test]
async fn minimum_rating_and_votes_hide_unrated_titles() {
    let http = server("").await;
    let (_, rating) = http
        .get("/api/library/movies?min_rating=7.5&page_size=100")
        .await;
    assert_eq!(titles(&rating), ["Alpha", "Beta"]);
    let (_, votes) = http
        .get("/api/library/movies?min_votes=200&page_size=100")
        .await;
    assert_eq!(titles(&votes), ["Beta", "Gamma"]);
    let (_, both) = http
        .get("/api/library/movies?min_rating=7&min_votes=200&page_size=100")
        .await;
    assert_eq!(titles(&both), ["Beta"]);
    let (_, shows) = http.get("/api/library/series?min_rating=8").await;
    assert_eq!(shows["total"], 0, "{shows}");
}

#[tokio::test]
async fn a_new_ratings_load_shows_in_the_lists() {
    let http = server("").await;
    let (_, before) = http.get("/api/library/movies?page_size=100").await;
    assert!(item(&before, "Epsilon")["imdb_rating"].is_null());
    load(&http.app.db, &[("tt9000005", 9.1, 77)]).await;
    let (_, after) = http.get("/api/library/movies?page_size=100").await;
    assert_eq!(item(&after, "Epsilon")["imdb_votes"], 77);
    assert!(item(&after, "Beta")["imdb_rating"].is_null());
}

#[tokio::test]
async fn switched_off_there_are_no_ratings_to_show_sort_or_filter() {
    let http = server("[imdb]\nenabled = false\n").await;
    let (_, page) = http.get("/api/library/movies?page_size=100").await;
    assert!(item(&page, "Beta")["imdb_rating"].is_null(), "{page}");
    for path in [
        "/api/library/movies?sort=rating",
        "/api/library/movies?min_rating=5",
        "/api/library/series?min_votes=5",
    ] {
        let (status, answer) = http.get(path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert!(
            answer["error"].as_str().is_some_and(|m| m.contains("IMDb")),
            "{answer}"
        );
    }
}

/// AC-11: ratings send no request to IMDb and keep the list within web-ui-library AC-04.
#[tokio::test]
async fn rated_lists_of_2500_movies_stay_fast() {
    let db = Db::in_memory().await.expect("db");
    crate::library::import_documents(&db, &crate::library::tests::documents(), Timestamp::now())
        .await
        .expect("settings");
    let now = Timestamp::now();
    let mut rows = Vec::new();
    for i in 0..2_500u32 {
        let id = format!("tt{:07}", 8_000_000 + i);
        db.upsert_movie(
            &movie(20_000 + i64::from(i), Some(&id), &format!("Film {i}")),
            now,
        )
        .await
        .expect("movie");
        rows.push((
            id,
            1.0 + f32::from(u16::try_from(i % 90).unwrap_or(0)) / 10.0,
            i,
        ));
    }
    let mut ratings = db.begin_imdb_load().await.expect("begin");
    ratings.add(&rows).await.expect("add");
    ratings.finish(now).await.expect("finish");
    // The dataset URL points nowhere: any request to it would fail the lists.
    let config = Config::parse("[imdb]\nurl = \"http://127.0.0.1:9/never\"\n").expect("config");
    let http = Http::start(App::with_db(config, db).expect("app")).await;
    let started = Instant::now();
    let (status, first) = http.get("/api/library/movies?sort=rating").await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert!(started.elapsed() < Duration::from_secs(1));
    let mut slowest = Duration::ZERO;
    for path in [
        "/api/library/movies?sort=rating&page=4",
        "/api/library/movies?min_rating=5&min_votes=100&sort=rating",
        "/api/library/movies?q=film%2012&min_rating=2",
    ] {
        let started = Instant::now();
        let (status, page) = http.get(path).await;
        slowest = slowest.max(started.elapsed());
        assert_eq!(status, StatusCode::OK, "{path}: {page}");
    }
    assert!(
        slowest < Duration::from_millis(200),
        "took up to {slowest:?}"
    );
}
