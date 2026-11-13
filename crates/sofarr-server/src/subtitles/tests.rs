//! Subtitle runs against a fake OpenSubtitles.com (spec subtitles).

use std::{path::PathBuf, sync::Arc};

use jiff::{SignedDuration, Timestamp};
use serde_json::json;
use sofarr_db::{Db, FileRef, SubtitleKind, SubtitleNeed};
use sofarr_mediainfo::MediaInfo;

use super::{
    Scope,
    fake::{self, SRT, Shared, VIDEO_HASH, add_movie, media, names, result, secret},
    run,
};
use crate::app::App;

fn now() -> Timestamp {
    "2026-11-12T12:00:00Z".parse().expect("now")
}

fn hours(h: i64) -> SignedDuration {
    SignedDuration::from_hours(h)
}

struct Setup {
    app: Arc<App>,
    fake: Shared,
    root: PathBuf,
    movie: i64,
    video: PathBuf,
}

impl Setup {
    /// One movie `Der Film`, imported an hour ago, with this media info.
    async fn new(name: &str, toml: &str, info: Option<&MediaInfo>) -> Self {
        Self::added(name, toml, info, now() - hours(1)).await
    }

    async fn added(name: &str, toml: &str, info: Option<&MediaInfo>, added: Timestamp) -> Self {
        let fake = fake::start(&secret()).await;
        let root = fake::temp(name);
        let db = Db::in_memory().await.expect("db");
        let (movie, video) = add_movie(&db, &root, 1, "Der Film", added, info).await;
        let toml = format!("{toml}\n{}", fake::account_toml(&fake));
        let app = fake::app_on(&toml, db);
        Self {
            app,
            fake,
            root,
            movie,
            video,
        }
    }

    fn folder(&self) -> PathBuf {
        self.video.parent().expect("folder").to_path_buf()
    }

    async fn needs(&self) -> Vec<SubtitleNeed> {
        self.app
            .db
            .subtitle_needs(Some(FileRef::Movie(self.movie)))
            .await
            .expect("needs")
    }

    fn queries(&self, needle: &str) -> usize {
        let fake = self.fake.lock().expect("lock");
        fake.queries.iter().filter(|q| q.contains(needle)).count()
    }
}

const LIVE: &str = "[automation]\nmode = \"live\"\n";

fn need<'a>(needs: &'a [SubtitleNeed], language: &str, kind: SubtitleKind) -> &'a SubtitleNeed {
    needs
        .iter()
        .find(|n| n.language == language && n.kind == kind)
        .unwrap_or_else(|| panic!("{language} {kind:?} not needed: {needs:?}"))
}

#[tokio::test]
async fn a_run_stores_the_best_candidate_in_utf8_next_to_the_video() {
    // English audio: German is needed, and a forced English subtitle (AC-02, AC-04).
    let s = Setup::new("best", LIVE, Some(&media(&["eng"], &[]))).await;
    {
        let mut fake = s.fake.lock().expect("lock");
        let mut hashed = result(3, "de", "");
        hashed["attributes"]["moviehash_match"] = json!(true);
        fake.results.insert(
            "de".to_owned(),
            vec![
                result(1, "de", "Der.Film.2019.720p.WEB.h264-OTHER"),
                result(2, "de", "Der.Film.2019.1080p.BluRay.x264-GRP"),
                hashed,
            ],
        );
        // Windows-1252: "Grüße".
        let mut latin = b"1\n00:00:01,000 --> 00:00:02,000\nGr".to_vec();
        latin.extend_from_slice(&[0xfc, 0xdf, 0x65, b'\n']);
        fake.files.insert(3, latin);
    }
    let report = run(&s.app, Scope::All, now()).await.expect("run");
    assert_eq!(report.downloaded, ["Der Film Bluray-1080p.de.srt"]);
    assert_eq!(report.not_found, 1);
    assert_eq!(s.fake.lock().expect("lock").downloads, [3]);
    let stored = std::fs::read_to_string(s.folder().join("Der Film Bluray-1080p.de.srt"))
        .expect("stored as UTF-8");
    assert!(stored.contains("Grüße"), "{stored}");
    let own = s
        .app
        .db
        .own_subtitles(Some(FileRef::Movie(s.movie)))
        .await
        .expect("own");
    assert_eq!(own.len(), 1);
    assert_eq!(
        (
            own[0].language.as_str(),
            own[0].kind,
            own[0].match_quality.as_str()
        ),
        ("de", SubtitleKind::Normal, "hash")
    );
    assert_eq!(own[0].provider_id, "3");
    // The search named the movie and its hash.
    assert_eq!(s.queries(&format!("moviehash={VIDEO_HASH}")), 2);
    assert_eq!(s.queries("imdb_id=1&"), 2);
    // The forced English subtitle has no candidate: searched again tomorrow (AC-05).
    let needs = s.needs().await;
    let forced = need(&needs, "en", SubtitleKind::Forced);
    assert_eq!(forced.found, Some(false));
    assert_eq!(forced.last_search, Some(now()));
    assert_eq!(forced.next_search, Some(now() + hours(24)));
    assert!(
        !needs
            .iter()
            .any(|n| n.language == "de" && n.kind == SubtitleKind::Normal),
        "covered by the own subtitle now: {needs:?}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

#[tokio::test]
async fn covered_languages_are_never_searched() {
    // German DL: German and English audio; only a German forced subtitle may be needed
    // (AC-03).
    let s = Setup::new("covered", LIVE, Some(&media(&["ger", "eng"], &[]))).await;
    let mut forced = result(7, "de", "");
    forced["attributes"]["foreign_parts_only"] = json!(true);
    s.fake
        .lock()
        .expect("lock")
        .results
        .insert("de-forced".to_owned(), vec![forced]);
    let report = run(&s.app, Scope::All, now()).await.expect("run");
    assert_eq!(report.searched, 1);
    assert_eq!(s.queries("foreign_parts_only=exclude"), 0);
    assert_eq!(report.downloaded, ["Der Film Bluray-1080p.de.forced.srt"]);
    // The next run finds everything covered: no search, still one own subtitle.
    let again = run(&s.app, Scope::All, now() + hours(48))
        .await
        .expect("again");
    assert_eq!(again.searched, 0);
    assert_eq!(s.app.db.own_subtitles(None).await.expect("own").len(), 1);
    assert!(s.needs().await.is_empty());
    let _ = std::fs::remove_dir_all(&s.root);
}

#[tokio::test]
async fn needs_without_candidates_are_retried_daily_then_weekly() {
    let toml = format!("{LIVE}[subtitles]\nlanguages = [\"de\"]\n");
    // Imported ten days ago: a week after the search (AC-05).
    let old = Setup::added("weekly", &toml, None, now() - hours(240)).await;
    run(&old.app, Scope::All, now()).await.expect("run");
    let needs = old.needs().await;
    assert_eq!(
        need(&needs, "de", SubtitleKind::Normal).next_search,
        Some(now() + hours(168))
    );
    run(&old.app, Scope::All, now() + hours(24))
        .await
        .expect("not due");
    assert_eq!(old.queries("languages=de"), 1);
    run(&old.app, Scope::All, now() + hours(169))
        .await
        .expect("due");
    assert_eq!(old.queries("languages=de"), 2);
    let _ = std::fs::remove_dir_all(&old.root);
}

#[tokio::test]
async fn foreign_subtitles_cover_their_language_and_stay_untouched() {
    let s = Setup::new("foreign", LIVE, Some(&media(&["eng"], &[]))).await;
    let foreign = s.folder().join("Der Film Bluray-1080p.DE.srt");
    std::fs::write(&foreign, b"fremd").expect("foreign");
    s.fake
        .lock()
        .expect("lock")
        .results
        .insert("de".to_owned(), vec![result(1, "de", "")]);
    let report = run(&s.app, Scope::All, now()).await.expect("run");
    assert_eq!(s.queries("languages=de"), 0, "de is covered (AC-06)");
    assert!(report.downloaded.is_empty());
    assert_eq!(std::fs::read(&foreign).expect("kept"), b"fremd");
    let _ = std::fs::remove_dir_all(&s.root);
}

#[tokio::test]
async fn a_used_up_quota_waits_for_the_reset_newest_first() {
    let toml = format!("{LIVE}[subtitles]\nlanguages = [\"de\"]\n");
    let s = Setup::added("quota", &toml, None, now() - hours(2)).await;
    // A second, newer movie.
    let (newer, newer_video) =
        add_movie(&s.app.db, &s.root, 2, "Neuer Film", now() - hours(1), None).await;
    let reset = now() + hours(5);
    {
        let mut fake = s.fake.lock().expect("lock");
        fake.remaining = 1;
        fake.reset = reset.to_string();
        fake.results
            .insert("de".to_owned(), vec![result(1, "de", "")]);
    }
    let report = run(&s.app, Scope::All, now()).await.expect("run");
    // The newest file gets the last download; the other one waits (AC-07).
    assert_eq!(report.downloaded, ["Neuer Film Bluray-1080p.de.srt"]);
    assert_eq!(report.waiting_for_quota, 1);
    assert!(
        newer_video
            .with_file_name("Neuer Film Bluray-1080p.de.srt")
            .is_file()
    );
    assert_eq!(s.app.subtitles.quota_until(now()), Some(reset));
    let waiting = s.needs().await;
    assert_eq!(waiting[0].found, Some(true));
    assert_eq!(waiting[0].next_search, None, "stays due (AC-05)");
    let _ = newer;

    // Before the reset: searches go on, nothing is downloaded.
    let before = run(&s.app, Scope::All, now() + hours(1))
        .await
        .expect("before");
    assert_eq!((before.searched, before.waiting_for_quota), (1, 1));
    assert_eq!(s.fake.lock().expect("lock").downloads.len(), 1);

    // After the reset the waiting subtitle comes.
    s.fake.lock().expect("lock").remaining = 5;
    let after = run(&s.app, Scope::All, reset + hours(1))
        .await
        .expect("after");
    assert_eq!(after.downloaded, ["Der Film Bluray-1080p.de.srt"]);
    let _ = std::fs::remove_dir_all(&s.root);
}

#[tokio::test]
async fn a_quota_refusal_stops_downloads_until_the_reset() {
    let s = Setup::new("refused", LIVE, None).await;
    let reset = now() + hours(3);
    {
        let mut fake = s.fake.lock().expect("lock");
        fake.remaining = 0;
        fake.reset = reset.to_string();
        fake.results
            .insert("de".to_owned(), vec![result(1, "de", "")]);
        fake.results
            .insert("en".to_owned(), vec![result(2, "en", "")]);
    }
    let report = run(&s.app, Scope::All, now()).await.expect("run");
    // The first download is refused; the second need is searched but not downloaded.
    assert_eq!(report.searched, 2);
    assert_eq!(report.waiting_for_quota, 2);
    assert_eq!(s.fake.lock().expect("lock").downloads, [1]);
    assert_eq!(s.app.subtitles.quota_until(now()), Some(reset));
    assert!(s.needs().await.iter().all(|n| n.next_search.is_none()));
    let _ = std::fs::remove_dir_all(&s.root);
}

#[tokio::test]
async fn provider_failures_stop_the_run_and_keep_what_was_stored() {
    let s = Setup::new("failure", LIVE, None).await;
    {
        let mut fake = s.fake.lock().expect("lock");
        fake.results
            .insert("de".to_owned(), vec![result(1, "de", "")]);
        fake.search_status.insert("en".to_owned(), 503);
    }
    let error = run(&s.app, Scope::All, now()).await.expect_err("stops");
    assert!(
        format!("{error:#}").contains("Serverfehler (HTTP 503)"),
        "{error:#}"
    );
    // German came before the failure and stays (AC-08).
    assert!(s.folder().join("Der Film Bluray-1080p.de.srt").is_file());
    assert_eq!(s.app.db.own_subtitles(None).await.expect("own").len(), 1);
    let needs = s.needs().await;
    assert_eq!(need(&needs, "en", SubtitleKind::Normal).last_search, None);

    // A refused login names no credentials.
    let refused = Setup::new("login", LIVE, None).await;
    let password = refused.fake.lock().expect("lock").password.clone();
    {
        let mut fake = refused.fake.lock().expect("lock");
        fake.login_status = Some(401);
        fake.results
            .insert("de".to_owned(), vec![result(1, "de", "")]);
    }
    let error = run(&refused.app, Scope::All, now())
        .await
        .expect_err("stops");
    let message = format!("{error:#}");
    assert!(message.contains("lehnt die Anmeldung ab"), "{message}");
    assert!(
        !message.contains(&password) && !message.contains("anna-k"),
        "{message}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
    let _ = std::fs::remove_dir_all(&refused.root);
}

#[tokio::test]
async fn shadow_mode_searches_but_writes_nothing() {
    let s = Setup::new("shadow", "", Some(&media(&["eng"], &[]))).await;
    s.fake.lock().expect("lock").results.insert(
        "de".to_owned(),
        vec![result(1, "de", "Der.Film.2019.1080p.BluRay.x264-GRP")],
    );
    let report = run(&s.app, Scope::All, now()).await.expect("run");
    assert_eq!(
        report.would_download,
        ["Der.Film.2019.1080p.BluRay.x264-GRP"]
    );
    assert!(report.downloaded.is_empty());
    assert!(s.fake.lock().expect("lock").downloads.is_empty());
    assert_eq!(names(&s.folder()), ["Der Film Bluray-1080p.mkv"]);
    let needs = s.needs().await;
    let de = need(&needs, "de", SubtitleKind::Normal);
    assert_eq!(
        de.would_download.as_deref(),
        Some("Der.Film.2019.1080p.BluRay.x264-GRP")
    );
    assert_eq!(de.next_search, None, "stays due (AC-05, AC-13)");
    let _ = std::fs::remove_dir_all(&s.root);
}

#[tokio::test]
async fn broken_candidates_are_discarded_and_the_next_one_tried() {
    let s = Setup::new("broken", LIVE, None).await;
    {
        let mut fake = s.fake.lock().expect("lock");
        let ranked: Vec<_> = (1..=4)
            .map(|id| {
                let mut r = result(id, "de", "");
                r["attributes"]["download_count"] = json!(100 - id);
                r
            })
            .collect();
        fake.results.insert("de".to_owned(), ranked);
        fake.download_status.insert(1, 404);
        fake.files.insert(2, b"PK\x03\x04\x00\x00binary".to_vec());
        fake.files.insert(3, Vec::new());
        fake.results
            .insert("en".to_owned(), vec![result(5, "en", "")]);
        fake.download_status.insert(5, 410);
    }
    let report = run(&s.app, Scope::All, now()).await.expect("run");
    assert_eq!(s.fake.lock().expect("lock").downloads, [1, 2, 3, 4, 5]);
    assert_eq!(report.failed_candidates, 4);
    assert_eq!(report.downloaded, ["Der Film Bluray-1080p.de.srt"]);
    assert_eq!(
        names(&s.folder()),
        ["Der Film Bluray-1080p.de.srt", "Der Film Bluray-1080p.mkv"]
    );
    assert_eq!(
        std::fs::read_to_string(s.folder().join("Der Film Bluray-1080p.de.srt")).expect("srt"),
        SRT
    );
    let own = s.app.db.own_subtitles(None).await.expect("own");
    assert_eq!(
        own.iter()
            .map(|o| o.provider_id.as_str())
            .collect::<Vec<_>>(),
        ["4"]
    );
    // Every English candidate failed: searched again like one without candidates (AC-14).
    let needs = s.needs().await;
    assert_eq!(
        need(&needs, "en", SubtitleKind::Normal).next_search,
        Some(now() + hours(24))
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

#[tokio::test]
async fn switched_off_subtitles_search_nothing() {
    let s = Setup::new(
        "off",
        &format!("{LIVE}[subtitles]\nenabled = false\n"),
        None,
    )
    .await;
    assert!(s.app.opensubtitles.is_none());
    let report = run(&s.app, Scope::All, now()).await.expect("run");
    assert_eq!(report, super::RunReport::default());
    assert!(s.fake.lock().expect("lock").queries.is_empty());
    assert_eq!(crate::status::Job::Subtitles.interval(&s.app), None);
    assert!(super::off_reason(&s.app.config).is_some_and(|r| r.contains("enabled = false")));
    let without = crate::config::Config::parse("").expect("defaults");
    assert!(super::off_reason(&without).is_some_and(|r| r.contains("[opensubtitles]")));
    // On by default with an account, every interval_hours.
    let on = Setup::new("on", LIVE, None).await;
    assert_eq!(
        crate::status::Job::Subtitles.interval(&on.app),
        Some(std::time::Duration::from_secs(12 * 3600))
    );
    let _ = std::fs::remove_dir_all(&s.root);
    let _ = std::fs::remove_dir_all(&on.root);
}

#[tokio::test]
async fn a_deleted_own_subtitle_is_needed_again() {
    let toml = format!("{LIVE}[subtitles]\nlanguages = [\"de\"]\n");
    let s = Setup::new("deleted", &toml, None).await;
    s.fake
        .lock()
        .expect("lock")
        .results
        .insert("de".to_owned(), vec![result(1, "de", "")]);
    run(&s.app, Scope::All, now()).await.expect("run");
    let srt = s.folder().join("Der Film Bluray-1080p.de.srt");
    std::fs::remove_file(&srt).expect("owner deletes it");
    // Without a candidate now: the record is gone and the need is back.
    s.fake.lock().expect("lock").results.clear();
    let again = run(&s.app, Scope::All, now() + hours(1))
        .await
        .expect("again");
    assert_eq!((again.searched, again.not_found), (1, 1));
    assert!(s.app.db.own_subtitles(None).await.expect("own").is_empty());
    s.fake
        .lock()
        .expect("lock")
        .results
        .insert("de".to_owned(), vec![result(1, "de", "")]);
    let later = run(&s.app, Scope::All, now() + hours(26))
        .await
        .expect("later");
    assert_eq!(later.downloaded, ["Der Film Bluray-1080p.de.srt"]);
    assert!(srt.is_file());
    assert_eq!(s.app.db.own_subtitles(None).await.expect("own").len(), 1);
    let _ = std::fs::remove_dir_all(&s.root);
}

#[tokio::test]
async fn a_video_that_is_not_there_is_left_alone() {
    let toml = format!(
        "{LIVE}[subtitles]
languages = [\"de\"]
"
    );
    let s = Setup::new("absent", &toml, None).await;
    s.fake
        .lock()
        .expect("lock")
        .results
        .insert("de".to_owned(), vec![result(1, "de", "")]);
    run(&s.app, Scope::All, now()).await.expect("run");
    // The library folder is gone (e.g. an unmounted share): nothing is searched, the
    // own subtitle stays recorded.
    std::fs::remove_dir_all(s.folder()).expect("gone");
    s.app
        .db
        .set_subtitle_needs(
            FileRef::Movie(s.movie),
            &[("de".to_owned(), SubtitleKind::Normal)],
        )
        .await
        .expect("needs");
    let report = run(&s.app, Scope::All, now() + hours(1))
        .await
        .expect("run");
    assert_eq!(report, super::RunReport::default());
    assert_eq!(s.app.db.own_subtitles(None).await.expect("own").len(), 1);
    let _ = std::fs::remove_dir_all(&s.root);
}
