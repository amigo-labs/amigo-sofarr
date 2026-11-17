//! Which candidate is due (spec reencode-job AC-02, AC-03, AC-09).

use jiff::{SignedDuration, Timestamp};
use sofarr_db::{FileRef, ReencodeAttempt, ReencodeOutcome};

use super::*;

const SIZE: u64 = 50_000;

fn now() -> Timestamp {
    "2026-11-17T12:00:00Z".parse().expect("timestamp")
}

fn ago(hours: i64) -> Timestamp {
    now() - SignedDuration::from_hours(hours)
}

/// An attempt that ended `hours` ago.
fn attempt(
    outcome: ReencodeOutcome,
    reason: Option<&str>,
    size_before: u64,
    size_after: Option<u64>,
    hours: i64,
) -> ReencodeAttempt {
    ReencodeAttempt {
        id: 0,
        file: Some(FileRef::Movie(1)),
        title: "Film".to_owned(),
        path: "/filme/Film/Film.mkv".to_owned(),
        runner: "server".to_owned(),
        started_at: ago(hours + 1),
        finished_at: Some(ago(hours)),
        outcome: Some(outcome),
        reason: reason.map(str::to_owned),
        size_before,
        size_after,
    }
}

fn failed(hours: i64) -> ReencodeAttempt {
    attempt(
        ReencodeOutcome::Failed,
        Some("ffmpeg: Conversion failed!"),
        SIZE,
        None,
        hours,
    )
}

fn due_now(attempts: &[ReencodeAttempt]) -> bool {
    is_due(SIZE, attempts, now())
}

#[test]
fn a_file_without_attempts_is_due() {
    assert!(due_now(&[]));
}

#[test]
fn a_settled_file_stays_settled_until_it_changes() {
    // Replaced: the file now has the size after.
    let replaced = attempt(ReencodeOutcome::Replaced, None, 90_000, Some(SIZE), 100);
    assert!(!due_now(std::slice::from_ref(&replaced)));
    // No gain: the file still has the size before.
    let no_gain = attempt(ReencodeOutcome::NoGain, None, SIZE, Some(45_000), 100);
    assert!(!due_now(std::slice::from_ref(&no_gain)));
    // Whatever came after: a failed hand attempt, a refusal, an interruption.
    assert!(!due_now(&[
        failed(1),
        failed(2),
        failed(3),
        no_gain.clone()
    ]));
    let interrupted = attempt(ReencodeOutcome::Failed, Some("abgebrochen"), SIZE, None, 1);
    assert!(!due_now(&[interrupted, replaced.clone()]));
    // Changed since (an upgrade with another size): due again.
    assert!(is_due(SIZE + 1, &[replaced], now()));
    assert!(is_due(SIZE + 1, &[no_gain], now()));
}

#[test]
fn failures_rest_seven_days_and_three_rest_until_the_file_changes() {
    // AC-09.
    assert!(!due_now(&[failed(1)]));
    assert!(!due_now(&[failed(7 * 24 - 1)]));
    assert!(due_now(&[failed(7 * 24)]));
    assert!(!due_now(&[failed(2), failed(200)]));
    assert!(due_now(&[failed(7 * 24 + 1), failed(400)]));
    // The third failure in a row: resting until the file changes.
    assert!(!due_now(&[failed(1000), failed(2000), failed(3000)]));
    assert!(is_due(
        SIZE + 1,
        &[failed(1000), failed(2000), failed(3000)],
        now()
    ));
    // A replaced or no-gain at another size ends the streak before it.
    let earlier = attempt(ReencodeOutcome::Replaced, None, 90_000, Some(70_000), 5000);
    assert!(due_now(&[failed(1000), failed(2000), earlier]));
    // Failures at another size do not count.
    let other = attempt(ReencodeOutcome::Failed, Some("x"), SIZE + 9, None, 1);
    assert!(due_now(&[other.clone(), other.clone(), other]));
}

#[test]
fn interruptions_and_refusals_neither_count_nor_break_the_streak() {
    let interrupted = attempt(ReencodeOutcome::Failed, Some("abgebrochen"), SIZE, None, 1);
    let stopped = attempt(
        ReencodeOutcome::Failed,
        Some("abgebrochen: sofarr wurde beendet"),
        SIZE,
        None,
        2,
    );
    assert!(due_now(&[
        interrupted.clone(),
        stopped.clone(),
        interrupted.clone()
    ]));
    let refused_old = attempt(
        ReencodeOutcome::Refused,
        Some("zu wenig Speicherplatz"),
        SIZE,
        None,
        48,
    );
    assert!(!due_now(&[
        interrupted.clone(),
        refused_old.clone(),
        failed(3000),
        stopped,
        failed(4000),
        failed(5000),
    ]));
    assert!(due_now(&[
        interrupted,
        refused_old,
        failed(3000),
        failed(4000)
    ]));
}

#[test]
fn a_refused_file_rests_a_day_unless_the_slot_was_taken() {
    // AC-03.
    let refused =
        |reason: &str, hours| attempt(ReencodeOutcome::Refused, Some(reason), SIZE, None, hours);
    assert!(!due_now(&[refused(
        "zu wenig Speicherplatz: 10 MB frei",
        1
    )]));
    assert!(!due_now(&[refused(
        "Dolby Vision/HDR10+ würde verloren gehen",
        23
    )]));
    assert!(due_now(&[refused("zu wenig Speicherplatz", 24)]));
    assert!(due_now(&[refused(
        "Es läuft bereits ein Encode: Anderer Film (2020)",
        0
    )]));
    // Only the latest attempt's refusal makes it rest.
    assert!(due_now(&[
        failed(7 * 24 + 1),
        refused("zu wenig Speicherplatz", 7 * 24 + 2)
    ]));
}

#[test]
fn due_keeps_the_order_of_the_candidate_list() {
    use crate::reencode::Candidate;
    let candidate = |id: i64, size: u64| Candidate {
        file: FileRef::Movie(id),
        kind: "movie",
        movie_id: Some(id),
        series_id: None,
        episode_ids: Vec::new(),
        title: format!("Film {id}"),
        path: format!("/filme/{id}.mkv"),
        size,
        duration_seconds: 6000.0,
        duration_source: "file",
        resolution: "1080p",
        video_codec: "h264".to_owned(),
        mb_per_minute: 100.0,
        threshold: 60.0,
    };
    let list = [
        candidate(1, 90_000),
        candidate(2, SIZE),
        candidate(3, 10_000),
    ];
    let mut resting = failed(1);
    resting.file = Some(FileRef::Movie(2));
    let mut gone = failed(1);
    gone.file = None;
    let due = due(&list, &[resting, gone], now());
    assert_eq!(
        due.iter().map(|c| c.file).collect::<Vec<_>>(),
        [FileRef::Movie(1), FileRef::Movie(3)]
    );
}

mod job {
    //! Rounds, start, stop and the system page (spec reencode-job) against a fake encoder.

    use std::{
        fs,
        sync::{Arc, atomic::Ordering},
        time::{Duration, Instant},
    };

    use jiff::Timestamp;
    use sofarr_db::{FileRef, NewReencode, ReencodeOutcome};
    use sofarr_mediainfo::Container;

    use super::super::*;
    use crate::reencode::{
        ffmpeg::{EncodeError, Job as EncodeJob},
        run::{
            self, Running, Tools,
            tests::{Behavior, Env, hevc, info, write_fake},
        },
    };

    /// Every 1080p file of 100 minutes above 0.00001 MB per minute is a candidate.
    const LIVE: &str = "[reencode]\nmax_mb_per_minute_1080p = 0.00001\nenabled = true\n";

    /// An encoder writing a good result of 40 % of the original.
    fn shrinks() -> Behavior {
        Box::new(|job: &EncodeJob| {
            let size = fs::metadata(&job.input).expect("original").len();
            write_fake(&job.output, &hevc(), size * 4 / 10);
            Ok(())
        })
    }

    fn install(env: &Env, tools: Tools) -> Arc<Tools> {
        let tools = Arc::new(tools);
        *env.app.reencode.tools.lock().expect("tools") = Some(Arc::clone(&tools));
        tools
    }

    async fn titles_of_rounds(env: &Env, tools: &Tools, rounds: usize) -> Vec<String> {
        let mut out = Vec::new();
        for _ in 0..rounds {
            match round(&env.app, tools, Timestamp::now())
                .await
                .expect("round")
            {
                RoundEnd::Encoded(report) => out.push(report.title),
                RoundEnd::NothingDue => out.push("nichts fällig".to_owned()),
                RoundEnd::SlotHeld => out.push("belegt".to_owned()),
            }
        }
        out
    }

    #[tokio::test]
    async fn rounds_take_the_largest_due_file_and_skip_settled_ones() {
        // AC-01, AC-02.
        let env = Env::with("job-rounds", "live", LIVE).await;
        let avc = info(Container::Matroska, "h264");
        let (_, _) = env.movie_sized(1, "Gross", &avc, 30_000).await;
        let (settled, _) = env.movie_sized(2, "Mittel", &avc, 20_000).await;
        env.movie_sized(3, "Klein", &avc, 10_000).await;
        // "Mittel" ended without gain at its size before.
        let new = NewReencode {
            file: FileRef::Movie(settled),
            title: "Mittel (2019)".to_owned(),
            path: "/x".to_owned(),
            runner: "cli".to_owned(),
            size_before: 20_000,
        };
        let id = env
            .db()
            .start_reencode(&new, Timestamp::now())
            .await
            .expect("start");
        env.db()
            .finish_reencode(
                id,
                ReencodeOutcome::NoGain,
                None,
                Some(19_000),
                Timestamp::now(),
            )
            .await
            .expect("finish");
        let (tools, encoder) = env.tools(Ok(()), shrinks());
        let titles = titles_of_rounds(&env, &tools, 3).await;
        assert_eq!(titles, ["Gross (2019)", "Klein (2019)", "nichts fällig"]);
        assert_eq!(encoder.jobs.lock().expect("jobs").len(), 2);
        let last = env
            .db()
            .last_reencode()
            .await
            .expect("read")
            .expect("attempt");
        assert_eq!(last.runner, "server");
    }

    #[tokio::test]
    async fn a_refused_file_rests_and_the_round_goes_on() {
        // AC-03: the stored media info missed Dolby Vision, the file itself has it.
        let env = Env::with("job-refused", "live", LIVE).await;
        let avc = info(Container::Matroska, "h264");
        let (_, special) = env.movie_sized(1, "Dolby", &avc, 30_000).await;
        let mut dolby = avc.clone();
        if let Some(v) = &mut dolby.video {
            v.dynamic_range.dolby_vision = true;
        }
        write_fake(&special, &dolby, 30_000);
        env.movie_sized(2, "Normal", &avc, 10_000).await;
        let (tools, _) = env.tools(Ok(()), shrinks());
        let titles = titles_of_rounds(&env, &tools, 2).await;
        assert_eq!(titles, ["Normal (2019)", "nichts fällig"]);
        let attempts = env.db().all_reencodes().await.expect("attempts");
        let refused: Vec<_> = attempts
            .iter()
            .filter(|a| a.outcome == Some(ReencodeOutcome::Refused))
            .collect();
        assert_eq!(refused.len(), 1, "refused once, then resting");
        assert_eq!(
            refused[0].reason.as_deref(),
            Some("Dolby Vision/HDR10+ würde verloren gehen")
        );
    }

    #[tokio::test]
    async fn a_held_slot_makes_the_job_wait_without_records() {
        // AC-04.
        let env = Env::with("job-slot", "live", LIVE).await;
        env.movie_sized(1, "Film", &info(Container::Matroska, "h264"), 10_000)
            .await;
        let (tools, encoder) = env.tools(Ok(()), shrinks());
        install(&env, tools);
        let running = Running {
            title: "Anderer Film (2020)".to_owned(),
            path: "/filme/a.mkv".to_owned(),
            started_at: Timestamp::now().to_string(),
            runner: "cli".to_owned(),
            attempt: None,
        };
        fs::write(
            &env.app.reencode.lock,
            serde_json::to_string(&running).expect("json"),
        )
        .expect("lock");
        let wait = next_wait(&env.app).await;
        assert_eq!(wait, SLOT_POLL);
        assert!(
            wait <= Duration::from_secs(60),
            "looks again within one minute"
        );
        assert!(encoder.jobs.lock().expect("jobs").is_empty());
        assert_eq!(env.db().last_reencode().await.expect("read"), None);
        // Free again: the next look encodes at once and asks for the next round.
        fs::remove_file(&env.app.reencode.lock).expect("unlock");
        assert_eq!(next_wait(&env.app).await, Duration::ZERO);
        // Nothing left: wait an hour.
        assert_eq!(next_wait(&env.app).await, LOOK);
    }

    #[tokio::test]
    async fn missing_ffmpeg_waits_and_the_system_page_says_why() {
        // AC-07, AC-08.
        let env = Env::with("job-ffmpeg", "live", LIVE).await;
        env.movie_sized(1, "Film", &info(Container::Matroska, "h264"), 10_000)
            .await;
        assert_eq!(
            support_answer(&env.app),
            ReencodeSupport {
                active: true,
                message: None,
                running: None
            }
        );
        let (tools, encoder) = env.tools(
            Err("ffmpeg mit libx265 nicht gefunden".to_owned()),
            shrinks(),
        );
        install(&env, tools);
        assert_eq!(next_wait(&env.app).await, LOOK);
        assert!(encoder.jobs.lock().expect("jobs").is_empty());
        assert_eq!(
            support_answer(&env.app).message.as_deref(),
            Some("ffmpeg mit libx265 nicht gefunden")
        );
        // Installed later: the next look uses it without a restart.
        let (tools, _) = env.tools(Ok(()), shrinks());
        install(&env, tools);
        assert_eq!(next_wait(&env.app).await, Duration::ZERO);
        assert!(support_answer(&env.app).active);

        let off = Env::with("job-off", "live", "").await;
        assert_eq!(
            support_answer(&off.app).message.as_deref(),
            Some("in der Konfiguration ausgeschaltet")
        );
        let shadow = Env::with("job-shadow", "shadow", LIVE).await;
        assert_eq!(
            support_answer(&shadow.app).message.as_deref(),
            Some("im Shadow-Mode")
        );
        assert!(spawn_schedule(&shadow.app).is_none());
    }

    #[tokio::test]
    async fn the_system_page_names_the_running_encode() {
        // AC-08.
        let env = Env::with("job-system", "live", LIVE).await;
        let running = Running {
            title: "Gross (2019)".to_owned(),
            path: "/filme/Gross/Gross.mkv".to_owned(),
            started_at: Timestamp::now().to_string(),
            runner: "server".to_owned(),
            attempt: None,
        };
        fs::write(
            &env.app.reencode.lock,
            serde_json::to_string(&running).expect("json"),
        )
        .expect("lock");
        let http = crate::testkit::Http::start_shared(Arc::clone(&env.app)).await;
        let (status, body) = http.get("/api/system").await;
        assert_eq!(status, 200);
        assert_eq!(
            body["reencode"],
            serde_json::json!({ "active": true, "message": null, "running": "Gross (2019)" })
        );
        let job = body["jobs"]
            .as_array()
            .expect("jobs")
            .iter()
            .find(|j| j["name"] == "reencode")
            .expect("the job is listed")
            .clone();
        assert_eq!(job["label"], "Neu encodieren");
    }

    #[tokio::test]
    async fn the_start_repairs_interrupted_attempts_even_when_switched_off() {
        // AC-05, AC-07.
        let env = Env::with("job-start", "live", "").await;
        let original = info(Container::Matroska, "h264");
        let (id, video) = env.movie_sized(1, "Film", &original, 10_000).await;
        let new = NewReencode {
            file: FileRef::Movie(id),
            title: "Film (2019)".to_owned(),
            path: video.to_string_lossy().into_owned(),
            runner: "server".to_owned(),
            size_before: 10_000,
        };
        env.db()
            .start_reencode(&new, Timestamp::now())
            .await
            .expect("start");
        fs::rename(&video, video.with_file_name(".Film.mkv.reencode-original")).expect("aside");
        write_fake(
            &video.with_file_name(".Film.mkv.reencode-partial"),
            &hevc(),
            4000,
        );
        let (tools, encoder) = env.tools(Ok(()), shrinks());
        install(&env, tools);
        let handle = spawn_schedule(&env.app).expect("live mode");
        tokio::time::timeout(Duration::from_secs(10), handle)
            .await
            .expect("a switched-off job ends after the repair")
            .expect("ran");
        assert_eq!(env.files(), ["filme/Film/Film.mkv"]);
        let last = env
            .db()
            .last_reencode()
            .await
            .expect("read")
            .expect("attempt");
        assert_eq!(last.outcome, Some(ReencodeOutcome::Failed));
        assert_eq!(last.reason.as_deref(), Some("abgebrochen"));
        assert!(
            encoder.jobs.lock().expect("jobs").is_empty(),
            "switched off: no round"
        );
    }

    #[tokio::test]
    async fn a_stopping_server_stops_the_encoder() {
        // AC-06.
        let env = Env::with("job-stop", "live", LIVE).await;
        let original = info(Container::Matroska, "h264");
        let (id, video) = env.movie_sized(1, "Film", &original, 10_000).await;
        let waits: Behavior = Box::new(|job: &EncodeJob| {
            write_fake(&job.output, &hevc(), 1000);
            let cancel = job.cancel.clone().expect("a stop flag");
            while !cancel.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(EncodeError::Cancelled)
        });
        let (tools, _) = env.tools(Ok(()), waits);
        let tools = install(&env, tools);
        let prepared = run::prepare(
            &env.app,
            &tools,
            run::Target::Movie(id),
            run::Runner::Server,
        )
        .await
        .expect("prepare")
        .expect("started");
        let app = Arc::clone(&env.app);
        let task = tokio::spawn(async move { run::run(&app, &tools, prepared).await });
        while env.app.reencode.in_flight.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let started = Instant::now();
        run::stop(&env.app, Duration::from_secs(10)).await;
        assert!(started.elapsed() < Duration::from_secs(10));
        let report = task.await.expect("task").expect("run");
        assert_eq!(report.outcome, ReencodeOutcome::Failed);
        assert_eq!(
            report.reason.as_deref(),
            Some("abgebrochen: sofarr wurde beendet")
        );
        assert_eq!(env.files(), ["filme/Film/Film.mkv"]);
        assert_eq!(
            crate::reencode::run::tests::read_fake(&video)
                .expect("read")
                .info,
            original
        );
        let last = env
            .db()
            .last_reencode()
            .await
            .expect("read")
            .expect("attempt");
        assert_eq!(
            last.reason.as_deref(),
            Some("abgebrochen: sofarr wurde beendet")
        );
        // An interruption is no failure of the file: it stays due.
        let due = due_files(&env.app, Timestamp::now()).await.expect("due");
        assert_eq!(due.len(), 1);
    }

    #[tokio::test]
    async fn reencode_job_round_large_library() {
        // AC-10: 2,500 movies and 20,000 episode files, 10,000 attempts.
        let env = Env::with("job-large", "live", LIVE).await;
        let db = env.db();
        let info_json = serde_json::to_string(&info(Container::Matroska, "h264")).expect("json");
        let numbers = |n: u32| {
            format!("WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < {n})")
        };
        for statement in [
            format!(
                "{} INSERT INTO movies (id, tmdb_id, title, year, runtime, monitored, quality_profile_id, path, added_at)
                 SELECT i, i, 'Film ' || i, 2019, 100, 1, 1, '/filme/' || i, '' FROM n",
                numbers(2500)
            ),
            "INSERT INTO movie_files (movie_id, relative_path, size, quality, added_at)
             SELECT id, 'f.mkv', 1, 'Remux-1080p', '' FROM movies"
                .to_owned(),
            "INSERT INTO media_info (movie_id, info, size, modified, read_at)
             SELECT id, ?1, 100000 + id, '', '' FROM movies"
                .to_owned(),
            format!(
                "{} INSERT INTO series (id, tvdb_id, title, runtime, series_type, monitored, season_folder, quality_profile_id, path, added_at)
                 SELECT i, i, 'Serie ' || i, 45, 'standard', 1, 1, 1, '/serien/' || i, '' FROM n",
                numbers(100)
            ),
            format!(
                "{} INSERT INTO episode_files (id, series_id, relative_path, size, quality, added_at)
                 SELECT s.id * 1000 + n.i, s.id, 'e' || n.i || '.mkv', 1, 'WEBDL-1080p', '' FROM series s, n",
                numbers(200)
            ),
            "INSERT INTO episodes (series_id, season, number, monitored, file_id)
             SELECT series_id, 1, id % 1000, 1, id FROM episode_files"
                .to_owned(),
            "INSERT INTO media_info (episode_file_id, info, size, modified, read_at)
             SELECT id, ?1, 50000 + id, '', '' FROM episode_files"
                .to_owned(),
            "INSERT INTO reencode_attempts (movie_id, title, path, runner, started_at, finished_at, outcome, reason, size_before)
             SELECT id, 't', 'p', 'server', '2026-11-01T00:00:00Z', '2026-11-01T01:00:00Z', 'failed', 'x', 100000 + id FROM movies"
                .to_owned(),
            "INSERT INTO reencode_attempts (episode_file_id, title, path, runner, started_at, finished_at, outcome, size_before, size_after)
             SELECT id, 't', 'p', 'server', '2026-11-01T00:00:00Z', '2026-11-01T01:00:00Z', 'no_gain', 50000 + id, 1 FROM episode_files
             WHERE id % 1000 <= 75"
                .to_owned(),
        ] {
            sqlx::query(&statement)
                .bind(&info_json)
                .execute(db.pool())
                .await
                .expect("bulk insert");
        }
        let attempts = db.all_reencodes().await.expect("attempts").len();
        assert_eq!(attempts, 10_000);
        let started = Instant::now();
        let due = due_files(&env.app, "2026-11-17T12:00:00Z".parse().expect("now"))
            .await
            .expect("due");
        let took = started.elapsed();
        println!("re-encode job: due files of 22,500 with 10,000 attempts in {took:?}");
        // Movies failed once 16 days ago: due again; 7,500 episode files settled.
        assert_eq!(due.len(), 2500 + 20_000 - 7500);
        let limit = if cfg!(debug_assertions) { 20 } else { 2 };
        assert!(took.as_secs() < limit, "took {took:?}");
    }
}

mod system_and_race {
    //! The job's runs on the system page (AC-08) and a slot taken during a round (AC-03).

    use std::{fs, path::PathBuf, sync::Arc, time::Duration};

    use jiff::Timestamp;
    use sofarr_db::ReencodeOutcome;
    use sofarr_mediainfo::Container;

    use super::super::*;
    use crate::reencode::{
        ffmpeg::{EncodeError, Encoder, Job as EncodeJob},
        run::{
            Running, Tools,
            tests::{Env, hevc, info, read_fake, write_fake},
        },
    };

    const LIVE: &str = "[reencode]\nmax_mb_per_minute_1080p = 0.00001\nenabled = true\n";

    #[tokio::test]
    async fn the_system_page_shows_the_last_run_and_the_next_look() {
        let env = Env::with("job-runs", "live", LIVE).await;
        env.movie_sized(1, "Film", &info(Container::Matroska, "h264"), 10_000)
            .await;
        let app = Arc::clone(&env.app);
        let seen = Arc::new(std::sync::Mutex::new(None));
        let inside = Arc::clone(&seen);
        let behavior: crate::reencode::run::tests::Behavior = Box::new(move |job: &EncodeJob| {
            // While it encodes, the job has no next look.
            *inside.lock().expect("seen") =
                Some(app.status.job(crate::status::Job::Reencode).next_run);
            write_fake(&job.output, &hevc(), 4000);
            Ok(())
        });
        let (tools, _) = env.tools(Ok(()), behavior);
        *env.app.reencode.tools.lock().expect("tools") = Some(Arc::new(tools));
        let http = crate::testkit::Http::start_shared(Arc::clone(&env.app)).await;
        let job = |body: &serde_json::Value| {
            body["jobs"]
                .as_array()
                .expect("jobs")
                .iter()
                .find(|j| j["name"] == "reencode")
                .expect("listed")
                .clone()
        };
        let (_, before) = http.get("/api/system").await;
        assert_eq!(job(&before)["interval_minutes"], 60);
        assert!(job(&before)["last_run"].is_null());

        // A round that encodes: a last run, and no next look while it encodes.
        assert_eq!(next_wait(&env.app).await, Duration::ZERO);
        assert_eq!(*seen.lock().expect("seen"), Some(None));
        let (_, after) = http.get("/api/system").await;
        assert!(job(&after)["last_run"].is_string());
        assert_eq!(job(&after)["result"]["state"], "ok");

        // Nothing due: the next look an hour later.
        let looked = Timestamp::now();
        assert_eq!(next_wait(&env.app).await, LOOK);
        let (_, waiting) = http.get("/api/system").await;
        let next: Timestamp = job(&waiting)["next_run"]
            .as_str()
            .expect("next look")
            .parse()
            .expect("timestamp");
        let ahead = next.duration_since(looked).as_secs();
        assert!((3590..=3610).contains(&ahead), "{ahead} s");
    }

    /// An encoder whose check lets another encode take the slot, as a CLI may between the
    /// job's look and its start.
    struct LockingEncoder {
        lock: PathBuf,
    }

    impl Encoder for LockingEncoder {
        fn check(&self) -> Result<(), String> {
            let running = Running {
                title: "Anderer Film (2020)".to_owned(),
                path: "/filme/a.mkv".to_owned(),
                started_at: Timestamp::now().to_string(),
                runner: "cli".to_owned(),
                attempt: None,
            };
            fs::write(&self.lock, serde_json::to_string(&running).expect("json")).expect("lock");
            Ok(())
        }

        fn encode(&self, _: &EncodeJob) -> Result<(), EncodeError> {
            Err(EncodeError::Failed("must not run".to_owned()))
        }
    }

    #[tokio::test]
    async fn a_slot_taken_during_a_round_ends_it_without_a_rest() {
        let env = Env::with("job-race", "live", LIVE).await;
        env.movie_sized(1, "Gross", &info(Container::Matroska, "h264"), 20_000)
            .await;
        env.movie_sized(2, "Klein", &info(Container::Matroska, "h264"), 10_000)
            .await;
        let tools = Tools {
            encoder: Arc::new(LockingEncoder {
                lock: env.app.reencode.lock.clone(),
            }),
            read: Arc::new(read_fake),
        };
        let end = round(&env.app, &tools, Timestamp::now())
            .await
            .expect("round");
        assert!(matches!(end, RoundEnd::SlotHeld), "{end:?}");
        let attempts = env.db().all_reencodes().await.expect("attempts");
        assert_eq!(attempts.len(), 1, "the round ended at the first refusal");
        assert_eq!(attempts[0].outcome, Some(ReencodeOutcome::Refused));
        assert!(
            attempts[0]
                .reason
                .as_deref()
                .is_some_and(|r| r.starts_with(SLOT_TAKEN))
        );
        // That refusal does not make the file rest.
        fs::remove_file(&env.app.reencode.lock).expect("unlock");
        let due = due_files(&env.app, Timestamp::now()).await.expect("due");
        assert_eq!(due.len(), 2);
    }
}

mod stop_during_swap {
    //! AC-06: a swap already under way is finished before sofarr stops.

    use std::{
        fs,
        sync::{Arc, atomic::Ordering},
        time::Duration,
    };

    use sofarr_db::ReencodeOutcome;
    use sofarr_mediainfo::Container;

    use crate::reencode::{
        ffmpeg::Job as EncodeJob,
        run::{
            self, Tools,
            tests::{Env, hevc, info, read_fake, write_fake},
        },
    };

    #[tokio::test]
    async fn a_swap_under_way_is_finished_first() {
        let env = Env::new("stop-swap", "live").await;
        let (id, video) = env
            .movie("Film", "Film.mkv", &info(Container::Matroska, "h264"))
            .await;
        let (encoding, _) = env.tools(
            Ok(()),
            Box::new(|job: &EncodeJob| {
                write_fake(&job.output, &hevc(), 4000);
                Ok(())
            }),
        );
        // The stop comes after the encoder ended, while the result is read for its check.
        let stopping = Arc::clone(&env.app.reencode.stopping);
        let tools = Arc::new(Tools {
            encoder: encoding.encoder,
            read: Arc::new(move |path: &std::path::Path| {
                if path.to_string_lossy().ends_with(".reencode-partial") {
                    stopping.store(true, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(300));
                }
                read_fake(path)
            }),
        });
        let prepared = run::prepare(
            &env.app,
            &tools,
            run::Target::Movie(id),
            run::Runner::Server,
        )
        .await
        .expect("prepare")
        .expect("started");
        let app = Arc::clone(&env.app);
        let task = tokio::spawn(async move { run::run(&app, &tools, prepared).await });
        while !env.app.reencode.stopping.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        run::stop(&env.app, Duration::from_secs(10)).await;
        // `stop` waited for the swap: the attempt is over when it returns.
        assert_eq!(env.app.reencode.in_flight.load(Ordering::SeqCst), 0);
        let last = env
            .db()
            .last_reencode()
            .await
            .expect("read")
            .expect("attempt");
        assert_eq!(last.outcome, Some(ReencodeOutcome::Replaced));
        let report = task.await.expect("task").expect("run");
        assert_eq!(report.outcome, ReencodeOutcome::Replaced);
        assert_eq!(fs::metadata(&video).expect("video").len(), 4000);
        assert_eq!(
            env.files(),
            ["filme/Film/Film.mkv", "papierkorb/Film/Film.mkv"]
        );
    }
}
