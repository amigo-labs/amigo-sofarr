//! The rename preview names files with the media info read from them, as the import does
//! (spec web-ui-library AC-14, spec mediainfo).

use std::{
    fs,
    path::{Path, PathBuf},
};

use reqwest::StatusCode;
use serde_json::{Value, json};
use sofarr_db::FileRef;

use crate::{
    app::App,
    library_files_tests::{movie_on_disk, series_on_disk},
    testkit::{Http, Library, app},
};

const LIVE: &str = "[automation]\nmode = \"live\"\n";
/// The release name `movie_on_disk` records: it says x264, the file says x265.
const RELEASE: &str = "Zweiter.Film.2002.German.1080p.WEB-DL.x264-GRP";

fn temp(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("sofarr-rename-media-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("dir");
    dir
}

/// A Matroska file with x265 video (PQ, so HDR10), German E-AC-3 5.1 and German subtitles.
fn video() -> Vec<u8> {
    crate::media_tests::mkv_full(
        Some(("A_EAC3", 6, "ger")),
        Some(("S_TEXT/UTF8", "ger")),
        Some("x265 (build 199)"),
    )
}

/// Writes the video to `path`, reads it and stores its media info for `file`.
async fn read_and_store(app: &App, file: FileRef, path: &Path) -> sofarr_mediainfo::MediaInfo {
    fs::write(path, video()).expect("video");
    let media = crate::media::read_path(path).expect("read");
    crate::media::store(&app.db, file, &media)
        .await
        .expect("store");
    media.info
}

fn new_paths(preview: &Value) -> Vec<String> {
    preview["files"]
        .as_array()
        .expect("files")
        .iter()
        .map(|f| f["new_path"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[tokio::test]
async fn a_movie_rename_takes_the_media_info_tokens_from_the_file_like_the_import() {
    let root = temp("movie");
    let app = app(Library::Movies, LIVE).await;
    let (id, folder) = movie_on_disk(&app, &root).await;
    app.db
        .put_setting(
            "config_naming",
            &json!({"renameMovies": true,
                    "standardMovieFormat": "{Movie Title} ({Release Year}) {MediaInfo Simple}{.MediaInfo VideoDynamicRange}",
                    "movieFolderFormat": "{Movie Title} ({Release Year})"}),
        )
        .await
        .expect("naming");
    let info = read_and_store(&app, FileRef::Movie(id), &folder.join("zweiter.mkv")).await;
    let download = root.join("download").join(RELEASE);
    fs::create_dir_all(&download).expect("download");
    fs::write(download.join("grp-zweiter.mkv"), video()).expect("download video");
    let movie = app.db.movie(id).await.expect("db").expect("movie");
    let naming = crate::folders::movie_naming(&app).await.expect("naming");
    let plan = crate::importer::plan(
        &movie,
        RELEASE,
        "WEBDL-1080p",
        &download,
        &naming,
        Some(&info),
    )
    .expect("plan");
    let imported = plan
        .destination
        .file_name()
        .expect("name")
        .to_string_lossy()
        .into_owned();
    assert_eq!(imported, "Zweiter Film (2002) x265 EAC3.HDR10.mkv");
    let http = Http::start(app).await;
    let (status, preview) = http.get(&format!("/api/library/movies/{id}/rename")).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(new_paths(&preview), [imported]);
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn an_episode_rename_takes_the_media_info_tokens_from_the_file() {
    let root = temp("series");
    let app = app(Library::Series, LIVE).await;
    let (show, folder) = series_on_disk(&app, &root).await;
    app.db
        .put_setting(
            "sonarr_config_naming",
            &json!({"renameEpisodes": true,
                    "standardEpisodeFormat": "{Series Title} - S{season:00}E{episode:00} {MediaInfo Simple} {MediaInfo VideoDynamicRange}",
                    "seasonFolderFormat": "Staffel {season:00}"}),
        )
        .await
        .expect("naming");
    let episodes = app.db.episodes(Some(show)).await.expect("episodes");
    let first = episodes
        .iter()
        .find(|e| e.data.season == 1 && e.data.number == 1)
        .and_then(|e| e.file_id)
        .expect("S01E01 file");
    read_and_store(
        &app,
        FileRef::Episode(first),
        &folder.join("Staffel 01/Kleine Insel - S01E01.mkv"),
    )
    .await;
    let http = Http::start(app).await;
    let (status, preview) = http
        .get(&format!("/api/library/series/{show}/rename"))
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    // S02E01 has neither media info nor a release name: no codec tokens.
    assert_eq!(
        new_paths(&preview),
        [
            "Staffel 01/Kleine Insel - S01E01 x265 EAC3 HDR10.mkv",
            "Staffel 02/Kleine Insel - S02E01.mkv"
        ]
    );
    let _ = fs::remove_dir_all(&root);
}
