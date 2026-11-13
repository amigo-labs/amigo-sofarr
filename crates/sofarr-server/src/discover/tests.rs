//! The discover page against a fake TMDB (spec discover AC-01..AC-03, AC-13..AC-16).

use std::time::Instant;

use axum::http::StatusCode;
use jiff::SignedDuration;
use serde_json::{Value, json};

use crate::{
    testkit::{Http, Library},
    tmdb_fake::{Providers, movie_detail, movie_entry, page, ratings, series_entry, settled},
};

/// Shelf paths of the fake TMDB.
const SHELF_PATHS: [&str; 7] = [
    "trending/all/day",
    "trending/all/week",
    "movie/popular",
    "movie/upcoming",
    "movie/now_playing",
    "discover/movie",
    "tv/popular",
];

fn titles(answer: &Value) -> Vec<String> {
    answer["items"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .map(|i| i["title"].as_str().unwrap_or_default().to_owned())
        .collect()
}

async fn discover_app(providers: &Providers, extra: &str) -> Http {
    let app = providers.app(Library::Movies, extra).await;
    app.db
        .put_setting("exclusions", &json!([{"tmdbId": 900}]))
        .await
        .expect("exclusions");
    ratings(&app.db, &[("tt0000901", 7.5, 1200), ("tt0000101", 6.0, 50)]).await;
    Http::start(app).await
}

/// AC-01, AC-16, AC-17: seven shelves in order, up to 20 titles each, poster through
/// sofarr, library state, and ratings that load in the background.
#[tokio::test]
async fn shelves_come_in_order_with_library_state_and_ratings_that_load() {
    let providers = Providers::start().await;
    {
        let mut fake = providers.lock();
        for (n, path) in (0i64..).zip(SHELF_PATHS) {
            let entries = (0..25)
                .map(|i| {
                    let id = 1000 + n * 100 + i;
                    let mut entry = if path == "tv/popular" {
                        series_entry(id, &format!("Serie {id}"), "2020-01-01", 1.0)
                    } else {
                        movie_entry(id, &format!("Film {id}"), "2020-01-01", 1.0)
                    };
                    // Trending answers name the media type of each entry.
                    if path.starts_with("trending") {
                        entry["media_type"] = json!("movie");
                    }
                    entry
                })
                .collect();
            fake.lists.insert(path.to_owned(), page(entries, 1, 3));
        }
        // The popular movies: one in the library, one excluded, one with a rating.
        fake.lists.insert(
            "movie/popular".to_owned(),
            page(
                vec![
                    movie_entry(101, "Erster Film", "2001-01-01", 9.0),
                    movie_entry(900, "Ausgeschlossen", "2002-01-01", 8.0),
                    movie_entry(901, "Bewertet", "2003-01-01", 7.0),
                    movie_entry(902, "Ohne IMDb", "2004-01-01", 6.0),
                ],
                1,
                1,
            ),
        );
        fake.movies.insert(
            901,
            movie_detail(901, "Bewertet", "2003-01-01", Some("tt0000901")),
        );
        fake.movies
            .insert(902, movie_detail(902, "Ohne IMDb", "2004-01-01", None));
    }
    let http = discover_app(&providers, "").await;

    let (status, answer) = http.get("/api/discover/shelves").await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let shelves = answer["shelves"].as_array().expect("shelves");
    let ids: Vec<&str> = shelves.iter().filter_map(|s| s["id"].as_str()).collect();
    assert_eq!(
        ids,
        [
            "trending_day",
            "trending_week",
            "popular_movies",
            "upcoming",
            "now_playing",
            "new_at_home",
            "popular_series"
        ]
    );
    assert_eq!(shelves[0]["title"], "Trending heute");
    assert_eq!(shelves[5]["title"], "Neu als Stream oder Disc");
    assert_eq!(shelves[0]["items"].as_array().expect("items").len(), 20);
    assert_eq!(shelves[0]["has_more"], true);
    assert_eq!(shelves[6]["items"][0]["kind"], "series");
    let popular = shelves[2]["items"].as_array().expect("popular");
    assert_eq!(
        popular[0]["library"],
        json!({"status": "in_library", "id": 1})
    );
    assert_eq!(popular[0]["poster"], "/api/images/tmdb/w342/m101.jpg");
    assert_eq!(popular[0]["year"], 2001);
    assert_eq!(popular[1]["library"]["status"], "excluded");
    assert_eq!(popular[2]["library"]["status"], "none");
    assert_eq!(popular[2]["rating"]["state"], "pending");
    assert!(answer["freshness"]["fetched_at"].is_string());
    assert_eq!(answer["freshness"]["stale"], false);

    // German texts and German release dates (AC-16).
    let upcoming = providers.requests_with("movie/upcoming");
    assert!(upcoming[0].contains("language=de-DE") && upcoming[0].contains("region=DE"));
    let at_home = providers.requests_with("discover/movie");
    assert!(
        at_home[0].contains("with_release_type=4%7C5"),
        "{at_home:?}"
    );

    settled(&http.app).await;
    let (status, states) = http
        .post(
            "/api/discover/states",
            json!({"titles": [{"kind": "movie", "tmdb_id": 901}, {"kind": "movie", "tmdb_id": 902},
                              {"kind": "movie", "tmdb_id": 101}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{states}");
    assert_eq!(
        states["titles"][0]["rating"],
        json!({"state": "rated", "rating": 7.5, "votes": 1200})
    );
    assert_eq!(states["titles"][1]["rating"]["state"], "none");
    // TMDB knows no detail of the library movie, and the library has no IMDb id.
    assert_eq!(states["titles"][2]["rating"]["state"], "none");

    // More of a shelf.
    let (status, more) = http.get("/api/discover/shelves/trending_day?page=2").await;
    assert_eq!(status, StatusCode::OK, "{more}");
    assert_eq!(more["next_page"], 3);
    let (status, _) = http.get("/api/discover/shelves/unknown").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// AC-02: filters give one list, movies and series merged by popularity.
#[tokio::test]
async fn the_filtered_list_merges_movies_and_series_by_popularity() {
    let providers = Providers::start().await;
    {
        let mut fake = providers.lock();
        fake.lists.insert(
            "discover/movie?page=1".to_owned(),
            page(
                vec![
                    movie_entry(11, "Film A", "2010-01-01", 50.0),
                    movie_entry(12, "Film B", "2011-01-01", 10.0),
                ],
                1,
                1,
            ),
        );
        fake.lists.insert(
            "discover/tv?page=1".to_owned(),
            page(vec![series_entry(21, "Serie A", "2012-01-01", 30.0)], 1, 1),
        );
    }
    let http = discover_app(&providers, "").await;
    let (status, list) = http
        .get("/api/discover/list?genre=18&year_from=2000&year_to=2020&language=en")
        .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(titles(&list), ["Film A", "Serie A", "Film B"]);
    assert_eq!(list["next_page"], Value::Null);
    let request = &providers.requests_with("discover/movie")[0];
    for part in [
        "with_genres=18",
        "primary_release_date.gte=2000-01-01",
        "primary_release_date.lte=2020-12-31",
        "with_original_language=en",
        "sort_by=popularity.desc",
        "language=de-DE",
    ] {
        assert!(request.contains(part), "{request}");
    }
    let (_, series) = http.get("/api/discover/list?kind=series").await;
    assert_eq!(titles(&series), ["Serie A"]);
    let (status, _) = http
        .get("/api/discover/list?year_from=2020&year_to=2010")
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = http.get("/api/discover/list?language=Deutsch").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// AC-02: the rating filter keeps known ratings that reach it and titles still
/// loading, and reads TMDB page after page until 20 titles are visible.
#[tokio::test]
async fn the_rating_filter_reads_on_until_twenty_titles_are_visible() {
    let providers = Providers::start().await;
    {
        let mut fake = providers.lock();
        for p in 1..=4i64 {
            let entries = (0..10)
                .map(|i| movie_entry(3000 + p * 10 + i, "R", "2015-01-01", 100.0))
                .collect();
            fake.lists.insert(
                format!("discover/movie?page={p}"),
                page(entries, u32::try_from(p).expect("page"), 9),
            );
        }
        for id in 3010..3050 {
            let imdb = format!("tt{id:07}");
            fake.movies
                .insert(id, movie_detail(id, "R", "2015-01-01", Some(&imdb)));
        }
    }
    let http = discover_app(&providers, "").await;
    // Every other title reaches 7.
    let rows: Vec<(String, f32, u32)> = (3010..3050)
        .map(|id| {
            (
                format!("tt{id:07}"),
                if id % 2 == 0 { 8.0 } else { 5.0 },
                100,
            )
        })
        .collect();
    let rows: Vec<(&str, f32, u32)> = rows.iter().map(|(i, r, v)| (i.as_str(), *r, *v)).collect();
    ratings(&http.app.db, &rows).await;

    // Unknown ratings stay visible with the placeholder.
    let (status, first) = http.get("/api/discover/list?kind=movie&min_rating=7").await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let items = first["items"].as_array().expect("items");
    assert_eq!(items.len(), 20);
    assert!(items.iter().all(|i| i["rating"]["state"] == "pending"));
    let titles: Vec<Value> = (3010..3050)
        .map(|id| json!({"kind": "movie", "tmdb_id": id}))
        .collect();
    http.post("/api/discover/states", json!({ "titles": titles }))
        .await;
    settled(&http.app).await;

    let (status, rated) = http.get("/api/discover/list?kind=movie&min_rating=7").await;
    assert_eq!(status, StatusCode::OK, "{rated}");
    let shown = rated["items"].as_array().expect("items");
    assert_eq!(shown.len(), 20, "{rated}");
    assert!(
        shown
            .iter()
            .all(|i| i["rating"]["rating"].as_f64() >= Some(7.0))
    );
    assert_eq!(rated["next_page"], 5);

    // The rating filter needs IMDb ratings.
    let off = Providers::start().await;
    let http = Http::start(off.app(Library::Movies, "[imdb]\nenabled = false\n").await).await;
    let (status, error) = http.get("/api/discover/list?min_rating=7").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error["error"].as_str().unwrap_or_default().contains("IMDb"));
}

#[tokio::test]
async fn genres_of_movies_and_series_are_merged() {
    let providers = Providers::start().await;
    {
        let mut fake = providers.lock();
        fake.lists.insert(
            "genre/movie/list".to_owned(),
            json!({"genres": [{"id": 28, "name": "Action"}, {"id": 18, "name": "Drama"}]}),
        );
        fake.lists.insert(
            "genre/tv/list".to_owned(),
            json!({"genres": [{"id": 18, "name": "Drama"}, {"id": 10759, "name": "Action & Adventure"}]}),
        );
    }
    let http = discover_app(&providers, "").await;
    let (status, answer) = http.get("/api/discover/genres").await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(
        answer["genres"],
        json!([
            {"id": 28, "name": "Action", "movie": true, "series": false},
            {"id": 10759, "name": "Action & Adventure", "movie": false, "series": true},
            {"id": 18, "name": "Drama", "movie": true, "series": true}
        ])
    );
}

/// AC-03: search answers movies, series and people in three groups.
#[tokio::test]
async fn search_groups_movies_series_and_people_with_library_state() {
    let providers = Providers::start().await;
    providers.lock().lists.insert(
        "search/multi".to_owned(),
        page(
            vec![
                json!({"media_type": "movie", "id": 101, "title": "Erster Film", "release_date": "2001-01-01"}),
                json!({"media_type": "tv", "id": 55, "name": "Erste Serie"}),
                json!({"media_type": "person", "id": 7, "name": "Erika Erst", "profile_path": "/e.jpg",
                       "known_for_department": "Acting", "known_for": [{"title": "Erster Film"}]}),
            ],
            1,
            2,
        ),
    );
    let http = discover_app(&providers, "").await;
    let (status, answer) = http.get("/api/discover/search?q=erst").await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["movies"][0]["library"]["status"], "in_library");
    assert_eq!(answer["series"][0]["title"], "Erste Serie");
    assert_eq!(answer["series"][0]["library"]["status"], "none");
    assert_eq!(answer["people"][0]["name"], "Erika Erst");
    assert_eq!(answer["people"][0]["photo"], "/api/images/tmdb/w185/e.jpg");
    assert_eq!(answer["next_page"], 2);
    assert!(providers.requests_with("search/multi")[0].contains("query=erst"));
    let (status, _) = http.get("/api/discover/search?q=%20").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// AC-13: lists, filters and searches answer from the cache for 6 hours, titles for 24
/// hours, with the time they were fetched.
#[tokio::test]
async fn answers_come_from_the_cache_for_six_and_twenty_four_hours() {
    let providers = Providers::start().await;
    providers.lock().movies.insert(
        901,
        movie_detail(901, "Bewertet", "2003-01-01", Some("tt0000901")),
    );
    providers.lock().people.insert(
        7,
        json!({"id": 7, "name": "Erika Erst", "known_for_department": "Acting",
               "combined_credits": {"cast": [], "crew": []}}),
    );
    let http = discover_app(&providers, "").await;
    let count = |needle: &str| providers.requests_with(needle).len();
    let counts = || {
        (
            count("movie/popular"),
            count("search/multi"),
            count("with_genres"),
            count("movie/901"),
            count("person/7"),
        )
    };
    let load_all = || async {
        http.get("/api/discover/shelves").await;
        http.get("/api/discover/search?q=garten").await;
        http.get("/api/discover/list?kind=movie&genre=18").await;
        let (status, person) = http.get("/api/discover/people/7").await;
        assert_eq!(status, StatusCode::OK, "{person}");
        http.get("/api/discover/titles/movie/901").await.1
    };
    let age = |minutes: i64| {
        let by = SignedDuration::from_mins(minutes);
        http.app.discover.lists.age(by);
        http.app.discover.search.age(by);
        http.app.discover.titles.age(by);
        http.app.discover.people.age(by);
    };

    let first = load_all().await;
    let fetched_at = first["freshness"]["fetched_at"].clone();
    assert!(fetched_at.is_string());
    assert_eq!(counts(), (1, 1, 1, 1, 1));
    // Lists, title and person 5 h 59 min old: all from the cache.
    age(5 * 60 + 59);
    let again = load_all().await;
    assert!(again["freshness"]["fetched_at"].is_string());
    assert_eq!(counts(), (1, 1, 1, 1, 1));
    // Six hours: the lists are fetched again, title and person stay for 24 hours.
    age(2);
    load_all().await;
    assert_eq!(counts(), (2, 2, 2, 1, 1));
    age(18 * 60);
    let fresh = load_all().await;
    assert_ne!(fresh["freshness"]["fetched_at"], fetched_at);
    assert_eq!(counts(), (3, 3, 3, 2, 2));
}

/// AC-14: while TMDB is down, cached answers come with a German notice and their age;
/// without one a German message, never an internal error.
#[tokio::test]
async fn tmdb_down_shows_cached_data_with_a_notice_or_a_german_message() {
    let providers = Providers::start().await;
    providers.lock().movies.insert(
        901,
        movie_detail(901, "Bewertet", "2003-01-01", Some("tt0000901")),
    );
    let http = discover_app(&providers, "").await;
    assert_eq!(http.get("/api/discover/shelves").await.0, StatusCode::OK);
    assert_eq!(
        http.get("/api/discover/titles/movie/901").await.0,
        StatusCode::OK
    );
    providers.lock().tmdb_down = true;
    http.app.discover.lists.age(SignedDuration::from_hours(7));
    http.app.discover.titles.age(SignedDuration::from_hours(30));

    let (status, shelves) = http.get("/api/discover/shelves").await;
    assert_eq!(status, StatusCode::OK, "{shelves}");
    assert_eq!(shelves["freshness"]["stale"], true);
    let notice = shelves["freshness"]["notice"].as_str().expect("notice");
    assert!(
        notice.contains("TMDB ist nicht erreichbar") && notice.contains("7 Stunden"),
        "{notice}"
    );
    let (status, title) = http.get("/api/discover/titles/movie/901").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        title["freshness"]["notice"]
            .as_str()
            .expect("notice")
            .contains("30 Stunden")
    );

    for path in [
        "/api/discover/titles/movie/902",
        "/api/discover/search?q=neu",
        "/api/discover/people/7",
        "/api/discover/genres",
        "/api/discover/list?kind=series",
    ] {
        let (status, error) = http.get(path).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY, "{path}: {error}");
        assert!(
            error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("TMDB ist nicht erreichbar"),
            "{path}: {error}"
        );
    }
}

/// AC-15: without `[tmdb]` the discover page explains it; nothing else is offered.
#[tokio::test]
async fn without_a_tmdb_key_the_pages_say_what_is_missing() {
    let app = crate::testkit::app(Library::Movies, "").await;
    let http = Http::start(app).await;
    let (status, answer) = http.get("/api/discover/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer["available"], false);
    assert!(
        answer["message"]
            .as_str()
            .expect("message")
            .contains("[tmdb]")
    );
    for path in [
        "/api/discover/shelves",
        "/api/discover/search?q=x",
        "/api/discover/titles/movie/1",
        "/api/discover/people/1",
        "/api/library/movies/1/cast",
    ] {
        let (status, error) = http.get(path).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert!(
            error["error"]
                .as_str()
                .unwrap_or_default()
                .contains("sofarr.toml"),
            "{path}"
        );
    }
    let (status, _) = http
        .post(
            "/api/discover/add/movie",
            json!({"tmdb_id": 1, "quality_profile_id": 1, "root_folder": "/x", "monitored": true,
                   "minimum_availability": "released", "search": false}),
        )
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

/// Background detail loads stay at 40 TMDB requests per second at most.
#[tokio::test]
async fn background_loads_stay_within_forty_requests_per_second() {
    let providers = Providers::start().await;
    {
        let mut fake = providers.lock();
        let entries = (0..41)
            .map(|i| movie_entry(5000 + i, "x", "2020-01-01", 1.0))
            .collect();
        fake.lists
            .insert("discover/movie".to_owned(), page(entries, 1, 1));
        for i in 0..41 {
            fake.movies
                .insert(5000 + i, movie_detail(5000 + i, "x", "2020-01-01", None));
        }
    }
    let http = discover_app(&providers, "").await;
    let started = Instant::now();
    let (status, _) = http.get("/api/discover/list?kind=movie").await;
    assert_eq!(status, StatusCode::OK);
    settled(&http.app).await;
    let elapsed = started.elapsed();
    let loads = providers
        .lock()
        .requests
        .iter()
        .filter(|r| r.starts_with("movie/5"))
        .count();
    assert_eq!(loads, 41);
    // 41 requests at 40 per second: the last starts a full second after the first.
    assert!(
        elapsed >= std::time::Duration::from_millis(1000),
        "{elapsed:?}"
    );
}
