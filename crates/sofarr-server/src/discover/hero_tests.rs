//! The hero of the discover start page against a fake TMDB (spec ui-cinema-hero AC-01,
//! AC-04..AC-08).

use std::time::{Duration, Instant};

use axum::http::StatusCode;
use jiff::{SignedDuration, Timestamp};
use serde_json::{Value, json};

use crate::{
    testkit::{Http, Library},
    tmdb_fake::{Providers, movie_detail, movie_entry, page, ratings, series_entry, show_detail},
};

/// A trending entry (trending answers name the media type).
fn trending(mut entry: Value, media_type: &str) -> Value {
    entry["media_type"] = json!(media_type);
    entry
}

fn movie(id: i64, title: &str) -> Value {
    trending(movie_entry(id, title, "2024-05-01", 10.0), "movie")
}

/// A movie detail without a backdrop.
fn without_backdrop(id: i64, title: &str) -> Value {
    let mut detail = movie_detail(id, title, "2024-05-01", None);
    detail["backdrop_path"] = Value::Null;
    detail
}

async fn hero_app(providers: &Providers, extra: &str) -> Http {
    let app = providers.app(Library::Movies, extra).await;
    app.db
        .put_setting("exclusions", &json!([{"tmdbId": 900}]))
        .await
        .expect("exclusions");
    ratings(&app.db, &[("tt0000904", 7.8, 12345)]).await;
    Http::start(app).await
}

async fn shelves(http: &Http) -> Value {
    let (status, answer) = http.get("/api/discover/shelves").await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    answer
}

fn detail_requests(providers: &Providers) -> Vec<String> {
    providers
        .lock()
        .requests
        .iter()
        .filter(|r| r.starts_with("movie/") || r.starts_with("tv/"))
        .filter(|r| {
            r.split(['/', '?'])
                .nth(1)
                .is_some_and(|id| id.parse::<i64>().is_ok())
        })
        .cloned()
        .collect()
}

/// AC-01, AC-06, AC-08: the first title of „Heute beliebt“ that is neither in the library
/// nor excluded and whose preview has a backdrop; every field of today stays; the
/// freshness names the preview's time too; the stale cache still gives a hero.
#[tokio::test]
async fn the_hero_is_the_first_candidate_with_a_backdrop() {
    let providers = Providers::start().await;
    {
        let mut fake = providers.lock();
        fake.lists.insert(
            "trending/all/day".to_owned(),
            page(
                vec![
                    movie(101, "Erster Film"),
                    movie(900, "Ausgeschlossen"),
                    movie(902, "Ohne Hintergrund"),
                    movie(904, "Sternenfahrt"),
                    movie(905, "Zweite Wahl"),
                ],
                1,
                1,
            ),
        );
        fake.movies
            .insert(101, movie_detail(101, "Erster Film", "2001-01-01", None));
        fake.movies
            .insert(900, movie_detail(900, "Ausgeschlossen", "2002-01-01", None));
        fake.movies
            .insert(902, without_backdrop(902, "Ohne Hintergrund"));
        let mut star = movie_detail(904, "Sternenfahrt", "2024-05-01", Some("tt0000904"));
        star["overview"] = json!("Eine Reise zu den Sternen.");
        fake.movies.insert(904, star);
        fake.movies
            .insert(905, movie_detail(905, "Zweite Wahl", "2024-05-01", None));
    }
    let http = hero_app(&providers, "").await;

    let answer = shelves(&http).await;
    assert_eq!(
        answer["hero"],
        json!({
            "kind": "movie",
            "tmdb_id": 904,
            "title": "Sternenfahrt",
            "year": 2024,
            "overview": "Eine Reise zu den Sternen.",
            "backdrop": "/api/images/tmdb/w1280/b904.jpg",
            "rating": {"state": "rated", "rating": 7.8, "votes": 12345},
            "library": {"status": "none", "id": null}
        })
    );
    // Every field of today stays.
    assert_eq!(answer["shelves"].as_array().map(Vec::len), Some(7));
    assert_eq!(answer["shelves"][0]["items"][3]["tmdb_id"], 904);
    assert_eq!(answer["freshness"]["stale"], false);
    // Only candidates are looked at: neither the library title nor the excluded one.
    let asked = detail_requests(&providers);
    assert!(
        !asked
            .iter()
            .any(|r| r.starts_with("movie/101?") || r.starts_with("movie/900?")),
        "{asked:?}"
    );

    // AC-08: the lists are fetched anew, the preview is an hour old: the line names it.
    http.app.discover.lists.age(SignedDuration::from_hours(7));
    http.app.discover.titles.age(SignedDuration::from_hours(1));
    let answer = shelves(&http).await;
    let fetched: Timestamp = answer["freshness"]["fetched_at"]
        .as_str()
        .expect("fetched_at")
        .parse()
        .expect("time");
    let age = Timestamp::now().duration_since(fetched);
    assert!(age >= SignedDuration::from_mins(59), "{age:?}");

    // TMDB down, everything stale: the hero comes from the cache, with the notice.
    providers.lock().tmdb_down = true;
    http.app.discover.lists.age(SignedDuration::from_hours(7));
    http.app.discover.titles.age(SignedDuration::from_hours(30));
    let answer = shelves(&http).await;
    assert_eq!(answer["hero"]["tmdb_id"], 904);
    assert_eq!(answer["freshness"]["stale"], true);
}

/// AC-01, AC-07: at most the first 10 candidates are looked at, their previews asked for
/// at once; with all previews cached TMDB is not asked for the hero; AC-04: none of them
/// with a backdrop gives no hero.
#[tokio::test]
async fn ten_candidates_at_most_asked_for_at_once() {
    let providers = Providers::start().await;
    let delay = Duration::from_millis(400);
    {
        let mut fake = providers.lock();
        let entries = (1..=12)
            .map(|n| movie(2000 + n, &format!("Film {n}")))
            .collect();
        fake.lists
            .insert("trending/all/day".to_owned(), page(entries, 1, 1));
        for n in 1..=12 {
            let id = 2000 + n;
            let detail = if n == 11 {
                movie_detail(id, "Elf", "2024-05-01", None)
            } else {
                without_backdrop(id, &format!("Film {n}"))
            };
            fake.movies.insert(id, detail);
        }
        fake.detail_delay = Some(delay);
    }
    // Without ratings no background load asks for details.
    let http = hero_app(&providers, "[imdb]\nenabled = false\n").await;
    let started = Instant::now();
    let answer = shelves(&http).await;
    let elapsed = started.elapsed();
    assert_eq!(answer["hero"], Value::Null, "{}", answer["hero"]);
    assert_eq!(
        answer["shelves"][0]["items"].as_array().map(Vec::len),
        Some(12)
    );
    let asked = detail_requests(&providers);
    assert_eq!(asked.len(), 10, "{asked:?}");
    assert!(
        !asked
            .iter()
            .any(|r| r.starts_with("movie/2011?") || r.starts_with("movie/2012?"))
    );
    // At once: ten requests of 400 ms one after the other would take 4 s.
    assert!(elapsed < delay * 3, "{elapsed:?}");

    // All previews cached: TMDB is not asked again for the hero.
    shelves(&http).await;
    assert_eq!(detail_requests(&providers).len(), 10);
}

/// AC-07: a candidate whose preview cannot be loaded is skipped.
#[tokio::test]
async fn a_preview_that_cannot_be_loaded_is_skipped() {
    let providers = Providers::start().await;
    {
        let mut fake = providers.lock();
        fake.lists.insert(
            "trending/all/day".to_owned(),
            page(
                vec![
                    movie(911, "Kaputt"),
                    movie(912, "Unbekannt"),
                    trending(series_entry(913, "Serie", "2023-01-01", 5.0), "tv"),
                ],
                1,
                1,
            ),
        );
        fake.movies
            .insert(911, movie_detail(911, "Kaputt", "2024-01-01", None));
        fake.broken.insert("movie/911".to_owned());
        // 912: TMDB knows no detail.
        let mut show = show_detail(913, "Serie", Some(81_300), None, 1);
        show["backdrop_path"] = json!("/b913.jpg");
        fake.shows.insert(913, show);
    }
    let http = hero_app(&providers, "").await;
    let answer = shelves(&http).await;
    assert_eq!(answer["hero"]["kind"], "series");
    assert_eq!(answer["hero"]["tmdb_id"], 913);
    assert_eq!(answer["hero"]["year"], 2020);
    assert_eq!(answer["hero"]["overview"], "Eine Serie.");
    assert_eq!(
        answer["hero"]["backdrop"],
        "/api/images/tmdb/w1280/b913.jpg"
    );
}

/// AC-04: no hero while „Heute beliebt“ cannot be loaded or has no candidate; the
/// shelves stand as today.
#[tokio::test]
async fn no_hero_without_trending_or_without_candidates() {
    let providers = Providers::start().await;
    {
        let mut fake = providers.lock();
        fake.lists.insert(
            "trending/all/day".to_owned(),
            page(
                vec![movie(101, "Erster Film"), movie(900, "Ausgeschlossen")],
                1,
                1,
            ),
        );
        fake.lists.insert(
            "movie/popular".to_owned(),
            page(
                vec![movie_entry(904, "Sternenfahrt", "2024-05-01", 1.0)],
                1,
                1,
            ),
        );
        fake.movies
            .insert(101, movie_detail(101, "Erster Film", "2001-01-01", None));
        fake.movies
            .insert(900, movie_detail(900, "Ausgeschlossen", "2002-01-01", None));
        fake.movies
            .insert(904, movie_detail(904, "Sternenfahrt", "2024-05-01", None));
    }
    let http = hero_app(&providers, "").await;
    let answer = shelves(&http).await;
    assert_eq!(answer["hero"], Value::Null);
    assert_eq!(
        answer["shelves"][0]["items"].as_array().map(Vec::len),
        Some(2)
    );

    // „Heute beliebt“ cannot be loaded, nothing of it is cached.
    let broken = Providers::start().await;
    broken.lock().broken.insert("trending/all/day".to_owned());
    broken.lock().lists.insert(
        "movie/popular".to_owned(),
        page(
            vec![movie_entry(904, "Sternenfahrt", "2024-05-01", 1.0)],
            1,
            1,
        ),
    );
    let http = hero_app(&broken, "").await;
    let answer = shelves(&http).await;
    assert_eq!(answer["hero"], Value::Null);
    assert!(
        answer["shelves"][0]["error"].is_string(),
        "{}",
        answer["shelves"][0]
    );
    assert_eq!(answer["shelves"][2]["items"][0]["tmdb_id"], 904);
}

/// AC-05: once the featured title is in the library, the next answer chooses again.
#[tokio::test]
async fn an_added_hero_gives_way_to_the_next_candidate() {
    let providers = Providers::start().await;
    {
        let mut fake = providers.lock();
        fake.lists.insert(
            "trending/all/day".to_owned(),
            page(
                vec![movie(904, "Sternenfahrt"), movie(905, "Zweite Wahl")],
                1,
                1,
            ),
        );
        fake.movies.insert(
            904,
            movie_detail(904, "Sternenfahrt", "2024-05-01", Some("tt0000904")),
        );
        fake.movies
            .insert(905, movie_detail(905, "Zweite Wahl", "2024-05-01", None));
    }
    let http = hero_app(&providers, "").await;
    let root = std::env::temp_dir().join(format!("sofarr-hero-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("root");
    let root = root.to_string_lossy().into_owned();
    http.app
        .db
        .put_setting("rootfolder", &json!([{"path": root}]))
        .await
        .expect("roots");
    assert_eq!(shelves(&http).await["hero"]["tmdb_id"], 904);

    let (status, added) = http
        .post(
            "/api/discover/add/movie",
            json!({"tmdb_id": 904, "quality_profile_id": 1, "root_folder": root,
                   "monitored": true, "minimum_availability": "released", "tags": [],
                   "search": false}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{added}");
    let answer = shelves(&http).await;
    assert_eq!(answer["hero"]["tmdb_id"], 905);
    assert_eq!(
        answer["shelves"][0]["items"][0]["library"]["status"],
        "in_library"
    );
    // An excluded title is no candidate either.
    http.app
        .db
        .put_setting("exclusions", &json!([{"tmdbId": 905}]))
        .await
        .expect("exclusions");
    assert_eq!(shelves(&http).await["hero"], Value::Null);
}
