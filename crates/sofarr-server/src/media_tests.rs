//! Media info in the server (spec mediainfo): storing at import and scan, naming,
//! the file endpoints; the test files and helpers the other media tests share.

use std::{path::Path, sync::Arc};

use jiff::Timestamp;
use serde_json::{Value, json};
use sofarr_db::{FileRef, MovieFile, NewMovie};
use sofarr_mediainfo::ReadStatus;

use crate::{
    app::App,
    media,
    testkit::{Library, app},
};

// --- a minimal Matroska file ----------------------------------------------------------

pub(crate) fn el(id: u32, data: &[u8]) -> Vec<u8> {
    let id_bytes: Vec<u8> = id
        .to_be_bytes()
        .into_iter()
        .skip_while(|b| *b == 0)
        .collect();
    let size = u64::try_from(data.len()).expect("size");
    let mut out = id_bytes;
    // An 8-byte size: 0x01 marker plus 7 bytes.
    out.push(0x01);
    out.extend_from_slice(&size.to_be_bytes()[1..]);
    out.extend_from_slice(data);
    out
}

pub(crate) fn uint(id: u32, value: u64) -> Vec<u8> {
    el(id, &value.to_be_bytes())
}

pub(crate) fn text(id: u32, value: &str) -> Vec<u8> {
    el(id, value.as_bytes())
}

/// An MKV with an HEVC video track (PQ transfer: HDR10) and, if given, one audio track.
pub(crate) fn mkv(audio: Option<(&str, u64, &str)>) -> Vec<u8> {
    mkv_full(audio, None, None)
}

/// A subtitle track of [`mkv_full`]: codec ID and language; it is forced, not default and
/// for the hearing impaired.
pub(crate) type SubtitleTrack<'a> = (&'a str, &'a str);

/// [`mkv`] plus, if given, a subtitle track and an `ENCODER` tag.
pub(crate) fn mkv_full(
    audio: Option<(&str, u64, &str)>,
    subtitle: Option<SubtitleTrack<'_>>,
    encoder: Option<&str>,
) -> Vec<u8> {
    let colour = uint(0x55BA, 16);
    let video = [uint(0xB0, 1920), uint(0xBA, 1080), el(0x55B0, &colour)].concat();
    let video_track = [
        uint(0xD7, 1),
        uint(0x83, 1),
        text(0x86, "V_MPEGH/ISO/HEVC"),
        el(0xE0, &video),
    ]
    .concat();
    let mut tracks = el(0xAE, &video_track);
    if let Some((codec, channels, language)) = audio {
        let audio_track = [
            uint(0xD7, 2),
            uint(0x83, 2),
            text(0x86, codec),
            text(0x22B59C, language),
            el(0xE1, &uint(0x9F, channels)),
        ]
        .concat();
        tracks.extend(el(0xAE, &audio_track));
    }
    if let Some((codec, language)) = subtitle {
        let subtitle_track = [
            uint(0xD7, 3),
            uint(0x83, 0x11),
            text(0x86, codec),
            text(0x22B59C, language),
            uint(0x88, 0),
            uint(0x55AA, 1),
            uint(0x55AB, 1),
        ]
        .concat();
        tracks.extend(el(0xAE, &subtitle_track));
    }
    let mut level1 = el(0x1654AE6B, &tracks);
    if let Some(encoder) = encoder {
        let simple = [text(0x45A3, "ENCODER"), text(0x4487, encoder)].concat();
        level1.extend(el(0x1254C367, &el(0x7373, &el(0x67C8, &simple))));
    }
    let header = el(0x1A45DFA3, &text(0x4282, "matroska"));
    let segment = el(0x18538067, &level1);
    [header, segment].concat()
}

pub(crate) fn dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("sofarr-media-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

/// Moves a library movie into `folder` and records `file` as its file.
pub(crate) async fn place(app: &App, tmdb: i64, folder: &Path, file: &str) -> i64 {
    let m = app
        .db
        .movie_by_tmdb(tmdb)
        .await
        .expect("db")
        .expect("movie");
    let moved = NewMovie {
        tmdb_id: m.tmdb_id,
        imdb_id: m.imdb_id.clone(),
        title: m.title.clone(),
        original_title: m.original_title.clone(),
        title_de: m.title_de.clone(),
        year: m.year,
        runtime_minutes: m.runtime_minutes,
        original_language: m.original_language.clone(),
        monitored: m.monitored,
        quality_profile_id: m.quality_profile_id,
        path: folder.to_string_lossy().into_owned(),
        radarr_id: m.radarr_id,
        alternative_titles: m.alternative_titles.clone(),
        availability: m.availability.clone(),
        tags: m.tags.clone(),
    };
    app.db
        .upsert_movie(&moved, Timestamp::now())
        .await
        .expect("move");
    app.db
        .set_file(
            m.id,
            &MovieFile {
                relative_path: file.to_owned(),
                size: 1,
                quality: "Bluray-1080p".to_owned(),
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
    m.id
}

pub(crate) async fn stored(app: &App, file: FileRef) -> Option<sofarr_mediainfo::MediaInfo> {
    app.db
        .media_info(file)
        .await
        .expect("db")
        .as_ref()
        .and_then(media::parse)
}

// --- naming (AC-06) -------------------------------------------------------------------

#[test]
fn naming_takes_each_part_from_the_file_where_known() {
    let root = dir("naming");
    let full = root.join("full.mkv");
    std::fs::write(&full, mkv(Some(("A_EAC3", 6, "ger")))).expect("full");
    let video_only = root.join("video.mkv");
    std::fs::write(&video_only, mkv(None)).expect("video only");
    let release = "Film.2020.1080p.BluRay.DTS-HD.MA.x264-GRP";

    let info = media::read_path(&full).expect("read").info;
    let parts = media::NamingParts::of(Some(&info));
    assert_eq!(parts.simple(release).as_deref(), Some("h265 EAC3"));
    assert_eq!(parts.dynamic_range(release).as_deref(), Some("HDR10"));

    // No audio track in the file: that part comes from the release name.
    let info = media::read_path(&video_only).expect("read").info;
    let parts = media::NamingParts::of(Some(&info));
    assert_eq!(parts.simple(release).as_deref(), Some("h265 DTS-HD MA"));

    // No media info at all: the name, as before.
    let none = media::NamingParts::of(None);
    assert_eq!(none.simple(release).as_deref(), Some("x264 DTS-HD MA"));
    let _ = std::fs::remove_dir_all(&root);
}

// --- import and scan (AC-01, AC-02, AC-04, AC-11, AC-12) ------------------------------

#[tokio::test]
async fn the_disk_scan_reads_new_and_changed_files_only() {
    let root = dir("scan");
    // Shadow mode: the scan still reads media info (AC-12).
    let app = app(Library::Movies, "").await;
    assert_eq!(app.config.automation.mode, crate::config::Mode::Shadow);
    let folder = root.join("Erster Film (2001)");
    std::fs::create_dir_all(&folder).expect("folder");
    let path = folder.join("Erster Film (2001).mkv");
    std::fs::write(&path, mkv(Some(("A_EAC3", 6, "ger")))).expect("file");
    let id = place(&app, 101, &folder, "Erster Film (2001).mkv").await;
    assert!(stored(&app, FileRef::Movie(id)).await.is_none());
    let before = std::fs::metadata(&path)
        .expect("meta")
        .modified()
        .expect("mtime");

    // An existing library file without media info gets it (AC-02).
    let (movies, _) = app.rescan().await.expect("scan");
    assert_eq!(movies.expect("movies").media_read, 1);
    let info = stored(&app, FileRef::Movie(id)).await.expect("stored");
    assert_eq!(info.status, ReadStatus::Complete);
    assert_eq!(info.audio[0].language, "ger");
    // Reading changed nothing about the file (AC-11).
    let after = std::fs::metadata(&path)
        .expect("meta")
        .modified()
        .expect("mtime");
    assert_eq!(before, after);

    // Unchanged: not read again (AC-01).
    let (movies, _) = app.rescan().await.expect("scan");
    assert_eq!(movies.expect("movies").media_read, 0);

    // Replaced with other content and size: read again.
    std::fs::write(&path, mkv(Some(("A_AC3", 2, "eng")))).expect("replace");
    let (movies, _) = app.rescan().await.expect("scan");
    assert_eq!(movies.expect("movies").media_read, 1);
    let info = stored(&app, FileRef::Movie(id)).await.expect("stored");
    assert_eq!(info.audio[0].codec, "ac3");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn broken_files_get_a_status_and_unreadable_ones_are_retried() {
    let root = dir("broken");
    let app = app(Library::Movies, "").await;
    let file = FileRef::Movie(place(&app, 101, &root, "notes.mkv").await);
    let text = root.join("notes.mkv");
    std::fs::write(&text, b"no matroska at all").expect("text");
    assert!(media::refresh(&app.db, file, &text).await.expect("refresh"));
    assert_eq!(
        stored(&app, file).await.map(|m| m.status),
        Some(ReadStatus::Unsupported)
    );
    let cut = root.join("cut.mkv");
    let bytes = mkv(Some(("A_EAC3", 6, "ger")));
    std::fs::write(&cut, &bytes[..bytes.len() / 2]).expect("cut");
    assert!(media::refresh(&app.db, file, &cut).await.expect("refresh"));
    let status = stored(&app, file).await.map(|m| m.status);
    assert!(
        matches!(status, Some(ReadStatus::Partial | ReadStatus::Damaged)),
        "{status:?}"
    );
    // A folder in place of the file cannot be read: unreadable, and tried again at
    // every scan.
    let folder = root.join("folder.mkv");
    std::fs::create_dir_all(&folder).expect("folder");
    assert!(
        media::refresh(&app.db, file, &folder)
            .await
            .expect("refresh")
    );
    assert_eq!(
        stored(&app, file).await.map(|m| m.status),
        Some(ReadStatus::Unreadable)
    );
    assert!(media::refresh(&app.db, file, &folder).await.expect("again"));
    // A file that is gone is left to the scan: nothing stored.
    assert!(
        !media::refresh(&app.db, file, &root.join("gone.mkv"))
            .await
            .expect("gone")
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn an_import_names_and_records_from_the_file() {
    use crate::testkit::Services;
    let root = dir("import");
    let release = "Erster.Film.2001.German.1080p.BluRay.DTS.x264-GRP";
    let download = root.join("downloads").join(release);
    std::fs::create_dir_all(&download).expect("download");
    std::fs::write(download.join("erster.mkv"), mkv(Some(("A_EAC3", 6, "ger")))).expect("video");
    let library = root.join("movies").join("Erster Film (2001)");
    let s = Services::start().await;
    s.feed("idx", &[s.item(release, "g1", 5_000_000_000)]);
    let config = format!(
        "[automation]\nmode = \"live\"\n{}{}[[download.path_map]]\nremote = \"/remote\"\nlocal = '{}'\n",
        s.indexer_toml("idx", &[]),
        s.nzbget_toml(),
        root.join("downloads").display()
    );
    let app = app(Library::Movies, &config).await;
    app.db
        .put_setting(
            "config_naming",
            &json!({"renameMovies": true,
                    "standardMovieFormat": "{Movie Title} ({Release Year}) {MediaInfo Simple}"}),
        )
        .await
        .expect("naming");
    let id = place(&app, 101, &library, "unused.mkv").await;
    app.db.remove_file(id).await.expect("no file yet");
    app.rss_sync().await.expect("rss");
    s.finish(77, release, "SUCCESS/ALL", &format!("/remote/{release}"));
    assert_eq!(app.check_downloads().await.expect("check"), 1);
    // The name holds the file's codecs, not the release name's (x264 DTS).
    let target = library.join("Erster Film (2001) h265 EAC3.mkv");
    assert!(
        target.exists(),
        "{:?}",
        std::fs::read_dir(&library).map(|d| d.count())
    );
    let info = stored(&app, FileRef::Movie(id)).await.expect("stored");
    assert_eq!(info.labels().audio.as_deref(), Some("EAC3"));
    // Recorded with the library file's size and time: the next scan does not read it.
    let (movies, _) = app.rescan().await.expect("scan");
    assert_eq!(movies.expect("movies").media_read, 0);
    let _ = std::fs::remove_dir_all(&root);
}

// --- the file endpoints (AC-10) ---------------------------------------------------------

/// The fields of a file answer the Technical notes name, as (JSON pointer, value).
fn assert_fields(answer: &Value, expected: &[(&str, Value)]) {
    for (pointer, value) in expected {
        assert_eq!(
            answer.pointer(pointer),
            Some(value),
            "{pointer} in {answer:#}"
        );
    }
}

#[tokio::test]
async fn the_file_endpoints_answer_with_media_info() {
    let root = dir("api");
    let app = app(Library::Movies, "").await;
    let folder = root.join("Erster Film (2001)");
    std::fs::create_dir_all(&folder).expect("folder");
    let file = mkv_full(
        Some(("A_EAC3", 6, "ger")),
        Some(("S_TEXT/UTF8", "ger")),
        Some("x265 (build 199)"),
    );
    std::fs::write(folder.join("film.mkv"), file).expect("file");
    let id = place(&app, 101, &folder, "film.mkv").await;
    app.rescan().await.expect("scan");
    let without = app
        .db
        .movie_by_tmdb(104)
        .await
        .expect("db")
        .expect("movie")
        .id;
    let app = Arc::new(app);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let routes = crate::api::routes(Arc::clone(&app));
    tokio::spawn(async move { axum::serve(listener, routes).await });
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client");
    let answer: Value = client
        .get(format!("http://{addr}/api/movies/{id}/file"))
        .send()
        .await
        .expect("get")
        .json()
        .await
        .expect("json");
    let path = folder.join("film.mkv").to_string_lossy().into_owned();
    assert_fields(
        &answer,
        &[
            ("/path", json!(path)),
            ("/size", json!(1)),
            ("/quality", json!("Bluray-1080p")),
            ("/media_info/status", json!("complete")),
            ("/media_info/container", json!("matroska")),
            ("/media_info/video/codec", json!("h265")),
            ("/media_info/video/encoder", json!("x265")),
            ("/media_info/video/width", json!(1920)),
            ("/media_info/video/height", json!(1080)),
            (
                "/media_info/video/dynamic_range",
                json!({"hdr10": true, "hdr10_plus": false, "hlg": false, "dolby_vision": false}),
            ),
            ("/media_info/audio/0/codec", json!("eac3")),
            ("/media_info/audio/0/channels", json!(6)),
            ("/media_info/audio/0/object_audio", json!(false)),
            ("/media_info/audio/0/language", json!("ger")),
            ("/media_info/subtitles/0/language", json!("ger")),
            ("/media_info/subtitles/0/format", json!("srt")),
            ("/media_info/subtitles/0/forced", json!(true)),
            ("/media_info/subtitles/0/default", json!(false)),
            ("/media_info/subtitles/0/hearing_impaired", json!(true)),
            ("/media_info/labels/video", json!("x265")),
            ("/media_info/labels/audio", json!("EAC3")),
            ("/media_info/labels/channels", json!("5.1")),
            ("/media_info/labels/dynamic_range", json!("HDR10")),
        ],
    );
    let missing = client
        .get(format!("http://{addr}/api/movies/{without}/file"))
        .send()
        .await
        .expect("get");
    assert_eq!(missing.status(), reqwest::StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn the_episode_file_endpoint() {
    let root = dir("api-episode");
    let app = app(Library::Series, "").await;
    let mut show = app.db.all_series().await.expect("series")[0].clone();
    show.data.path = root.to_string_lossy().into_owned();
    app.db
        .upsert_series(&show.data, Timestamp::now())
        .await
        .expect("move");
    let episodes = app.db.episodes(Some(show.id)).await.expect("episodes");
    let with_file = episodes.iter().find(|e| e.file.is_some()).expect("E01");
    let relative = with_file.file.as_ref().expect("file").relative_path.clone();
    let path = root.join(&relative);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("season");
    let file = mkv_full(Some(("A_AAC", 2, "eng")), Some(("S_HDMV/PGS", "eng")), None);
    std::fs::write(&path, file).expect("file");
    app.rescan().await.expect("scan");
    let without = episodes.iter().find(|e| e.file.is_none()).expect("E02").id;
    let app = Arc::new(app);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let routes = crate::api::routes(Arc::clone(&app));
    tokio::spawn(async move { axum::serve(listener, routes).await });
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client");
    let url = |e: i64| format!("http://{addr}/api/series/{}/episodes/{e}/file", show.id);
    let answer: Value = client
        .get(url(with_file.id))
        .send()
        .await
        .expect("get")
        .json()
        .await
        .expect("json");
    assert_fields(
        &answer,
        &[
            ("/path", json!(path.to_string_lossy())),
            ("/size", json!(5)),
            ("/quality", json!("WEBDL-1080p")),
            ("/media_info/status", json!("complete")),
            ("/media_info/container", json!("matroska")),
            ("/media_info/video/codec", json!("h265")),
            ("/media_info/video/encoder", Value::Null),
            ("/media_info/video/width", json!(1920)),
            ("/media_info/video/height", json!(1080)),
            ("/media_info/video/dynamic_range/hdr10", json!(true)),
            ("/media_info/audio/0/codec", json!("aac")),
            ("/media_info/audio/0/channels", json!(2)),
            ("/media_info/audio/0/object_audio", json!(false)),
            ("/media_info/audio/0/language", json!("eng")),
            ("/media_info/subtitles/0/language", json!("eng")),
            ("/media_info/subtitles/0/format", json!("pgs")),
            ("/media_info/subtitles/0/forced", json!(true)),
            ("/media_info/subtitles/0/default", json!(false)),
            ("/media_info/subtitles/0/hearing_impaired", json!(true)),
            ("/media_info/labels/video", json!("h265")),
            ("/media_info/labels/audio", json!("AAC")),
            ("/media_info/labels/channels", json!("2.0")),
            ("/media_info/labels/dynamic_range", json!("HDR10")),
        ],
    );
    let missing = client.get(url(without)).send().await.expect("get");
    assert_eq!(missing.status(), reqwest::StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(&root);
}
