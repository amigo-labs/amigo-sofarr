//! Overview and calendar over HTTP (spec web-ui-library AC-01, AC-07).

use jiff::{SignedDuration, Timestamp, ToSpan, civil::Date};
use reqwest::StatusCode;
use serde_json::{Value, json};
use sofarr_db::{GrabStatus, NewGrab};

use crate::{
    app::App,
    testkit::{Http, Library, Services, app},
};

fn grab(movie_id: i64, title: &str, status: GrabStatus, download: Option<&str>) -> NewGrab {
    NewGrab {
        movie_id,
        title: title.to_owned(),
        guid: title.to_owned(),
        indexer: "idx".to_owned(),
        download_url: None,
        quality: "Bluray-1080p".to_owned(),
        custom_format_score: 0,
        size: 1_048_576_000,
        status,
        download_id: download.map(str::to_owned),
    }
}

fn today() -> Date {
    crate::library_view::today(Timestamp::now())
}

fn day(offset: i64) -> String {
    today()
        .checked_add(offset.days())
        .expect("date")
        .to_string()
}

async fn movie_id(app: &App, tmdb: i64) -> i64 {
    app.db
        .movie_by_tmdb(tmdb)
        .await
        .expect("db")
        .expect("movie")
        .id
}

async fn set_dates(app: &App, id: i64, cinema: &str, digital: &str, physical: &str) {
    sqlx::query(
        "UPDATE movies SET in_cinemas = ?, digital_release = ?, physical_release = ? WHERE id = ?",
    )
    .bind(cinema)
    .bind(digital)
    .bind(physical)
    .bind(id)
    .execute(app.db.pool())
    .await
    .expect("dates");
}

#[tokio::test]
async fn queue_shows_progress_size_and_remaining_time() {
    let s = Services::start().await;
    let toml = format!("[automation]\nmode = \"live\"\n{}", s.nzbget_toml());
    let app = app(Library::Movies, &toml).await;
    let first = movie_id(&app, 101).await;
    let second = movie_id(&app, 102).await;
    let now = Timestamp::now();
    app.db
        .add_grab(
            &grab(
                first,
                "Erster.Film.2001.1080p.BluRay-A",
                GrabStatus::Downloading,
                Some("77"),
            ),
            now,
        )
        .await
        .expect("grab");
    app.db
        .add_grab(
            &grab(
                second,
                "Zweiter.Film.2002.1080p.BluRay-B",
                GrabStatus::Queued,
                Some("78"),
            ),
            now,
        )
        .await
        .expect("grab");
    app.db
        .add_grab(
            &grab(
                second,
                "Zweiter.Film.2002.720p-C",
                GrabStatus::Imported,
                Some("70"),
            ),
            now,
        )
        .await
        .expect("done");
    {
        let mut fake = s.lock();
        fake.rate = 1_048_576;
        fake.queue = vec![
            json!({"NZBID": 77, "NZBName": "Erster", "FileSizeMB": 1000, "RemainingSizeMB": 250,
                   "Status": "DOWNLOADING"}),
            json!({"NZBID": 78, "NZBName": "Zweiter", "FileSizeMB": 500, "RemainingSizeMB": 500,
                   "Status": "PAUSED"}),
        ];
    }
    let http = Http::start(app).await;
    let (status, queue) = http.get("/api/queue").await;
    assert_eq!(status, StatusCode::OK, "{queue}");
    let items = queue["items"].as_array().expect("items");
    assert_eq!(items.len(), 2, "only active grabs: {queue}");
    assert_eq!(items[0]["media_title"], "Erster Film");
    assert_eq!(items[0]["download_state"], "downloading");
    assert_eq!(items[0]["progress"], 0.75);
    assert_eq!(items[0]["size"], 1000 * 1_048_576_u64);
    assert_eq!(items[0]["remaining_seconds"], 250);
    assert_eq!(items[1]["download_state"], "paused");
    assert_eq!(items[1]["progress"], 0.0);
    assert_eq!(
        items[1]["remaining_seconds"],
        Value::Null,
        "paused: no time"
    );
    assert_eq!(queue["notice"], Value::Null);

    // The client goes away: the last known state stays, with a German notice.
    s.lock().nzbget_down = true;
    let (status, stale) = http.get("/api/queue").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stale["items"][0]["progress"], 0.75);
    assert!(
        stale["notice"]
            .as_str()
            .is_some_and(|n| n.starts_with("Download-Client nicht erreichbar – Stand von")),
        "{stale}"
    );
    assert!(!stale.to_string().contains("http://"), "no URLs in answers");
}

#[tokio::test]
async fn overview_shows_activity_problems_and_the_next_days() {
    let app = app(Library::Movies, "").await;
    let first = movie_id(&app, 101).await;
    let second = movie_id(&app, 102).await;
    let now = Timestamp::now();
    let old = now - SignedDuration::from_hours(8 * 24);
    for i in 0..22 {
        app.db
            .add_grab(
                &grab(
                    second,
                    &format!("Zweiter.Film.{i}"),
                    GrabStatus::Shadow,
                    None,
                ),
                now,
            )
            .await
            .expect("grab");
    }
    let failed = app
        .db
        .add_grab(
            &grab(first, "Erster.Film.kaputt", GrabStatus::Queued, Some("5")),
            now,
        )
        .await
        .expect("grab");
    app.db
        .update_grab(failed, GrabStatus::Failed, None, Some("FAILURE/PAR"), now)
        .await
        .expect("failed");
    let problem = app
        .db
        .add_grab(
            &grab(
                first,
                "Erster.Film.ohne.Video",
                GrabStatus::Queued,
                Some("6"),
            ),
            now,
        )
        .await
        .expect("grab");
    app.db
        .update_grab(
            problem,
            GrabStatus::Completed,
            None,
            Some("keine Videodatei in /downloads/x"),
            now,
        )
        .await
        .expect("problem");
    let gone = app
        .db
        .add_grab(
            &grab(first, "Erster.Film.alt", GrabStatus::Queued, None),
            old,
        )
        .await
        .expect("grab");
    app.db
        .update_grab(gone, GrabStatus::Failed, None, Some("alt"), old)
        .await
        .expect("old failure");
    set_dates(&app, first, &day(-30), &day(3), &day(30)).await;
    let http = Http::start(app).await;
    let (status, overview) = http.get("/api/overview").await;
    assert_eq!(status, StatusCode::OK, "{overview}");
    assert_eq!(overview["activity"].as_array().map(Vec::len), Some(20));
    let problems: Vec<&str> = overview["problems"]
        .as_array()
        .expect("problems")
        .iter()
        .map(|p| p["release_title"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(problems.contains(&"Erster.Film.kaputt"));
    assert!(problems.contains(&"Erster.Film.ohne.Video"));
    // Only the digital release lies in the next 7 days.
    assert_eq!(
        overview["upcoming"],
        json!([{
            "date": day(3), "kind": "movie", "release": "digital", "movie_id": first,
            "series_id": null, "episode_id": null, "season": null, "episode": null,
            "title": "Erster Film", "episode_title": null, "year": 2001,
            "state": "upcoming", "monitored": true
        }])
    );
    assert_eq!(
        overview["queue"]["notice"], "Kein Download-Client konfiguriert",
        "{overview}"
    );
    let (_, five) = http.get("/api/activity?limit=5").await;
    assert_eq!(five.as_array().map(Vec::len), Some(5));
    let (status, _) = http.get("/api/activity?limit=0").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn calendar_marks_every_date_of_a_range() {
    let app = app(Library::Movies, "").await;
    let first = movie_id(&app, 101).await;
    let third = movie_id(&app, 103).await;
    let fourth = movie_id(&app, 104).await;
    // Erster Film: no file, released before → missing (past), upcoming (ahead).
    set_dates(&app, first, &day(-20), &day(-2), &day(5)).await;
    // Dritter Film has a file: present on every date.
    set_dates(&app, third, &day(-1), "", "").await;
    // Vierter Film is not monitored: not in the calendar.
    set_dates(&app, fourth, &day(-1), "", "").await;
    let http = Http::start(app).await;
    let path = format!("/api/calendar?from={}&to={}", day(-21), day(10));
    let (status, cal) = http.get(&path).await;
    assert_eq!(status, StatusCode::OK, "{cal}");
    let seen: Vec<(String, String, String)> = cal["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|e| {
            (
                e["title"].as_str().unwrap_or_default().to_owned(),
                e["release"].as_str().unwrap_or_default().to_owned(),
                e["state"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    let row = |t: &str, r: &str, s: &str| (t.to_owned(), r.to_owned(), s.to_owned());
    assert_eq!(
        seen,
        [
            row("Erster Film", "cinema", "missing"),
            row("Erster Film", "digital", "missing"),
            row("Dritter Film", "cinema", "present"),
            row("Erster Film", "physical", "upcoming"),
        ]
    );
    // A movie not available by its minimum availability is neutral.
    set_dates(&http.app, first, &day(-20), &day(20), "").await;
    let (_, cal) = http.get(&path).await;
    assert_eq!(cal["entries"][0]["state"], "neutral");

    let (_, empty) = http
        .get("/api/calendar?from=1990-02-01&to=1990-02-28")
        .await;
    assert_eq!(empty["entries"], json!([]));
    assert_eq!(empty["note"], "Keine Veröffentlichungen in diesem Zeitraum");
    for bad in [
        "/api/calendar?from=2026-01-10&to=2026-01-01",
        "/api/calendar?from=2026-01-01&to=2026-06-01",
        "/api/calendar?from=morgen&to=2026-01-01",
        "/api/calendar?from=2026-01-01",
    ] {
        let (status, body) = http.get(bad).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
        assert!(
            body["error"]
                .as_str()
                .is_some_and(|e| e.starts_with("Ungültiger Zeitraum"))
        );
    }
}

#[tokio::test]
async fn calendar_shows_all_episodes_of_monitored_series() {
    let app = app(Library::Series, "").await;
    let show = app.db.all_series().await.expect("db")[0].id;
    let eps = app.db.episodes(Some(show)).await.expect("eps");
    let dates = [day(-3), day(-2), day(4)];
    for (e, date) in eps.iter().zip(&dates) {
        sqlx::query("UPDATE episodes SET air_date = ? WHERE id = ?")
            .bind(date)
            .bind(e.id)
            .execute(app.db.pool())
            .await
            .expect("air date");
    }
    // S01E01 has a file; S01E02 is not monitored.
    app.db
        .set_episodes_monitored(&[eps[1].id], false)
        .await
        .expect("unmonitor");
    let http = Http::start(app).await;
    let (_, cal) = http
        .get(&format!("/api/calendar?from={}&to={}", day(-7), day(7)))
        .await;
    let states: Vec<&str> = cal["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|e| e["state"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(states, ["present", "neutral", "upcoming"]);
    assert_eq!(cal["entries"][1]["monitored"], false);
    assert_eq!(cal["entries"][2]["episode_title"], "Zukunft");
    let (_, overview) = http.get("/api/overview").await;
    let upcoming = overview["upcoming"].as_array().expect("upcoming");
    assert_eq!(upcoming.len(), 1, "{overview}");
    assert_eq!(upcoming[0]["episode"], 3);
}
