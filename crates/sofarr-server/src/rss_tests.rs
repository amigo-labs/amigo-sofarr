//! The RSS sync end to end and its decisions (spec rss-sync).

use std::{collections::BTreeMap, time::Duration};

use jiff::Timestamp;
use serde_json::json;
use sofarr_db::{Db, GrabStatus, NewGrab};
use sofarr_indexer::IndexerRelease;

use crate::{
    library::{self, tests::documents},
    rss::{self, MovieIndex, SyncReport},
    series,
    status::Job,
    testkit::{Library, Services, app},
};

const G1: &str = "Erster.Film.2001.German.1080p.BluRay.x264-GRP";
const S1: &str = "Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP";

fn live(s: &Services, extra: &str) -> String {
    format!(
        "[automation]\nmode = \"live\"\n{}{}{extra}",
        s.indexer_toml("idx", &[]),
        s.nzbget_toml()
    )
}

fn release(title: &str, guid: &str, size: u64) -> IndexerRelease {
    IndexerRelease {
        guid: guid.to_owned(),
        title: title.to_owned(),
        size_bytes: size,
        published: None,
        download_url: None,
        categories: vec![2000],
        imdb_id: None,
        indexer: "idx".to_owned(),
    }
}

#[tokio::test]
async fn asks_every_indexer_and_goes_on_after_a_failing_one() {
    let s = Services::start().await;
    s.feed("a", &[s.item(G1, "g1", 5_000_000_000)]);
    s.lock().indexer_status.insert("b".to_owned(), 500);
    let config = format!("{}{}", s.indexer_toml("b", &[]), s.indexer_toml("a", &[]));
    let app = app(Library::Movies, &config).await;
    let report = app.rss_sync().await.expect("rss");
    assert_eq!(
        (report.releases, report.matched, report.approved),
        (1, 1, 1)
    );
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert!(report.errors[0].starts_with("b: "), "{:?}", report.errors);
    // The latest releases in the movie and TV categories, in one request per indexer.
    let listing = s.requests_with("t=search");
    assert_eq!(listing.len(), 2, "{listing:?}");
    for request in &listing {
        assert!(
            request.contains("cat=2000%2C5000") || request.contains("cat=2000,5000"),
            "{request}"
        );
    }
    let grabs = app.db.grabs(None, 10).await.expect("grabs");
    assert_eq!(grabs.len(), 1);
    assert_eq!(grabs[0].title, G1);
}

#[tokio::test]
async fn releases_match_by_imdb_id_or_title_and_year() {
    let db = Db::in_memory().await.expect("db");
    library::import_documents(&db, &documents(), Timestamp::now())
        .await
        .expect("import");
    let mut movies = db.movies().await.expect("movies");
    movies[0].imdb_id = Some("tt0000101".to_owned());
    movies[1].title_de = Some("Der Zweite".to_owned());
    movies[1].original_title = Some("The Second".to_owned());
    let index = MovieIndex::new(&movies);
    let titles = |r: &IndexerRelease| {
        index
            .lookup(r, &sofarr_parser::parse(&r.title))
            .into_iter()
            .map(|m| m.title.clone())
            .collect::<Vec<_>>()
    };
    let mut by_imdb = release("Ganz.Anders.1999.1080p.BluRay.x264-A", "1", 1);
    by_imdb.imdb_id = Some("tt0000101".to_owned());
    assert_eq!(titles(&by_imdb), ["Erster Film"]);
    // Title (also the German, original and alternative title), year ±1.
    for (title, expected) in [
        ("Erster.Film.2002.1080p.BluRay.x264-A", vec!["Erster Film"]),
        ("First.Film.2000.1080p.BluRay.x264-A", vec!["Erster Film"]),
        ("Erster.Film.2004.1080p.BluRay.x264-A", vec![]),
        (
            "Der.Zweite.2002.German.1080p.BluRay.x264-A",
            vec!["Zweiter Film"],
        ),
        ("The.Second.2003.1080p.BluRay.x264-A", vec!["Zweiter Film"]),
        (
            "Zweiter.Film.German.1080p.BluRay.x264-A",
            vec!["Zweiter Film"],
        ),
        ("Unbekannt.2001.1080p.BluRay.x264-A", vec![]),
    ] {
        assert_eq!(titles(&release(title, "1", 1)), expected, "{title}");
    }
}

#[tokio::test]
async fn the_best_release_wins_by_quality_score_revision_and_size() {
    let db = Db::in_memory().await.expect("db");
    let now: Timestamp = "2026-11-01T00:00:00Z".parse().expect("ts");
    library::import_documents(&db, &documents(), now)
        .await
        .expect("import");
    let movies = db.movies().await.expect("movies");
    let rules = library::load_rules(&db).await.expect("rules");
    const GB: u64 = 1_000_000_000;
    let a = release("Erster.Film.2001.1080p.BluRay.x264-A", "a", 5 * GB);
    let bigger = release("Erster.Film.2001.1080p.BluRay.x264-B", "b", 6 * GB);
    let proper = release("Erster.Film.2001.PROPER.1080p.BluRay.x264-C", "c", 4 * GB);
    let german_web = release("Erster.Film.2001.German.1080p.WEB-DL.x264-D", "d", 6 * GB);
    let german = release("Erster.Film.2001.German.1080p.BluRay.x264-E", "e", 3 * GB);
    let pick = |releases: &[&IndexerRelease]| {
        let releases: Vec<IndexerRelease> = releases.iter().map(|r| (*r).clone()).collect();
        let picks = rss::decide(
            &releases,
            &movies,
            &rules,
            &BTreeMap::new(),
            &|_, _| false,
            now,
            &mut SyncReport::default(),
        );
        assert_eq!(picks.len(), 1, "one release per movie");
        picks[0].release.guid.clone()
    };
    assert_eq!(pick(&[&a, &bigger]), "b", "size last");
    assert_eq!(pick(&[&bigger, &proper]), "c", "revision before size");
    assert_eq!(pick(&[&proper, &german]), "e", "score before revision");
    assert_eq!(pick(&[&german_web, &proper]), "c", "quality before score");
}

#[tokio::test]
async fn an_open_grab_counts_as_the_file() {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[s.item("Erster.Film.2001.1080p.WEB-DL.x264-A", "w1", 4_000_000_000)],
    );
    let app = app(Library::Movies, &s.indexer_toml("idx", &[])).await;
    app.rss_sync().await.expect("first");
    // Another WEB-DL is no better than the pending one; the Blu-ray is.
    s.feed(
        "idx",
        &[
            s.item("Erster.Film.2001.1080p.WEB-DL.x264-B", "w2", 4_000_000_000),
            s.item("Erster.Film.2001.1080p.BluRay.x264-C", "b1", 5_000_000_000),
        ],
    );
    app.rss_sync().await.expect("second");
    s.feed(
        "idx",
        &[s.item("Erster.Film.2001.1080p.WEB-DL.x264-D", "w3", 4_000_000_000)],
    );
    app.rss_sync().await.expect("third");
    let mut guids: Vec<String> = app
        .db
        .grabs(None, 10)
        .await
        .expect("grabs")
        .into_iter()
        .map(|g| g.guid)
        .collect();
    guids.sort();
    assert_eq!(guids, ["b1", "w1"]);
}

#[tokio::test]
async fn an_open_series_grab_counts_as_the_episode_file() {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[s.item(
            "Kleine.Insel.S01E02.1080p.WEB-DL.h264-A",
            "w1",
            2_000_000_000,
        )],
    );
    let app = app(Library::Series, &s.indexer_toml("idx", &[])).await;
    assert_eq!(app.rss_sync().await.expect("first").series_grabs, 1);
    s.feed(
        "idx",
        &[s.item(
            "Kleine.Insel.S01E02.1080p.WEB-DL.h264-B",
            "w2",
            2_000_000_000,
        )],
    );
    assert_eq!(app.rss_sync().await.expect("same quality").series_grabs, 0);
    s.feed(
        "idx",
        &[s.item(
            "Kleine.Insel.S01E02.1080p.BluRay.x264-C",
            "b1",
            2_000_000_000,
        )],
    );
    assert_eq!(app.rss_sync().await.expect("better").series_grabs, 1);
}

#[tokio::test]
async fn shadow_mode_records_without_touching_the_client() {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[
            s.item(G1, "g1", 5_000_000_000),
            s.item(S1, "s1", 2_000_000_000),
        ],
    );
    let config = format!("{}{}", s.indexer_toml("idx", &[]), s.nzbget_toml());
    for library in [Library::Movies, Library::Series] {
        let app = app(library, &config).await;
        let report = app.rss_sync().await.expect("rss");
        match library {
            Library::Movies => {
                assert_eq!(report.approved, 1, "{report:?}");
                let grabs = app.db.grabs(None, 10).await.expect("grabs");
                assert_eq!(grabs.len(), 1);
                assert_eq!(grabs[0].status, GrabStatus::Shadow);
                assert_eq!(grabs[0].download_id, None);
            }
            Library::Series => {
                assert_eq!(report.series_grabs, 1, "{report:?}");
                let grabs = app.db.series_grabs(None).await.expect("grabs");
                assert_eq!(grabs.len(), 1);
                assert_eq!(grabs[0].status, GrabStatus::Shadow);
            }
        }
    }
    let fake = s.lock();
    assert!(fake.nzb_fetched.is_empty(), "{:?}", fake.nzb_fetched);
    assert!(fake.rpc.is_empty(), "{:?}", fake.rpc);
}

#[tokio::test]
async fn live_grabs_go_to_the_client_in_the_category_of_their_kind() {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[
            s.item(G1, "g1", 5_000_000_000),
            s.item(S1, "s1", 2_000_000_000),
        ],
    );
    let movies = app(Library::Movies, &live(&s, "category = \"filme\"\n")).await;
    movies.rss_sync().await.expect("rss");
    let shows = app(Library::Series, &live(&s, "")).await;
    shows.rss_sync().await.expect("rss");
    {
        let fake = s.lock();
        assert_eq!(fake.nzb_fetched, ["g1", "s1"]);
        assert_eq!(fake.appended, [format!("{G1}.nzb"), format!("{S1}.nzb")]);
        let appends: Vec<&String> = fake
            .rpc
            .iter()
            .filter(|r| r.starts_with("append"))
            .collect();
        assert_eq!(appends.len(), 2);
        assert!(appends[0].contains("\"filme\""), "{}", appends[0]);
        // Series go to `tv` unless `[nzbget] tv_category` says otherwise.
        assert!(appends[1].contains("\"tv\""), "{}", appends[1]);
    }
    let grab = &movies.db.grabs(None, 10).await.expect("grabs")[0];
    assert_eq!(grab.status, GrabStatus::Queued);
    assert_eq!(grab.download_id.as_deref(), Some("77"));
    let grab = &shows.db.series_grabs(None).await.expect("grabs")[0];
    assert_eq!(grab.status, GrabStatus::Queued);
    assert_eq!(grab.download_id.as_deref(), Some("78"));
}

#[tokio::test]
async fn a_release_named_like_an_imported_one_is_not_taken_again() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(G1, "new-guid", 5_000_000_000)]);
    let app = app(Library::Movies, &s.indexer_toml("idx", &[])).await;
    let movie = app.db.movie_by_tmdb(101).await.expect("db").expect("movie");
    app.db
        .add_grab(
            &NewGrab {
                movie_id: movie.id,
                title: G1.to_lowercase(),
                guid: "old-guid".to_owned(),
                indexer: "other".to_owned(),
                download_url: None,
                quality: "Bluray-1080p".to_owned(),
                custom_format_score: 10,
                size: 5_000_000_000,
                status: GrabStatus::Imported,
                download_id: None,
            },
            Timestamp::now(),
        )
        .await
        .expect("grab");
    let report = app.rss_sync().await.expect("rss");
    assert_eq!(report.matched, 1);
    assert_eq!(report.approved, 0, "{report:?}");
    assert_eq!(app.db.grabs(None, 10).await.expect("grabs").len(), 1);
}

/// Spec rss-sync AC-10 (series): a release named like an imported series grab (letter
/// case aside) is not taken for that series again.
#[tokio::test]
async fn a_series_release_named_like_an_imported_one_is_not_taken_again() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(S1, "new-guid", 2_000_000_000)]);
    let app = app(Library::Series, &s.indexer_toml("idx", &[])).await;
    let show = app.db.all_series().await.expect("series")[0].clone();
    let e02 = app
        .db
        .episodes(Some(show.id))
        .await
        .expect("episodes")
        .into_iter()
        .find(|e| e.data.season == 1 && e.data.number == 2)
        .expect("E02");
    assert!(e02.file.is_none(), "E02 is wanted");
    app.db
        .add_series_grab(
            show.id,
            &[e02.id],
            &NewGrab {
                movie_id: 0,
                title: S1.to_lowercase(),
                guid: "old-guid".to_owned(),
                indexer: "other".to_owned(),
                download_url: None,
                quality: "WEBDL-1080p".to_owned(),
                custom_format_score: 0,
                size: 2_000_000_000,
                status: GrabStatus::Imported,
                download_id: None,
            },
            Timestamp::now(),
        )
        .await
        .expect("grab");
    let report = app.rss_sync().await.expect("rss");
    assert_eq!(report.series_grabs, 0, "{report:?}");
    assert_eq!(app.db.series_grabs(None).await.expect("grabs").len(), 1);
}

#[tokio::test]
async fn a_tagged_indexer_serves_only_movies_with_its_tag() {
    let s = Services::start().await;
    s.feed("kids", &[s.item(G1, "g1", 5_000_000_000)]);
    let app = app(Library::Movies, &s.indexer_toml("kids", &["Kids"])).await;
    let report = app.rss_sync().await.expect("rss");
    assert_eq!(report.matched, 1);
    assert!(app.db.grabs(None, 10).await.expect("grabs").is_empty());

    // The movie gets the tag (label compared without case): now the indexer serves it.
    let mut docs = documents();
    docs.insert("tag".to_owned(), json!([{"id": 5, "label": "kids"}]));
    docs.get_mut("movie").expect("movies")[0]["tags"] = json!([5]);
    library::import_documents(&app.db, &docs, Timestamp::now())
        .await
        .expect("tags");
    app.rss_sync().await.expect("rss");
    assert_eq!(app.db.grabs(None, 10).await.expect("grabs").len(), 1);
}

#[tokio::test]
async fn series_releases_go_to_monitored_series_by_title_alias_or_year() {
    let db = Db::in_memory().await.expect("db");
    let now: Timestamp = "2026-11-01T00:00:00Z".parse().expect("ts");
    let (docs, eps) = series::tests::sonarr_documents();
    series::import_documents(&db, &docs, &eps, now)
        .await
        .expect("import");
    let rules = series::load_rules(&db).await.expect("rules");
    let mut shows = db.all_series().await.expect("series");
    let episodes = db.episodes(None).await.expect("episodes");
    let picks = |shows: &[sofarr_db::Series], title: &str| {
        let r = release(title, "1", 2_000_000_000);
        let info = sofarr_parser::parse_episode(title).expect("episode");
        series::decide(
            &[(r, info)],
            shows,
            &episodes,
            &[],
            &rules,
            &|_, _| false,
            now,
        )
        .len()
    };
    for title in [
        "Kleine.Insel.S01E02.1080p.WEB-DL.h264-A",
        "Small.Island.S01E02.1080p.WEB-DL.h264-A",
        "Kleine.Insel.2019.S01E02.1080p.WEB-DL.h264-A",
    ] {
        assert_eq!(picks(&shows, title), 1, "{title}");
    }
    assert_eq!(picks(&shows, "Grosse.Insel.S01E02.1080p.WEB-DL.h264-A"), 0);
    shows[0].data.monitored = false;
    assert_eq!(picks(&shows, "Kleine.Insel.S01E02.1080p.WEB-DL.h264-A"), 0);
}

#[tokio::test]
async fn rss_and_download_jobs_follow_the_config() {
    let s = Services::start().await;
    let minutes = |m: u64| Some(Duration::from_secs(m * 60));
    let shadow = app(
        Library::Movies,
        &format!("{}{}", s.indexer_toml("idx", &[]), s.nzbget_toml()),
    )
    .await;
    assert_eq!(Job::Rss.interval(&shadow), minutes(30));
    // Shadow mode never checks the download client.
    assert_eq!(Job::Downloads.interval(&shadow), None);

    let live_app = app(Library::Movies, &live(&s, "[rss]\ninterval_minutes = 15\n")).await;
    assert_eq!(Job::Rss.interval(&live_app), minutes(15));
    assert_eq!(Job::Downloads.interval(&live_app), minutes(2));
    let tuned = app(
        Library::Movies,
        &live(&s, "[download]\ncheck_interval_minutes = 5\n"),
    )
    .await;
    assert_eq!(Job::Downloads.interval(&tuned), minutes(5));

    let off = app(
        Library::Movies,
        &live(
            &s,
            "[rss]\ninterval_minutes = 0\n[download]\ncheck_interval_minutes = 0\n",
        ),
    )
    .await;
    assert_eq!(Job::Rss.interval(&off), None);
    assert_eq!(Job::Downloads.interval(&off), None);
    // Without an indexer there is no RSS sync, without nzbget no download check.
    let bare = app(Library::Movies, "[automation]\nmode = \"live\"\n").await;
    assert_eq!(Job::Rss.interval(&bare), None);
    assert_eq!(Job::Downloads.interval(&bare), None);
}

/// An app with the movie test library plus the series of `sonarr_documents()`.
async fn movies_and_series(config: &str) -> crate::app::App {
    let app = app(Library::Movies, config).await;
    let (docs, eps) = series::tests::sonarr_documents();
    series::import_documents(&app.db, &docs, &eps, Timestamp::now())
        .await
        .expect("series");
    app
}

#[tokio::test]
async fn the_report_counts_movie_releases_and_matches_to_monitored_available_movies() {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[
            s.item(G1, "g1", 5_000_000_000),
            // An episode: not a movie release.
            s.item(S1, "s1", 2_000_000_000),
            // Vierter Film is not monitored.
            s.item("Vierter.Film.1080p.BluRay.x264-A", "v1", 5_000_000_000),
            // Fuenfter Film is monitored but only announced.
            s.item(
                "Fuenfter.Film.2030.1080p.BluRay.x264-A",
                "f1",
                5_000_000_000,
            ),
        ],
    );
    let app = movies_and_series(&s.indexer_toml("idx", &[])).await;
    let mut docs = documents();
    docs.get_mut("movie")
        .expect("movies")
        .as_array_mut()
        .expect("array")
        .push(
            json!({"id": 9, "tmdbId": 109, "title": "Fuenfter Film", "year": 2030,
            "monitored": true, "qualityProfileId": 1, "path": "/x/Fuenfter Film (2030)",
            "minimumAvailability": "released", "status": "announced"}),
        );
    library::import_documents(&app.db, &docs, Timestamp::now())
        .await
        .expect("fifth movie");
    let report = app.rss_sync().await.expect("rss");
    assert_eq!(
        (
            report.releases,
            report.matched,
            report.approved,
            report.series_grabs
        ),
        (3, 1, 1, 1),
        "{report:?}"
    );
    let grabs = app.db.grabs(None, 10).await.expect("grabs");
    assert_eq!(grabs.len(), 1);
    assert_eq!(grabs[0].guid, "g1");
}

#[tokio::test]
async fn an_episode_release_is_decided_for_series_only() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(S1, "s1", 2_000_000_000)]);
    let app = movies_and_series(&s.indexer_toml("idx", &[])).await;
    // A monitored movie without a file that has the series' title.
    let mut docs = documents();
    docs.get_mut("movie")
        .expect("movies")
        .as_array_mut()
        .expect("array")
        .push(
            json!({"id": 8, "tmdbId": 108, "title": "Kleine Insel", "year": 2019,
            "monitored": true, "qualityProfileId": 1, "path": "/x/Kleine Insel (2019)"}),
        );
    library::import_documents(&app.db, &docs, Timestamp::now())
        .await
        .expect("movie");
    // The same name without the episode reads as that movie.
    let movies = app.db.movies().await.expect("movies");
    let index = MovieIndex::new(&movies);
    let as_movie = release("Kleine.Insel.2019.German.1080p.WEB-DL.h264-GRP", "m", 1);
    assert_eq!(
        index
            .lookup(&as_movie, &sofarr_parser::parse(&as_movie.title))
            .len(),
        1
    );

    let report = app.rss_sync().await.expect("rss");
    assert_eq!(
        (report.releases, report.matched, report.series_grabs),
        (0, 0, 1),
        "{report:?}"
    );
    assert!(app.db.grabs(None, 10).await.expect("grabs").is_empty());
    let grabs = app.db.series_grabs(None).await.expect("grabs");
    assert_eq!(grabs.len(), 1);
    assert_eq!(grabs[0].title, S1);
}

#[tokio::test]
async fn a_tagged_indexer_serves_only_series_with_its_tag() {
    let s = Services::start().await;
    s.feed("kids", &[s.item(S1, "s1", 2_000_000_000)]);
    let app = app(Library::Series, &s.indexer_toml("kids", &["Kids"])).await;
    // The series carries tag id 2, which has no label yet.
    assert_eq!(app.rss_sync().await.expect("rss").series_grabs, 0);
    assert!(app.db.series_grabs(None).await.expect("grabs").is_empty());

    // Tag 2 is labelled `KIDS` (compared without case): now the indexer serves it.
    app.db
        .put_setting(
            &format!("{}tag", series::PREFIX),
            &json!([{"id": 2, "label": "KIDS"}]),
        )
        .await
        .expect("tag");
    assert_eq!(app.rss_sync().await.expect("rss").series_grabs, 1);
    assert_eq!(app.db.series_grabs(None).await.expect("grabs").len(), 1);
}
