use serde_json::json;

use super::*;

pub fn sonarr_documents() -> (Documents, EpisodeDocuments) {
    let mut docs = BTreeMap::new();
    docs.insert(
        "qualityprofile".to_owned(),
        json!([{"id": 1, "name": "HD", "upgradeAllowed": true, "cutoff": 7,
        "items": [
            {"quality": {"id": 3, "name": "WEBDL-1080p"}, "allowed": true},
            {"quality": {"id": 7, "name": "Bluray-1080p"}, "allowed": true}
        ]}]),
    );
    docs.insert("qualitydefinition".to_owned(), json!([]));
    docs.insert("customformat".to_owned(), json!([]));
    docs.insert(
        "config_naming".to_owned(),
        json!({"renameEpisodes": true, "multiEpisodeStyle": 5,
               "standardEpisodeFormat": "{Series Title} - S{season:00}E{episode:00} - {Episode Title} {Quality Full}",
               "seasonFolderFormat": "Staffel {season:00}"}),
    );
    docs.insert(
        "series".to_owned(),
        json!([{"id": 3, "tvdbId": 77, "title": "Kleine Insel", "year": 2019, "runtime": 45,
                "seriesType": "standard", "monitored": true, "seasonFolder": true,
                "qualityProfileId": 1, "path": "/tv/Kleine Insel",
                "alternateTitles": [{"title": "Small Island", "seasonNumber": -1}]}]),
    );
    let eps = vec![
        json!({"id": 30, "seasonNumber": 1, "episodeNumber": 1, "title": "Ankunft", "airDate": "2019-01-01",
               "monitored": true, "episodeFileId": 300}),
        json!({"id": 31, "seasonNumber": 1, "episodeNumber": 2, "title": "Sturm", "airDate": "2019-01-08",
               "monitored": true, "episodeFileId": 0}),
        json!({"id": 32, "seasonNumber": 1, "episodeNumber": 3, "title": "Zukunft", "airDate": "2099-01-01",
               "monitored": true}),
    ];
    let files = vec![
        json!({"id": 300, "relativePath": "Staffel 01/Kleine Insel - S01E01.mkv", "size": 5,
        "quality": {"quality": {"name": "WEBDL-1080p"}, "revision": {"version": 1, "real": 0}}}),
    ];
    (docs, BTreeMap::from([(3, (eps, files))]))
}

fn release(title: &str, guid: &str) -> (IndexerRelease, EpisodeInfo) {
    (
        IndexerRelease {
            guid: guid.to_owned(),
            title: title.to_owned(),
            size_bytes: 2_000_000_000,
            published: None,
            download_url: None,
            categories: vec![5000],
            imdb_id: None,
            indexer: "idx".to_owned(),
        },
        parse_episode(title).expect("episode"),
    )
}

#[tokio::test]
async fn imports_sonarr_and_decides() {
    let db = Db::in_memory().await.expect("db");
    let now: Timestamp = "2026-11-01T00:00:00Z".parse().expect("ts");
    let (docs, eps) = sonarr_documents();
    let summary = import_documents(&db, &docs, &eps, now)
        .await
        .expect("import");
    assert_eq!(
        summary,
        ImportSummary {
            settings: 4,
            series: 1,
            episodes: 3,
            files: 1
        }
    );
    let rules = load_rules(&db).await.expect("rules");
    let series = db.all_series().await.expect("series");
    let episodes = db.episodes(None).await.expect("episodes");
    let releases = vec![
        release("Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-A", "1"),
        release("Small.Island.S01E02.1080p.BluRay.x264-B", "2"),
        release("Kleine.Insel.S01E01.1080p.WEB-DL.h264-C", "3"),
        release("Kleine.Insel.S01E03.1080p.WEB-DL.h264-D", "4"),
        release("Kleine.Insel.S01.1080p.BluRay.x264-E", "5"),
    ];
    let picks = decide(
        &releases,
        &series,
        &episodes,
        &[],
        &rules,
        &|_, _| false,
        now,
    );
    let titles: Vec<&str> = picks.iter().map(|p| p.release.title.as_str()).collect();
    // The Bluray season pack covers E01 (upgrade) and E02 (missing) and beats the singles.
    assert_eq!(titles, ["Kleine.Insel.S01.1080p.BluRay.x264-E"]);
}

#[test]
fn names_episodes() {
    let series = Series {
        id: 1,
        data: series_from_sonarr(&sonarr_documents().0["series"][0]).expect("series"),
        tags: vec![],
    };
    let ep = |n: u16, title: &str| Episode {
        id: i64::from(n),
        series_id: 1,
        data: NewEpisode {
            tvdb_id: None,
            season: 1,
            number: n,
            absolute: None,
            title: Some(title.to_owned()),
            title_de: None,
            air_date: None,
            monitored: true,
        },
        file_id: None,
        file: None,
    };
    let naming = NamingConfig::from_json(&sonarr_documents().0["config_naming"]);
    let (a, b) = (ep(1, "Ankunft"), ep(2, "Sturm"));
    assert_eq!(
        episode_name(
            &series,
            &[&a, &b],
            "Kleine.Insel.S01E01E02.1080p.WEB-DL-G",
            "WEBDL-1080p",
            &naming,
            None
        ),
        "Kleine Insel - S01E01-E02 - Ankunft WEBDL-1080p"
    );
    assert_eq!(
        season_folder(&series, 1, &naming),
        PathBuf::from("/tv/Kleine Insel/Staffel 01")
    );
}

#[test]
fn episode_format_by_series_type_and_season_folders() {
    let mut series = Series {
        id: 1,
        data: series_from_sonarr(&sonarr_documents().0["series"][0]).expect("series"),
        tags: vec![],
    };
    let naming = NamingConfig {
        standard_episode_format: "standard".to_owned(),
        daily_episode_format: "daily".to_owned(),
        anime_episode_format: "anime".to_owned(),
        ..NamingConfig::default()
    };
    for (kind, expected) in [
        ("standard", "standard"),
        ("daily", "daily"),
        ("anime", "anime"),
        ("other", "standard"),
    ] {
        series.data.series_type = kind.to_owned();
        assert_eq!(episode_pattern(&series, &naming), expected, "{kind}");
    }
    let ep = Episode {
        id: 1,
        series_id: 1,
        data: NewEpisode {
            tvdb_id: None,
            season: 1,
            number: 4,
            absolute: None,
            title: Some("Nebel".to_owned()),
            title_de: None,
            air_date: Some("2019-01-22".to_owned()),
            monitored: true,
        },
        file_id: None,
        file: None,
    };
    let naming = NamingConfig::default();
    series.data.series_type = "daily".to_owned();
    assert_eq!(
        episode_name(
            &series,
            &[&ep],
            "Kleine.Insel.2019.01.22.1080p.WEB-DL-G",
            "WEBDL-1080p",
            &naming,
            None
        ),
        "Kleine Insel - 2019-01-22 - Nebel WEBDL-1080p"
    );
    // Default season folder format, and none at all when the series has no season folders.
    assert_eq!(
        season_folder(&series, 1, &naming),
        PathBuf::from("/tv/Kleine Insel/Season 1")
    );
    series.data.season_folder = false;
    assert_eq!(
        season_folder(&series, 1, &naming),
        PathBuf::from("/tv/Kleine Insel")
    );
}

#[test]
fn waits_for_episode_titles() {
    let ep = |title: Option<&str>, air: &str| Episode {
        id: 1,
        series_id: 1,
        data: NewEpisode {
            tvdb_id: None,
            season: 1,
            number: 1,
            absolute: None,
            title: title.map(str::to_owned),
            title_de: None,
            air_date: Some(air.to_owned()),
            monitored: true,
        },
        file_id: None,
        file: None,
    };
    let now: Timestamp = "2026-11-02T12:00:00Z".parse().expect("ts");
    let pattern = "{Series Title} - S{season:00}E{episode:00} - {Episode CleanTitle}";
    let tba = ep(Some("TBA"), "2026-11-01");
    assert!(waits_for_titles(
        Some("always"),
        pattern,
        &[&tba],
        false,
        now
    ));
    assert!(!waits_for_titles(
        Some("never"),
        pattern,
        &[&tba],
        false,
        now
    ));
    assert!(!waits_for_titles(
        Some("bulkSeasonReleases"),
        pattern,
        &[&tba],
        false,
        now
    ));
    assert!(waits_for_titles(
        Some("bulkSeasonReleases"),
        pattern,
        &[&tba],
        true,
        now
    ));
    assert!(!waits_for_titles(
        Some("always"),
        "S{season:00}E{episode:00}",
        &[&tba],
        false,
        now
    ));
    let old = ep(None, "2026-10-01");
    assert!(!waits_for_titles(
        Some("always"),
        pattern,
        &[&old],
        false,
        now
    ));
    let titled = ep(Some("Sturm"), "2026-11-01");
    assert!(!waits_for_titles(
        Some("always"),
        pattern,
        &[&titled],
        false,
        now
    ));
    let always =
        |pattern: &str, e: &Episode| waits_for_titles(Some("always"), pattern, &[e], false, now);
    // Token spelling as the renderer accepts it.
    assert!(always("{Episode.Title}", &tba));
    // `:DE` uses the German title, falling back to the default one.
    let mut german = ep(Some("Storm"), "2026-11-01");
    german.data.title_de = Some("TBA".to_owned());
    assert!(always("{Episode Title:DE}", &german));
    assert!(!always("{Episode Title}", &german));
    let mut undated = ep(None, "2026-11-01");
    undated.data.air_date = None;
    assert!(!always(pattern, &undated));
}

/// Spec discover AC-08: new episodes follow their season; a new season follows the
/// series' switch; specials and unmonitored series stay off.
#[test]
fn new_episodes_follow_their_season_and_new_seasons_the_switch() {
    let episode = |season: u16, monitored: bool| Episode {
        id: 1,
        series_id: 1,
        data: NewEpisode {
            tvdb_id: None,
            season,
            number: 1,
            absolute: None,
            title: None,
            title_de: None,
            air_date: None,
            monitored,
        },
        file_id: None,
        file: None,
    };
    let existing = [episode(1, true), episode(2, false)];
    assert!(monitors_new_episode(true, false, &existing, 1));
    assert!(!monitors_new_episode(true, true, &existing, 2));
    assert!(monitors_new_episode(true, true, &existing, 3));
    assert!(!monitors_new_episode(true, false, &existing, 3));
    assert!(!monitors_new_episode(true, true, &existing, 0));
    assert!(!monitors_new_episode(false, true, &existing, 1));
}

/// Spec series: a TVDB refresh writes the German title, adds aliases without
/// duplicates, takes poster and German overview, updates known episodes while keeping
/// their monitoring and files, adds new episodes (D28) and keeps episodes TVDB no longer
/// lists; a series TVDB cannot give is left as it is and the others are refreshed.
#[tokio::test]
async fn refresh_takes_titles_and_episodes_from_tvdb() {
    use crate::{testkit::Library, tmdb_fake::Providers};

    let providers = Providers::start().await;
    let app = providers.app(Library::Series, "").await;
    let lost = series_from_sonarr(&json!({"id": 4, "tvdbId": 99, "title": "Verloren",
        "monitored": true, "qualityProfileId": 1, "path": "/tv/Verloren"}))
    .expect("series");
    app.db
        .upsert_series(&lost, Timestamp::now())
        .await
        .expect("second series");
    let before = app.db.episodes(None).await.expect("episodes");
    app.db
        .set_episodes_monitored(&[before[1].id], false)
        .await
        .expect("unmonitor S01E02");
    {
        let mut fake = providers.lock();
        fake.tvdb_series.insert(
            77,
            json!({"id": 77, "name": "Small Island", "year": "2019", "originalLanguage": "eng",
                   "aliases": [{"name": "Small Island"}, {"name": "Die Insel"}],
                   "image": "https://artworks.thetvdb.com/banners/v4/series/77/posters/insel.jpg",
                   "overview": "An island.",
                   "translations": {"nameTranslations": [{"language": "deu", "name": "Kleine Insel"}],
                       "overviewTranslations": [{"language": "deu", "overview": "Eine Insel."}]}}),
        );
        let episode = |id: i64, season: u16, number: u16, name: &str, aired: &str| {
            json!({"id": id, "seasonNumber": season, "number": number, "name": name,
                   "aired": aired})
        };
        // S01E03 is no longer listed.
        fake.tvdb_episodes.insert(
            77,
            vec![
                episode(700, 0, 1, "Special", "2019-06-01"),
                episode(701, 1, 1, "Ankunft (neu)", "2019-01-02"),
                episode(702, 1, 2, "Sturm", "2019-01-08"),
                episode(704, 1, 4, "Abschied", "2019-01-22"),
                episode(705, 2, 1, "Rückkehr", "2020-01-01"),
            ],
        );
    }

    let report = app.refresh(false, true, false).await.expect("refresh");
    assert_eq!(report.episodes, 5, "only the series TVDB knows");

    let show = app
        .db
        .series_by_tvdb(77)
        .await
        .expect("db")
        .expect("series");
    assert_eq!(show.data.title, "Kleine Insel");
    assert_eq!(show.data.title_de.as_deref(), Some("Kleine Insel"));
    assert_eq!(
        show.data.alternative_titles,
        ["Small Island", "Die Insel", "Kleine Insel"]
    );
    let presentation = app.db.series_presentation().await.expect("presentation");
    assert_eq!(
        presentation[&show.id].overview.as_deref(),
        Some("Eine Insel.")
    );
    assert!(
        app.db
            .poster_path(sofarr_db::MediaRef::Series(show.id))
            .await
            .expect("poster")
            .is_some()
    );
    let unchanged = app
        .db
        .series_by_tvdb(99)
        .await
        .expect("db")
        .expect("series");
    assert_eq!(unchanged.data.title_de, None);

    let episodes = app.db.episodes(Some(show.id)).await.expect("episodes");
    /// Season, number, title, air date, monitored, has a file.
    type State<'a> = (u16, u16, Option<&'a str>, Option<&'a str>, bool, bool);
    let state: Vec<State> = episodes
        .iter()
        .map(|e| {
            (
                e.data.season,
                e.data.number,
                e.data.title.as_deref(),
                e.data.air_date.as_deref(),
                e.data.monitored,
                e.file.is_some(),
            )
        })
        .collect();
    assert_eq!(
        state,
        [
            (0, 1, Some("Special"), Some("2019-06-01"), false, false),
            (1, 1, Some("Ankunft (neu)"), Some("2019-01-02"), true, true),
            (1, 2, Some("Sturm"), Some("2019-01-08"), false, false),
            (1, 3, Some("Zukunft"), Some("2099-01-01"), true, false),
            (1, 4, Some("Abschied"), Some("2019-01-22"), true, false),
            (2, 1, Some("Rückkehr"), Some("2020-01-01"), true, false),
        ]
    );
}

/// Spec series: without `[tvdb]` the series refresh asks nobody and changes nothing.
#[tokio::test]
async fn without_tvdb_series_are_not_refreshed() {
    let app = crate::testkit::app(crate::testkit::Library::Series, "").await;
    let before = app.db.episodes(None).await.expect("episodes");
    let report = app.refresh(false, true, true).await.expect("refresh");
    assert_eq!(report.episodes, 0);
    assert_eq!(app.db.episodes(None).await.expect("episodes"), before);
    let show = &app.db.all_series().await.expect("series")[0];
    assert_eq!(show.data.title_de, None);
}

/// Spec series: the metadata refresh runs every `[library] refresh_interval_hours`
/// (default 24), also in shadow mode; 0 switches it off.
#[tokio::test]
async fn the_refresh_is_scheduled_by_its_interval() {
    use crate::{
        status::Job,
        testkit::{Library, app},
    };

    let default = app(Library::Series, "").await;
    assert_eq!(
        Job::Refresh.interval(&default),
        Some(std::time::Duration::from_secs(24 * 3600))
    );
    let off = app(Library::Series, "[library]\nrefresh_interval_hours = 0\n").await;
    assert_eq!(Job::Refresh.interval(&off), None);
}

/// Spec series: a series in Sonarr's export without TVDB id is not imported.
#[tokio::test]
async fn sonarr_series_without_tvdb_id_are_skipped() {
    let db = Db::in_memory().await.expect("db");
    let (mut docs, eps) = sonarr_documents();
    docs.get_mut("series")
        .and_then(Value::as_array_mut)
        .expect("series")
        .push(json!({"id": 9, "title": "Ohne TVDB", "monitored": true, "path": "/tv/Ohne"}));
    let summary = import_documents(&db, &docs, &eps, Timestamp::now())
        .await
        .expect("import");
    assert_eq!(summary.series, 1);
    let all = db.all_series().await.expect("series");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].data.tvdb_id, 77);
    assert_eq!(all[0].data.alternative_titles, ["Small Island"]);
    let episodes = db.episodes(None).await.expect("episodes");
    assert_eq!(
        episodes
            .iter()
            .map(|e| (e.data.number, e.data.monitored, e.file.is_some()))
            .collect::<Vec<_>>(),
        [(1, true, true), (2, true, false), (3, true, false)]
    );
}

/// The library of `sonarr_documents()` with its rules, series and episodes.
async fn decision_library(now: Timestamp) -> (Db, Rules) {
    let db = Db::in_memory().await.expect("db");
    let (docs, eps) = sonarr_documents();
    import_documents(&db, &docs, &eps, now)
        .await
        .expect("import");
    let rules = load_rules(&db).await.expect("rules");
    (db, rules)
}

/// Spec series: a release belongs to a monitored series whose title, German title,
/// alias or title with year it names (case and separators aside); an unmonitored
/// series takes no release.
#[tokio::test]
async fn releases_match_series_by_title_german_title_alias_or_year() {
    let now: Timestamp = "2026-11-01T00:00:00Z".parse().expect("ts");
    let (db, rules) = decision_library(now).await;
    let mut show = db.all_series().await.expect("series").remove(0);
    show.data.title_de = Some("Die kleine Insel".to_owned());
    db.upsert_series(&show.data, now).await.expect("title_de");
    let picks = |title: &str, series: &[Series], episodes: &[Episode]| {
        decide(
            &[release(title, "g")],
            series,
            episodes,
            &[],
            &rules,
            &|_, _| false,
            now,
        )
        .len()
    };
    let series = db.all_series().await.expect("series");
    let episodes = db.episodes(None).await.expect("episodes");
    for title in [
        "Kleine.Insel.S01E02.1080p.WEB-DL.h264-A",
        "kleine insel S01E02 1080p WEB-DL h264-A",
        "Die.kleine.Insel.S01E02.1080p.WEB-DL.h264-A",
        "Small.Island.S01E02.1080p.WEB-DL.h264-A",
        "Kleine.Insel.2019.S01E02.1080p.WEB-DL.h264-A",
    ] {
        assert_eq!(picks(title, &series, &episodes), 1, "{title}");
    }
    assert_eq!(
        picks(
            "Grosse.Insel.S01E02.1080p.WEB-DL.h264-A",
            &series,
            &episodes
        ),
        0
    );
    let mut unmonitored = series.clone();
    unmonitored[0].data.monitored = false;
    assert_eq!(
        picks(
            "Kleine.Insel.S01E02.1080p.WEB-DL.h264-A",
            &unmonitored,
            &episodes
        ),
        0
    );
}

/// Spec series: of several approved releases for the same episodes the best one is
/// taken; an episode with a grab in flight counts as having the grabbed release, so only
/// a better release replaces it.
#[tokio::test]
async fn the_best_release_wins_and_grabs_count_as_files() {
    let now: Timestamp = "2026-11-01T00:00:00Z".parse().expect("ts");
    let (db, rules) = decision_library(now).await;
    let series = db.all_series().await.expect("series");
    let episodes = db.episodes(None).await.expect("episodes");
    let releases = vec![
        release("Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-A", "1"),
        release("Kleine.Insel.S01E02.1080p.BluRay.x264-B", "2"),
    ];
    let titles = |pending: &[SeriesGrab]| -> Vec<String> {
        decide(
            &releases,
            &series,
            &episodes,
            pending,
            &rules,
            &|_, _| false,
            now,
        )
        .into_iter()
        .map(|p| p.release.title)
        .collect()
    };
    assert_eq!(titles(&[]), ["Kleine.Insel.S01E02.1080p.BluRay.x264-B"]);

    let grab = |quality: &str, title: &str| SeriesGrab {
        id: 1,
        series_id: series[0].id,
        episode_ids: vec![episodes[1].id],
        title: title.to_owned(),
        guid: "old".to_owned(),
        indexer: "idx".to_owned(),
        download_url: None,
        quality: quality.to_owned(),
        custom_format_score: 0,
        size: 1,
        status: sofarr_db::GrabStatus::Queued,
        download_id: None,
        message: None,
        grabbed_at: now,
        absent_since: None,
    };
    // A WEB-DL in flight: the Bluray is still an upgrade.
    assert_eq!(
        titles(&[grab(
            "WEBDL-1080p",
            "Kleine.Insel.S01E02.1080p.WEB-DL.h264-OLD"
        )]),
        ["Kleine.Insel.S01E02.1080p.BluRay.x264-B"]
    );
    // A Bluray (the cutoff) in flight: nothing beats it.
    assert!(
        titles(&[grab(
            "Bluray-1080p",
            "Kleine.Insel.S01E02.1080p.BluRay.x264-OLD"
        )])
        .is_empty()
    );
}

/// Spec series: the file name follows the episode format of the series type, and
/// without season folders the file goes straight into the series folder.
#[test]
fn episode_formats_follow_the_series_type_and_season_folders() {
    let mut series = Series {
        id: 1,
        data: series_from_sonarr(&sonarr_documents().0["series"][0]).expect("series"),
        tags: vec![],
    };
    let naming = NamingConfig::from_json(&json!({
        "standardEpisodeFormat": "S", "dailyEpisodeFormat": "D", "animeEpisodeFormat": "A",
        "seasonFolderFormat": "Staffel {season}"}));
    let mut formats = Vec::new();
    for kind in ["standard", "daily", "anime"] {
        series.data.series_type = kind.to_owned();
        formats.push(episode_pattern(&series, &naming).to_owned());
    }
    assert_eq!(formats, ["S", "D", "A"]);
    series.data.season_folder = false;
    assert_eq!(
        season_folder(&series, 1, &naming),
        PathBuf::from("/tv/Kleine Insel")
    );
}
