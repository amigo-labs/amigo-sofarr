//! Media info at an episode import (spec mediainfo AC-01, AC-06): the name takes its
//! `{MediaInfo…}` parts from the file, and the library file's media info is stored.

use jiff::Timestamp;
use serde_json::json;
use sofarr_db::FileRef;

use crate::{
    media_tests::{dir, mkv, stored},
    series,
    testkit::{Library, Services, app},
};

#[tokio::test]
async fn an_episode_import_names_and_records_from_the_file() {
    let root = dir("import-episode");
    // The release name says h264 and nothing about HDR or audio; the file is HEVC HDR10
    // with German EAC3 5.1.
    let release = "Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP";
    let download = root.join("downloads").join(release);
    std::fs::create_dir_all(&download).expect("download");
    std::fs::write(
        download.join("kleine.insel.s01e02.mkv"),
        mkv(Some(("A_EAC3", 6, "ger"))),
    )
    .expect("video");
    let show_dir = root.join("tv").join("Kleine Insel");
    let s = Services::start().await;
    s.feed("idx", &[s.item(release, "s1", 2_000_000_000)]);
    let config = format!(
        "[automation]\nmode = \"live\"\n{}{}[[download.path_map]]\nremote = \"/remote\"\nlocal = '{}'\n",
        s.indexer_toml("idx", &[]),
        s.nzbget_toml(),
        root.join("downloads").display()
    );
    let app = app(Library::Series, &config).await;
    let mut show = app.db.all_series().await.expect("series")[0].clone();
    show.data.path = show_dir.to_string_lossy().into_owned();
    app.db
        .upsert_series(&show.data, Timestamp::now())
        .await
        .expect("move");
    app.db
        .put_setting(
            &format!("{}config_naming", series::PREFIX),
            &json!({"renameEpisodes": true, "multiEpisodeStyle": 5,
                    "standardEpisodeFormat":
                        "{Series Title} - S{season:00}E{episode:00} {MediaInfo Simple} {MediaInfo VideoDynamicRange}",
                    "seasonFolderFormat": "Staffel {season:00}"}),
        )
        .await
        .expect("naming");
    let report = app.rss_sync().await.expect("rss");
    assert_eq!(report.series_grabs, 1, "{report:?}");
    s.finish(77, release, "SUCCESS/ALL", &format!("/remote/{release}"));
    assert_eq!(app.check_downloads().await.expect("check"), 1);

    // Video, audio and dynamic range from the file, not from the release name (AC-06).
    let season = show_dir.join("Staffel 01");
    let target = season.join("Kleine Insel - S01E02 h265 EAC3 HDR.mkv");
    let found: Vec<String> = std::fs::read_dir(&season)
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    assert!(target.exists(), "{found:?}");

    // Stored for the new episode file (AC-01).
    let episode = app
        .db
        .episodes(Some(show.id))
        .await
        .expect("episodes")
        .into_iter()
        .find(|e| e.data.season == 1 && e.data.number == 2)
        .expect("E02");
    let file_id = episode.file_id.expect("file recorded");
    let info = stored(&app, FileRef::Episode(file_id))
        .await
        .expect("stored");
    assert_eq!(info.labels().audio.as_deref(), Some("EAC3"));
    assert_eq!(info.labels().dynamic_range.as_deref(), Some("HDR"));
    // Recorded with the library file's size and time: the next scan does not read it.
    let (_, shows) = app.rescan().await.expect("scan");
    assert_eq!(shows.expect("series scan").media_read, 0);
    let _ = std::fs::remove_dir_all(&root);
}
