//! Monitoring, profile and availability changes, the missing page and its searches
//! (spec web-ui-library AC-08 to AC-11).

use reqwest::{Method, StatusCode};
use serde_json::{Value, json};

use crate::{
    app::App,
    testkit::{Http, Library, Services, app},
};

async fn movie_id(app: &App, tmdb: i64) -> i64 {
    app.db
        .movie_by_tmdb(tmdb)
        .await
        .expect("db")
        .expect("movie")
        .id
}

fn labels(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|i| i["label"].as_str().unwrap_or_default().to_owned())
        .collect()
}

/// Profiles: 1 "HD" (cutoff Bluray-1080p), 2 "Web" (cutoff WEBDL-1080p).
async fn two_profiles(app: &App) {
    app.db
        .put_setting(
            "qualityprofile",
            &json!([
                {"id": 1, "name": "HD", "upgradeAllowed": true, "cutoff": 7,
                 "items": [
                    {"quality": {"id": 3, "name": "WEBDL-1080p"}, "allowed": true},
                    {"quality": {"id": 7, "name": "Bluray-1080p"}, "allowed": true}]},
                {"id": 2, "name": "Web", "upgradeAllowed": true, "cutoff": 3,
                 "items": [
                    {"quality": {"id": 3, "name": "WEBDL-1080p"}, "allowed": true},
                    {"quality": {"id": 7, "name": "Bluray-1080p"}, "allowed": true}]}
            ]),
        )
        .await
        .expect("profiles");
}

#[tokio::test]
async fn monitoring_one_or_many_items_is_saved_at_once_or_not_at_all() {
    let app = app(Library::Movies, "").await;
    let first = movie_id(&app, 101).await;
    let second = movie_id(&app, 102).await;
    let http = Http::start(app).await;
    // The list is computed (and kept) before the change.
    let (_, before) = http.get("/api/library/movies?monitored=no").await;
    assert_eq!(before["total"], 1);
    let (status, answer) = http
        .post(
            "/api/library/monitor",
            json!({"monitored": false, "items": [{"kind": "movie", "id": first}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["changed"], 1);
    let (_, after) = http.get("/api/library/movies?monitored=no").await;
    assert_eq!(after["total"], 2, "the list follows the change");
    // One unknown item: nothing changes.
    let (status, refused) = http
        .post(
            "/api/library/monitor",
            json!({"monitored": false, "items": [
                {"kind": "movie", "id": second}, {"kind": "movie", "id": 9999}]}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        refused["error"],
        "Film 9999 ist nicht in der Bibliothek – nichts wurde geändert"
    );
    assert!(
        http.app
            .db
            .movie(second)
            .await
            .expect("db")
            .expect("movie")
            .monitored
    );
    for bad in [
        json!({"monitored": true, "items": []}),
        json!({"monitored": true, "items": [{"kind": "film", "id": 1}]}),
        json!({"items": [{"kind": "movie", "id": first}]}),
    ] {
        let (status, body) = http.post("/api/library/monitor", bad.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
        assert!(
            body["error"]
                .as_str()
                .is_some_and(|e| e.starts_with("Ungültige"))
        );
    }
}

#[tokio::test]
async fn switching_a_series_or_season_switches_its_episodes() {
    let app = app(Library::Series, "").await;
    let show = app.db.all_series().await.expect("db")[0].id;
    let http = Http::start(app).await;
    let (status, _) = http
        .post(
            "/api/library/monitor",
            json!({"monitored": false, "items": [{"kind": "season", "series_id": show, "season": 1}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let eps = http.app.db.episodes(Some(show)).await.expect("eps");
    assert!(eps.iter().all(|e| !e.data.monitored));
    let (_, detail) = http.get(&format!("/api/library/series/{show}")).await;
    assert_eq!(detail["seasons"][0]["monitored"], false);
    assert_eq!(detail["missing"], 0, "unmonitored episodes are not missing");
    http.post(
        "/api/library/monitor",
        json!({"monitored": true, "items": [{"kind": "series", "id": show}]}),
    )
    .await;
    let eps = http.app.db.episodes(Some(show)).await.expect("eps");
    assert!(eps.iter().all(|e| e.data.monitored));
    let ids: Vec<Value> = eps
        .iter()
        .map(|e| json!({"kind": "episode", "id": e.id}))
        .collect();
    let (status, answer) = http
        .post(
            "/api/library/monitor",
            json!({"monitored": false, "items": ids}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer["changed"], 3);
}

#[tokio::test]
async fn a_profile_change_shows_what_it_does_and_applies_to_the_next_decision() {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[s.item(
            "Zweiter.Film.2002.German.1080p.BluRay.x264-GRP",
            "z1",
            8_000_000_000,
        )],
    );
    let app = app(Library::Movies, &s.indexer_toml("idx", &[])).await;
    two_profiles(&app).await;
    let second = movie_id(&app, 102).await;
    let http = Http::start(app).await;
    let path = format!("/api/library/movies/{second}");
    let (status, preview) = http
        .post(
            &format!("{path}/change-preview"),
            json!({"quality_profile_id": 2}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(
        preview["no_longer_cutoff_unmet"],
        json!([{"kind": "movie", "id": second, "label": "Zweiter Film (2002)"}])
    );
    assert_eq!(preview["becomes_missing"], json!([]));
    let (status, detail) = http
        .send(Method::PATCH, &path, Some(json!({"quality_profile_id": 2})))
        .await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["quality_profile"], json!({"id": 2, "name": "Web"}));
    assert_eq!(detail["cutoff_unmet"], false);
    // The next decision uses profile 2: the Bluray is no upgrade any more.
    let report = http.app.rss_sync().await.expect("rss");
    assert_eq!(report.approved, 0);
    let (_, back) = http
        .post(
            &format!("{path}/change-preview"),
            json!({"quality_profile_id": 1}),
        )
        .await;
    assert_eq!(back["becomes_cutoff_unmet"][0]["id"], second);
    http.send(Method::PATCH, &path, Some(json!({"quality_profile_id": 1})))
        .await;
    let report = http.app.rss_sync().await.expect("rss");
    assert_eq!(report.approved, 1, "profile 1 wants the Bluray");

    for bad in [
        json!({"quality_profile_id": 99}),
        json!({"minimum_availability": "bald"}),
        json!({}),
        json!({"tags": [1]}),
    ] {
        let (status, body) = http.send(Method::PATCH, &path, Some(bad.clone())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}: {body}");
    }
}

#[tokio::test]
async fn an_availability_change_shows_movies_that_become_missing() {
    let app = app(Library::Movies, "").await;
    let first = movie_id(&app, 101).await;
    sqlx::query(
        "UPDATE movies SET in_cinemas = '2020-01-01', digital_release = '2099-01-01' WHERE id = ?",
    )
    .bind(first)
    .execute(app.db.pool())
    .await
    .expect("dates");
    let http = Http::start(app).await;
    let path = format!("/api/library/movies/{first}");
    let (_, detail) = http.get(&path).await;
    assert_eq!(detail["missing"], false, "not available before 2099");
    assert_eq!(detail["stage"], "in_cinemas");
    let (_, preview) = http
        .post(
            &format!("{path}/change-preview"),
            json!({"minimum_availability": "inCinemas"}),
        )
        .await;
    assert_eq!(preview["becomes_missing"][0]["label"], "Erster Film (2001)");
    let (status, detail) = http
        .send(
            Method::PATCH,
            &path,
            Some(json!({"minimum_availability": "inCinemas"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["missing"], true);
    assert_eq!(detail["minimum_availability"], "inCinemas");
}

#[tokio::test]
async fn a_series_profile_change_previews_its_episodes() {
    let app = app(Library::Series, "").await;
    let show = app.db.all_series().await.expect("db")[0].id;
    app.db
        .put_setting(
            "sonarr_qualityprofile",
            &json!([
                {"id": 1, "name": "HD", "upgradeAllowed": true, "cutoff": 7,
                 "items": [{"quality": {"id": 3, "name": "WEBDL-1080p"}, "allowed": true},
                           {"quality": {"id": 7, "name": "Bluray-1080p"}, "allowed": true}]},
                {"id": 2, "name": "Web", "upgradeAllowed": true, "cutoff": 3,
                 "items": [{"quality": {"id": 3, "name": "WEBDL-1080p"}, "allowed": true}]}
            ]),
        )
        .await
        .expect("profiles");
    let http = Http::start(app).await;
    let (status, preview) = http
        .post(
            &format!("/api/library/series/{show}/change-preview"),
            json!({"quality_profile_id": 2}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(
        preview["no_longer_cutoff_unmet"][0]["label"],
        "Kleine Insel S01E01 – Ankunft"
    );
    let (status, detail) = http
        .send(
            Method::PATCH,
            &format!("/api/library/series/{show}"),
            Some(json!({"quality_profile_id": 2})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["cutoff_unmet"], 0);
    let (status, _) = http
        .send(
            Method::PATCH,
            &format!("/api/library/series/{show}"),
            Some(json!({"minimum_availability": "released"})),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "series have no availability"
    );
}

#[tokio::test]
async fn missing_page_lists_both_tabs_with_reasons() {
    let http = Http::start(app(Library::Movies, "").await).await;
    let (status, missing) = http.get("/api/library/wanted?list=missing").await;
    assert_eq!(status, StatusCode::OK, "{missing}");
    assert_eq!(labels(&missing), ["Erster Film (2001)"]);
    assert_eq!(missing["items"][0]["reason"], "fehlt");
    assert_eq!(missing["items"][0]["profile"], "HD");
    let (_, cutoff) = http.get("/api/library/wanted?list=cutoff_unmet").await;
    assert_eq!(labels(&cutoff), ["Zweiter Film (2002)"]);
    assert_eq!(
        cutoff["items"][0]["reason"],
        "WEBDL-1080p statt Bluray-1080p"
    );
    let (status, _) = http.get("/api/library/wanted?list=alles").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let http = Http::start(app(Library::Series, "").await).await;
    let (_, episodes) = http
        .get("/api/library/wanted?list=missing&media=episodes")
        .await;
    assert_eq!(labels(&episodes), ["Kleine Insel S01E02 – Sturm"]);
    assert_eq!(episodes["items"][0]["season"], 1);
    assert_eq!(episodes["items"][0]["episode"], 2);
}

#[tokio::test]
async fn a_tab_is_searched_as_one_request() {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[s.item(
            "Erster.Film.2001.German.1080p.BluRay.x264-GRP",
            "e1",
            8_000_000_000,
        )],
    );
    let http = Http::start(app(Library::Movies, &s.indexer_toml("idx", &[])).await).await;
    let (status, report) = http
        .post(
            "/api/command/search-bulk",
            json!({"all": {"list": "missing", "media": "movies"}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["searched"], 1);
    assert_eq!(report["grabs"], 1);
    let first = movie_id(&http.app, 101).await;
    let (status, report) = http
        .post(
            "/api/command/search-bulk",
            json!({"items": [{"movie_id": first}, {"movie_id": 9999}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["searched"], 1);
    assert_eq!(
        report["errors"],
        json!(["Film 9999 ist nicht in der Bibliothek"])
    );
    for bad in [
        json!({}),
        json!({"items": []}),
        json!({"items": [{"season": 1}]}),
        json!({"items": [{"movie_id": first}], "all": {"list": "missing", "media": "movies"}}),
    ] {
        let (status, _) = http.post("/api/command/search-bulk", bad.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    }
}

#[tokio::test]
async fn all_missing_episodes_are_searched_by_series() {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[s.item(
            "Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP",
            "k2",
            2_000_000_000,
        )],
    );
    let http = Http::start(app(Library::Series, &s.indexer_toml("idx", &[])).await).await;
    let (status, report) = http
        .post(
            "/api/command/search-bulk",
            json!({"all": {"list": "missing", "media": "episodes"}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["searched"], 1);
    assert_eq!(report["grabs"], 1);
}
