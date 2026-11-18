//! `amigo-sofarr reencode` and `POST/GET /api/reencode/encode` against a fake encoder.

use std::{sync::Arc, time::Duration};

use serde_json::{Value, json};
use sofarr_db::ReencodeOutcome;
use sofarr_mediainfo::Container;

use super::*;
use crate::{
    reencode::{
        ffmpeg::{EncodeError, Job},
        run::{
            Tools,
            tests::{Behavior, Env, hevc, info, writes},
        },
    },
    testkit::Http,
};

fn install(env: &Env, tools: Tools) {
    *env.app.reencode.tools.lock().expect("tools") = Some(Arc::new(tools));
}

/// Runs the CLI flow and returns what it printed and how it ended.
async fn cli_run(env: &Env, target: Target) -> (Vec<String>, Result<(), String>) {
    let mut lines = Vec::new();
    let result = cli(&env.app, target, &mut |line| lines.push(line.to_owned()))
        .await
        .map_err(|e| format!("{e:#}"));
    (lines, result)
}

#[tokio::test]
async fn the_cli_prints_start_and_outcome_and_fails_on_failures() {
    // AC-01.
    let env = Env::new("cli", "live").await;
    let (id, video) = env
        .movie("Film", "Film.mkv", &info(Container::Matroska, "h264"))
        .await;
    let gib = 1024 * 1024 * 1024;
    let report = |outcome, reason: Option<&str>, after| Report {
        outcome,
        reason: reason.map(str::to_owned),
        title: "Film (2019)".to_owned(),
        path: video.clone(),
        size_before: 40 * gib,
        size_after: after,
    };
    assert_eq!(
        outcome_line(&report(ReencodeOutcome::Replaced, None, Some(10 * gib))),
        "Ersetzt: 40,0 GB → 10,0 GB (75 % kleiner)"
    );
    assert_eq!(
        outcome_line(&report(
            ReencodeOutcome::NoGain,
            Some("Ergebnis nur 12 % kleiner"),
            Some(35 * gib)
        )),
        "Kein Gewinn, das Original bleibt: Ergebnis nur 12 % kleiner"
    );

    let (tools, _) = env.tools(Ok(()), writes(hevc(), 4000));
    install(&env, tools);
    let (lines, result) = cli_run(&env, Target::Movie(id)).await;
    assert_eq!(result, Ok(()));
    assert_eq!(
        lines,
        [
            format!("Encodiere Film (2019) (0,0 GB): {}", video.display()),
            "Ersetzt: 0,0 GB → 0,0 GB (60 % kleiner)".to_owned(),
        ]
    );

    let failing: Behavior = Box::new(|job: &Job| {
        crate::reencode::run::tests::write_fake(&job.output, &hevc(), 1000);
        Err(EncodeError::Failed("ffmpeg: Conversion failed!".to_owned()))
    });
    let (tools, _) = env.tools(Ok(()), failing);
    install(&env, tools);
    let (lines, result) = cli_run(&env, Target::Movie(id)).await;
    assert_eq!(lines.len(), 1, "only the start line before the error");
    assert_eq!(
        result,
        Err("Fehlgeschlagen: ffmpeg: Conversion failed!".to_owned())
    );

    let (tools, _) = env.tools(Ok(()), Box::new(|_: &Job| Err(EncodeError::TooLarge)));
    install(&env, tools);
    let (lines, result) = cli_run(&env, Target::Movie(id)).await;
    assert_eq!(result, Ok(()), "no gain is no error");
    assert!(lines[1].starts_with("Kein Gewinn, das Original bleibt: "));

    let (lines, result) = cli_run(&env, Target::Movie(999)).await;
    assert!(lines.is_empty());
    assert_eq!(
        result,
        Err("Abgelehnt: Kein Film und keine Episode mit Datei unter dieser ID".to_owned())
    );
}

#[tokio::test]
async fn the_cli_finds_an_episode_by_series_season_and_number() {
    let env = Env::new("cli-target", "live").await;
    assert!(matches!(
        cli_target(env.db(), Some(4), None, None, None).await,
        Ok(Target::Movie(4))
    ));
    let error = cli_target(env.db(), None, Some(1), Some(1), Some(2))
        .await
        .expect_err("no such episode");
    assert_eq!(error.to_string(), "Serie 1 hat keine Episode S01E02");
    for (movie, series, season, episode) in [
        (None, None, None, None),
        (Some(1), Some(1), Some(1), Some(1)),
        (None, Some(1), Some(1), None),
        (None, Some(1), None, Some(1)),
    ] {
        let error = cli_target(env.db(), movie, series, season, episode)
            .await
            .expect_err("incomplete");
        assert!(error.to_string().starts_with("Ziel fehlt"));
    }
}

async fn wait_for_last(http: &Http) -> Value {
    for _ in 0..200 {
        let (_, body) = http.get("/api/reencode/encode").await;
        if body["last"]["outcome"].is_string() && body["running"].is_null() {
            return body;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the background encode did not finish");
}

#[tokio::test]
async fn the_api_starts_an_encode_in_the_background() {
    // AC-02.
    let env = Env::new("api", "live").await;
    let (id, video) = env
        .movie("Film", "Film.mkv", &info(Container::Matroska, "h264"))
        .await;
    let (tools, _) = env.tools(Ok(()), writes(hevc(), 4000));
    install(&env, tools);
    let http = Http::start_shared(Arc::clone(&env.app)).await;
    let (status, body) = http.get("/api/reencode/encode").await;
    assert_eq!(status, 200);
    assert_eq!(body, json!({ "running": null, "last": null }));

    let (status, body) = http
        .post("/api/reencode/encode", json!({ "movie_id": id }))
        .await;
    assert_eq!(status, 202);
    assert_eq!(body["title"], "Film (2019)");
    assert_eq!(body["path"], video.to_string_lossy().as_ref());
    let state = wait_for_last(&http).await;
    let last = &state["last"];
    assert_eq!(last["outcome"], "replaced");
    assert_eq!(last["runner"], "server");
    assert_eq!(last["title"], "Film (2019)");
    assert_eq!(
        (last["size_before"].as_u64(), last["size_after"].as_u64()),
        (Some(10_000), Some(4000))
    );
    assert!(last["finished_at"].is_string());
    assert_eq!(std::fs::metadata(&video).expect("video").len(), 4000);

    // Refusals are answered at once.
    let (status, body) = http
        .post("/api/reencode/encode", json!({ "movie_id": 999 }))
        .await;
    assert_eq!(status, 404);
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|e| e.starts_with("Kein Film"))
    );
    for wrong in [json!({}), json!({ "movie_id": id, "episode_id": 1 })] {
        let (status, _) = http.post("/api/reencode/encode", wrong).await;
        assert_eq!(status, 400);
    }
    let (status, body) = http
        .post("/api/reencode/encode", json!({ "movie_id": id }))
        .await;
    // The result is HEVC now and the original is gone: the next attempt runs again,
    // and its fake result is no gain at 4,000 of 4,000 bytes.
    assert_eq!(status, 202, "{body}");
    let state = wait_for_last(&http).await;
    assert_eq!(state["last"]["outcome"], "no_gain");

    // A running CLI encode is shown, and blocks a start.
    let running = Running {
        title: "Anderer Film (2020)".to_owned(),
        path: "/filme/Anderer Film/Anderer Film.mkv".to_owned(),
        started_at: "2026-11-17T08:00:00Z".to_owned(),
        runner: "cli".to_owned(),
        attempt: None,
    };
    std::fs::write(
        &env.app.reencode.lock,
        serde_json::to_string(&running).expect("json"),
    )
    .expect("lock");
    let (_, state) = http.get("/api/reencode/encode").await;
    assert_eq!(
        state["running"],
        json!({
            "title": "Anderer Film (2020)",
            "path": "/filme/Anderer Film/Anderer Film.mkv",
            "started_at": "2026-11-17T08:00:00Z",
            "runner": "cli",
            "attempt": null,
        })
    );
    let (status, body) = http
        .post("/api/reencode/encode", json!({ "movie_id": id }))
        .await;
    assert_eq!(status, 409);
    assert_eq!(
        body["error"],
        "Es läuft bereits ein Encode: Anderer Film (2020)"
    );
    std::fs::remove_file(&env.app.reencode.lock).expect("unlock");
}

#[tokio::test]
async fn the_api_refuses_in_shadow_mode() {
    let env = Env::new("api-shadow", "shadow").await;
    let (id, _) = env
        .movie("Film", "Film.mkv", &info(Container::Matroska, "h264"))
        .await;
    let (tools, encoder) = env.tools(Ok(()), writes(hevc(), 4000));
    install(&env, tools);
    let http = Http::start_shared(Arc::clone(&env.app)).await;
    let (status, body) = http
        .post("/api/reencode/encode", json!({ "movie_id": id }))
        .await;
    assert_eq!(status, 409);
    assert_eq!(body["error"], "Im Shadow-Mode encodiert sofarr nichts");
    assert!(encoder.jobs.lock().expect("jobs").is_empty());
}
