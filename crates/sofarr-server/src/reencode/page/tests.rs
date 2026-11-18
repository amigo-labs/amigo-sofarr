//! The answer of the page "Neu encodieren" and the exclusions (spec ui-reencode).

use std::{fs, sync::Arc, time::Instant};

use jiff::Timestamp;
use serde_json::{Value, json};
use sofarr_db::{FileRef, NewReencode, ReencodeOutcome};
use sofarr_mediainfo::Container;

use super::*;
use crate::{
    reencode::run::{
        Running,
        tests::{Env, hevc, info, writes},
    },
    testkit::Http,
};

/// Every 1080p file of 100 minutes above 0.00001 MB per minute is a candidate.
const LIVE: &str = "[reencode]\nmax_mb_per_minute_1080p = 0.00001\nenabled = true\n";

async fn record(
    env: &Env,
    file: FileRef,
    outcome: ReencodeOutcome,
    before: u64,
    after: Option<u64>,
    reason: Option<&str>,
) {
    let new = NewReencode {
        file,
        title: "t".to_owned(),
        path: "p".to_owned(),
        runner: "cli".to_owned(),
        size_before: before,
    };
    let id = env
        .db()
        .start_reencode(&new, Timestamp::now())
        .await
        .expect("start");
    env.db()
        .finish_reencode(id, outcome, reason, after, Timestamp::now())
        .await
        .expect("finish");
}

fn by_title<'a>(page: &'a Value, title: &str) -> &'a Value {
    page["candidates"]
        .as_array()
        .expect("candidates")
        .iter()
        .find(|c| c["title"] == title)
        .unwrap_or_else(|| panic!("{title} listed"))
}

#[tokio::test]
async fn the_page_shows_job_candidates_states_saved_space_and_history() {
    // AC-02, AC-03, AC-06.
    let env = Env::with("page", "live", LIVE).await;
    let avc = info(Container::Matroska, "h264");
    let (gross, _) = env.movie_sized(1, "Gross", &avc, 60_000).await;
    let (done, _) = env.movie_sized(2, "Erledigt", &avc, 50_000).await;
    let (no_gain, _) = env.movie_sized(3, "Effizient", &avc, 40_000).await;
    let (rests, _) = env.movie_sized(4, "Ruht", &avc, 30_000).await;
    let (broken, _) = env.movie_sized(5, "Kaputt", &avc, 20_000).await;
    let (kept, _) = env.movie_sized(6, "Behalten", &avc, 10_000).await;
    let mut blind = avc.clone();
    blind.video = None;
    env.movie_sized(7, "Blind", &blind, 5_000).await;
    record(
        &env,
        FileRef::Movie(done),
        ReencodeOutcome::Replaced,
        90_000,
        Some(50_000),
        None,
    )
    .await;
    record(
        &env,
        FileRef::Movie(no_gain),
        ReencodeOutcome::NoGain,
        40_000,
        Some(39_000),
        Some("Ergebnis nur 2 % kleiner"),
    )
    .await;
    record(
        &env,
        FileRef::Movie(rests),
        ReencodeOutcome::Failed,
        30_000,
        None,
        Some("ffmpeg: kaputt"),
    )
    .await;
    for _ in 0..3 {
        record(
            &env,
            FileRef::Movie(broken),
            ReencodeOutcome::Failed,
            20_000,
            None,
            Some("ffmpeg: kaputt"),
        )
        .await;
    }
    env.db()
        .exclude_from_reencode(FileRef::Movie(kept), Timestamp::now())
        .await
        .expect("exclude");
    let running = Running {
        title: "Gross (2019)".to_owned(),
        path: by_path(&env, gross).await,
        started_at: "2026-11-18T08:00:00Z".to_owned(),
        runner: "cli".to_owned(),
        attempt: None,
    };
    fs::write(
        &env.app.reencode.lock,
        serde_json::to_string(&running).expect("json"),
    )
    .expect("lock");

    let http = Http::start_shared(Arc::clone(&env.app)).await;
    let (status, page) = http.get("/api/reencode").await;
    assert_eq!(status, 200);
    assert_eq!(
        page["job"],
        json!({ "active": true, "message": null, "running": "Gross (2019)" })
    );
    assert_eq!(page["running"]["runner"], "cli");
    assert_eq!(page["running"]["started_at"], "2026-11-18T08:00:00Z");
    assert_eq!(
        (page["saved_files"].as_u64(), page["saved_bytes"].as_u64()),
        (Some(1), Some(40_000))
    );
    assert_eq!(page["count"], 6);
    assert_eq!(page["total_size"], 210_000);
    assert_eq!(page["excluded"]["no_media_info"], 1);
    assert_eq!(page["shadow"], false);
    let titles: Vec<&str> = page["candidates"]
        .as_array()
        .expect("list")
        .iter()
        .map(|c| c["title"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(
        titles,
        [
            "Gross (2019)",
            "Erledigt (2019)",
            "Effizient (2019)",
            "Ruht (2019)",
            "Kaputt (2019)",
            "Behalten (2019)"
        ]
    );
    for (title, state) in [
        ("Gross (2019)", "running"),
        ("Erledigt (2019)", "done"),
        ("Effizient (2019)", "no_gain"),
        ("Ruht (2019)", "rests_until"),
        ("Kaputt (2019)", "rests_until_change"),
        ("Behalten (2019)", "excluded"),
    ] {
        assert_eq!(by_title(&page, title)["state"], state, "{title}");
    }
    let rest = by_title(&page, "Ruht (2019)");
    let until: Timestamp = rest["rests_until"]
        .as_str()
        .expect("until")
        .parse()
        .expect("timestamp");
    assert!(until > Timestamp::now());
    let first = by_title(&page, "Gross (2019)");
    assert_eq!(first["kind"], "movie");
    assert_eq!(first["movie_id"], gross);
    assert_eq!(first["resolution"], "1080p");
    assert_eq!(first["video_codec"], "h264");
    assert_eq!(first["size"], 60_000);
    let history = page["history"].as_array().expect("history");
    assert_eq!(history.len(), 6);
    assert_eq!(history[0]["outcome"], "failed");
    assert!(
        history
            .iter()
            .any(|h| h["outcome"] == "replaced" && h["size_after"] == 50_000)
    );
}

async fn by_path(env: &Env, id: i64) -> String {
    let movie = env.db().movie(id).await.expect("read").expect("movie");
    std::path::Path::new(&movie.path)
        .join(movie.file.expect("file").relative_path)
        .to_string_lossy()
        .into_owned()
}

#[tokio::test]
async fn exclusions_keep_files_out_of_the_job_and_can_be_undone() {
    // AC-05.
    let env = Env::with("page-exclude", "live", LIVE).await;
    let avc = info(Container::Matroska, "h264");
    let (big, _) = env.movie_sized(1, "Gross", &avc, 20_000).await;
    env.movie_sized(2, "Klein", &avc, 10_000).await;
    let http = Http::start_shared(Arc::clone(&env.app)).await;
    let (status, _) = http
        .post("/api/reencode/exclusions", json!({ "movie_id": big }))
        .await;
    assert_eq!(status, 204);
    let due = super::job::due_files(&env.app, Timestamp::now())
        .await
        .expect("due");
    assert_eq!(
        due.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(),
        ["Klein (2019)"]
    );
    let (_, page) = http.get("/api/reencode").await;
    assert_eq!(by_title(&page, "Gross (2019)")["state"], "excluded");
    let (status, _) = http
        .send(
            reqwest::Method::DELETE,
            "/api/reencode/exclusions",
            Some(json!({ "movie_id": big })),
        )
        .await;
    assert_eq!(status, 204);
    let (_, page) = http.get("/api/reencode").await;
    assert_eq!(by_title(&page, "Gross (2019)")["state"], "due");
    // Unknown ids and wrong bodies.
    let (status, body) = http
        .post("/api/reencode/exclusions", json!({ "movie_id": 999 }))
        .await;
    assert_eq!(status, 404);
    assert_eq!(
        body["error"],
        "Kein Film und keine Episode mit Datei unter dieser ID"
    );
    let (status, _) = http.post("/api/reencode/exclusions", json!({})).await;
    assert_eq!(status, 400);
    // "Jetzt encodieren" ignores the exclusion (the owner's request).
    let (tools, _) = env.tools(Ok(()), writes(hevc(), 4000));
    *env.app.reencode.tools.lock().expect("tools") = Some(Arc::new(tools));
    let (status, _) = http
        .post("/api/reencode/exclusions", json!({ "movie_id": big }))
        .await;
    assert_eq!(status, 204);
    let (status, _) = http
        .post("/api/reencode/encode", json!({ "movie_id": big }))
        .await;
    assert_eq!(status, 202);
}

#[tokio::test]
async fn shadow_mode_changes_nothing() {
    // AC-08.
    let env = Env::with("page-shadow", "shadow", LIVE).await;
    let (id, _) = env
        .movie_sized(1, "Film", &info(Container::Matroska, "h264"), 10_000)
        .await;
    let http = Http::start_shared(Arc::clone(&env.app)).await;
    let (_, page) = http.get("/api/reencode").await;
    assert_eq!(page["shadow"], true);
    for method in [reqwest::Method::POST, reqwest::Method::DELETE] {
        let (status, body) = http
            .send(
                method,
                "/api/reencode/exclusions",
                Some(json!({ "movie_id": id })),
            )
            .await;
        assert_eq!(status, 409);
        assert!(
            body["error"]
                .as_str()
                .is_some_and(|e| e.contains("im Shadow-Mode"))
        );
    }
    assert!(
        env.db()
            .reencode_exclusions()
            .await
            .expect("read")
            .is_empty()
    );
    let (status, body) = http
        .post("/api/reencode/encode", json!({ "movie_id": id }))
        .await;
    assert_eq!(status, 409);
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|e| e.to_lowercase().contains("im shadow-mode"))
    );
}

#[tokio::test]
async fn reencode_page_large_library() {
    // AC-10: 2,500 movies and 20,000 episode files, 10,000 attempts.
    let env = Env::with("page-large", "live", LIVE).await;
    let db = env.db();
    let info_json = serde_json::to_string(&info(Container::Matroska, "h264")).expect("json");
    let numbers = |n: u32| {
        format!("WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < {n})")
    };
    for statement in [
        format!("{} INSERT INTO movies (id, tmdb_id, title, year, runtime, monitored, quality_profile_id, path, added_at) SELECT i, i, 'Film ' || i, 2019, 100, 1, 1, '/filme/' || i, '' FROM n", numbers(2500)),
        "INSERT INTO movie_files (movie_id, relative_path, size, quality, added_at) SELECT id, 'f.mkv', 1, 'Remux-1080p', '' FROM movies".to_owned(),
        "INSERT INTO media_info (movie_id, info, size, modified, read_at) SELECT id, ?1, 100000 + id, '', '' FROM movies".to_owned(),
        format!("{} INSERT INTO series (id, tvdb_id, title, runtime, series_type, monitored, season_folder, quality_profile_id, path, added_at) SELECT i, i, 'Serie ' || i, 45, 'standard', 1, 1, 1, '/serien/' || i, '' FROM n", numbers(100)),
        format!("{} INSERT INTO episode_files (id, series_id, relative_path, size, quality, added_at) SELECT s.id * 1000 + n.i, s.id, 'e' || n.i || '.mkv', 1, 'WEBDL-1080p', '' FROM series s, n", numbers(200)),
        "INSERT INTO episodes (series_id, season, number, monitored, file_id) SELECT series_id, 1, id % 1000, 1, id FROM episode_files".to_owned(),
        "INSERT INTO media_info (episode_file_id, info, size, modified, read_at) SELECT id, ?1, 50000 + id, '', '' FROM episode_files".to_owned(),
        "INSERT INTO reencode_attempts (movie_id, title, path, runner, started_at, finished_at, outcome, reason, size_before) SELECT id, 't', 'p', 'server', '2026-11-01T00:00:00Z', '2026-11-01T01:00:00Z', 'failed', 'x', 100000 + id FROM movies".to_owned(),
        "INSERT INTO reencode_attempts (episode_file_id, title, path, runner, started_at, finished_at, outcome, size_before, size_after) SELECT id, 't', 'p', 'server', '2026-11-01T00:00:00Z', '2026-11-01T01:00:00Z', 'replaced', 90000, 50000 + id FROM episode_files WHERE id % 1000 <= 75".to_owned(),
    ] {
        sqlx::query(&statement).bind(&info_json).execute(db.pool()).await.expect("bulk insert");
    }
    let started = Instant::now();
    let page = answer(&env.app, Timestamp::now()).await.expect("answer");
    let took = started.elapsed();
    println!("re-encode page for 22,500 files with 10,000 attempts in {took:?}");
    assert_eq!(page.count, 22_500);
    assert_eq!(page.history.len(), 50);
    assert_eq!(page.saved_files, 7500);
    let limit = if cfg!(debug_assertions) { 20 } else { 2 };
    assert!(took.as_secs() < limit, "took {took:?}");
}

#[tokio::test]
async fn an_episode_id_names_its_file_and_an_unknown_one_is_not_found() {
    // AC-05, the episode side.
    let env = Env::with("page-episodes", "live", LIVE).await;
    let db = env.db();
    let series = db
        .upsert_series(
            &sofarr_db::NewSeries {
                tvdb_id: 1,
                imdb_id: None,
                title: "Insel".to_owned(),
                title_de: None,
                year: None,
                runtime_minutes: 45,
                original_language: None,
                series_type: "standard".to_owned(),
                monitored: true,
                season_folder: true,
                quality_profile_id: 1,
                path: "/serien/Insel".to_owned(),
                sonarr_id: None,
                alternative_titles: vec![],
            },
            Timestamp::now(),
        )
        .await
        .expect("series");
    let mut episodes = Vec::new();
    for number in [2, 3, 4] {
        let episode = sofarr_db::NewEpisode {
            tvdb_id: None,
            season: 1,
            number,
            absolute: None,
            title: None,
            title_de: None,
            air_date: None,
            monitored: true,
        };
        episodes.push(db.upsert_episode(series, &episode).await.expect("episode"));
    }
    // A double episode with one file; episode 4 has no file.
    let file = sofarr_db::MovieFile {
        relative_path: "Season 01/S01E02.mkv".to_owned(),
        size: 1,
        quality: "WEBDL-1080p".to_owned(),
        revision_version: 1,
        revision_real: 0,
        release_group: None,
        scene_name: None,
        custom_format_score: 0,
    };
    let file_id = db
        .set_episode_file(series, &episodes[..2], &file, Timestamp::now())
        .await
        .expect("file");
    let http = Http::start_shared(Arc::clone(&env.app)).await;
    // The second episode of the double episode excludes its file …
    let (status, _) = http
        .post(
            "/api/reencode/exclusions",
            json!({ "episode_id": episodes[1] }),
        )
        .await;
    assert_eq!(status, 204);
    assert_eq!(
        db.reencode_exclusions().await.expect("read"),
        [FileRef::Episode(file_id)]
    );
    // … and the first takes it back.
    let (status, _) = http
        .send(
            reqwest::Method::DELETE,
            "/api/reencode/exclusions",
            Some(json!({ "episode_id": episodes[0] })),
        )
        .await;
    assert_eq!(status, 204);
    assert!(db.reencode_exclusions().await.expect("read").is_empty());
    // An episode without a file and an unknown one are not found.
    for id in [episodes[2], 999_999] {
        let (status, body) = http
            .post("/api/reencode/exclusions", json!({ "episode_id": id }))
            .await;
        assert_eq!(status, 404, "{id}");
        assert_eq!(
            body["error"],
            "Kein Film und keine Episode mit Datei unter dieser ID"
        );
    }
}
