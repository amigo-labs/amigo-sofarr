//! The trailer API (spec trailers AC-09, AC-10, AC-12): state per item and the manual
//! download, over HTTP.

use reqwest::StatusCode;

use super::*;

impl Setup {
    /// Serves the API routes; returns the base URL.
    async fn serve(&self) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let routes = crate::api::routes(Arc::clone(&self.app));
        tokio::spawn(async move { axum::serve(listener, routes).await });
        format!("http://{addr}")
    }
}

async fn call(method: reqwest::Method, url: String) -> (StatusCode, Value) {
    let response = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client")
        .request(method, url)
        .send()
        .await
        .expect("request");
    let status = response.status();
    (status, response.json().await.expect("json"))
}

async fn get(base: &str, path: &str) -> Value {
    let (status, body) = call(reqwest::Method::GET, format!("{base}{path}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn post(base: &str, path: &str) -> (StatusCode, Value) {
    call(reqwest::Method::POST, format!("{base}{path}")).await
}

#[tokio::test]
async fn a_manual_download_works_at_once_outside_the_budget() {
    // per_day = 0: the daily run is off, a manual download still works.
    let s = Setup::new("manual", "[trailers]\nper_day = 0\n").await;
    s.app.trailers.check_tools().await.expect("tools");
    let base = s.serve().await;
    // A wanted movie without a file can get a trailer by hand.
    let wanted = s
        .movie(950, "Gesucht (2026)", "2026-05-01T00:00:00Z", false)
        .await;
    s.videos(
        "movie/950",
        vec![youtube("gesucht_de1", "Trailer", Some("de"))],
    );
    s.behave("gesucht_de1", Behavior::Write(10));
    let before = get(&base, &format!("/api/movies/{wanted}/trailer")).await;
    assert_eq!(before["status"], "not_eligible");
    assert_eq!(before["blocked"], Value::Null);

    let (status, body) = post(&base, &format!("/api/movies/{wanted}/trailer")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "downloaded");
    assert_eq!(body["language"], "Deutsch");
    assert_eq!(body["link"], "https://www.youtube.com/watch?v=gesucht_de1");
    assert_eq!(
        body["files"],
        json!(["trailers/Gesucht (2026) - Trailer (Deutsch).mkv"])
    );
    // The own trailer is replaced by a new one (here: another language).
    s.videos(
        "movie/950",
        vec![youtube("gesucht_en1", "Trailer", Some("en"))],
    );
    s.behave("gesucht_en1", Behavior::Write(20));
    let (status, body) = post(&base, &format!("/api/movies/{wanted}/trailer")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["files"],
        json!(["trailers/Gesucht (2026) - Trailer (Englisch).mkv"])
    );
    assert_eq!(body["language"], "Englisch");
}

#[tokio::test]
async fn a_failed_manual_download_starts_the_attempts_over() {
    let s = Setup::new("manual-fail", "").await;
    s.app.trailers.check_tools().await.expect("tools");
    let base = s.serve().await;
    let heat = s
        .movie(949, "Heat (1995)", "2026-01-01T00:00:00Z", true)
        .await;
    s.videos(
        "movie/949",
        vec![youtube("heat_de_tr1", "Trailer", Some("de"))],
    );
    s.behave("heat_de_tr1", Behavior::Fail("Video unavailable"));
    s.app
        .db
        .put_trailer(
            MediaRef::Movie(heat),
            &record(TrailerState::Failed, MAX_ATTEMPTS, None),
            now(),
        )
        .await
        .expect("exhausted");
    let exhausted = get(&base, &format!("/api/movies/{heat}/trailer")).await;
    assert_eq!(exhausted["status"], "failed");
    assert!(
        exhausted["message"]
            .as_str()
            .is_some_and(|m| m.contains("nur noch von Hand")),
        "{exhausted}"
    );
    let (status, body) = post(&base, &format!("/api/movies/{heat}/trailer")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "failed");
    assert_eq!(body["attempts"], 1);
    assert!(body["next_attempt"].is_string(), "{body}");
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|m| m.contains("heat_de_tr1: yt-dlp: Video unavailable")),
        "{body}"
    );
}

#[tokio::test]
async fn manual_downloads_are_refused_with_a_german_message() {
    let s = Setup::new("refuse", "").await;
    s.app.trailers.check_tools().await.expect("tools");
    let base = s.serve().await;
    // A trailer sofarr did not write.
    let heat = s
        .movie(949, "Heat (1995)", "2026-01-01T00:00:00Z", true)
        .await;
    let own = s
        .library()
        .join("movies/Heat (1995)/trailers/Mein Trailer.mkv");
    fs::create_dir_all(own.parent().expect("dir")).expect("dir");
    fs::write(&own, b"mine").expect("own");
    s.videos(
        "movie/949",
        vec![youtube("heat_de_tr1", "Trailer", Some("de"))],
    );
    s.behave("heat_de_tr1", Behavior::Write(10));
    let info = get(&base, &format!("/api/movies/{heat}/trailer")).await;
    assert_eq!(info["status"], "foreign");
    assert_eq!(info["files"], json!(["trailers/Mein Trailer.mkv"]));
    assert!(
        info["blocked"]
            .as_str()
            .is_some_and(|b| b.contains("nicht geschrieben"))
    );
    let (status, body) = post(&base, &format!("/api/movies/{heat}/trailer")).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|e| e.contains("nicht geschrieben hat: trailers/Mein Trailer.mkv")),
        "{body}"
    );
    assert_eq!(fs::read(&own).expect("own"), b"mine");
    // An item without a folder on disk.
    let gone = s
        .movie(950, "Weg (2026)", "2026-05-01T00:00:00Z", false)
        .await;
    fs::remove_dir_all(s.library().join("movies/Weg (2026)")).expect("remove");
    let (status, body) = post(&base, &format!("/api/movies/{gone}/trailer")).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|e| e.contains("existiert nicht")),
        "{body}"
    );
    // Unknown items and ids.
    let (status, body) = post(&base, "/api/movies/999/trailer").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "Film 999 nicht gefunden");
    let (status, _) = post(&base, "/api/series/x/trailer").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(s.calls().is_empty());
}

#[tokio::test]
async fn shadow_mode_refuses_a_manual_download() {
    let s = Setup::build("manual-shadow", "shadow", "").await;
    let base = s.serve().await;
    let heat = s
        .movie(949, "Heat (1995)", "2026-01-01T00:00:00Z", true)
        .await;
    s.videos(
        "movie/949",
        vec![youtube("heat_de_tr1", "Trailer", Some("de"))],
    );
    let (status, body) = post(&base, &format!("/api/movies/{heat}/trailer")).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|e| e.contains("Schattenmodus")),
        "{body}"
    );
    assert!(s.calls().is_empty());
    assert_eq!(s.record(MediaRef::Movie(heat)).await, None);
}

#[tokio::test]
async fn the_item_shows_its_trailer_or_why_there_is_none() {
    let s = Setup::new("show", "").await;
    s.app.trailers.check_tools().await.expect("tools");
    let base = s.serve().await;
    let heat = s
        .movie(949, "Heat (1995)", "2026-01-01T00:00:00Z", true)
        .await;
    let path = format!("/api/movies/{heat}/trailer");
    let waiting = get(&base, &path).await;
    assert_eq!(waiting["status"], "waiting");
    assert!(
        waiting["message"]
            .as_str()
            .is_some_and(|m| m.contains("Tagesbudget"))
    );
    for (state, status) in [
        (TrailerState::NoCandidate, "no_candidate"),
        (TrailerState::Failed, "failed"),
        (TrailerState::LowSpace, "low_space"),
    ] {
        let stored = TrailerRecord {
            message: Some("zu wenig Speicherplatz: 5 MB frei".to_owned()),
            ..record(state, 1, Some("2026-11-19T03:00:00Z"))
        };
        s.app
            .db
            .put_trailer(MediaRef::Movie(heat), &stored, now())
            .await
            .expect("put");
        let info = get(&base, &path).await;
        assert_eq!(info["status"], status, "{info}");
        assert_eq!(info["attempts"], 1);
        assert_eq!(info["next_attempt"], "2026-11-19T03:00:00Z");
        assert!(info["message"].is_string(), "{info}");
    }
    // Downloaded: language and source.
    s.videos("movie/949", vec![youtube("heat_xx_tr1", "Trailer", None)]);
    s.behave("heat_xx_tr1", Behavior::Write(10));
    s.app
        .db
        .put_trailer(MediaRef::Movie(heat), &TrailerRecord::default(), now())
        .await
        .expect("reset");
    run(&s.app, now()).await.expect("run");
    let info = get(&base, &path).await;
    assert_eq!(info["status"], "downloaded");
    assert_eq!(info["language"], "Original");
    assert_eq!(info["link"], "https://www.youtube.com/watch?v=heat_xx_tr1");
    assert_eq!(info["message"], Value::Null);
    assert_eq!(info["blocked"], Value::Null);
    // The system page says trailers are on.
    let system = get(&base, "/api/system").await;
    assert_eq!(system["trailers"], json!({"active": true, "message": null}));
    assert!(system["jobs"].as_array().is_some_and(|jobs| {
        jobs.iter().any(|j| {
            j["name"] == "trailers" && j["label"] == "Trailer" && j["interval_minutes"] == 1440
        })
    }));
}
