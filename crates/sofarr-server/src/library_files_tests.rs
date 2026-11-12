//! File actions over HTTP on real folders (spec web-ui-library AC-12 to AC-17, AC-24).

use std::{
    fs,
    path::{Path, PathBuf},
};

use jiff::Timestamp;
use reqwest::StatusCode;
use serde_json::{Value, json};
use sofarr_db::{GrabStatus, MovieFile, NewEpisode, NewGrab};

use crate::{
    app::App,
    testkit::{Http, Library, Services, app},
};

const LIVE: &str = "[automation]\nmode = \"live\"\n";
const RELEASE: &str = "Zweiter.Film.2002.German.1080p.WEB-DL.x264-GRP";

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sofarr-files-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("dir");
    dir
}

fn text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn error(body: &Value) -> String {
    body["error"].as_str().unwrap_or_default().to_owned()
}

/// Zweiter Film in `<root>/movies/Zweiter Film (2002)/zweiter.mkv`.
async fn movie_on_disk(app: &App, root: &Path) -> (i64, PathBuf) {
    let id = app
        .db
        .movie_by_tmdb(102)
        .await
        .expect("db")
        .expect("movie")
        .id;
    let folder = root.join("movies").join("Zweiter Film (2002)");
    fs::create_dir_all(&folder).expect("folder");
    fs::write(folder.join("zweiter.mkv"), b"video").expect("video");
    app.db
        .set_movie_path(id, &text(&folder))
        .await
        .expect("path");
    app.db
        .set_file(
            id,
            &MovieFile {
                relative_path: "zweiter.mkv".to_owned(),
                size: 5,
                quality: "WEBDL-1080p".to_owned(),
                revision_version: 1,
                revision_real: 0,
                release_group: Some("GRP".to_owned()),
                scene_name: Some(RELEASE.to_owned()),
                custom_format_score: 10,
            },
            Timestamp::now(),
        )
        .await
        .expect("file");
    app.db
        .put_setting(
            "config_naming",
            &json!({"renameMovies": true,
                    "standardMovieFormat": "{Movie Title} ({Release Year}) {Quality Full}{-Release Group}",
                    "movieFolderFormat": "{Movie Title} ({Release Year})"}),
        )
        .await
        .expect("naming");
    (id, folder)
}

async fn active_grab(app: &App, movie_id: i64, download_id: &str) {
    app.db
        .add_grab(
            &NewGrab {
                movie_id,
                title: "Zweiter.Film.2002.1080p.BluRay-X".to_owned(),
                guid: "x".to_owned(),
                indexer: "idx".to_owned(),
                download_url: None,
                quality: "Bluray-1080p".to_owned(),
                custom_format_score: 0,
                size: 1,
                status: GrabStatus::Downloading,
                download_id: Some(download_id.to_owned()),
            },
            Timestamp::now(),
        )
        .await
        .expect("grab");
}

#[tokio::test]
async fn rename_gives_the_name_the_import_would_give() {
    let root = temp("rename");
    let app = app(Library::Movies, LIVE).await;
    let (id, folder) = movie_on_disk(&app, &root).await;
    // The import's name for the same release (AC-14).
    let download = root.join("download").join(RELEASE);
    fs::create_dir_all(&download).expect("download");
    fs::write(download.join("grp-zweiter.mkv"), b"v").expect("video");
    let movie = app.db.movie(id).await.expect("db").expect("movie");
    let naming = crate::folders::movie_naming(&app).await.expect("naming");
    let plan = crate::importer::plan(&movie, RELEASE, "WEBDL-1080p", &download, &naming, None)
        .expect("plan");
    let imported = plan
        .destination
        .file_name()
        .expect("name")
        .to_string_lossy()
        .into_owned();
    assert_eq!(imported, "Zweiter Film (2002) WEBDL-1080p-GRP.mkv");

    let http = Http::start(app).await;
    let path = format!("/api/library/movies/{id}/rename");
    let (status, preview) = http.get(&path).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(
        preview["files"],
        json!([{"file_id": null, "episode_ids": [], "old_path": "zweiter.mkv", "new_path": imported}])
    );
    assert_eq!(preview["file_actions"]["available"], true);
    let (status, done) = http.post(&path, json!({})).await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["renamed"], 1);
    assert!(folder.join(&imported).exists());
    assert!(!folder.join("zweiter.mkv").exists());
    let stored = http.app.db.movie(id).await.expect("db").expect("movie");
    assert_eq!(stored.file.map(|f| f.relative_path), Some(imported));
    let (_, after) = http.get(&path).await;
    assert_eq!(after["files"], json!([]), "nothing differs any more");
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn an_existing_target_is_never_overwritten() {
    let root = temp("taken");
    let app = app(Library::Movies, LIVE).await;
    let (id, folder) = movie_on_disk(&app, &root).await;
    let target = folder.join("Zweiter Film (2002) WEBDL-1080p-GRP.mkv");
    fs::write(&target, b"other").expect("other");
    let http = Http::start(app).await;
    let (status, body) = http
        .post(&format!("/api/library/movies/{id}/rename"), json!({}))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        error(&body),
        "„Zweiter Film (2002) WEBDL-1080p-GRP.mkv“ existiert bereits – nichts wurde verschoben \
         oder überschrieben"
    );
    assert_eq!(fs::read(&target).expect("other"), b"other");
    assert_eq!(fs::read(folder.join("zweiter.mkv")).expect("own"), b"video");
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn shadow_mode_refuses_file_actions_but_removes_from_the_library() {
    let root = temp("shadow");
    let app = app(Library::Movies, "").await;
    let (id, folder) = movie_on_disk(&app, &root).await;
    let http = Http::start(app).await;
    for path in [
        format!("/api/library/movies/{id}/rename"),
        format!("/api/library/movies/{id}/folder"),
    ] {
        let (status, body) = http.post(&path, json!({})).await;
        assert_eq!(status, StatusCode::CONFLICT, "{path}");
        assert!(error(&body).contains("nur im Live-Modus"), "{body}");
    }
    let (status, body) = http
        .post(
            &format!("/api/library/movies/{id}/remove"),
            json!({"delete_files": true}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(folder.join("zweiter.mkv").exists());
    let (status, answer) = http
        .post(
            &format!("/api/library/movies/{id}/remove"),
            json!({"delete_files": false, "add_exclusion": true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["files_removed"], 0);
    assert!(http.app.db.movie(id).await.expect("db").is_none());
    assert!(folder.join("zweiter.mkv").exists(), "files stay");
    let excluded = crate::import_lists::exclusions(
        http.app
            .db
            .setting("exclusions")
            .await
            .expect("db")
            .as_ref(),
    );
    assert!(excluded.contains(&102), "import lists skip it");
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn an_active_download_blocks_file_actions_and_goes_with_the_item() {
    let root = temp("busy");
    let s = Services::start().await;
    let app = app(Library::Movies, &format!("{LIVE}{}", s.nzbget_toml())).await;
    let (id, folder) = movie_on_disk(&app, &root).await;
    active_grab(&app, id, "77").await;
    let http = Http::start(app).await;
    let (_, detail) = http.get(&format!("/api/library/movies/{id}")).await;
    assert_eq!(detail["file_actions"]["available"], false);
    for (path, body) in [
        (format!("/api/library/movies/{id}/rename"), json!({})),
        (
            format!("/api/library/movies/{id}/remove"),
            json!({"delete_files": true}),
        ),
    ] {
        let (status, answer) = http.post(&path, body).await;
        assert_eq!(status, StatusCode::CONFLICT, "{path}");
        assert!(
            error(&answer).contains("läuft ein Download oder Import"),
            "{answer}"
        );
    }
    assert!(folder.join("zweiter.mkv").exists());
    let (status, answer) = http
        .post(&format!("/api/library/movies/{id}/remove"), json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["downloads_removed"], 1);
    assert!(
        s.lock().rpc.iter().any(|c| c.starts_with("editqueue")
            && c.contains("GroupFinalDelete")
            && c.contains("77")),
        "the download left nzbget"
    );
    assert!(http.app.db.movie(id).await.expect("db").is_none());
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_changed_title_moves_the_folder_also_in_letter_case() {
    let root = temp("folder");
    let app = app(Library::Movies, LIVE).await;
    let (id, folder) = movie_on_disk(&app, &root).await;
    sqlx::query("UPDATE movies SET title = 'Zweiter Teil' WHERE id = ?")
        .bind(id)
        .execute(app.db.pool())
        .await
        .expect("title");
    let http = Http::start(app).await;
    let path = format!("/api/library/movies/{id}/folder");
    let (status, preview) = http.get(&path).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let expected = text(&root.join("movies").join("Zweiter Teil (2002)"));
    assert_eq!(preview["folder"]["expected"], expected.as_str());
    assert_eq!(preview["folder"]["differs"], true);
    let (status, moved) = http.post(&path, json!({})).await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(moved["path"], expected.as_str());
    assert!(PathBuf::from(&expected).join("zweiter.mkv").exists());
    assert!(!folder.exists());
    let stored = http.app.db.movie(id).await.expect("db").expect("movie");
    assert_eq!(stored.path, expected);
    let (status, body) = http.post(&path, json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT, "nothing to move: {body}");

    // Only the letter case changes: moved through a temporary name.
    sqlx::query("UPDATE movies SET title = 'Zweiter TEIL' WHERE id = ?")
        .bind(id)
        .execute(http.app.db.pool())
        .await
        .expect("title");
    let (status, moved) = http.post(&path, json!({})).await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    let names: Vec<String> = fs::read_dir(root.join("movies"))
        .expect("list")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["Zweiter TEIL (2002)"]);

    // A target that exists, or a source that is gone: nothing moves.
    sqlx::query("UPDATE movies SET title = 'Belegt' WHERE id = ?")
        .bind(id)
        .execute(http.app.db.pool())
        .await
        .expect("title");
    fs::create_dir_all(root.join("movies").join("Belegt (2002)")).expect("taken");
    let (status, body) = http.post(&path, json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        error(&body).contains("Belegt (2002)“ existiert bereits"),
        "{body}"
    );
    fs::remove_dir_all(root.join("movies")).expect("gone");
    let (status, body) = http.post(&path, json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(error(&body).contains("fehlt"), "{body}");
    let stored = http.app.db.movie(id).await.expect("db").expect("movie");
    assert!(
        stored.path.ends_with("Zweiter TEIL (2002)"),
        "library unchanged"
    );
    let _ = fs::remove_dir_all(&root);
}

/// Kleine Insel in `<root>/tv/Kleine Insel` with S01E01 and S02E01 on disk.
async fn series_on_disk(app: &App, root: &Path) -> (i64, PathBuf) {
    let show = app.db.all_series().await.expect("db")[0].id;
    let folder = root.join("tv").join("Kleine Insel");
    fs::create_dir_all(folder.join("Staffel 01")).expect("s1");
    fs::create_dir_all(folder.join("Staffel 02")).expect("s2");
    fs::write(folder.join("Staffel 01/Kleine Insel - S01E01.mkv"), b"1").expect("e1");
    fs::write(folder.join("Staffel 02/k.s02e01.mkv"), b"2").expect("e2");
    fs::write(folder.join("tvshow.nfo"), b"n").expect("nfo");
    app.db
        .set_series_path(show, &text(&folder))
        .await
        .expect("path");
    let e = app
        .db
        .upsert_episode(
            show,
            &NewEpisode {
                tvdb_id: None,
                season: 2,
                number: 1,
                absolute: None,
                title: Some("Rückkehr".to_owned()),
                title_de: None,
                air_date: Some("2020-01-01".to_owned()),
                monitored: true,
            },
        )
        .await
        .expect("episode");
    app.db
        .set_episode_file(
            show,
            &[e],
            &MovieFile {
                relative_path: "Staffel 02/k.s02e01.mkv".to_owned(),
                size: 1,
                quality: "WEBDL-1080p".to_owned(),
                revision_version: 1,
                revision_real: 0,
                release_group: None,
                scene_name: None,
                custom_format_score: 0,
            },
            Timestamp::now(),
        )
        .await
        .expect("file");
    (show, folder)
}

fn recorded(eps: &[sofarr_db::Episode]) -> Vec<String> {
    let mut paths: Vec<String> = eps
        .iter()
        .filter_map(|e| e.file.as_ref().map(|f| f.relative_path.clone()))
        .collect();
    paths.sort();
    paths
}

#[tokio::test]
async fn a_series_rename_that_fails_midway_moves_back() {
    let root = temp("series-rename");
    let app = app(Library::Series, LIVE).await;
    let (show, folder) = series_on_disk(&app, &root).await;
    app.db
        .put_setting(
            "sonarr_config_naming",
            &json!({"renameEpisodes": true,
                    "standardEpisodeFormat": "{Series Title} - S{season:00}E{episode:00} - {Episode Title} {Quality Full}",
                    "seasonFolderFormat": "Season {season:00}"}),
        )
        .await
        .expect("naming");
    // A file where the second season's new folder would go.
    fs::write(folder.join("Season 02"), b"x").expect("blocker");
    let http = Http::start(app).await;
    let path = format!("/api/library/series/{show}/rename");
    let (_, preview) = http.get(&path).await;
    let news: Vec<&str> = preview["files"]
        .as_array()
        .expect("files")
        .iter()
        .map(|f| f["new_path"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        news,
        [
            "Season 01/Kleine Insel - S01E01 - Ankunft WEBDL-1080p.mkv",
            "Season 02/Kleine Insel - S02E01 - Rückkehr WEBDL-1080p.mkv"
        ]
    );
    let (status, body) = http.post(&path, json!({})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert!(
        error(&body).starts_with("„Staffel 02/k.s02e01.mkv“ ließ sich nicht"),
        "{body}"
    );
    assert!(
        folder.join("Staffel 01/Kleine Insel - S01E01.mkv").exists(),
        "moved back"
    );
    assert!(folder.join("Staffel 02/k.s02e01.mkv").exists());
    let eps = http.app.db.episodes(Some(show)).await.expect("eps");
    assert_eq!(
        recorded(&eps),
        [
            "Staffel 01/Kleine Insel - S01E01.mkv",
            "Staffel 02/k.s02e01.mkv"
        ]
    );
    fs::remove_file(folder.join("Season 02")).expect("unblock");
    let (status, done) = http.post(&path, json!({})).await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["renamed"], 2);
    let eps = http.app.db.episodes(Some(show)).await.expect("eps");
    assert_eq!(recorded(&eps), news);
    assert!(
        !folder.join("Staffel 01").exists(),
        "emptied season folders go"
    );

    // Two files with the same new name: refused before anything moves.
    http.app
        .db
        .put_setting(
            "sonarr_config_naming",
            &json!({"standardEpisodeFormat": "{Series Title} {Quality Full}",
                    "seasonFolderFormat": "Alle"}),
        )
        .await
        .expect("naming");
    let (status, body) = http.post(&path, json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        error(&body).starts_with("Zwei Dateien bekämen denselben Namen"),
        "{body}"
    );
    assert_eq!(
        recorded(&http.app.db.episodes(Some(show)).await.expect("eps")),
        news
    );
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn removing_a_series_with_files_uses_the_recycle_bin() {
    let root = temp("series-remove");
    let app = app(Library::Series, LIVE).await;
    let (show, folder) = series_on_disk(&app, &root).await;
    let bin = root.join("bin");
    app.db
        .put_setting(
            "sonarr_config_mediamanagement",
            &json!({"recycleBin": text(&bin)}),
        )
        .await
        .expect("bin");
    // Something in the bin blocks the second season.
    fs::create_dir_all(bin.join("Kleine Insel")).expect("bin");
    fs::write(bin.join("Kleine Insel").join("Staffel 02"), b"x").expect("blocker");
    let http = Http::start(app).await;
    let path = format!("/api/library/series/{show}/remove");
    let body = json!({"delete_files": true, "add_exclusion": true});
    let (status, answer) = http.post(&path, body.clone()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{answer}");
    assert_eq!(
        error(&answer),
        "„Staffel 02/k.s02e01.mkv“ ließ sich nicht in den Papierkorb verschieben – die Serie \
         bleibt in der Bibliothek"
    );
    assert!(
        bin.join("Kleine Insel/Staffel 01/Kleine Insel - S01E01.mkv")
            .exists()
    );
    let eps = http.app.db.episodes(Some(show)).await.expect("eps");
    assert_eq!(
        recorded(&eps),
        ["Staffel 02/k.s02e01.mkv"],
        "only what is still there"
    );
    assert_eq!(http.app.db.all_series().await.expect("db").len(), 1);

    fs::remove_file(bin.join("Kleine Insel").join("Staffel 02")).expect("unblock");
    let (status, answer) = http.post(&path, body).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["files_removed"], 2, "the episode and the nfo");
    assert_eq!(answer["recycle_bin"], true);
    assert!(bin.join("Kleine Insel/Staffel 02/k.s02e01.mkv").exists());
    assert!(bin.join("Kleine Insel/tvshow.nfo").exists());
    assert!(!folder.exists(), "empty folders go");
    assert!(http.app.db.all_series().await.expect("db").is_empty());
    let excluded = http
        .app
        .db
        .setting("sonarr_importlistexclusion")
        .await
        .expect("db");
    assert_eq!(
        excluded,
        Some(json!([{"tvdbId": 77, "title": "Kleine Insel"}]))
    );
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn removing_a_movie_without_recycle_bin_deletes_its_files() {
    let root = temp("movie-remove");
    let app = app(Library::Movies, LIVE).await;
    let (id, folder) = movie_on_disk(&app, &root).await;
    let http = Http::start(app).await;
    let (status, answer) = http
        .post(
            &format!("/api/library/movies/{id}/remove"),
            json!({"delete_files": true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["files_removed"], 1);
    assert_eq!(answer["recycle_bin"], false);
    assert!(!folder.exists());
    assert!(http.app.db.movie(id).await.expect("db").is_none());
    let (status, body) = http
        .post(
            &format!("/api/library/movies/{id}/remove"),
            json!({"löschen": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let _ = fs::remove_dir_all(&root);
}
