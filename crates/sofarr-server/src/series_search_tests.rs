//! Series counterparts of the movie cases of failure handling and search
//! (spec fail-handling: AC-08, AC-21, AC-23).

use sofarr_db::GrabStatus;

use crate::{
    search::SearchTarget,
    testkit::{Library, Services, app},
};

const S1: &str = "Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP";
const ALT: &str = "Kleine.Insel.S01E02.1080p.WEB-DL.h264-ALT";

fn live(s: &Services) -> String {
    format!(
        "[automation]\nmode = \"live\"\n{}{}",
        s.indexer_toml("idx", &[]),
        s.nzbget_toml()
    )
}

#[tokio::test]
async fn re_search_off_frees_the_episodes_for_the_rss_sync() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(S1, "s1", 2_000_000_000)]);
    let app = app(Library::Series, &live(&s)).await;
    app.rss_sync().await.expect("rss");
    s.finish(77, S1, "FAILURE/PAR", "/remote/a");
    app.check_downloads().await.expect("check");
    assert!(s.requests_with("t=tvsearch").is_empty(), "no re-search");
    // The failed grab no longer holds S01E02: the next RSS sync takes another release.
    s.feed(
        "idx",
        &[
            s.item(S1, "s1", 2_000_000_000),
            s.item(ALT, "s2", 2_000_000_000),
        ],
    );
    let report = app.rss_sync().await.expect("rss again");
    assert_eq!(report.series_grabs, 1);
    assert_eq!(
        s.lock().appended,
        [format!("{S1}.nzb"), format!("{ALT}.nzb")]
    );
}

#[tokio::test]
async fn shadow_mode_series_searches_only_record() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(S1, "s1", 2_000_000_000)]);
    let config = format!("{}{}", s.indexer_toml("idx", &[]), s.nzbget_toml());
    let app = app(Library::Series, &config).await;
    let series_id = app.db.all_series().await.expect("series")[0].id;
    let report = app
        .search(SearchTarget::Season {
            series_id,
            season: 1,
        })
        .await
        .expect("search");
    assert_eq!(report.grabs, 1);
    let grabs = app.db.series_grabs(None).await.expect("grabs");
    assert_eq!(grabs.len(), 1);
    assert_eq!(grabs[0].status, GrabStatus::Shadow);
    let fake = s.lock();
    assert!(fake.rpc.is_empty(), "{:?}", fake.rpc);
    assert!(fake.nzb_fetched.is_empty());
}

#[tokio::test]
async fn no_allowed_indexer_searches_nothing_for_a_series() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(S1, "s1", 2_000_000_000)]);
    let config = format!(
        "[automation]\nmode = \"live\"\n{}{}",
        s.indexer_toml("idx", &["kids"]),
        s.nzbget_toml()
    );
    let app = app(Library::Series, &config).await;
    let series_id = app.db.all_series().await.expect("series")[0].id;
    let report = app
        .search(SearchTarget::Series(series_id))
        .await
        .expect("search");
    assert_eq!((report.releases, report.grabs), (0, 0));
    let note = report.note.unwrap_or_default();
    assert!(note.contains("Kein Indexer"), "{note}");
    assert!(s.lock().requests.is_empty());
}
