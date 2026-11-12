//! Re-search after failures and manual search (spec fail-handling: AC-05 to AC-10,
//! AC-12 to AC-14, AC-21, AC-23, AC-24).

use jiff::Timestamp;
use serde_json::json;
use sofarr_db::{GrabStatus, MediaRef, NewBlock, NewGrab};

use crate::{
    app::App,
    failures::MediaKind,
    search::{SearchError, SearchTarget},
    testkit::{Library, Services, app},
};

const G1: &str = "Erster.Film.2001.German.1080p.BluRay.x264-GRP";
const G2: &str = "Erster.Film.2001.1080p.WEB-DL.x264-LOW";
const S1: &str = "Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP";
const PACK: &str = "Kleine.Insel.S01.German.1080p.BluRay.x264-PACK";

fn live(s: &Services, extra: &str) -> String {
    format!(
        "[automation]\nmode = \"live\"\n{}{}{extra}",
        s.indexer_toml("idx", &[]),
        s.nzbget_toml()
    )
}

const MOVIES_ON: &str = "[download]\nredownload_failed_movies = true\n";
const SERIES_ON: &str = "[download]\nredownload_failed_series = true\n";

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

async fn movie(app: &App, tmdb: i64) -> i64 {
    app.db
        .movie_by_tmdb(tmdb)
        .await
        .expect("db")
        .expect("movie")
        .id
}

async fn series(app: &App) -> i64 {
    app.db.all_series().await.expect("series")[0].id
}

async fn episode(app: &App, number: u16) -> i64 {
    app.db
        .episodes(None)
        .await
        .expect("episodes")
        .iter()
        .find(|e| e.data.number == number)
        .expect("episode")
        .id
}

fn queued(title: &str, guid: &str, download_id: &str) -> NewGrab {
    NewGrab {
        movie_id: 0,
        title: title.to_owned(),
        guid: guid.to_owned(),
        indexer: "idx".to_owned(),
        download_url: None,
        quality: "WEBDL-1080p".to_owned(),
        custom_format_score: 0,
        size: 1,
        status: GrabStatus::Queued,
        download_id: Some(download_id.to_owned()),
    }
}

async fn set_series_type(app: &App, kind: &str) {
    let mut show = app.db.all_series().await.expect("series")[0].clone();
    show.data.series_type = kind.to_owned();
    app.db
        .upsert_series(&show.data, Timestamp::now())
        .await
        .expect("series type");
}

fn appended(s: &Services) -> Vec<String> {
    s.lock().appended.clone()
}

// --- re-search (AC-06 to AC-10) ---------------------------------------------------

#[tokio::test]
async fn failed_movie_grab_is_re_searched_once() {
    let s = movie_services().await;
    let app = app(Library::Movies, &live(&s, MOVIES_ON)).await;
    app.rss_sync().await.expect("rss");
    // A second active grab of the same movie that fails in the same check.
    let mut other = queued("Erster.Film.2001.1080p.HDTV.x264-OLD", "g0", "78");
    other.movie_id = movie(&app, 101).await;
    app.db
        .add_grab(&other, Timestamp::now())
        .await
        .expect("grab");
    s.finish(77, G1, "FAILURE/PAR", "/remote/a");
    s.finish(78, "old", "FAILURE/UNPACK", "/remote/b");
    app.check_downloads().await.expect("check");

    assert_eq!(s.requests_with("t=movie").len(), 1);
    // G1 is blocklisted: the re-search takes the next best release.
    assert_eq!(appended(&s), [format!("{G1}.nzb"), format!("{G2}.nzb")]);
}

#[tokio::test]
async fn unmonitored_movie_is_not_re_searched() {
    let s = movie_services().await;
    let app = app(Library::Movies, &live(&s, MOVIES_ON)).await;
    app.rss_sync().await.expect("rss");
    app.db
        .set_monitored(movie(&app, 101).await, false)
        .await
        .expect("unmonitor");
    s.finish(77, G1, "FAILURE/PAR", "/remote/a");
    app.check_downloads().await.expect("check");
    assert!(s.requests_with("t=movie").is_empty());
    assert_eq!(
        app.db.grabs(None, 10).await.expect("grabs")[0].status,
        GrabStatus::Failed
    );
}

#[tokio::test]
async fn failed_season_pack_and_episode_search_the_season_once() {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[
            s.item(PACK, "p1", 9_000_000_000),
            s.item(S1, "s1", 2_000_000_000),
        ],
    );
    let app = app(Library::Series, &live(&s, SERIES_ON)).await;
    app.rss_sync().await.expect("rss");
    assert_eq!(appended(&s), [format!("{PACK}.nzb")]);
    let e2 = episode(&app, 2).await;
    app.db
        .add_series_grab(
            series(&app).await,
            &[e2],
            &queued("Kleine.Insel.S01E02.1080p.WEB-DL.h264-OLD", "s0", "78"),
            Timestamp::now(),
        )
        .await
        .expect("grab");
    s.finish(77, PACK, "FAILURE/HEALTH", "/remote/a");
    s.finish(78, "old", "FAILURE/PAR", "/remote/b");
    app.check_downloads().await.expect("check");

    assert_eq!(s.requests_with("Kleine+Insel+S01&").len(), 1, "season once");
    assert!(s.requests_with("S01E02").is_empty(), "no episode search");
    assert_eq!(appended(&s), [format!("{PACK}.nzb"), format!("{S1}.nzb")]);
}

#[tokio::test]
async fn failed_episode_grab_searches_its_episodes() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(S1, "s1", 2_000_000_000)]);
    let app = app(Library::Series, &live(&s, SERIES_ON)).await;
    app.rss_sync().await.expect("rss");
    s.finish(77, S1, "FAILURE/PAR", "/remote/a");
    app.check_downloads().await.expect("check");
    assert_eq!(s.requests_with("Kleine+Insel+S01E02").len(), 1);
    assert!(s.requests_with("Kleine+Insel+S01&").is_empty());
}

#[test]
fn series_research_scope() {
    use crate::failures::SeriesResearch;
    let mut plan = SeriesResearch::default();
    // A pack over seasons 1 and 2 and an episode of season 2: both seasons, no episode.
    plan.add(true, &[(10, 1), (11, 1), (20, 2)]);
    plan.add(false, &[(21, 2)]);
    // A single episode of season 3.
    plan.add(false, &[(30, 3)]);
    assert_eq!(plan.seasons(), [1, 2]);
    assert_eq!(plan.episodes(), [30]);
}

#[tokio::test]
async fn re_search_off_leaves_the_item_to_the_rss_sync() {
    let s = movie_services().await;
    let app = app(Library::Movies, &live(&s, "")).await;
    app.rss_sync().await.expect("rss");
    s.finish(77, G1, "FAILURE/PAR", "/remote/a");
    app.check_downloads().await.expect("check");
    assert!(s.requests_with("t=movie").is_empty());
    // The failed grab no longer holds the movie.
    app.rss_sync().await.expect("rss again");
    assert_eq!(appended(&s), [format!("{G1}.nzb"), format!("{G2}.nzb")]);

    let s = Services::start().await;
    s.feed("idx", &[s.item(S1, "s1", 2_000_000_000)]);
    let app = crate::testkit::app(Library::Series, &live(&s, "")).await;
    app.rss_sync().await.expect("rss");
    s.finish(77, S1, "FAILURE/UNPACK", "/remote/a");
    app.check_downloads().await.expect("check");
    assert!(s.requests_with("t=tvsearch").is_empty());
    assert!(s.requests_with("t=caps").is_empty());
}

#[tokio::test]
async fn re_search_setting_comes_from_config_or_import() {
    let app = app(Library::Movies, "").await;
    assert!(
        !app.redownload_failed(MediaKind::Movies)
            .await
            .expect("none")
    );
    app.db
        .put_setting(
            "config_downloadclient",
            &json!({"autoRedownloadFailed": true}),
        )
        .await
        .expect("radarr");
    app.db
        .put_setting(
            "sonarr_config_downloadclient",
            &json!({"autoRedownloadFailed": false}),
        )
        .await
        .expect("sonarr");
    assert!(
        app.redownload_failed(MediaKind::Movies)
            .await
            .expect("radarr")
    );
    assert!(
        !app.redownload_failed(MediaKind::Series)
            .await
            .expect("sonarr")
    );

    let configured = crate::testkit::app(
        Library::Movies,
        "[download]\nredownload_failed_movies = false\nredownload_failed_series = true\n",
    )
    .await;
    for (key, value) in [
        ("config_downloadclient", true),
        ("sonarr_config_downloadclient", false),
    ] {
        configured
            .db
            .put_setting(key, &json!({ "autoRedownloadFailed": value }))
            .await
            .expect("setting");
    }
    assert!(
        !configured
            .redownload_failed(MediaKind::Movies)
            .await
            .expect("movies")
    );
    assert!(
        configured
            .redownload_failed(MediaKind::Series)
            .await
            .expect("series")
    );
}

#[tokio::test]
async fn vanished_download_is_re_searched() {
    let s = movie_services().await;
    let app = app(Library::Movies, &live(&s, MOVIES_ON)).await;
    app.rss_sync().await.expect("rss");
    let t0: Timestamp = "2026-11-12T08:00:00Z".parse().expect("ts");
    app.check_downloads_at(t0).await.expect("absent");
    app.check_downloads_at(t0 + jiff::SignedDuration::from_hours(24))
        .await
        .expect("vanished");
    assert_eq!(s.requests_with("t=movie").len(), 1);
    // Not blocklisted: the re-search may take G1 again.
    assert_eq!(appended(&s), [format!("{G1}.nzb"), format!("{G1}.nzb")]);
}

#[tokio::test]
async fn local_failure_of_a_movie_grab_is_re_searched() {
    let s = movie_services().await;
    let app = app(Library::Movies, &live(&s, MOVIES_ON)).await;
    app.rss_sync().await.expect("rss");
    s.finish(77, G1, "FAILURE/MOVE", "/remote/a");
    app.check_downloads().await.expect("check");
    assert_eq!(s.requests_with("t=movie").len(), 1);
    // Not blocklisted: the re-search may take G1 again.
    assert_eq!(appended(&s), [format!("{G1}.nzb"), format!("{G1}.nzb")]);
}

#[tokio::test]
async fn broken_nzb_in_the_rss_sync_is_re_searched() {
    let s = movie_services().await;
    s.lock().nzb_status.insert("g1".to_owned(), 404);
    let app = app(Library::Movies, &live(&s, MOVIES_ON)).await;
    app.rss_sync().await.expect("rss");
    assert_eq!(s.requests_with("t=movie").len(), 1);
    assert_eq!(appended(&s), [format!("{G2}.nzb")]);
}

async fn series_services(items: &[(&str, &str)]) -> Services {
    let s = Services::start().await;
    let items: Vec<String> = items
        .iter()
        .map(|(title, guid)| s.item(title, guid, 2_000_000_000))
        .collect();
    s.feed("idx", &items);
    s
}

#[tokio::test]
async fn local_failure_of_a_series_grab_is_re_searched() {
    let s = series_services(&[(S1, "s1")]).await;
    let app = app(Library::Series, &live(&s, SERIES_ON)).await;
    app.rss_sync().await.expect("rss");
    s.finish(77, S1, "FAILURE/MOVE", "/remote/a");
    app.check_downloads().await.expect("check");
    assert_eq!(s.requests_with("Kleine+Insel+S01E02").len(), 1);
    assert_eq!(appended(&s), [format!("{S1}.nzb"), format!("{S1}.nzb")]);
}

#[tokio::test]
async fn broken_season_pack_in_the_rss_sync_searches_the_season() {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[
            s.item(PACK, "p1", 9_000_000_000),
            s.item(S1, "s1", 2_000_000_000),
        ],
    );
    s.lock().nzb_status.insert("p1".to_owned(), 404);
    let app = app(Library::Series, &live(&s, SERIES_ON)).await;
    app.rss_sync().await.expect("rss");
    assert_eq!(s.requests_with("Kleine+Insel+S01&").len(), 1);
    assert_eq!(appended(&s), [format!("{S1}.nzb")]);
}

#[tokio::test]
async fn vanished_series_download_is_re_searched() {
    let s = series_services(&[(S1, "s1")]).await;
    let app = app(Library::Series, &live(&s, SERIES_ON)).await;
    app.rss_sync().await.expect("rss");
    let t0: Timestamp = "2026-11-12T08:00:00Z".parse().expect("ts");
    app.check_downloads_at(t0).await.expect("absent");
    app.check_downloads_at(t0 + jiff::SignedDuration::from_hours(24))
        .await
        .expect("vanished");
    assert_eq!(s.requests_with("Kleine+Insel+S01E02").len(), 1);
    assert_eq!(appended(&s), [format!("{S1}.nzb"), format!("{S1}.nzb")]);
}

// --- manual search (AC-12, AC-13, AC-14, AC-21, AC-23, AC-24) -----------------------

#[tokio::test]
async fn manual_movie_search_reports_releases_grabs_and_errors() {
    let s = movie_services().await;
    s.feed(
        "idx2",
        &[
            s.item(G1, "g1", 5_000_000_000),
            s.item("Erster.Film.2001.720p.HDTV.x264-D", "g3", 1_000_000_000),
        ],
    );
    s.lock().indexer_status.insert("idx3".to_owned(), 500);
    let config = format!(
        "{}{}{}",
        live(&s, ""),
        s.indexer_toml("idx2", &[]),
        s.indexer_toml("idx3", &[])
    );
    let app = app(Library::Movies, &config).await;
    let report = app
        .search(SearchTarget::Movie(movie(&app, 101).await))
        .await
        .expect("search");
    assert_eq!(report.releases, 3, "distinct GUIDs");
    assert_eq!(report.grabs, 1);
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(appended(&s), [format!("{G1}.nzb")]);

    // An active grab only gives way to a better release.
    let again = app
        .search(SearchTarget::Movie(movie(&app, 101).await))
        .await
        .expect("search");
    assert_eq!(again.grabs, 0);
}

#[tokio::test]
async fn manual_search_replaces_an_active_grab_only_with_a_better_release() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(G2, "g2", 4_000_000_000)]);
    let app = app(Library::Movies, &live(&s, "")).await;
    app.rss_sync().await.expect("rss");
    s.feed(
        "idx",
        &[
            s.item(G1, "g1", 5_000_000_000),
            s.item(G2, "g2", 4_000_000_000),
        ],
    );
    let report = app
        .search(SearchTarget::Movie(movie(&app, 101).await))
        .await
        .expect("search");
    assert_eq!(report.grabs, 1);
    assert_eq!(appended(&s), [format!("{G2}.nzb"), format!("{G1}.nzb")]);
}

#[tokio::test]
async fn manual_search_skips_blocklisted_releases() {
    let s = movie_services().await;
    let app = app(Library::Movies, &live(&s, "")).await;
    let id = movie(&app, 101).await;
    app.db
        .add_block(
            &NewBlock {
                media: MediaRef::Movie(id),
                title: G1.to_owned(),
                normalized_title: sofarr_core::normalize_release_title(G1),
                guid: "old".to_owned(),
                indexer: "idx".to_owned(),
                reason: "Test".to_owned(),
            },
            Timestamp::now(),
        )
        .await
        .expect("block");
    app.search(SearchTarget::Movie(id)).await.expect("search");
    assert_eq!(appended(&s), [format!("{G2}.nzb")]);
}

#[tokio::test]
async fn manual_search_skips_a_blocklisted_guid_under_another_title() {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[
            s.item(
                "Erster.Film.2001.German.1080p.BluRay.x264-RENAMED",
                "g1",
                5_000_000_000,
            ),
            s.item(G2, "g2", 4_000_000_000),
        ],
    );
    let app = app(Library::Movies, &live(&s, "")).await;
    let id = movie(&app, 101).await;
    app.db
        .add_block(
            &NewBlock {
                media: MediaRef::Movie(id),
                title: G1.to_owned(),
                normalized_title: sofarr_core::normalize_release_title(G1),
                guid: "g1".to_owned(),
                indexer: "idx".to_owned(),
                reason: "Test".to_owned(),
            },
            Timestamp::now(),
        )
        .await
        .expect("block");
    app.search(SearchTarget::Movie(id)).await.expect("search");
    assert_eq!(appended(&s), [format!("{G2}.nzb")]);
}

#[tokio::test]
async fn series_search_counts_every_release_returned() {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[
            s.item(S1, "s1", 2_000_000_000),
            // No episode in its name: still a release the indexer returned.
            s.item(
                "Kleine.Insel.Making.Of.1080p.WEB-DL.h264-DOKU",
                "x1",
                1_000_000_000,
            ),
        ],
    );
    let app = app(Library::Series, &live(&s, "")).await;
    let report = app
        .search(SearchTarget::Episode {
            series_id: series(&app).await,
            season: 1,
            episode: 2,
        })
        .await
        .expect("search");
    assert_eq!((report.releases, report.grabs), (2, 1));
}

#[tokio::test]
async fn a_search_for_one_movie_covers_it_when_unmonitored() {
    let s = movie_services().await;
    let app = app(Library::Movies, &live(&s, "")).await;
    let id = movie(&app, 101).await;
    app.db.set_monitored(id, false).await.expect("unmonitor");
    // The RSS sync leaves an unmonitored movie alone …
    app.rss_sync().await.expect("rss");
    assert!(appended(&s).is_empty());
    // … a search that names it does not.
    let report = app.search(SearchTarget::Movie(id)).await.expect("search");
    assert_eq!(report.grabs, 1);
}

#[tokio::test]
async fn manual_series_season_and_episode_searches() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(S1, "s1", 2_000_000_000)]);
    let app = app(Library::Series, &live(&s, "")).await;
    let id = series(&app).await;
    // E01 is below the cutoff, E02 missing, E03 not aired: one season to search.
    let report = app.search(SearchTarget::Series(id)).await.expect("series");
    assert_eq!((report.releases, report.grabs), (1, 1));
    assert_eq!(s.requests_with("Kleine+Insel+S01&").len(), 1);

    // A season without monitored episodes: nothing is grabbed, no error.
    let e1 = episode(&app, 1).await;
    let e2 = episode(&app, 2).await;
    app.db
        .set_episodes_monitored(&[e1, e2], false)
        .await
        .expect("unmonitor");
    for grab in app.db.series_grabs(None).await.expect("grabs") {
        app.db
            .update_series_grab(
                grab.id,
                GrabStatus::Failed,
                None,
                Some("FAILURE/MOVE"),
                Timestamp::now(),
            )
            .await
            .expect("free");
    }
    let season = app
        .search(SearchTarget::Season {
            series_id: id,
            season: 1,
        })
        .await
        .expect("season");
    assert_eq!(season.grabs, 0);
    // One episode named explicitly is searched even when unmonitored (S1 is free: its
    // only grab failed without a blocklist entry).
    let episode = app
        .search(SearchTarget::Episode {
            series_id: id,
            season: 1,
            episode: 2,
        })
        .await
        .expect("episode");
    assert_eq!(episode.grabs, 1);
    assert_eq!(s.requests_with("Kleine+Insel+S01E02").len(), 1);
}

#[tokio::test]
async fn unknown_targets_are_not_found() {
    let app = app(Library::Series, "").await;
    let id = series(&app).await;
    for target in [
        SearchTarget::Movie(999),
        SearchTarget::Series(999),
        SearchTarget::Season {
            series_id: id,
            season: 9,
        },
        SearchTarget::Episode {
            series_id: id,
            season: 1,
            episode: 99,
        },
    ] {
        match app.search(target).await {
            Err(SearchError::NotFound(message)) => {
                assert!(message.contains("nicht in der Bibliothek"), "{message}");
            }
            other => panic!("{target:?}: {other:?}"),
        }
    }
}

#[tokio::test]
async fn series_search_asks_each_indexer_in_its_own_form() {
    let s = Services::start().await;
    s.lock().caps.insert(
        "idx".to_owned(),
        r#"<caps><searching><tv-search available="yes" supportedParams="q,tvdbid,season,ep"/></searching></caps>"#
            .to_owned(),
    );
    // By id the release belongs to the series whatever its title says.
    s.feed(
        "idx",
        &[s.item(
            "Isla.Pequena.S01E02.German.1080p.BluRay.x264-ID",
            "i1",
            3_000_000_000,
        )],
    );
    s.feed("idx2", &[s.item(S1, "s1", 2_000_000_000)]);
    let config = format!("{}{}", live(&s, ""), s.indexer_toml("idx2", &[]));
    let app = app(Library::Series, &config).await;
    let id = series(&app).await;
    let report = app
        .search(SearchTarget::Episode {
            series_id: id,
            season: 1,
            episode: 2,
        })
        .await
        .expect("search");
    assert_eq!((report.releases, report.grabs), (2, 1));
    let by_id = s.requests_with("idx?");
    assert!(
        by_id
            .iter()
            .any(|r| r.contains("tvdbid=77") && r.contains("season=1") && r.contains("ep=2")),
        "{by_id:?}"
    );
    assert_eq!(
        s.requests_with("idx2?t=tvsearch&q=Kleine+Insel+S01E02")
            .len(),
        1
    );
    assert_eq!(
        appended(&s),
        ["Isla.Pequena.S01E02.German.1080p.BluRay.x264-ID.nzb"]
    );
}

#[tokio::test]
async fn title_search_results_must_match_the_series() {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[s.item(
            "Fremde.Serie.S01E02.German.1080p.BluRay.x264-NOPE",
            "f1",
            3_000_000_000,
        )],
    );
    let app = app(Library::Series, &live(&s, "")).await;
    let report = app
        .search(SearchTarget::Episode {
            series_id: series(&app).await,
            season: 1,
            episode: 2,
        })
        .await
        .expect("search");
    assert_eq!(report.grabs, 0);
}

#[tokio::test]
async fn daily_and_anime_series_are_asked_in_their_numbering() {
    let s = Services::start().await;
    let app = app(Library::Series, &live(&s, "")).await;
    let id = series(&app).await;
    let target = SearchTarget::Episode {
        series_id: id,
        season: 1,
        episode: 2,
    };
    set_series_type(&app, "daily").await;
    app.search(target).await.expect("daily");
    assert_eq!(s.requests_with("q=Kleine+Insel+2019.01.08").len(), 1);
    set_series_type(&app, "anime").await;
    let mut e2 = app
        .db
        .episodes(Some(id))
        .await
        .expect("episodes")
        .into_iter()
        .find(|e| e.data.number == 2)
        .expect("e2");
    e2.data.absolute = Some(2);
    app.db.upsert_episode(id, &e2.data).await.expect("absolute");
    app.search(target).await.expect("anime");
    assert_eq!(s.requests_with("q=Kleine+Insel+02&").len(), 1);
}

#[tokio::test]
async fn shadow_mode_searches_only_record() {
    let s = movie_services().await;
    let config = format!("{}{}", s.indexer_toml("idx", &[]), s.nzbget_toml());
    let app = app(Library::Movies, &config).await;
    let report = app
        .search(SearchTarget::Movie(movie(&app, 101).await))
        .await
        .expect("search");
    assert_eq!(report.grabs, 1);
    let grab = &app.db.grabs(None, 10).await.expect("grabs")[0];
    assert_eq!(grab.status, GrabStatus::Shadow);
    let fake = s.lock();
    assert!(fake.rpc.is_empty() && fake.nzb_fetched.is_empty());
}

#[tokio::test]
async fn no_allowed_indexer_searches_nothing() {
    let s = movie_services().await;
    let config = format!(
        "[automation]\nmode = \"live\"\n{}{}",
        s.indexer_toml("idx", &["kids"]),
        s.nzbget_toml()
    );
    let app = app(Library::Movies, &config).await;
    let report = app
        .search(SearchTarget::Movie(movie(&app, 101).await))
        .await
        .expect("search");
    assert_eq!((report.releases, report.grabs), (0, 0));
    let note = report.note.unwrap_or_default();
    assert!(note.contains("Indexer"), "{note}");
    assert!(s.lock().requests.is_empty());
}

#[tokio::test]
async fn a_broken_nzb_falls_back_to_the_next_candidate() {
    let s = movie_services().await;
    s.lock().nzb_status.insert("g1".to_owned(), 404);
    let app = app(Library::Movies, &live(&s, "")).await;
    let report = app
        .search(SearchTarget::Movie(movie(&app, 101).await))
        .await
        .expect("search");
    assert_eq!(report.grabs, 1);
    assert_eq!(s.requests_with("t=movie").len(), 1, "no new indexer search");
    assert_eq!(appended(&s), [format!("{G2}.nzb")]);
    assert_eq!(app.db.blocks().await.expect("blocks").len(), 1);

    // A broken season pack falls back to the episode.
    let s = Services::start().await;
    s.feed(
        "idx",
        &[
            s.item(PACK, "p1", 9_000_000_000),
            s.item(S1, "s1", 2_000_000_000),
        ],
    );
    s.lock().nzb_status.insert("p1".to_owned(), 410);
    let app = crate::testkit::app(Library::Series, &live(&s, "")).await;
    let report = app
        .search(SearchTarget::Season {
            series_id: series(&app).await,
            season: 1,
        })
        .await
        .expect("search");
    assert_eq!(report.grabs, 1);
    assert_eq!(appended(&s), [format!("{S1}.nzb")]);
}

#[tokio::test]
async fn an_outage_ends_the_search_for_the_item() {
    let s = movie_services().await;
    s.lock().nzb_status.insert("g1".to_owned(), 503);
    let app = app(Library::Movies, &live(&s, "")).await;
    let report = app
        .search(SearchTarget::Movie(movie(&app, 101).await))
        .await
        .expect("search");
    assert_eq!(report.grabs, 0);
    assert_eq!(s.lock().nzb_fetched, ["g1"]);
    assert!(app.db.blocks().await.expect("blocks").is_empty());
}
