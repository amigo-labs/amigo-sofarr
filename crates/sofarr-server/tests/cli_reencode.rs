//! `amigo-sofarr reencode-candidates` (spec reencode-rule AC-07, AC-08).

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use sofarr_db::{Db, FileRef, MovieFile, NewMovie, StoredMediaInfo};

fn root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("sofarr-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("root");
    root
}

/// Writes the config and runs the command; returns what it printed.
fn run(root: &Path, mode: &str) -> String {
    let config = root.join("sofarr.toml");
    std::fs::write(
        &config,
        format!(
            "[database]\npath = '{}'\n[log]\nlevel = \"error\"\n[automation]\nmode = \"{mode}\"\n",
            root.join("sofarr.db").display()
        ),
    )
    .expect("config");
    let output = Command::new(env!("CARGO_BIN_EXE_amigo-sofarr"))
        .arg("--config")
        .arg(&config)
        .arg("reencode-candidates")
        .output()
        .expect("binary runs");
    assert!(output.status.success(), "the command succeeds");
    String::from_utf8(output.stdout).expect("utf-8")
}

/// A movie whose 1080p file of `megabytes` runs 100 minutes by its header (or has no
/// media info at all).
async fn add_movie(db: &Db, tmdb_id: i64, title: &str, megabytes: Option<u64>) {
    let now = jiff::Timestamp::now();
    let movie = NewMovie {
        tmdb_id,
        imdb_id: None,
        title: title.to_owned(),
        original_title: None,
        title_de: None,
        year: Some(2019),
        runtime_minutes: 100,
        original_language: None,
        monitored: true,
        quality_profile_id: 1,
        path: format!("/filme/{title}"),
        radarr_id: None,
        alternative_titles: vec![],
        availability: sofarr_db::Availability::default(),
        tags: vec![],
    };
    let id = db.upsert_movie(&movie, now).await.expect("movie");
    let file = MovieFile {
        relative_path: format!("{title}.mkv"),
        size: 1,
        quality: "Bluray-1080p".to_owned(),
        revision_version: 1,
        revision_real: 0,
        release_group: None,
        scene_name: None,
        custom_format_score: 0,
    };
    db.set_file(id, &file, now).await.expect("file");
    let Some(megabytes) = megabytes else { return };
    let info = r#"{"status":"complete","container":"matroska","video":{"codec":"h264",
        "encoder":null,"width":1920,"height":1080,"dynamic_range":{"hdr10":false,
        "hdr10_plus":false,"hlg":false,"dolby_vision":false}},"audio":[],"subtitles":[],
        "duration_ms":6000000}"#;
    let stored = StoredMediaInfo {
        info: info.to_owned(),
        size: megabytes * 1024 * 1024,
        modified: String::new(),
    };
    db.put_media_info(FileRef::Movie(id), &stored, now)
        .await
        .expect("media info");
}

#[tokio::test]
async fn the_command_prints_the_candidates_largest_first() {
    let root = root("reencode");
    let db = Db::open(&root.join("sofarr.db")).await.expect("db");
    add_movie(&db, 1, "Mittel", Some(8192)).await;
    add_movie(&db, 2, "Gross", Some(20_480)).await;
    add_movie(&db, 3, "Klein", Some(2048)).await;
    add_movie(&db, 4, "Blind", None).await;
    let expected = "20,0 GB  204,8 > 60,0 MB/min  /filme/Gross/Gross.mkv\n\
                    8,0 GB  81,9 > 60,0 MB/min  /filme/Mittel/Mittel.mkv\n\
                    2 Dateien, 28,0 GB\n\
                    1 × keine Media-Infos\n";
    // The command reads the database itself while this test (like a running server)
    // holds it open, and prints the same in both modes.
    assert_eq!(run(&root, "shadow"), expected);
    assert_eq!(run(&root, "live"), expected);
    assert_eq!(db.movies().await.expect("movies").len(), 4);
    assert_eq!(db.all_media_info().await.expect("media info").len(), 3);
    db.pool().close().await;
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_empty_library_has_no_file_that_is_too_large() {
    let root = root("reencode-empty");
    assert_eq!(run(&root, "shadow"), "Keine Datei ist zu groß.\n");
    let _ = std::fs::remove_dir_all(&root);
}
