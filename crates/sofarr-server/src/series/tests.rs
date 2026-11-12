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
