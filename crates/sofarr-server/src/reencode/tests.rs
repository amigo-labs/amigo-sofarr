use std::time::Instant;

use jiff::Timestamp;
use serde_json::{Value, json};
use sofarr_db::{MovieFile, NewEpisode, NewMovie, NewSeries};
use sofarr_mediainfo::{Container, DynamicRange, MediaInfo, ReadStatus, Video};

use super::*;
use crate::{app::App, config::Config, testkit::Http};

const GB: u64 = 1024 * MB;

fn now() -> Timestamp {
    "2026-11-17T08:00:00Z".parse().expect("timestamp")
}

fn media(width: u32, height: u32, minutes: Option<u64>) -> MediaInfo {
    MediaInfo {
        status: ReadStatus::Complete,
        container: Some(Container::Matroska),
        video: Some(Video {
            codec: "h264".to_owned(),
            encoder: None,
            width: Some(width),
            height: Some(height),
            dynamic_range: DynamicRange::default(),
        }),
        audio: Vec::new(),
        subtitles: Vec::new(),
        duration_ms: minutes.map(|m| m * 60_000),
    }
}

fn file(name: &str) -> MovieFile {
    MovieFile {
        relative_path: name.to_owned(),
        // The record's own size is not what the rule uses.
        size: 1,
        quality: "Bluray-1080p".to_owned(),
        revision_version: 1,
        revision_real: 0,
        release_group: None,
        scene_name: None,
        custom_format_score: 0,
    }
}

async fn store(db: &Db, file: FileRef, size: u64, info: Option<&MediaInfo>) {
    let Some(info) = info else { return };
    let stored = StoredMediaInfo {
        info: serde_json::to_string(info).expect("json"),
        size,
        modified: String::new(),
    };
    db.put_media_info(file, &stored, now())
        .await
        .expect("media info");
}

/// A movie in `/filme/<title>` with a file of `size` bytes and this stored media info.
async fn add_movie(
    db: &Db,
    tmdb_id: i64,
    title: &str,
    runtime: u32,
    size: u64,
    info: Option<&MediaInfo>,
) -> i64 {
    let movie = NewMovie {
        tmdb_id,
        imdb_id: None,
        title: title.to_owned(),
        original_title: None,
        title_de: None,
        year: Some(2019),
        runtime_minutes: runtime,
        original_language: None,
        monitored: true,
        quality_profile_id: 1,
        path: format!("/filme/{title}"),
        radarr_id: None,
        alternative_titles: vec![],
        availability: sofarr_db::Availability::default(),
        tags: vec![],
    };
    let id = db.upsert_movie(&movie, now()).await.expect("movie");
    db.set_file(id, &file(&format!("{title}.mkv")), now())
        .await
        .expect("file");
    store(db, FileRef::Movie(id), size, info).await;
    id
}

async fn add_series(db: &Db, tvdb_id: i64, title: &str, runtime: u32) -> i64 {
    let series = NewSeries {
        tvdb_id,
        imdb_id: None,
        title: title.to_owned(),
        title_de: None,
        year: Some(2020),
        runtime_minutes: runtime,
        original_language: None,
        series_type: "standard".to_owned(),
        monitored: true,
        season_folder: true,
        quality_profile_id: 1,
        path: format!("/serien/{title}"),
        sonarr_id: None,
        alternative_titles: vec![],
    };
    db.upsert_series(&series, now()).await.expect("series")
}

/// One file for these episodes of season 1; returns the episode ids.
async fn add_episode_file(
    db: &Db,
    series: i64,
    numbers: &[u16],
    size: u64,
    info: Option<&MediaInfo>,
) -> Vec<i64> {
    let mut ids = Vec::new();
    for number in numbers {
        let episode = NewEpisode {
            tvdb_id: None,
            season: 1,
            number: *number,
            absolute: None,
            title: None,
            title_de: None,
            air_date: None,
            monitored: true,
        };
        ids.push(db.upsert_episode(series, &episode).await.expect("episode"));
    }
    let name = format!("Season 01/S01E{:02}.mkv", numbers[0]);
    let file_id = db
        .set_episode_file(series, &ids, &file(&name), now())
        .await
        .expect("episode file");
    store(db, FileRef::Episode(file_id), size, info).await;
    ids
}

/// A library with one file per verdict and reason.
async fn library() -> Db {
    let db = Db::in_memory().await.expect("db");
    let hd = media(1920, 1080, Some(100));
    // Candidates: 100 and 70 MB per minute at a threshold of 60.
    add_movie(&db, 1, "Gross", 0, 10_000 * MB, Some(&hd)).await;
    add_movie(
        &db,
        2,
        "Breitbild",
        0,
        7000 * MB,
        Some(&media(1920, 800, Some(100))),
    )
    .await;
    // Fine: exactly at the threshold.
    add_movie(&db, 3, "Genau", 0, 6000 * MB, Some(&hd)).await;
    // A movie without a file is no library file.
    let wanted = add_movie(&db, 4, "Gesucht", 100, 0, None).await;
    db.remove_file(wanted).await.expect("remove");
    // Excluded, one per reason.
    add_movie(&db, 5, "Blind", 100, 0, None).await;
    add_movie(
        &db,
        6,
        "Zeitlos",
        0,
        9000 * MB,
        Some(&media(1920, 1080, None)),
    )
    .await;
    let mut dolby = media(3840, 2160, Some(100));
    if let Some(video) = &mut dolby.video {
        video.dynamic_range.dolby_vision = true;
        video.codec = "hevc".to_owned();
    }
    add_movie(&db, 7, "Dolby", 0, 40_000 * MB, Some(&dolby)).await;
    // Dolby Vision below the threshold is fine, not excluded.
    add_movie(&db, 8, "Dolby klein", 0, 10_000 * MB, Some(&dolby)).await;
    // A double episode without a header duration: 2 × 45 minutes by the metadata.
    let series = add_series(&db, 1, "Insel", 45).await;
    add_episode_file(
        &db,
        series,
        &[2, 3],
        9000 * MB,
        Some(&media(1920, 1080, None)),
    )
    .await;
    // A single episode whose header (20 min) wins over the metadata (45 min).
    add_episode_file(
        &db,
        series,
        &[4],
        1000 * MB,
        Some(&media(1280, 720, Some(20))),
    )
    .await;
    db
}

fn thresholds() -> Thresholds {
    Thresholds::default()
}

#[tokio::test]
async fn the_list_holds_the_too_large_files_largest_first() {
    // AC-06.
    let db = library().await;
    let list = candidates(&db, &thresholds()).await.expect("list");
    let titles: Vec<&str> = list.candidates.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Gross (2019)",
            "Insel – S01E02-E03",
            "Breitbild (2019)",
            "Insel – S01E04"
        ]
    );
    let first = &list.candidates[0];
    assert_eq!(first.kind, "movie");
    assert_eq!((first.movie_id.is_some(), first.series_id), (true, None));
    assert!(first.episode_ids.is_empty());
    assert_eq!(first.path, "/filme/Gross/Gross.mkv");
    assert_eq!(first.size, 10_000 * MB);
    assert_eq!(first.duration_seconds, 6000.0);
    assert_eq!(first.duration_source, "file");
    assert_eq!(first.resolution, "1080p");
    assert_eq!(first.video_codec, "h264");
    assert_eq!((first.mb_per_minute, first.threshold), (100.0, 60.0));

    assert_eq!((list.count, list.total_size), (4, 27_000 * MB));
    assert_eq!(list.fine, 2);
    assert_eq!(
        list.excluded,
        Excluded {
            no_media_info: 1,
            resolution_off: 0,
            unknown_duration: 1,
            dynamic_metadata: 1,
        }
    );
    // The summary adds up to the library files: 7 movie files (one movie has none) and 2 episode files.
    let excluded = list.excluded;
    let left_out = excluded.no_media_info
        + excluded.resolution_off
        + excluded.unknown_duration
        + excluded.dynamic_metadata;
    assert_eq!(list.count + list.fine + left_out, 9);
}

#[tokio::test]
async fn the_metadata_runtime_fills_in_for_a_missing_header_duration() {
    // AC-02, AC-05: files recorded before the feature have no stored duration.
    let db = library().await;
    let list = candidates(&db, &thresholds()).await.expect("list");
    let double = &list.candidates[1];
    assert_eq!(double.kind, "episode");
    assert_eq!(double.episode_ids.len(), 2);
    assert!(double.movie_id.is_none() && double.series_id.is_some());
    assert_eq!(double.path, "/serien/Insel/Season 01/S01E02.mkv");
    assert_eq!(double.duration_source, "metadata");
    assert_eq!(double.duration_seconds, 5400.0);
    assert_eq!(double.mb_per_minute, 100.0);
    let single = &list.candidates[3];
    assert_eq!(single.duration_source, "file");
    assert_eq!(single.duration_seconds, 1200.0);
    assert_eq!((single.resolution, single.threshold), ("720p", 30.0));

    // Stored JSON from before the feature has no `duration_ms` key at all.
    let old = r#"{"status":"complete","container":"matroska","video":{"codec":"h264",
        "encoder":null,"width":1920,"height":1080,"dynamic_range":{"hdr10":false,
        "hdr10_plus":false,"hlg":false,"dolby_vision":false}},"audio":[],"subtitles":[]}"#;
    let db = Db::in_memory().await.expect("db");
    let id = add_movie(&db, 1, "Alt", 100, 0, None).await;
    let stored = StoredMediaInfo {
        info: old.to_owned(),
        size: 10_000 * MB,
        modified: String::new(),
    };
    db.put_media_info(FileRef::Movie(id), &stored, now())
        .await
        .expect("media info");
    let list = candidates(&db, &thresholds()).await.expect("list");
    assert_eq!(list.candidates[0].duration_source, "metadata");
    assert_eq!(list.candidates[0].mb_per_minute, 100.0);
    // Once the file is read again, the header duration is stored and used.
    store(
        &db,
        FileRef::Movie(id),
        10_000 * MB,
        Some(&media(1920, 1080, Some(50))),
    )
    .await;
    let list = candidates(&db, &thresholds()).await.expect("list");
    assert_eq!(list.candidates[0].duration_source, "file");
    assert_eq!(list.candidates[0].mb_per_minute, 200.0);
}

#[tokio::test]
async fn a_switched_off_resolution_and_broken_media_info_are_counted() {
    // AC-04, AC-12.
    let db = library().await;
    let id = add_movie(&db, 20, "Kaputt", 100, 0, None).await;
    let broken = StoredMediaInfo {
        info: "{not json".to_owned(),
        size: 50_000 * MB,
        modified: String::new(),
    };
    db.put_media_info(FileRef::Movie(id), &broken, now())
        .await
        .expect("media info");
    let off = Thresholds {
        p2160: 0.0,
        ..thresholds()
    };
    let list = candidates(&db, &off).await.expect("list");
    assert_eq!(
        list.excluded,
        Excluded {
            no_media_info: 2,
            resolution_off: 2,
            unknown_duration: 1,
            dynamic_metadata: 0,
        }
    );
    assert_eq!((list.count, list.fine), (4, 1));
}

#[test]
fn the_cli_report_lists_sizes_and_reasons_in_german() {
    // AC-07.
    let candidate = |title: &str, size: u64, per_minute: f64| Candidate {
        kind: "movie",
        movie_id: Some(1),
        series_id: None,
        episode_ids: Vec::new(),
        title: title.to_owned(),
        path: format!("/filme/{title}.mkv"),
        size,
        duration_seconds: 6000.0,
        duration_source: "file",
        resolution: "1080p",
        video_codec: "h264".to_owned(),
        mb_per_minute: per_minute,
        threshold: 60.0,
    };
    let list = Candidates {
        candidates: vec![
            candidate("Gross", 40 * GB + GB / 2, 414.72),
            candidate("Klein", 7 * GB, 71.68),
        ],
        count: 2,
        total_size: 47 * GB + GB / 2,
        fine: 5,
        excluded: Excluded {
            no_media_info: 1,
            resolution_off: 0,
            unknown_duration: 3,
            dynamic_metadata: 0,
        },
    };
    assert_eq!(
        report(&list),
        "40,5 GB  414,7 > 60,0 MB/min  /filme/Gross.mkv\n\
         7,0 GB  71,7 > 60,0 MB/min  /filme/Klein.mkv\n\
         2 Dateien, 47,5 GB\n\
         1 × keine Media-Infos\n\
         3 × Laufzeit unbekannt\n"
    );
    let none = Candidates {
        candidates: Vec::new(),
        count: 0,
        total_size: 0,
        ..list
    };
    assert_eq!(
        report(&none),
        "Keine Datei ist zu groß.\n1 × keine Media-Infos\n3 × Laufzeit unbekannt\n"
    );
    assert_eq!(report(&Candidates::default()), "Keine Datei ist zu groß.\n");
    assert_eq!(reason(Exclusion::ResolutionOff), "Auflösung ausgeschaltet");
    assert_eq!(
        reason(Exclusion::DynamicMetadata),
        "Dolby Vision/HDR10+ würde verloren gehen"
    );
}

async fn answer(mode: &str) -> (Value, Db) {
    let db = library().await;
    let config = Config::parse(&format!(
        "[automation]\nmode = \"{mode}\"\n[reencode]\nmax_mb_per_minute_1080p = 65\n"
    ))
    .expect("config");
    let http = Http::start(App::with_db(config, db.clone()).expect("app")).await;
    let (status, body) = http.get("/api/reencode/candidates").await;
    assert_eq!(status, 200);
    (body, db)
}

/// What the requests must leave alone.
async fn state(db: &Db) -> (usize, usize, usize) {
    (
        db.movies().await.expect("movies").len(),
        db.episodes(None).await.expect("episodes").len(),
        db.all_media_info().await.expect("media info").len(),
    )
}

#[tokio::test]
async fn the_api_answers_the_same_in_shadow_and_live_mode_and_changes_nothing() {
    // AC-06, AC-08. The library paths do not exist: the list never opens a file.
    let before = state(&library().await).await;
    let (shadow, shadow_db) = answer("shadow").await;
    let (live, live_db) = answer("live").await;
    assert_eq!(shadow, live);
    assert_eq!(state(&shadow_db).await, before);
    assert_eq!(state(&live_db).await, before);
    // The configured threshold (65) applies: 70 and 100 MB per minute stay candidates.
    assert_eq!(shadow["count"], 4);
    assert_eq!(shadow["fine"], 2);
    assert_eq!(shadow["total_size"], json!(27_000 * MB));
    assert_eq!(
        shadow["excluded"],
        json!({
            "no_media_info": 1,
            "resolution_off": 0,
            "unknown_duration": 1,
            "dynamic_metadata": 1,
        })
    );
    assert_eq!(
        shadow["candidates"][1],
        json!({
            "kind": "episode",
            "movie_id": null,
            "series_id": shadow["candidates"][3]["series_id"],
            "episode_ids": shadow["candidates"][1]["episode_ids"],
            "title": "Insel – S01E02-E03",
            "path": "/serien/Insel/Season 01/S01E02.mkv",
            "size": 9000 * MB,
            "duration_seconds": 5400.0,
            "duration_source": "metadata",
            "resolution": "1080p",
            "video_codec": "h264",
            "mb_per_minute": 100.0,
            "threshold": 65.0,
        })
    );
    assert_eq!(
        shadow["candidates"][1]["episode_ids"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
}

#[tokio::test]
async fn reencode_candidates_large_library() {
    // AC-11: 2,500 movies and 20,000 episode files, half of them candidates.
    let db = Db::in_memory().await.expect("db");
    let hd = media(1920, 1080, Some(45));
    let size = |i: u64| if i.is_multiple_of(2) { 5000 * MB } else { 1000 * MB } + i;
    for i in 0..2500u64 {
        add_movie(
            &db,
            i as i64 + 1,
            &format!("Film {i}"),
            100,
            size(i),
            Some(&hd),
        )
        .await;
    }
    // The episode files are written in bulk: 200 per series, ids `series * 1000 + number`.
    let info = serde_json::to_string(&hd).expect("json");
    for show in 0..100i64 {
        let series = add_series(&db, show + 1, &format!("Serie {show}"), 45).await;
        let numbers =
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 200)";
        let statements = [
            format!(
                "{numbers} INSERT INTO episode_files (id, series_id, relative_path, size, quality, added_at)
                 SELECT ?1 * 1000 + i, ?1, 'Season 01/E' || i || '.mkv', 1, 'WEBDL-1080p', '' FROM n"
            ),
            format!(
                "{numbers} INSERT INTO episodes (series_id, season, number, monitored, file_id)
                 SELECT ?1, 1, i, 1, ?1 * 1000 + i FROM n"
            ),
            "INSERT INTO media_info (episode_file_id, info, size, modified, read_at)
             SELECT id, ?2, (CASE WHEN id % 2 = 0 THEN 5000 ELSE 1000 END) * 1048576 + id, '', ''
             FROM episode_files WHERE series_id = ?1"
                .to_owned(),
        ];
        for statement in &statements {
            sqlx::query(statement)
                .bind(series)
                .bind(&info)
                .execute(db.pool())
                .await
                .expect("bulk insert");
        }
    }
    let started = Instant::now();
    let list = candidates(&db, &thresholds()).await.expect("list");
    let took = started.elapsed();
    println!("reencode candidates for 22,500 files: {took:?}");
    assert_eq!((list.count, list.fine), (11_250, 11_250));
    assert!(
        list.candidates
            .windows(2)
            .all(|pair| pair[0].size >= pair[1].size)
    );
    // The 2 s of AC-11 hold for a release build; a debug build gets more room.
    let limit = if cfg!(debug_assertions) { 20 } else { 2 };
    assert!(took.as_secs() < limit, "took {took:?}");
}
