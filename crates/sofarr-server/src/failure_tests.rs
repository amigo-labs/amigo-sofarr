//! Failure handling in the download check, at grab time and in the RSS sync
//! (spec fail-handling: AC-01 to AC-05, AC-10, AC-11, AC-22).

use jiff::{SignedDuration, Timestamp};
use sofarr_db::{GrabStatus, MediaRef, NewBlock};

use crate::testkit::{Library, Services, app};

const G1: &str = "Erster.Film.2001.German.1080p.BluRay.x264-GRP";
const G2: &str = "Erster.Film.2001.1080p.WEB-DL.x264-LOW";
const S1: &str = "Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP";

fn live(s: &Services, extra: &str) -> String {
    format!(
        "[automation]\nmode = \"live\"\n{}{}{extra}",
        s.indexer_toml("idx", &[]),
        s.nzbget_toml()
    )
}

/// A fake serving both Erster Film releases (G1 is the better one).
async fn movie_services() -> Services {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[
            s.item(G1, "g1", 5_000_000_000),
            s.item(G2, "g2", 4_000_000_000),
        ],
    );
    s
}

fn block(media: MediaRef, title: &str, guid: &str) -> NewBlock {
    NewBlock {
        media,
        title: title.to_owned(),
        normalized_title: sofarr_core::normalize_release_title(title),
        guid: guid.to_owned(),
        indexer: "idx".to_owned(),
        reason: "Test".to_owned(),
    }
}

async fn movie_id(app: &crate::app::App, tmdb: i64) -> i64 {
    app.db
        .movie_by_tmdb(tmdb)
        .await
        .expect("db")
        .expect("movie")
        .id
}

#[tokio::test]
async fn release_failure_fails_the_grab_and_blocklists_the_release() {
    let s = movie_services().await;
    let app = app(Library::Movies, &live(&s, "")).await;
    app.rss_sync().await.expect("rss");
    assert_eq!(s.lock().appended, [format!("{G1}.nzb")]);
    s.finish(77, G1, "FAILURE/PAR", "/remote/x");
    app.check_downloads().await.expect("check");

    let grabs = app.db.grabs(None, 10).await.expect("grabs");
    assert_eq!(grabs[0].status, GrabStatus::Failed);
    assert_eq!(grabs[0].message.as_deref(), Some("FAILURE/PAR"));
    let blocks = app.db.blocks().await.expect("blocks");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].media, MediaRef::Movie(movie_id(&app, 101).await));
    assert_eq!(blocks[0].title, G1);
    assert_eq!(blocks[0].reason, "FAILURE/PAR");
}

#[tokio::test]
async fn release_failure_of_a_series_grab_blocklists_for_the_series() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(S1, "s1", 2_000_000_000)]);
    let app = app(Library::Series, &live(&s, "")).await;
    app.rss_sync().await.expect("rss");
    s.finish(77, S1, "DELETED/MANUAL", "/remote/x");
    app.check_downloads().await.expect("check");

    let grabs = app.db.series_grabs(None).await.expect("grabs");
    assert_eq!(grabs[0].status, GrabStatus::Failed);
    assert_eq!(grabs[0].message.as_deref(), Some("DELETED/MANUAL"));
    let blocks = app.db.blocks().await.expect("blocks");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].media, MediaRef::Series(grabs[0].series_id));
}

#[tokio::test]
async fn local_failure_fails_without_blocklist_and_frees_the_release() {
    let s = movie_services().await;
    let app = app(Library::Movies, &live(&s, "")).await;
    app.rss_sync().await.expect("rss");
    s.finish(77, G1, "DELETED/DUPE", "/remote/x");
    app.check_downloads().await.expect("check");

    let grabs = app.db.grabs(None, 10).await.expect("grabs");
    assert_eq!(grabs[0].status, GrabStatus::Failed);
    assert_eq!(grabs[0].message.as_deref(), Some("DELETED/DUPE"));
    assert!(app.db.blocks().await.expect("blocks").is_empty());
    // AC-22: a grab that failed without a blocklist entry does not hold its GUID.
    app.rss_sync().await.expect("rss again");
    assert_eq!(
        s.lock().appended,
        [format!("{G1}.nzb"), format!("{G1}.nzb")]
    );
}

#[tokio::test]
async fn broken_nzb_at_grab_time_blocklists() {
    for (status, body) in [(Some(404), None), (Some(410), None), (None, Some(300))] {
        let s = movie_services().await;
        {
            let mut fake = s.lock();
            if let Some(status) = status {
                fake.nzb_status.insert("g1".to_owned(), status);
            }
            if let Some(code) = body {
                fake.nzb_body.insert(
                    "g1".to_owned(),
                    format!(r#"<error code="{code}" description="No such item"/>"#),
                );
            }
        }
        let app = app(Library::Movies, &live(&s, "")).await;
        app.rss_sync().await.expect("rss");
        let grab = &app.db.grabs(None, 10).await.expect("grabs")[0];
        assert_eq!(grab.status, GrabStatus::Failed, "{status:?} {body:?}");
        let message = grab.message.as_deref().unwrap_or_default();
        assert!(message.contains("NZB nicht mehr verfügbar"), "{message}");
        let blocks = app.db.blocks().await.expect("blocks");
        assert_eq!(blocks.len(), 1, "{status:?} {body:?}");
        assert_eq!(blocks[0].reason, message);
    }
}

#[tokio::test]
async fn nzb_rejected_by_the_download_client_blocklists() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(S1, "s1", 2_000_000_000)]);
    s.lock().append_result = Some(0);
    let app = app(Library::Series, &live(&s, "")).await;
    app.rss_sync().await.expect("rss");
    let grab = &app.db.series_grabs(None).await.expect("grabs")[0];
    assert_eq!(grab.status, GrabStatus::Failed);
    assert_eq!(app.db.blocks().await.expect("blocks").len(), 1);
}

#[tokio::test]
async fn outage_at_grab_time_fails_without_blocklist_or_search() {
    let config = "[download]\nredownload_failed_movies = true\n";
    // The indexer refuses the NZB for now, or nzbget is down.
    for nzbget_down in [false, true] {
        let s = movie_services().await;
        if nzbget_down {
            s.lock().nzbget_down = true;
        } else {
            s.lock().nzb_status.insert("g1".to_owned(), 503);
        }
        let app = app(Library::Movies, &live(&s, config)).await;
        app.rss_sync().await.expect("rss");
        let grab = &app.db.grabs(None, 10).await.expect("grabs")[0];
        assert_eq!(grab.status, GrabStatus::Failed);
        assert!(grab.message.is_some());
        assert!(app.db.blocks().await.expect("blocks").is_empty());
        assert!(s.requests_with("t=movie").is_empty(), "no re-search");
        // Back minutes later: the next RSS sync may take the same release.
        {
            let mut fake = s.lock();
            fake.nzbget_down = false;
            fake.nzb_status.clear();
        }
        app.rss_sync().await.expect("rss again");
        assert_eq!(
            s.lock().appended.last().map(String::as_str),
            Some(format!("{G1}.nzb").as_str())
        );
    }
}

#[tokio::test]
async fn blocklisted_releases_are_skipped_by_title_per_movie() {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[
            // G1 under another GUID with other separators: still blocklisted.
            s.item(
                "erster film 2001 german 1080p bluray x264 grp",
                "g9",
                5_000_000_000,
            ),
            s.item(G2, "g2", 4_000_000_000),
        ],
    );
    let app = app(Library::Movies, &live(&s, "")).await;
    let erster = movie_id(&app, 101).await;
    let zweiter = movie_id(&app, 102).await;
    app.db
        .add_block(&block(MediaRef::Movie(erster), G1, "g1"), Timestamp::now())
        .await
        .expect("block");
    // A blocklist entry of another movie does not hold G2 for Erster Film.
    app.db
        .add_block(&block(MediaRef::Movie(zweiter), G2, "g2"), Timestamp::now())
        .await
        .expect("block");
    app.rss_sync().await.expect("rss");
    assert_eq!(s.lock().appended, [format!("{G2}.nzb")]);
}

#[tokio::test]
async fn a_blocklisted_guid_is_skipped_under_another_title() {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[
            // The blocklisted GUID, renamed by the indexer.
            s.item(
                "Erster.Film.2001.German.1080p.BluRay.x264-RENAMED",
                "g1",
                5_000_000_000,
            ),
            s.item(G2, "g2", 4_000_000_000),
        ],
    );
    let app = app(Library::Movies, &live(&s, "")).await;
    let erster = movie_id(&app, 101).await;
    app.db
        .add_block(&block(MediaRef::Movie(erster), G1, "g1"), Timestamp::now())
        .await
        .expect("block");
    app.rss_sync().await.expect("rss");
    assert_eq!(s.lock().appended, [format!("{G2}.nzb")]);
}

#[tokio::test]
async fn blocklisted_series_releases_are_skipped() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(S1, "s1-new", 2_000_000_000)]);
    let app = app(Library::Series, &live(&s, "")).await;
    let series = app.db.all_series().await.expect("series")[0].id;
    app.db
        .add_block(&block(MediaRef::Series(series), S1, "s1"), Timestamp::now())
        .await
        .expect("block");
    let report = app.rss_sync().await.expect("rss");
    assert_eq!(report.series_grabs, 0);
    assert!(s.lock().appended.is_empty());
}

#[tokio::test]
async fn earlier_grabs_hold_their_guid_unless_failed_without_entry() {
    // Shadow grabs count as earlier grabs: a weak shadow grab of G1 that G1 itself would
    // beat still holds its GUID.
    let s = movie_services().await;
    let shadow = app(Library::Movies, &s.indexer_toml("idx", &[])).await;
    let erster = movie_id(&shadow, 101).await;
    shadow
        .db
        .add_grab(
            &sofarr_db::NewGrab {
                movie_id: erster,
                title: "Erster.Film.2001.DVDRip.x264-OLD".to_owned(),
                guid: "g1".to_owned(),
                indexer: "idx".to_owned(),
                download_url: None,
                quality: "SDTV".to_owned(),
                custom_format_score: -1000,
                size: 1,
                status: GrabStatus::Shadow,
                download_id: None,
            },
            Timestamp::now(),
        )
        .await
        .expect("shadow grab");
    shadow.rss_sync().await.expect("rss");
    let titles: Vec<String> = shadow
        .db
        .grabs(None, 10)
        .await
        .expect("grabs")
        .into_iter()
        .map(|g| g.title)
        .collect();
    assert_eq!(titles, [G2, "Erster.Film.2001.DVDRip.x264-OLD"]);

    // A blocklisted failure frees the GUID once its entry is removed.
    let s = movie_services().await;
    let app = app(Library::Movies, &live(&s, "")).await;
    app.rss_sync().await.expect("rss");
    s.finish(77, G1, "FAILURE/HEALTH", "/remote/x");
    app.check_downloads().await.expect("check");
    app.rss_sync().await.expect("blocked");
    assert_eq!(
        s.lock().appended,
        [format!("{G1}.nzb"), format!("{G2}.nzb")]
    );
    let entry = app.db.blocks().await.expect("blocks")[0].id;
    assert!(app.db.remove_block(entry).await.expect("remove"));
    // G2 is pending now; with it failed locally, G1 is free again.
    let g2 = app.db.grabs(None, 10).await.expect("grabs")[0].clone();
    assert_eq!(g2.title, G2);
    app.db
        .update_grab(
            g2.id,
            GrabStatus::Failed,
            None,
            Some("FAILURE/MOVE"),
            Timestamp::now(),
        )
        .await
        .expect("update");
    app.rss_sync().await.expect("free again");
    assert_eq!(
        s.lock().appended.last().map(String::as_str),
        Some(format!("{G1}.nzb").as_str())
    );
}

fn hours(n: i64) -> SignedDuration {
    SignedDuration::from_hours(n)
}

#[tokio::test]
async fn a_download_absent_for_24_hours_has_vanished() {
    let s = movie_services().await;
    let app = app(Library::Movies, &live(&s, "")).await;
    app.rss_sync().await.expect("rss");
    let t0: Timestamp = "2026-11-12T08:00:00Z".parse().expect("ts");
    let status = |app: &crate::app::App| {
        let db = app.db.clone();
        async move { db.grabs(None, 10).await.expect("grabs")[0].clone() }
    };
    // Absent from the start: queue and history are empty.
    app.check_downloads_at(t0).await.expect("check");
    assert_eq!(status(&app).await.absent_since, Some(t0));
    app.check_downloads_at(t0 + hours(23)).await.expect("check");
    assert_eq!(status(&app).await.status, GrabStatus::Queued);
    app.check_downloads_at(t0 + hours(24)).await.expect("check");
    let grab = status(&app).await;
    assert_eq!(grab.status, GrabStatus::Failed);
    let message = grab.message.unwrap_or_default();
    assert!(message.contains("verschwunden"), "{message}");
    assert!(app.db.blocks().await.expect("blocks").is_empty());
}

#[tokio::test]
async fn a_download_seen_again_restarts_the_24_hours() {
    let s = movie_services().await;
    let app = app(Library::Movies, &live(&s, "")).await;
    app.rss_sync().await.expect("rss");
    let t0: Timestamp = "2026-11-12T08:00:00Z".parse().expect("ts");
    app.check_downloads_at(t0).await.expect("absent");
    s.lock().queue = vec![serde_json::json!({"NZBID": 77, "NZBName": G1, "Status": "QUEUED"})];
    app.check_downloads_at(t0 + hours(1)).await.expect("seen");
    let grab = app.db.grabs(None, 10).await.expect("grabs")[0].clone();
    assert_eq!(grab.absent_since, None);
    s.lock().queue.clear();
    app.check_downloads_at(t0 + hours(2))
        .await
        .expect("absent again");
    app.check_downloads_at(t0 + hours(25))
        .await
        .expect("23 hours");
    let grab = app.db.grabs(None, 10).await.expect("grabs")[0].clone();
    assert_eq!(grab.status, GrabStatus::Queued);
    app.check_downloads_at(t0 + hours(26))
        .await
        .expect("24 hours");
    let grab = app.db.grabs(None, 10).await.expect("grabs")[0].clone();
    assert_eq!(grab.status, GrabStatus::Failed);
}

#[tokio::test]
async fn an_unreachable_client_changes_nothing() {
    let s = movie_services().await;
    let app = app(Library::Movies, &live(&s, "")).await;
    app.rss_sync().await.expect("rss");
    let t0: Timestamp = "2026-11-12T08:00:00Z".parse().expect("ts");
    s.lock().nzbget_down = true;
    assert!(app.check_downloads_at(t0).await.is_err());
    let grab = app.db.grabs(None, 10).await.expect("grabs")[0].clone();
    assert_eq!((grab.status, grab.absent_since), (GrabStatus::Queued, None));

    s.lock().nzbget_down = false;
    app.check_downloads_at(t0).await.expect("absent");
    // A failed check neither ends nor extends the absence.
    s.lock().nzbget_down = true;
    assert!(app.check_downloads_at(t0 + hours(30)).await.is_err());
    let grab = app.db.grabs(None, 10).await.expect("grabs")[0].clone();
    assert_eq!(
        (grab.status, grab.absent_since),
        (GrabStatus::Queued, Some(t0))
    );
    s.lock().nzbget_down = false;
    app.check_downloads_at(t0 + hours(30))
        .await
        .expect("absent");
    let grab = app.db.grabs(None, 10).await.expect("grabs")[0].clone();
    assert_eq!(grab.status, GrabStatus::Failed);
}
