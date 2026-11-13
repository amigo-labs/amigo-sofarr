//! The subtitle schedule: only monitored items (AC-01, AC-02) and the wake at the quota's
//! reset (AC-07).

use std::{path::Path, sync::Arc, time::Duration};

use jiff::{SignedDuration, Timestamp};
use sofarr_db::{Db, FileRef, MovieFile};

use super::{
    Scope,
    fake::{self, Shared, add_movie, result, secret},
    imported, next_wait, run,
};
use crate::{
    app::App,
    testkit::{self, Library},
};

const LIVE_DE: &str = "[automation]\nmode = \"live\"\n[subtitles]\nlanguages = [\"de\"]\n";

fn queries(fake: &Shared, needle: &str) -> usize {
    let fake = fake.lock().expect("lock");
    fake.queries.iter().filter(|q| q.contains(needle)).count()
}

#[tokio::test]
async fn unmonitored_movies_are_skipped_by_the_schedule_and_after_an_import() {
    let fake = fake::start(&secret()).await;
    let root = fake::temp("unmonitored-movie");
    let db = Db::in_memory().await.expect("db");
    let added = Timestamp::now() - SignedDuration::from_hours(1);
    add_movie(&db, &root, 1, "Der Film", added, None).await;
    let (unwatched, video) = add_movie(&db, &root, 2, "Anderer Film", added, None).await;
    db.set_monitored(unwatched, false).await.expect("unmonitor");
    let app = fake::app_on(&format!("{LIVE_DE}{}", fake::account_toml(&fake)), db);
    fake.lock()
        .expect("lock")
        .results
        .insert("de".to_owned(), vec![result(1, "de", "")]);

    // The scheduled run searches the monitored movie only (AC-02).
    let report = run(&app, Scope::All, Timestamp::now()).await.expect("run");
    assert_eq!(report.downloaded, ["Der Film Bluray-1080p.de.srt"]);
    assert_eq!(queries(&fake, "imdb_id=1&"), 1);
    assert_eq!(queries(&fake, "imdb_id=2&"), 0, "unmonitored");
    let needs = app.db.subtitle_needs(None).await.expect("needs");
    assert!(needs.is_empty(), "{needs:?}");

    // An import of the unmonitored movie's file searches nothing either (AC-01).
    imported(&app, FileRef::Movie(unwatched)).await;
    let report = run(&app, Scope::Pending, Timestamp::now())
        .await
        .expect("pending");
    assert_eq!(report.searched, 0);
    assert_eq!(queries(&fake, "imdb_id=2&"), 0, "unmonitored");
    assert!(
        !video
            .with_file_name("Anderer Film Bluray-1080p.de.srt")
            .exists()
    );

    // Monitored, the same file is searched after its import.
    app.db
        .set_monitored(unwatched, true)
        .await
        .expect("monitor");
    imported(&app, FileRef::Movie(unwatched)).await;
    let report = run(&app, Scope::Pending, Timestamp::now())
        .await
        .expect("pending");
    assert_eq!(report.downloaded, ["Anderer Film Bluray-1080p.de.srt"]);
    let _ = std::fs::remove_dir_all(&root);
}

/// Kleine Insel in `root` with S01E01 and S01E02 on disk; returns the series id and the
/// episodes' ids and file ids.
async fn series_on_disk(app: &App, root: &Path) -> (i64, [(i64, i64); 2]) {
    let mut show = app.db.all_series().await.expect("series")[0].clone();
    show.data.path = root.to_string_lossy().into_owned();
    app.db
        .upsert_series(&show.data, Timestamp::now())
        .await
        .expect("move");
    let season = root.join("Staffel 01");
    std::fs::create_dir_all(&season).expect("season");
    std::fs::write(season.join("Kleine Insel - S01E01.mkv"), b"1").expect("e1");
    std::fs::write(season.join("Kleine Insel - S01E02.mkv"), b"2").expect("e2");
    let episodes = app.db.episodes(Some(show.id)).await.expect("episodes");
    let first = episodes.iter().find(|e| e.data.number == 1).expect("E01");
    let second = episodes.iter().find(|e| e.data.number == 2).expect("E02");
    let file = app
        .db
        .set_episode_file(
            show.id,
            &[second.id],
            &MovieFile {
                relative_path: "Staffel 01/Kleine Insel - S01E02.mkv".to_owned(),
                size: 1,
                quality: "WEBDL-1080p".to_owned(),
                revision_version: 1,
                revision_real: 0,
                release_group: None,
                scene_name: None,
                custom_format_score: 0,
            },
            Timestamp::now(),
        )
        .await
        .expect("file");
    (
        show.id,
        [
            (first.id, first.file_id.expect("E01 file")),
            (second.id, file),
        ],
    )
}

#[tokio::test]
async fn episodes_of_unmonitored_series_or_unmonitored_episodes_are_skipped() {
    let fake = fake::start(&secret()).await;
    let root = fake::temp("unmonitored-episodes");
    let config = format!("{LIVE_DE}{}", fake::account_toml(&fake));
    let app = testkit::app(Library::Series, &config).await;
    let (show, [_, (e2, file2)]) = series_on_disk(&app, &root).await;
    let season = root.join("Staffel 01");
    fake.lock()
        .expect("lock")
        .results
        .insert("de".to_owned(), vec![result(1, "de", "")]);

    // An unmonitored episode is skipped by the schedule and after its import.
    app.db
        .set_episodes_monitored(&[e2], false)
        .await
        .expect("unmonitor E02");
    let report = run(&app, Scope::All, Timestamp::now()).await.expect("run");
    assert_eq!(report.downloaded, ["Kleine Insel - S01E01.de.srt"]);
    assert_eq!(queries(&fake, "episode_number=2"), 0, "unmonitored episode");
    imported(&app, FileRef::Episode(file2)).await;
    let report = run(&app, Scope::Pending, Timestamp::now())
        .await
        .expect("pending");
    assert_eq!(report.searched, 0);

    // In an unmonitored series even a monitored episode is skipped.
    app.db
        .set_episodes_monitored(&[e2], true)
        .await
        .expect("monitor E02");
    let mut series = app
        .db
        .all_series()
        .await
        .expect("series")
        .into_iter()
        .find(|s| s.id == show)
        .expect("show");
    series.data.monitored = false;
    app.db
        .upsert_series(&series.data, Timestamp::now())
        .await
        .expect("unmonitor series");
    let report = run(&app, Scope::All, Timestamp::now()).await.expect("run");
    assert_eq!(report.searched, 0);
    imported(&app, FileRef::Episode(file2)).await;
    let report = run(&app, Scope::Pending, Timestamp::now())
        .await
        .expect("pending");
    assert_eq!(report.searched, 0);
    assert_eq!(queries(&fake, "episode_number=2"), 0, "unmonitored series");
    assert!(!season.join("Kleine Insel - S01E02.de.srt").exists());

    // Monitored again, the episode's subtitle comes with the next run.
    series.data.monitored = true;
    app.db
        .upsert_series(&series.data, Timestamp::now())
        .await
        .expect("monitor series");
    let report = run(&app, Scope::All, Timestamp::now()).await.expect("run");
    assert_eq!(report.downloaded, ["Kleine Insel - S01E02.de.srt"]);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn the_schedule_waits_for_the_quota_reset_also_without_an_interval() {
    let fake = fake::start(&secret()).await;
    let app = fake::app_on(
        &fake::account_toml(&fake),
        Db::in_memory().await.expect("db"),
    );
    let now = Timestamp::now();
    let twelve = Duration::from_secs(12 * 3600);
    let three = Duration::from_secs(3 * 3600);
    // Before the first run: at once; after it: the interval; without one: only imports.
    assert_eq!(
        next_wait(&app, None, Some(twelve), now),
        Some(Duration::ZERO)
    );
    let last = Some(tokio::time::Instant::now());
    let by_interval = next_wait(&app, last, Some(twelve), now).expect("interval");
    assert!(by_interval <= twelve && by_interval > twelve - Duration::from_secs(60));
    assert_eq!(next_wait(&app, last, None, now), None);
    // A used-up quota wakes the schedule at its reset when that comes first (AC-07).
    app.subtitles
        .set_quota_until(now + SignedDuration::from_hours(3));
    assert_eq!(next_wait(&app, last, Some(twelve), now), Some(three));
    assert_eq!(next_wait(&app, last, None, now), Some(three));
    // A reset after the interval's end changes nothing.
    let hour = Duration::from_secs(3600);
    let by_interval = next_wait(&app, last, Some(hour), now).expect("interval");
    assert!(by_interval <= hour && by_interval > hour - Duration::from_secs(60));
    // A reset in the past is no reason to wake.
    app.subtitles
        .set_quota_until(now - SignedDuration::from_hours(1));
    assert_eq!(next_wait(&app, last, None, now), None);
}

/// Waits up to ten seconds for `done`.
async fn eventually(mut done: impl FnMut() -> bool) -> bool {
    for _ in 0..500 {
        if done() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    done()
}

#[tokio::test]
async fn the_running_schedule_continues_at_the_quota_reset() {
    // interval_hours = 0: nothing but the quota's reset wakes the schedule here (AC-07).
    let fake = fake::start(&secret()).await;
    let root = fake::temp("quota-wake");
    let db = Db::in_memory().await.expect("db");
    let added = Timestamp::now() - SignedDuration::from_hours(1);
    let (_, video) = add_movie(&db, &root, 1, "Der Film", added, None).await;
    let toml = format!("{LIVE_DE}interval_hours = 0\n{}", fake::account_toml(&fake));
    let app: Arc<App> = fake::app_on(&toml, db);
    let reset = Timestamp::now() + SignedDuration::from_secs(2);
    {
        let mut fake = fake.lock().expect("lock");
        fake.remaining = 0;
        fake.reset = reset.to_string();
        fake.results
            .insert("de".to_owned(), vec![result(1, "de", "")]);
    }
    let report = run(&app, Scope::All, Timestamp::now()).await.expect("run");
    assert_eq!(report.waiting_for_quota, 1);
    fake.lock().expect("lock").remaining = 5;
    let handle = super::spawn_schedule(&app).expect("scheduled");
    let srt = video.with_file_name("Der Film Bluray-1080p.de.srt");
    let stored = eventually(|| srt.is_file()).await;
    handle.abort();
    assert!(stored, "not downloaded after the reset");
    let _ = std::fs::remove_dir_all(&root);
}
