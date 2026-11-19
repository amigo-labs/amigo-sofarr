//! The download check imports finished downloads into the library (spec download-import).

use std::path::{Path, PathBuf};

use jiff::Timestamp;
use serde_json::json;
use sofarr_db::{GrabStatus, NewEpisode};

use crate::{
    series,
    testkit::{Library, Services, app},
};

const G1: &str = "Erster.Film.2001.German.1080p.BluRay.x264-GRP";
const S1: &str = "Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP";

/// A fresh folder for one test.
fn root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sofarr-dl-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("root");
    dir
}

/// Live mode with the fake's indexer and nzbget; nzbget's `/remote` is `<root>/downloads`.
fn live(s: &Services, root: &Path, extra: &str) -> String {
    format!(
        "[automation]\nmode = \"live\"\n{}{}{extra}[[download.path_map]]\nremote = \"/remote\"\nlocal = '{}'\n",
        s.indexer_toml("idx", &[]),
        s.nzbget_toml(),
        root.join("downloads").display()
    )
}

/// A download folder with the given files (name, size).
fn download(root: &Path, release: &str, files: &[(&str, usize)]) -> PathBuf {
    let dir = root.join("downloads").join(release);
    for (name, size) in files {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, vec![1u8; *size]).expect("file");
    }
    dir
}

async fn movie_app(
    s: &Services,
    root: &Path,
    extra: &str,
    tmdb: i64,
) -> (crate::app::App, PathBuf) {
    let app = app(Library::Movies, &live(s, root, extra)).await;
    let movie = app
        .db
        .movie_by_tmdb(tmdb)
        .await
        .expect("db")
        .expect("movie");
    let folder = root.join("movies").join(&movie.title);
    app.db
        .set_movie_path(movie.id, &folder.to_string_lossy())
        .await
        .expect("path");
    (app, folder)
}

#[tokio::test]
async fn queued_paused_and_downloading_downloads() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(G1, "g1", 5_000_000_000)]);
    let root = root("states");
    let (app, _) = movie_app(&s, &root, "", 101).await;
    app.rss_sync().await.expect("rss");
    let status = || async { app.db.grabs(None, 10).await.expect("grabs")[0].status };
    for (state, expected) in [
        ("QUEUED", GrabStatus::Queued),
        ("PAUSED", GrabStatus::Queued),
        ("DOWNLOADING", GrabStatus::Downloading),
    ] {
        s.lock().queue = vec![json!({"NZBID": 77, "NZBName": G1, "Status": state})];
        assert_eq!(app.check_downloads().await.expect("check"), 0);
        assert_eq!(status().await, expected, "{state}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn without_renaming_the_video_keeps_its_name_and_the_extras_stay() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(G1, "g1", 5_000_000_000)]);
    let root = root("keep-name");
    let dir = download(
        &root,
        G1,
        &[
            ("grp-erster.MKV", 4096),
            ("grp-erster.nfo", 10),
            ("grp-erster.de.srt", 10),
            ("Sample/grp-erster-sample.mkv", 8192),
        ],
    );
    let (app, folder) = movie_app(&s, &root, "[download]\nremove_completed = false\n", 101).await;
    app.db
        .put_setting("config_naming", &json!({"renameMovies": false}))
        .await
        .expect("naming");
    app.rss_sync().await.expect("rss");
    s.finish(77, G1, "SUCCESS/ALL", &format!("/remote/{G1}"));
    assert_eq!(app.check_downloads().await.expect("check"), 1);

    assert!(
        folder.join("grp-erster.mkv").is_file(),
        "moved, extension in lower case"
    );
    assert!(!dir.join("grp-erster.MKV").exists(), "moved, not copied");
    for extra in [
        "grp-erster.nfo",
        "grp-erster.de.srt",
        "Sample/grp-erster-sample.mkv",
    ] {
        assert!(dir.join(extra).is_file(), "{extra} stays in the download");
    }
    assert_eq!(std::fs::read_dir(&folder).expect("folder").count(), 1);
    // Without "remove completed" the download stays in the client's history.
    assert!(!s.lock().rpc.iter().any(|r| r.starts_with("editqueue")));

    let movie = app.db.movie_by_tmdb(101).await.expect("db").expect("movie");
    let file = movie.file.expect("file recorded");
    assert_eq!(file.relative_path, "grp-erster.mkv");
    assert_eq!(file.size, 4096);
    assert_eq!(file.quality, "Bluray-1080p");
    assert_eq!(file.release_group.as_deref(), Some("GRP"));
    assert_eq!(file.scene_name.as_deref(), Some(G1));
    assert_eq!(file.custom_format_score, 10, "German custom format");
    let grab = &app.db.grabs(None, 10).await.expect("grabs")[0];
    assert_eq!(grab.status, GrabStatus::Imported);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn remove_completed_clears_the_history_and_the_leftover_folder() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(G1, "g1", 5_000_000_000)]);
    let root = root("remove-completed");
    let dir = download(
        &root,
        G1,
        &[("grp-erster.mkv", 4096), ("grp-erster.nfo", 10)],
    );
    let (app, folder) = movie_app(&s, &root, "", 101).await;
    app.db
        .put_setting(
            "config_naming",
            &json!({"renameMovies": true,
                    "standardMovieFormat": "{Movie Title} ({Release Year}) {Quality Full}"}),
        )
        .await
        .expect("naming");
    app.rss_sync().await.expect("rss");
    s.finish(77, G1, "SUCCESS/ALL", &format!("/remote/{G1}"));
    assert_eq!(app.check_downloads().await.expect("check"), 1);
    assert!(folder.join("Erster Film (2001) Bluray-1080p.mkv").is_file());
    assert!(!dir.exists(), "the folder without videos is gone");
    assert!(
        s.lock()
            .rpc
            .iter()
            .any(|r| r.starts_with("editqueue") && r.contains("HistoryDelete") && r.contains("77")),
        "{:?}",
        s.lock().rpc
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn an_upgrade_recycles_or_deletes_the_old_file() {
    const UPGRADE: &str = "Zweiter.Film.2002.German.1080p.BluRay.x264-F";
    for recycle in [true, false] {
        let s = Services::start().await;
        s.feed("idx", &[s.item(UPGRADE, "u1", 6_000_000_000)]);
        let root = root(if recycle {
            "upgrade-bin"
        } else {
            "upgrade-delete"
        });
        download(&root, UPGRADE, &[("zweiter.mkv", 4096)]);
        let (app, folder) = movie_app(&s, &root, "", 102).await;
        std::fs::create_dir_all(&folder).expect("folder");
        std::fs::write(folder.join("Zweiter Film (2002).mkv"), b"old").expect("old file");
        let bin = root.join("recycle");
        if recycle {
            app.db
                .put_setting(
                    "config_mediamanagement",
                    &json!({"recycleBin": bin.to_string_lossy()}),
                )
                .await
                .expect("media management");
        }
        app.db
            .put_setting(
                "config_naming",
                &json!({"renameMovies": true,
                        "standardMovieFormat": "{Movie Title} ({Release Year}) {Quality Full}"}),
            )
            .await
            .expect("naming");
        app.rss_sync().await.expect("rss");
        s.finish(77, UPGRADE, "SUCCESS/ALL", &format!("/remote/{UPGRADE}"));
        assert_eq!(app.check_downloads().await.expect("check"), 1);
        assert!(
            folder
                .join("Zweiter Film (2002) Bluray-1080p.mkv")
                .is_file()
        );
        assert!(!folder.join("Zweiter Film (2002).mkv").exists());
        assert_eq!(bin.join("Zweiter Film (2002).mkv").is_file(), recycle);
        let movie = app.db.movie_by_tmdb(102).await.expect("db").expect("movie");
        assert_eq!(movie.file.expect("file").quality, "Bluray-1080p");
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[tokio::test]
async fn a_failed_import_stays_completed_and_is_tried_again() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(G1, "g1", 5_000_000_000)]);
    let root = root("retry");
    let dir = download(&root, G1, &[("grp-erster.nfo", 10)]);
    let (app, folder) = movie_app(&s, &root, "", 101).await;
    app.rss_sync().await.expect("rss");
    s.finish(77, G1, "SUCCESS/ALL", &format!("/remote/{G1}"));
    assert_eq!(app.check_downloads().await.expect("check"), 0);
    let grab = &app.db.grabs(None, 10).await.expect("grabs")[0];
    assert_eq!(grab.status, GrabStatus::Completed);
    let message = grab.message.as_deref().unwrap_or_default();
    assert!(message.contains("keine Videodatei"), "{message}");
    assert!(
        app.db
            .movie_by_tmdb(101)
            .await
            .expect("db")
            .expect("movie")
            .file
            .is_none()
    );

    std::fs::write(dir.join("grp-erster.mkv"), vec![1u8; 4096]).expect("video");
    assert_eq!(app.check_downloads().await.expect("again"), 1);
    let grab = &app.db.grabs(None, 10).await.expect("grabs")[0];
    assert_eq!(grab.status, GrabStatus::Imported);
    assert_eq!(std::fs::read_dir(&folder).expect("folder").count(), 1);
    let _ = std::fs::remove_dir_all(&root);
}

/// The test series in a temporary folder, with its old E01 file on disk.
async fn series_app(s: &Services, root: &Path) -> (crate::app::App, PathBuf, i64) {
    let app = app(Library::Series, &live(s, root, "")).await;
    let show = app.db.all_series().await.expect("series")[0].clone();
    let folder = root.join("tv").join("Kleine Insel");
    app.db
        .set_series_path(show.id, &folder.to_string_lossy())
        .await
        .expect("path");
    std::fs::create_dir_all(folder.join("Staffel 01")).expect("season");
    std::fs::write(folder.join("Staffel 01/Kleine Insel - S01E01.mkv"), b"old").expect("old");
    (app, folder, show.id)
}

#[tokio::test]
async fn a_season_pack_imports_each_grabbed_episode() {
    const PACK: &str = "Kleine.Insel.S01.1080p.BluRay.x264-E";
    let s = Services::start().await;
    s.feed("idx", &[s.item(PACK, "p1", 4_000_000_000)]);
    let root = root("pack");
    let dir = download(
        &root,
        PACK,
        &[
            ("kleine.insel.s01e01.mkv", 2048),
            ("kleine.insel.s01e02.mkv", 2048),
            ("kleine.insel.s01e03.mkv", 2048),
            ("kleine.insel.s02e01.mkv", 2048),
        ],
    );
    let (app, folder, _) = series_app(&s, &root).await;
    let report = app.rss_sync().await.expect("rss");
    assert_eq!(report.series_grabs, 1, "{report:?}");
    s.finish(77, PACK, "SUCCESS/ALL", &format!("/remote/{PACK}"));
    assert_eq!(app.check_downloads().await.expect("check"), 3);

    let season = folder.join("Staffel 01");
    assert!(
        season
            .join("Kleine Insel - S01E01 - Ankunft Bluray-1080p.mkv")
            .is_file()
    );
    assert!(
        season
            .join("Kleine Insel - S01E02 - Sturm Bluray-1080p.mkv")
            .is_file()
    );
    assert!(
        !season.join("Kleine Insel - S01E01.mkv").exists(),
        "replaced"
    );
    // The pack holds the whole season, E03 (not aired yet) included.
    assert!(
        season
            .join("Kleine Insel - S01E03 - Zukunft Bluray-1080p.mkv")
            .is_file()
    );
    // S02E01 belongs to no grabbed episode: its file stays, and so does the folder.
    assert!(dir.join("kleine.insel.s02e01.mkv").is_file());
    let eps = app.db.episodes(None).await.expect("episodes");
    let path = |n: u16| {
        eps.iter()
            .find(|e| e.data.number == n)
            .and_then(|e| e.file.as_ref())
            .map(|f| f.relative_path.clone())
    };
    assert_eq!(
        path(1).as_deref(),
        Some("Staffel 01/Kleine Insel - S01E01 - Ankunft Bluray-1080p.mkv")
    );
    assert_eq!(
        path(2).as_deref(),
        Some("Staffel 01/Kleine Insel - S01E02 - Sturm Bluray-1080p.mkv")
    );
    assert_eq!(
        path(3).as_deref(),
        Some("Staffel 01/Kleine Insel - S01E03 - Zukunft Bluray-1080p.mkv")
    );
    let grab = &app.db.series_grabs(None).await.expect("grabs")[0];
    assert_eq!(grab.status, GrabStatus::Imported);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn an_import_waits_for_a_missing_episode_title() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(S1, "s1", 2_000_000_000)]);
    let root = root("titles");
    // An obfuscated single file: the grab's title names the episode.
    download(&root, S1, &[("a1b2c3.mkv", 2048)]);
    let (app, folder, show) = series_app(&s, &root).await;
    let today = series::today(Timestamp::now());
    let e02 = |title: &str| NewEpisode {
        tvdb_id: None,
        season: 1,
        number: 2,
        absolute: None,
        title: Some(title.to_owned()),
        title_de: None,
        air_date: Some(today.clone()),
        monitored: true,
    };
    app.db
        .upsert_episode(show, &e02("TBA"))
        .await
        .expect("episode");
    app.db
        .put_setting(
            &format!("{}config_mediamanagement", series::PREFIX),
            &json!({"episodeTitleRequired": "always"}),
        )
        .await
        .expect("media management");
    let report = app.rss_sync().await.expect("rss");
    assert_eq!(report.series_grabs, 1, "{report:?}");
    s.finish(77, S1, "SUCCESS/ALL", &format!("/remote/{S1}"));
    assert_eq!(app.check_downloads().await.expect("check"), 0);
    let grab = &app.db.series_grabs(None).await.expect("grabs")[0];
    assert_eq!(grab.status, GrabStatus::Completed);
    assert_eq!(
        grab.message.as_deref(),
        Some("Import wartet auf Episodentitel (höchstens 48 Stunden nach Ausstrahlung)")
    );

    app.db
        .upsert_episode(show, &e02("Sturm"))
        .await
        .expect("title");
    assert_eq!(app.check_downloads().await.expect("again"), 1);
    assert!(
        folder
            .join("Staffel 01/Kleine Insel - S01E02 - Sturm WEBDL-1080p.mkv")
            .is_file()
    );
    let _ = std::fs::remove_dir_all(&root);
}
