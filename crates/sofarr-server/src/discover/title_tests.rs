//! Title previews, person pages, the cast of library titles and TMDB images against a
//! fake TMDB (spec discover AC-04..AC-06, AC-16, AC-17).

use axum::http::StatusCode;
use serde_json::json;

use crate::{
    testkit::{Http, Library},
    tmdb_fake::{Providers, movie_detail, ratings, settled, show_detail},
};

/// AC-04, AC-16, AC-17: a title outside the library shows its preview; one in the
/// library carries its library id.
#[tokio::test]
async fn previews_show_the_title_and_say_whether_it_is_in_the_library() {
    let providers = Providers::start().await;
    {
        let mut fake = providers.lock();
        fake.movies.insert(
            901,
            movie_detail(901, "Der Garten", "2003-05-01", Some("tt0000901")),
        );
        fake.movies
            .insert(101, movie_detail(101, "Erster Film", "2001-01-01", None));
        let mut show = show_detail(1399, "Kleine Insel", Some(88), Some("tt0000555"), 2);
        // No German overview: the English one (AC-16).
        show["overview"] = json!("");
        show["translations"] = json!({"translations": [
            {"iso_639_1": "en", "data": {"overview": "A small island."}}]});
        fake.shows.insert(1399, show);
    }
    let app = providers.app(Library::Movies, "").await;
    ratings(&app.db, &[("tt0000901", 7.5, 1200)]).await;
    let http = Http::start(app).await;

    let (status, movie) = http.get("/api/discover/titles/movie/901").await;
    assert_eq!(status, StatusCode::OK, "{movie}");
    assert_eq!(movie["title"], "Der Garten");
    assert_eq!(movie["year"], 2003);
    assert_eq!(movie["genres"], json!(["Drama"]));
    assert_eq!(movie["runtime_minutes"], 100);
    assert_eq!(movie["overview"], "Ein Film.");
    assert_eq!(movie["poster"], "/api/images/tmdb/w780/m901.jpg");
    assert_eq!(movie["backdrop"], "/api/images/tmdb/w1280/b901.jpg");
    assert_eq!(movie["rating"]["state"], "rated");
    assert_eq!(movie["cast"].as_array().expect("cast").len(), 12);
    assert_eq!(movie["cast"][0]["photo"], "/api/images/tmdb/w185/p0.jpg");
    assert_eq!(movie["crew"][0]["job"], "Director");
    assert_eq!(movie["trailer"], "https://www.youtube.com/watch?v=abc");
    assert_eq!(movie["library"], json!({"status": "none", "id": null}));
    assert!(
        providers.requests_with("movie/901")[0]
            .contains("append_to_response=credits%2Cexternal_ids%2Cvideos")
    );

    let (status, known) = http.get("/api/discover/titles/movie/101").await;
    assert_eq!(status, StatusCode::OK, "{known}");
    assert_eq!(known["library"], json!({"status": "in_library", "id": 1}));

    let (status, show) = http.get("/api/discover/titles/series/1399").await;
    assert_eq!(status, StatusCode::OK, "{show}");
    assert_eq!(show["overview"], "A small island.");
    assert_eq!(show["seasons"].as_array().expect("seasons").len(), 3);
    assert_eq!(show["runtime_minutes"], 45);

    let (status, error) = http.get("/api/discover/titles/movie/404").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        error["error"]
            .as_str()
            .expect("error")
            .contains("TMDB kennt")
    );
    let (status, _) = http.get("/api/discover/titles/film/1").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// AC-05: every movie and series of a person, cast and crew departments apart, each
/// with its library state and rating.
#[tokio::test]
async fn person_pages_list_cast_and_crew_departments_with_library_state() {
    let providers = Providers::start().await;
    {
        let mut fake = providers.lock();
        fake.people.insert(
            7,
            json!({
                "id": 7, "name": "Erika Erst", "biography": "Schauspielerin.",
                "birthday": "1970-02-03", "deathday": null, "place_of_birth": "Berlin",
                "profile_path": "/erika.jpg", "known_for_department": "Acting",
                "combined_credits": {
                    "cast": [
                        {"media_type": "movie", "id": 101, "title": "Erster Film", "release_date": "2001-01-01", "character": "Anna"},
                        {"media_type": "movie", "id": 900, "title": "Ausgeschlossen", "release_date": "2010-01-01"},
                        {"media_type": "tv", "id": 55, "name": "Erste Serie", "first_air_date": "2015-01-01"},
                        {"media_type": "movie", "id": 77, "title": "Ohne Datum"}
                    ],
                    "crew": [
                        {"media_type": "movie", "id": 901, "title": "Der Garten", "release_date": "2003-01-01",
                         "department": "Directing", "job": "Director"},
                        {"media_type": "movie", "id": 902, "title": "Die Mauer", "release_date": "2008-01-01",
                         "department": "Writing", "job": "Screenplay"}
                    ]
                }
            }),
        );
        fake.movies.insert(
            901,
            movie_detail(901, "Der Garten", "2003-01-01", Some("tt0000901")),
        );
    }
    let app = providers.app(Library::Movies, "").await;
    app.db
        .put_setting("exclusions", &json!([{"tmdbId": 900}]))
        .await
        .expect("exclusions");
    ratings(&app.db, &[("tt0000901", 7.5, 1200)]).await;
    let http = Http::start(app).await;

    let (status, person) = http.get("/api/discover/people/7").await;
    assert_eq!(status, StatusCode::OK, "{person}");
    assert_eq!(person["name"], "Erika Erst");
    assert_eq!(person["photo"], "/api/images/tmdb/h632/erika.jpg");
    assert_eq!(person["birthday"], "1970-02-03");
    let cast: Vec<&str> = person["cast"]
        .as_array()
        .expect("cast")
        .iter()
        .filter_map(|c| c["title"]["title"].as_str())
        .collect();
    // Newest first, undated last.
    assert_eq!(
        cast,
        ["Erste Serie", "Ausgeschlossen", "Erster Film", "Ohne Datum"]
    );
    assert_eq!(person["cast"][0]["title"]["kind"], "series");
    assert_eq!(person["cast"][1]["title"]["library"]["status"], "excluded");
    assert_eq!(
        person["cast"][2]["title"]["library"]["status"],
        "in_library"
    );
    assert_eq!(person["cast"][2]["character"], "Anna");
    let departments: Vec<&str> = person["crew"]
        .as_array()
        .expect("crew")
        .iter()
        .filter_map(|d| d["department"].as_str())
        .collect();
    assert_eq!(departments, ["Directing", "Writing"]);
    assert_eq!(person["crew"][0]["credits"][0]["job"], "Director");

    // Ratings appear once their detail is loaded.
    settled(&http.app).await;
    let (_, again) = http.get("/api/discover/people/7").await;
    assert_eq!(
        again["crew"][0]["credits"][0]["title"]["rating"]["rating"],
        7.5
    );
}

/// AC-06: library pages get the first 10 billed cast members; a series without TMDB id
/// gets none.
#[tokio::test]
async fn library_titles_get_their_first_ten_cast_members() {
    let providers = Providers::start().await;
    providers
        .lock()
        .movies
        .insert(101, movie_detail(101, "Erster Film", "2001-01-01", None));
    let http = Http::start(providers.app(Library::Movies, "").await).await;
    let (status, cast) = http.get("/api/library/movies/1/cast").await;
    assert_eq!(status, StatusCode::OK, "{cast}");
    let members = cast["cast"].as_array().expect("cast");
    assert_eq!(members.len(), 10);
    assert_eq!(members[0]["person_id"], 9000);
    assert_eq!(members[0]["character"], "Rolle 0");
    assert_eq!(members[9]["name"], "Person 9");
    assert!(cast["freshness"]["fetched_at"].is_string());
    let (status, _) = http.get("/api/library/movies/999/cast").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let series = Providers::start().await;
    series
        .lock()
        .shows
        .insert(1399, show_detail(1399, "Kleine Insel", Some(77), None, 1));
    let app = series.app(Library::Series, "").await;
    let id = app.db.all_series().await.expect("series")[0].id;
    let http = Http::start(app).await;
    let (status, none) = http.get(&format!("/api/library/series/{id}/cast")).await;
    assert_eq!(status, StatusCode::OK, "{none}");
    assert_eq!(none, json!({"cast": [], "freshness": null}));
    http.app
        .db
        .set_series_tmdb_id(id, 1399)
        .await
        .expect("tmdb");
    let (_, known) = http.get(&format!("/api/library/series/{id}/cast")).await;
    assert_eq!(known["cast"][0]["name"], "Ada Actor");
}

/// AC-17: TMDB images come through sofarr's image cache.
#[tokio::test]
async fn tmdb_images_are_loaded_once_and_served_from_the_cache() {
    let providers = Providers::start().await;
    let root = std::env::temp_dir().join(format!("sofarr-tmdb-img-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let mut app = providers.app(Library::Movies, "").await;
    app.images.cache = root.join("cache");
    let http = Http::start(app).await;
    for _ in 0..2 {
        let response = http
            .client
            .get(format!("{}/api/images/tmdb/w342/abc.jpg", http.base))
            .send()
            .await
            .expect("image");
        assert_eq!(response.status().as_u16(), 200);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok()),
            Some("image/jpeg")
        );
        assert_eq!(response.bytes().await.expect("bytes").as_ref(), b"image");
    }
    assert_eq!(providers.lock().images, ["/w342/abc.jpg"]);
    let (status, _) = http.get("/api/images/tmdb/w9999/abc.jpg").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = http.get("/api/images/tmdb/w342/..%2Fx.jpg").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // The library posters keep their route.
    let (status, _) = http.get("/api/images/movie/1/list").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(&root);
}
