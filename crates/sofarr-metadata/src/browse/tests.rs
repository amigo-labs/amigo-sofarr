use std::sync::{Arc, Mutex};

use axum::{Json, Router, extract::RawQuery, routing::get};
use serde_json::json;

use super::*;

#[test]
fn list_entries_of_movies_series_and_people() {
    let page = titles_from_json(
        &json!({"page": 1, "total_pages": 3, "results": [
            {"media_type": "movie", "id": 1, "title": "Der Garten", "original_title": "The Garden",
             "release_date": "2024-05-02", "poster_path": "/g.jpg", "popularity": 12.5,
             "genre_ids": [18], "original_language": "en"},
            {"media_type": "tv", "id": 2, "name": "", "original_name": "Ostrov",
             "first_air_date": "", "popularity": 3.0},
            {"media_type": "person", "id": 3, "name": "Jemand"}
        ]}),
        None,
    );
    assert!(page.has_more());
    assert_eq!(page.results.len(), 2);
    let movie = &page.results[0];
    assert_eq!(
        (movie.kind, movie.title.as_str(), movie.year),
        (Kind::Movie, "Der Garten", Some(2024))
    );
    assert_eq!(movie.genre_ids, [18]);
    // An empty German title falls back to the original one (AC-16).
    let series = &page.results[1];
    assert_eq!(
        (series.kind, series.title.as_str(), series.year),
        (Kind::Series, "Ostrov", None)
    );

    let hits = search_from_json(&json!({"page": 1, "total_pages": 1, "results": [
        {"media_type": "person", "id": 9, "name": "Ada Actor", "known_for_department": "Acting",
         "known_for": [{"title": "Der Garten"}, {"name": "Ostrov"}]},
        {"media_type": "movie", "id": 1, "title": "Der Garten"}
    ]}));
    assert!(!hits.has_more());
    assert!(matches!(&hits.results[0], SearchHit::Person(p)
        if p.name == "Ada Actor" && p.known_for == ["Der Garten", "Ostrov"]));
    assert!(matches!(&hits.results[1], SearchHit::Title(t) if t.tmdb_id == 1));
}

#[test]
fn details_fall_back_to_original_texts_and_find_ids_and_the_trailer() {
    let detail = detail_from_json(
        &json!({
            "id": 1399, "name": "Kleine Insel", "original_name": "Small Island",
            "first_air_date": "2019-01-01", "overview": "", "original_language": "en",
            "genres": [{"id": 18, "name": "Drama"}], "episode_run_time": [45],
            "seasons": [{"season_number": 0, "name": "Specials", "episode_count": 2},
                        {"season_number": 1, "name": "Staffel 1", "episode_count": 8, "air_date": "2019-01-01"}],
            "external_ids": {"imdb_id": "tt1000001", "tvdb_id": 77},
            "credits": {"cast": [{"id": 5, "name": "Ada Actor", "character": "Ada"}],
                        "crew": [{"id": 6, "name": "Dora Director", "job": "Director", "department": "Directing"}]},
            "videos": {"results": [
                {"site": "YouTube", "type": "Teaser", "key": "teaser", "iso_639_1": "de"},
                {"site": "YouTube", "type": "Trailer", "key": "english", "iso_639_1": "en"},
                {"site": "YouTube", "type": "Trailer", "key": "deutsch", "iso_639_1": "de"}
            ]},
            "translations": {"translations": [
                {"iso_639_1": "en", "data": {"name": "Small Island", "overview": "An island."}}
            ]}
        }),
        Kind::Series,
    )
    .expect("detail");
    assert_eq!(detail.overview.as_deref(), Some("An island."));
    assert_eq!(detail.imdb_id.as_deref(), Some("tt1000001"));
    assert_eq!(detail.tvdb_id, Some(77));
    assert_eq!(detail.runtime_minutes, Some(45));
    assert_eq!(detail.seasons.len(), 2);
    assert_eq!(detail.summary.genre_ids, [18]);
    assert_eq!(detail.genres, ["Drama"]);
    assert_eq!(
        detail.trailer.as_deref(),
        Some("https://www.youtube.com/watch?v=deutsch")
    );
    assert_eq!(detail.cast[0].character.as_deref(), Some("Ada"));
    assert_eq!(detail.crew[0].department, "Directing");
}

/// AC-16: without a German text a title shows its original one, not the English one.
#[test]
fn a_title_without_german_texts_falls_back_to_its_original_language() {
    let detail = detail_from_json(
        &json!({
            "id": 496243, "title": "", "original_title": "기생충",
            "release_date": "2019-05-30", "overview": "", "original_language": "ko",
            "translations": {"translations": [
                {"iso_639_1": "en", "data": {"title": "Parasite", "overview": "A poor family."}},
                {"iso_639_1": "ko", "data": {"overview": "가난한 가족."}}
            ]}
        }),
        Kind::Movie,
    )
    .expect("detail");
    assert_eq!(detail.summary.title, "기생충");
    assert_eq!(detail.overview.as_deref(), Some("가난한 가족."));
}

#[test]
fn person_credits_once_as_cast_and_once_per_department() {
    let person = person_detail_from_json(&json!({
        "id": 9, "name": "Ada Actor", "biography": "", "birthday": "1970-02-03",
        "translations": {"translations": [{"iso_639_1": "en", "data": {"biography": "Born."}}]},
        "combined_credits": {
            "cast": [{"media_type": "movie", "id": 1, "title": "A", "character": "X"},
                     {"media_type": "movie", "id": 1, "title": "A", "character": "Y"}],
            "crew": [{"media_type": "tv", "id": 2, "name": "B", "department": "Writing", "job": "Writer"},
                     {"media_type": "tv", "id": 2, "name": "B", "department": "Writing", "job": "Story"},
                     {"media_type": "tv", "id": 2, "name": "B", "department": "Directing", "job": "Director"}]
        }
    }))
    .expect("person");
    assert_eq!(person.biography.as_deref(), Some("Born."));
    assert_eq!(person.credits.len(), 3);
    assert_eq!(
        person.credits[1].role,
        Role::Crew {
            department: "Writing".to_owned(),
            job: "Writer, Story".to_owned()
        }
    );
}

/// AC-16: every browse request asks for German texts, the movie lists for German
/// release dates.
#[tokio::test]
async fn requests_ask_for_german_texts_and_german_releases() {
    let seen: Arc<Mutex<Vec<String>>> = Arc::default();
    let record = |seen: Arc<Mutex<Vec<String>>>, path: &'static str| {
        get(move |RawQuery(q): RawQuery| async move {
            seen.lock()
                .expect("seen")
                .push(format!("{path}?{}", q.unwrap_or_default()));
            Json(json!({"page": 1, "total_pages": 1, "results": [], "genres": []}))
        })
    };
    let app = Router::new()
        .route("/3/movie/upcoming", record(Arc::clone(&seen), "upcoming"))
        .route("/3/discover/movie", record(Arc::clone(&seen), "discover"))
        .route("/3/search/multi", record(Arc::clone(&seen), "search"))
        .route("/3/genre/tv/list", record(Arc::clone(&seen), "genres"));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move { axum::serve(listener, app).await });
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client");
    let tmdb = Tmdb::with_base(client, "k", &format!("http://{addr}/3"));
    tmdb.movie_list(MovieList::Upcoming, 1).await.expect("list");
    tmdb.new_at_home("2026-10-01", "2026-11-12", 2)
        .await
        .expect("new");
    tmdb.search("garten", 1).await.expect("search");
    tmdb.genres(Kind::Series).await.expect("genres");
    let seen = seen.lock().expect("seen").clone();
    assert_eq!(seen.len(), 4);
    assert!(
        seen.iter().all(|q| q.contains("language=de-DE")),
        "{seen:?}"
    );
    assert!(seen[0].contains("region=DE"), "{}", seen[0]);
    assert!(
        seen[1].contains("with_release_type=4%7C5") && seen[1].contains("page=2"),
        "{}",
        seen[1]
    );
    assert!(seen[2].contains("query=garten"), "{}", seen[2]);
}
