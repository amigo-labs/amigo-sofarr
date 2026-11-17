//! A re-encode of one file against a fake encoder (spec reencode-file). Library files are
//! fakes too: a line of JSON with their media info, padded to their size.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use jiff::Timestamp;
use serde_json::json;
use sofarr_db::{
    Db, FileRef, MovieFile, NewEpisode, NewMovie, NewOwnSubtitle, NewSeries, ReencodeOutcome,
    SubtitleKind,
};
use sofarr_mediainfo::{Audio, Container, DynamicRange, MediaInfo, ReadStatus, Subtitle, Video};

use super::*;
use crate::{config::Config, reencode::ffmpeg::Job};

const SIZE: u64 = 10_000;

pub(crate) fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sofarr-reencode-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// Media info of a fake video.
pub(crate) fn info(container: Container, codec: &str) -> MediaInfo {
    let audio = |language: &str| Audio {
        codec: "eac3".to_owned(),
        channels: Some(6),
        object_audio: false,
        language: language.to_owned(),
    };
    MediaInfo {
        status: ReadStatus::Complete,
        container: Some(container),
        video: Some(Video {
            codec: codec.to_owned(),
            encoder: None,
            width: Some(1920),
            height: Some(1080),
            dynamic_range: DynamicRange::default(),
        }),
        audio: vec![audio("ger"), audio("eng")],
        subtitles: vec![Subtitle {
            language: "ger".to_owned(),
            format: "srt".to_owned(),
            forced: true,
            default: false,
            hearing_impaired: false,
        }],
        duration_ms: Some(6_000_000),
    }
}

/// Writes a fake video: its media info as JSON, padded to `size` bytes.
pub(crate) fn write_fake(path: &Path, info: &MediaInfo, size: u64) {
    let mut bytes = serde_json::to_vec(info).expect("json");
    bytes.push(b'\n');
    let size = usize::try_from(size).expect("size");
    assert!(
        bytes.len() <= size,
        "fake video too small for its media info"
    );
    bytes.resize(size, b' ');
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("folder");
    }
    fs::write(path, bytes).expect("fake video");
}

/// Reads a fake video like the real reader reads a real one.
pub(crate) fn read_fake(path: &Path) -> Option<FileMedia> {
    let meta = fs::metadata(path).ok()?;
    let bytes = fs::read(path).ok()?;
    let first = bytes.split(|b| *b == b'\n').next().unwrap_or_default();
    let info = serde_json::from_slice(first).unwrap_or_else(|_| sofarr_mediainfo::unreadable());
    Some(FileMedia {
        info,
        size: meta.len(),
        modified: media::modified(&meta),
    })
}

pub(crate) type Behavior = Box<dyn Fn(&Job) -> Result<(), EncodeError> + Send + Sync>;

/// An encoder that does what its test says and remembers its jobs.
pub(crate) struct FakeEncoder {
    check: Result<(), String>,
    behavior: Behavior,
    pub(crate) jobs: Mutex<Vec<Job>>,
}

impl Encoder for FakeEncoder {
    fn check(&self) -> Result<(), String> {
        self.check.clone()
    }

    fn encode(&self, job: &Job) -> Result<(), EncodeError> {
        self.jobs.lock().expect("jobs").push(job.clone());
        (self.behavior)(job)
    }
}

/// An encoder that writes a good result of `size` bytes.
pub(crate) fn writes(result: MediaInfo, size: u64) -> Behavior {
    Box::new(move |job: &Job| {
        write_fake(&job.output, &result, size);
        Ok(())
    })
}

pub(crate) fn hevc() -> MediaInfo {
    info(Container::Matroska, "h265")
}

pub(crate) struct Env {
    pub(crate) root: PathBuf,
    pub(crate) app: Arc<App>,
    pub(crate) bin: PathBuf,
}

impl Env {
    pub(crate) async fn new(name: &str, mode: &str) -> Self {
        Self::with(name, mode, "").await
    }

    /// With more TOML after the `[automation]` section.
    pub(crate) async fn with(name: &str, mode: &str, extra: &str) -> Self {
        let root = temp(name);
        let config = Config::parse(&format!(
            "[database]\npath = '{}'\n[automation]\nmode = \"{mode}\"\n{extra}",
            root.join("config").join("sofarr.db").display()
        ))
        .expect("config");
        fs::create_dir_all(root.join("config")).expect("config folder");
        // A database file: one test changes it from another thread's runtime.
        let db = Db::open(&root.join("config").join("sofarr.db"))
            .await
            .expect("db");
        let bin = root.join("papierkorb");
        for key in [
            "config_mediamanagement".to_owned(),
            format!("{}config_mediamanagement", crate::series::PREFIX),
        ] {
            db.put_setting(&key, &json!({ "recycleBin": bin.to_string_lossy() }))
                .await
                .expect("settings");
        }
        let app = Arc::new(App::with_db(config, db).expect("app"));
        Self { root, app, bin }
    }

    pub(crate) fn db(&self) -> &Db {
        &self.app.db
    }

    pub(crate) fn tools(
        &self,
        check: Result<(), String>,
        behavior: Behavior,
    ) -> (Tools, Arc<FakeEncoder>) {
        let encoder = Arc::new(FakeEncoder {
            check,
            behavior,
            jobs: Mutex::new(Vec::new()),
        });
        let tools = Tools {
            encoder: Arc::clone(&encoder) as Arc<dyn Encoder>,
            read: Arc::new(read_fake),
        };
        (tools, encoder)
    }

    /// A movie in `<root>/filme/<title>` with a fake video `file_name`.
    pub(crate) async fn movie(
        &self,
        title: &str,
        file_name: &str,
        info: &MediaInfo,
    ) -> (i64, PathBuf) {
        let folder = self.root.join("filme").join(title);
        let id = self
            .db()
            .upsert_movie(
                &NewMovie {
                    tmdb_id: 1,
                    imdb_id: None,
                    title: title.to_owned(),
                    original_title: None,
                    title_de: None,
                    year: Some(2019),
                    runtime_minutes: 100,
                    original_language: None,
                    monitored: true,
                    quality_profile_id: 1,
                    path: folder.to_string_lossy().into_owned(),
                    radarr_id: None,
                    alternative_titles: vec![],
                    availability: sofarr_db::Availability::default(),
                    tags: vec![],
                },
                Timestamp::now(),
            )
            .await
            .expect("movie");
        self.db()
            .set_file(id, &library_file(file_name), Timestamp::now())
            .await
            .expect("file");
        let video = folder.join(file_name);
        write_fake(&video, info, SIZE);
        (id, video)
    }

    /// A movie whose fake video has `size` bytes, with its media info stored like the
    /// disk scan does.
    pub(crate) async fn movie_sized(
        &self,
        tmdb_id: i64,
        title: &str,
        info: &MediaInfo,
        size: u64,
    ) -> (i64, PathBuf) {
        let folder = self.root.join("filme").join(title);
        let file_name = format!("{title}.mkv");
        let id = self
            .db()
            .upsert_movie(
                &NewMovie {
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
                    path: folder.to_string_lossy().into_owned(),
                    radarr_id: None,
                    alternative_titles: vec![],
                    availability: sofarr_db::Availability::default(),
                    tags: vec![],
                },
                Timestamp::now(),
            )
            .await
            .expect("movie");
        self.db()
            .set_file(id, &library_file(&file_name), Timestamp::now())
            .await
            .expect("file");
        let video = folder.join(&file_name);
        write_fake(&video, info, size);
        let read = read_fake(&video).expect("read");
        media::store(self.db(), FileRef::Movie(id), &read)
            .await
            .expect("media info");
        (id, video)
    }

    pub(crate) async fn attempt(&self, tools: &Tools, target: Target) -> Result<Report, Refusal> {
        attempt(&self.app, tools, target, Runner::Cli)
            .await
            .expect("attempt")
    }

    /// Every file below the root, relative and sorted (the database files left out).
    pub(crate) fn files(&self) -> Vec<String> {
        fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
            for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, root, out);
                } else if let Ok(relative) = path.strip_prefix(root) {
                    out.push(relative.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &self.root, &mut out);
        out.retain(|path| !path.starts_with("config/"));
        out.sort();
        out
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub(crate) fn library_file(name: &str) -> MovieFile {
    MovieFile {
        relative_path: name.to_owned(),
        size: SIZE,
        quality: "Remux-1080p".to_owned(),
        revision_version: 1,
        revision_real: 0,
        release_group: Some("GRP".to_owned()),
        scene_name: Some("Film.2019.1080p.Remux-GRP".to_owned()),
        custom_format_score: 120,
    }
}

pub(crate) async fn last(env: &Env) -> sofarr_db::ReencodeAttempt {
    env.db()
        .last_reencode()
        .await
        .expect("read")
        .expect("an attempt")
}

#[tokio::test]
async fn a_good_result_replaces_the_original_which_goes_to_the_recycle_bin() {
    // AC-06, AC-13, AC-15: a 10,000-byte file is not a candidate by any threshold, and is
    // encoded anyway.
    let env = Env::new("replace", "live").await;
    let (id, video) = env
        .movie("Film", "Film.mkv", &info(Container::Matroska, "h264"))
        .await;
    let (tools, encoder) = env.tools(Ok(()), writes(hevc(), 4000));
    let report = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect("started");
    assert_eq!(report.outcome, ReencodeOutcome::Replaced);
    assert_eq!(report.reason, None);
    assert_eq!((report.size_before, report.size_after), (SIZE, Some(4000)));

    // The result took the original's place; the original is in the recycle bin.
    assert_eq!(fs::metadata(&video).expect("video").len(), 4000);
    assert_eq!(read_fake(&video).expect("read").info, hevc());
    assert_eq!(
        env.files(),
        ["filme/Film/Film.mkv", "papierkorb/Film/Film.mkv"]
    );
    assert_eq!(
        read_fake(&env.bin.join("Film/Film.mkv"))
            .expect("in the bin")
            .info,
        info(Container::Matroska, "h264")
    );

    // The library records the new size; everything else stays.
    let movie = env.db().movie(id).await.expect("read").expect("movie");
    assert_eq!(
        movie.file,
        Some(MovieFile {
            size: 4000,
            ..library_file("Film.mkv")
        })
    );
    let stored = env
        .db()
        .media_info(FileRef::Movie(id))
        .await
        .expect("read")
        .expect("media info stored");
    assert_eq!(media::parse(&stored), Some(hevc()));
    assert_eq!(stored.size, 4000);

    let attempt = last(&env).await;
    assert_eq!(attempt.outcome, Some(ReencodeOutcome::Replaced));
    assert_eq!(attempt.runner, "cli");
    assert_eq!(attempt.title, "Film (2019)");
    assert_eq!(
        (attempt.size_before, attempt.size_after),
        (SIZE, Some(4000))
    );
    assert!(attempt.finished_at.is_some());

    // The encoder wrote a hidden work file next to the original, within the size limit.
    let jobs = encoder.jobs.lock().expect("jobs");
    assert_eq!(jobs[0].input, video);
    assert_eq!(
        jobs[0].output,
        video.with_file_name(".Film.mkv.reencode-partial")
    );
    assert!(!jobs[0].mp4);
    assert_eq!(jobs[0].limits.max_bytes, 8000);
    assert_eq!(jobs[0].limits.timeout, Duration::from_secs(24 * 3600));
    // The slot is free again.
    assert!(!env.app.reencode.lock.exists());
}

#[tokio::test]
async fn without_a_recycle_bin_the_original_is_deleted() {
    let env = Env::new("delete", "live").await;
    env.db()
        .put_setting("config_mediamanagement", &json!({}))
        .await
        .expect("settings");
    let (id, _) = env
        .movie("Film", "Film.mkv", &info(Container::Matroska, "h264"))
        .await;
    let (tools, _) = env.tools(Ok(()), writes(hevc(), 4000));
    let report = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect("started");
    assert_eq!(report.outcome, ReencodeOutcome::Replaced);
    assert_eq!(env.files(), ["filme/Film/Film.mkv"]);
}

#[tokio::test]
async fn an_mp4_original_becomes_mkv_and_its_subtitles_stay() {
    // AC-03 (MP4 handling), AC-14.
    let env = Env::new("mp4", "live").await;
    let (id, video) = env
        .movie("Film", "Film.mp4", &info(Container::Mp4, "h264"))
        .await;
    let subtitle = video.with_file_name("Film.de.forced.srt");
    fs::write(&subtitle, "1\n").expect("subtitle");
    env.db()
        .add_own_subtitle(
            &NewOwnSubtitle {
                file: FileRef::Movie(id),
                language: "de".to_owned(),
                kind: SubtitleKind::Forced,
                hearing_impaired: false,
                file_name: "Film.de.forced.srt".to_owned(),
                provider_id: "1".to_owned(),
                release: "Film".to_owned(),
                match_quality: "hash".to_owned(),
            },
            Timestamp::now(),
        )
        .await
        .expect("own subtitle");
    let (tools, encoder) = env.tools(Ok(()), writes(hevc(), 4000));
    let report = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect("started");
    assert_eq!(report.outcome, ReencodeOutcome::Replaced);
    assert!(encoder.jobs.lock().expect("jobs")[0].mp4);
    assert_eq!(
        env.files(),
        [
            "filme/Film/Film.de.forced.srt",
            "filme/Film/Film.mkv",
            "papierkorb/Film/Film.mp4"
        ]
    );
    let movie = env.db().movie(id).await.expect("read").expect("movie");
    assert_eq!(
        movie.file.map(|f| f.relative_path).as_deref(),
        Some("Film.mkv")
    );
    let own = env
        .db()
        .own_subtitles(Some(FileRef::Movie(id)))
        .await
        .expect("own");
    assert_eq!(own.len(), 1);
    assert_eq!(own[0].file_name, "Film.de.forced.srt");
}

#[tokio::test]
async fn odd_file_names_are_encoded_and_replaced_as_they_are() {
    // AC-04.
    let env = Env::new("odd", "live").await;
    let name = "-x; rm $(echo \"a b\") 'c'.mkv";
    let (id, video) = env
        .movie("Film", name, &info(Container::Matroska, "h264"))
        .await;
    let (tools, encoder) = env.tools(Ok(()), writes(hevc(), 4000));
    let report = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect("started");
    assert_eq!(report.outcome, ReencodeOutcome::Replaced);
    assert_eq!(encoder.jobs.lock().expect("jobs")[0].input, video);
    assert_eq!(fs::metadata(&video).expect("video").len(), 4000);
    assert_eq!(
        env.files(),
        [
            format!("filme/Film/{name}"),
            format!("papierkorb/Film/{name}")
        ]
    );
}

#[tokio::test]
async fn a_double_episode_keeps_both_episodes() {
    let env = Env::new("episode", "live").await;
    let folder = env.root.join("serien").join("Insel");
    let series = env
        .db()
        .upsert_series(
            &NewSeries {
                tvdb_id: 1,
                imdb_id: None,
                title: "Insel".to_owned(),
                title_de: None,
                year: None,
                runtime_minutes: 45,
                original_language: None,
                series_type: "standard".to_owned(),
                monitored: true,
                season_folder: true,
                quality_profile_id: 1,
                path: folder.to_string_lossy().into_owned(),
                sonarr_id: None,
                alternative_titles: vec![],
            },
            Timestamp::now(),
        )
        .await
        .expect("series");
    let mut episodes = Vec::new();
    for number in [2, 3] {
        let episode = NewEpisode {
            tvdb_id: None,
            season: 1,
            number,
            absolute: None,
            title: None,
            title_de: None,
            air_date: None,
            monitored: true,
        };
        episodes.push(
            env.db()
                .upsert_episode(series, &episode)
                .await
                .expect("episode"),
        );
    }
    let file_id = env
        .db()
        .set_episode_file(
            series,
            &episodes,
            &library_file("Season 01/S01E02.mp4"),
            Timestamp::now(),
        )
        .await
        .expect("file");
    write_fake(
        &folder.join("Season 01/S01E02.mp4"),
        &info(Container::Mp4, "h264"),
        SIZE,
    );
    let (tools, _) = env.tools(Ok(()), writes(hevc(), 4000));
    let report = env
        .attempt(&tools, Target::Episode(episodes[1]))
        .await
        .expect("started");
    assert_eq!(report.outcome, ReencodeOutcome::Replaced);
    assert_eq!(report.title, "Insel – S01E02-E03");
    for episode in env.db().episodes(None).await.expect("episodes") {
        assert_eq!(episode.file_id, Some(file_id));
        assert_eq!(
            episode.file.map(|f| f.relative_path).as_deref(),
            Some("Season 01/S01E02.mkv")
        );
    }
    assert_eq!(
        env.files(),
        [
            "papierkorb/Insel/Season 01/S01E02.mp4",
            "serien/Insel/Season 01/S01E02.mkv"
        ]
    );
}

#[tokio::test]
async fn a_result_above_the_size_limit_is_no_gain() {
    // AC-07: stopped while being written, and finished but too large; exactly at the
    // limit is good.
    let env = Env::new("no-gain", "live").await;
    let original = info(Container::Matroska, "h264");
    let (id, video) = env.movie("Film", "Film.mkv", &original).await;

    let (tools, _) = env.tools(
        Ok(()),
        Box::new(|job: &Job| {
            write_fake(&job.output, &hevc(), 9000);
            Err(EncodeError::TooLarge)
        }),
    );
    let report = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect("started");
    assert_eq!(report.outcome, ReencodeOutcome::NoGain);
    assert_eq!(report.size_after, None);
    assert_eq!(env.files(), ["filme/Film/Film.mkv"]);
    assert_eq!(read_fake(&video).expect("read").info, original);

    let (tools, _) = env.tools(Ok(()), writes(hevc(), 8001));
    let report = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect("started");
    assert_eq!(report.outcome, ReencodeOutcome::NoGain);
    assert_eq!(report.size_after, Some(8001));
    assert_eq!(env.files(), ["filme/Film/Film.mkv"]);
    let attempt = last(&env).await;
    assert_eq!(attempt.outcome, Some(ReencodeOutcome::NoGain));
    assert_eq!(
        (attempt.size_before, attempt.size_after),
        (SIZE, Some(8001))
    );

    let (tools, _) = env.tools(Ok(()), writes(hevc(), 8000));
    let report = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect("started");
    assert_eq!(report.outcome, ReencodeOutcome::Replaced);
}

#[tokio::test]
async fn a_failed_or_bad_result_keeps_the_original() {
    // AC-08.
    let env = Env::new("failed", "live").await;
    let mut original = info(Container::Matroska, "h264");
    if let Some(video) = &mut original.video {
        video.dynamic_range.hdr10 = true;
    }
    let (id, video) = env.movie("Film", "Film.mkv", &original).await;
    let mut hdr = hevc();
    if let Some(video) = &mut hdr.video {
        video.dynamic_range.hdr10 = true;
    }
    let mut cases: Vec<(MediaInfo, &str)> = Vec::new();
    let mut avc = hdr.clone();
    if let Some(video) = &mut avc.video {
        video.codec = "h264".to_owned();
    }
    cases.push((avc, "Video ist nicht HEVC"));
    let mut small = hdr.clone();
    if let Some(video) = &mut small.video {
        video.height = Some(720);
    }
    cases.push((small, "andere Auflösung"));
    let mut quiet = hdr.clone();
    quiet.audio.pop();
    cases.push((quiet, "andere Zahl an Tonspuren"));
    let mut bare = hdr.clone();
    bare.subtitles.clear();
    cases.push((bare, "andere Zahl an Untertitelspuren"));
    let mut short = hdr.clone();
    short.duration_ms = Some(6_001_001);
    cases.push((short, "andere Laufzeit"));
    let mut timeless = hdr.clone();
    timeless.duration_ms = None;
    cases.push((timeless, "keine Laufzeit"));
    cases.push((hevc(), "HDR ging verloren"));
    let mut partial = hdr.clone();
    partial.status = ReadStatus::Partial;
    cases.push((partial, "nicht vollständig lesbar"));
    for (result, reason) in cases {
        let (tools, _) = env.tools(Ok(()), writes(result, 4000));
        let report = env
            .attempt(&tools, Target::Movie(id))
            .await
            .expect("started");
        assert_eq!(report.outcome, ReencodeOutcome::Failed, "{reason}");
        assert_eq!(
            report.reason.as_deref(),
            Some(format!("Ergebnis unbrauchbar: {reason}").as_str())
        );
        assert_eq!(env.files(), ["filme/Film/Film.mkv"], "{reason}");
        assert_eq!(read_fake(&video).expect("read").info, original);
    }
    // A duration within one second is good.
    let mut close = hdr.clone();
    close.duration_ms = Some(6_001_000);
    assert_eq!(check_result(&original, &close), Ok(()));

    let (tools, _) = env.tools(
        Ok(()),
        Box::new(|job: &Job| {
            write_fake(&job.output, &hevc(), 1000);
            Err(EncodeError::Failed("ffmpeg: Conversion failed!".to_owned()))
        }),
    );
    let report = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect("started");
    assert_eq!(report.outcome, ReencodeOutcome::Failed);
    assert_eq!(report.reason.as_deref(), Some("ffmpeg: Conversion failed!"));
    assert_eq!(env.files(), ["filme/Film/Film.mkv"]);
    let recorded = last(&env).await;
    assert_eq!(recorded.outcome, Some(ReencodeOutcome::Failed));
    assert_eq!(
        recorded.reason.as_deref(),
        Some("ffmpeg: Conversion failed!")
    );

    // A configured time limit reaches the encoder.
    let jobs_env = Env::new("timeout", "live").await;
    let config = Config::parse(&format!(
        "[database]\npath = '{}'\n[automation]\nmode = \"live\"\n[reencode]\ntimeout_hours = 3\nmin_saving_percent = 50\n",
        jobs_env.root.join("config").join("sofarr.db").display()
    ))
    .expect("config");
    let app = App::with_db(config, jobs_env.db().clone()).expect("app");
    let (id, _) = jobs_env
        .movie("Film", "Film.mkv", &info(Container::Matroska, "h264"))
        .await;
    let (tools, encoder) = jobs_env.tools(Ok(()), writes(hevc(), 4000));
    attempt(&app, &tools, Target::Movie(id), Runner::Cli)
        .await
        .expect("attempt")
        .expect("started");
    let jobs = encoder.jobs.lock().expect("jobs");
    assert_eq!(jobs[0].limits.timeout, Duration::from_secs(3 * 3600));
    assert_eq!(jobs[0].limits.max_bytes, 5000);
}

#[tokio::test]
async fn a_change_during_the_encode_discards_the_result() {
    // AC-09.
    let env = Env::new("changed", "live").await;
    let original = info(Container::Matroska, "h264");

    // The original was replaced (an upgrade was imported).
    let (id, video) = env.movie("Film", "Film.mkv", &original).await;
    let upgraded = video.clone();
    let (tools, _) = env.tools(
        Ok(()),
        Box::new(move |job: &Job| {
            write_fake(&upgraded, &info(Container::Matroska, "h264"), SIZE + 5);
            write_fake(&job.output, &hevc(), 4000);
            Ok(())
        }),
    );
    let report = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect("started");
    assert_eq!(report.outcome, ReencodeOutcome::Failed);
    assert_eq!(
        report.reason.as_deref(),
        Some("Original hat sich während des Encodes geändert")
    );
    assert_eq!(env.files(), ["filme/Film/Film.mkv"]);
    assert_eq!(fs::metadata(&video).expect("video").len(), SIZE + 5);

    // The original is gone.
    write_fake(&video, &original, SIZE);
    let gone = video.clone();
    let (tools, _) = env.tools(
        Ok(()),
        Box::new(move |job: &Job| {
            fs::remove_file(&gone).expect("remove");
            write_fake(&job.output, &hevc(), 4000);
            Ok(())
        }),
    );
    let report = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect("started");
    assert_eq!(report.outcome, ReencodeOutcome::Failed);
    assert!(env.files().is_empty());

    // The library moved the file (a rename).
    write_fake(&video, &original, SIZE);
    let db = env.db().clone();
    let (tools, _) = env.tools(
        Ok(()),
        Box::new(move |job: &Job| {
            let db = db.clone();
            std::thread::spawn(move || {
                tokio::runtime::Runtime::new()
                    .expect("runtime")
                    .block_on(db.set_movie_file_path(id, "Film (2019).mkv"))
            })
            .join()
            .expect("thread")
            .expect("rename");
            write_fake(&job.output, &hevc(), 4000);
            Ok(())
        }),
    );
    let report = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect("started");
    assert_eq!(
        report.reason.as_deref(),
        Some("Die Bibliothek führt das Original nicht mehr unter diesem Pfad")
    );
    assert_eq!(env.files(), ["filme/Film/Film.mkv"]);

    // The final name of an MP4 original is taken.
    let (mp4, mp4_video) = {
        let env_mp4 = &env;
        env_mp4.db().delete_movie(id).await.expect("remove");
        env_mp4
            .movie("Clip", "Clip.mp4", &info(Container::Mp4, "h264"))
            .await
    };
    let own = mp4_video.with_file_name("Clip.mkv");
    fs::write(&own, "the owner's own copy").expect("own copy");
    let (tools, _) = env.tools(Ok(()), writes(hevc(), 4000));
    let report = env
        .attempt(&tools, Target::Movie(mp4))
        .await
        .expect("started");
    assert_eq!(report.outcome, ReencodeOutcome::Failed);
    assert!(
        report
            .reason
            .as_deref()
            .is_some_and(|r| r.starts_with("Zieldatei existiert bereits"))
    );
    assert_eq!(
        fs::read_to_string(&own).expect("own copy"),
        "the owner's own copy"
    );
    assert_eq!(
        env.files(),
        [
            "filme/Clip/Clip.mkv",
            "filme/Clip/Clip.mp4",
            "filme/Film/Film.mkv"
        ]
    );
}

#[tokio::test]
async fn a_failed_swap_puts_the_original_back() {
    // AC-11: the library update fails.
    let env = Env::new("swap", "live").await;
    let original = info(Container::Mp4, "h264");
    let (id, video) = env.movie("Film", "Film.mp4", &original).await;
    sqlx::query(
        "CREATE TRIGGER broken BEFORE UPDATE ON movie_files BEGIN SELECT RAISE(ABORT, 'kaputt'); END",
    )
    .execute(env.db().pool())
    .await
    .expect("trigger");
    let (tools, _) = env.tools(Ok(()), writes(hevc(), 4000));
    let report = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect("started");
    assert_eq!(report.outcome, ReencodeOutcome::Failed);
    assert!(
        report
            .reason
            .as_deref()
            .is_some_and(|r| r.starts_with("Bibliothek nicht aktualisierbar"))
    );
    assert_eq!(env.files(), ["filme/Film/Film.mp4"]);
    assert_eq!(read_fake(&video).expect("read").info, original);
    let movie = env.db().movie(id).await.expect("read").expect("movie");
    assert_eq!(movie.file, Some(library_file("Film.mp4")));
    sqlx::query("DROP TRIGGER broken")
        .execute(env.db().pool())
        .await
        .expect("drop trigger");

    // Only the recycle bin fails (it is a file): the result stays, the original waits
    // under its hidden name.
    fs::create_dir_all(&env.root).expect("root");
    fs::write(&env.bin, "no folder").expect("bin as a file");
    let report = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect("started");
    assert_eq!(report.outcome, ReencodeOutcome::Replaced);
    assert!(
        report.reason.as_deref().is_some_and(
            |r| r.starts_with("Original liegt noch als .Film.mp4.reencode-original im Ordner")
        ),
        "{:?}",
        report.reason
    );
    assert_eq!(
        env.files(),
        [
            "filme/Film/.Film.mp4.reencode-original",
            "filme/Film/Film.mkv",
            "papierkorb"
        ]
    );
}

#[tokio::test]
async fn requests_are_refused_and_recorded() {
    // AC-10.
    let env = Env::new("refused", "live").await;
    let original = info(Container::Matroska, "h264");
    let (id, video) = env.movie("Film", "Film.mkv", &original).await;
    let refused = |result: Result<Report, Refusal>| result.expect_err("refused");

    // Unknown ids and movies without a file record nothing.
    let (tools, encoder) = env.tools(Ok(()), writes(hevc(), 4000));
    let refusal = refused(env.attempt(&tools, Target::Movie(999)).await);
    assert!(refusal.not_found);
    let refusal = refused(env.attempt(&tools, Target::Episode(999)).await);
    assert!(refusal.not_found);
    assert_eq!(env.db().last_reencode().await.expect("read"), None);

    let expect = |refusal: Refusal, reason: &str| {
        assert!(!refusal.not_found);
        assert!(
            refusal.reason.starts_with(reason),
            "{} vs {reason}",
            refusal.reason
        );
    };
    // The encoder cannot be run.
    let (missing, _) = env.tools(
        Err("ffmpeg mit libx265 nicht gefunden".to_owned()),
        writes(hevc(), 4000),
    );
    expect(
        refused(env.attempt(&missing, Target::Movie(id)).await),
        "ffmpeg mit libx265 nicht gefunden",
    );
    // Media info incomplete, or without a video size.
    let mut partial = original.clone();
    partial.status = ReadStatus::Partial;
    write_fake(&video, &partial, SIZE);
    expect(
        refused(env.attempt(&tools, Target::Movie(id)).await),
        "Media-Infos des Originals nicht vollständig lesbar",
    );
    let mut sizeless = original.clone();
    if let Some(v) = &mut sizeless.video {
        v.width = None;
    }
    write_fake(&video, &sizeless, SIZE);
    expect(
        refused(env.attempt(&tools, Target::Movie(id)).await),
        "Media-Infos des Originals nicht vollständig lesbar",
    );
    // Dolby Vision and HDR10+.
    for flag in ["dolby_vision", "hdr10_plus"] {
        let mut special = original.clone();
        if let Some(v) = &mut special.video {
            match flag {
                "dolby_vision" => v.dynamic_range.dolby_vision = true,
                _ => v.dynamic_range.hdr10_plus = true,
            }
        }
        write_fake(&video, &special, SIZE);
        expect(
            refused(env.attempt(&tools, Target::Movie(id)).await),
            "Dolby Vision/HDR10+ würde verloren gehen",
        );
    }
    write_fake(&video, &original, SIZE);
    // Too little free space: the import's minimum is out of reach.
    env.db()
        .put_setting(
            "config_mediamanagement",
            &json!({ "minimumFreeSpaceWhenImporting": u64::MAX / (2 * 1024 * 1024) }),
        )
        .await
        .expect("settings");
    expect(
        refused(env.attempt(&tools, Target::Movie(id)).await),
        "zu wenig Speicherplatz",
    );
    // ... unless the check is skipped.
    env.db()
        .put_setting(
            "config_mediamanagement",
            &json!({
                "minimumFreeSpaceWhenImporting": u64::MAX / (2 * 1024 * 1024),
                "skipFreeSpaceCheckWhenImporting": true,
                "recycleBin": env.bin.to_string_lossy(),
            }),
        )
        .await
        .expect("settings");
    // The encode slot is taken by a running encode.
    let running = Running {
        title: "Anderer Film (2020)".to_owned(),
        path: "/filme/Anderer Film/Anderer Film.mkv".to_owned(),
        started_at: Timestamp::now().to_string(),
        runner: "server".to_owned(),
        attempt: None,
    };
    fs::write(
        &env.app.reencode.lock,
        serde_json::to_string(&running).expect("json"),
    )
    .expect("lock");
    expect(
        refused(env.attempt(&tools, Target::Movie(id)).await),
        "Es läuft bereits ein Encode: Anderer Film (2020)",
    );
    fs::remove_file(&env.app.reencode.lock).expect("unlock");
    // The file is missing on disk.
    fs::remove_file(&video).expect("remove");
    expect(
        refused(env.attempt(&tools, Target::Movie(id)).await),
        "Datei fehlt",
    );
    write_fake(&video, &original, SIZE);

    // Nothing was encoded; every refusal of a known file is recorded.
    assert!(encoder.jobs.lock().expect("jobs").is_empty());
    assert_eq!(env.files(), ["filme/Film/Film.mkv"]);
    let attempts = env
        .db()
        .reencodes_of(FileRef::Movie(id))
        .await
        .expect("attempts");
    assert_eq!(attempts.len(), 8);
    assert!(
        attempts
            .iter()
            .all(|a| a.outcome == Some(ReencodeOutcome::Refused))
    );
    assert_eq!(
        attempts[0].reason.as_deref(),
        Some("Datei fehlt: ".to_owned() + &video.to_string_lossy()).as_deref()
    );

    // With everything in order, it runs.
    let report = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect("started");
    assert_eq!(report.outcome, ReencodeOutcome::Replaced);
}

#[tokio::test]
async fn shadow_mode_encodes_nothing() {
    let env = Env::new("shadow", "shadow").await;
    let (id, _) = env
        .movie("Film", "Film.mkv", &info(Container::Matroska, "h264"))
        .await;
    let (tools, encoder) = env.tools(Ok(()), writes(hevc(), 4000));
    let refusal = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect_err("refused");
    assert_eq!(refusal.reason, "Im Shadow-Mode encodiert sofarr nichts");
    assert!(encoder.jobs.lock().expect("jobs").is_empty());
    assert_eq!(env.files(), ["filme/Film/Film.mkv"]);
    assert_eq!(last(&env).await.outcome, Some(ReencodeOutcome::Refused));
}

#[tokio::test]
async fn an_interrupted_attempt_is_cleaned_up_first() {
    // AC-12.
    let env = Env::new("cleanup", "live").await;
    let original = info(Container::Mp4, "h264");
    let (id, video) = env.movie("Film", "Film.mp4", &original).await;
    let new = sofarr_db::NewReencode {
        file: FileRef::Movie(id),
        title: "Film (2019)".to_owned(),
        path: video.to_string_lossy().into_owned(),
        runner: "server".to_owned(),
        size_before: SIZE,
    };
    let interrupted = env
        .db()
        .start_reencode(&new, Timestamp::now())
        .await
        .expect("start");

    // The swap stopped after the original went aside: it is restored, the work file goes.
    let aside = video.with_file_name(".Film.mp4.reencode-original");
    fs::rename(&video, &aside).expect("aside");
    write_fake(
        &video.with_file_name(".Film.mp4.reencode-partial"),
        &hevc(),
        3000,
    );
    let (failing, _) = env.tools(
        Err("ffmpeg mit libx265 nicht gefunden".to_owned()),
        writes(hevc(), 4000),
    );
    let refusal = env
        .attempt(&failing, Target::Movie(id))
        .await
        .expect_err("refused");
    assert_eq!(refusal.reason, "ffmpeg mit libx265 nicht gefunden");
    assert_eq!(env.files(), ["filme/Film/Film.mp4"]);
    assert_eq!(read_fake(&video).expect("restored").info, original);
    let closed = env
        .db()
        .reencodes_of(FileRef::Movie(id))
        .await
        .expect("attempts")
        .into_iter()
        .find(|a| a.id == interrupted)
        .expect("the interrupted attempt");
    assert_eq!(closed.outcome, Some(ReencodeOutcome::Failed));
    assert_eq!(closed.reason.as_deref(), Some("abgebrochen"));

    // The swap was complete (the library's video is there): the hidden original goes to
    // the recycle bin.
    write_fake(&aside, &original, SIZE);
    env.attempt(&failing, Target::Movie(id))
        .await
        .expect_err("refused");
    assert_eq!(
        env.files(),
        ["filme/Film/Film.mp4", "papierkorb/Film/Film.mp4"]
    );

    // A running encode of this file is left alone, and so is its attempt.
    let held = env
        .db()
        .start_reencode(&new, Timestamp::now())
        .await
        .expect("start");
    let work = video.with_file_name(".Film.mp4.reencode-partial");
    write_fake(&work, &hevc(), 3000);
    let running = Running {
        title: "Film (2019)".to_owned(),
        path: video.to_string_lossy().into_owned(),
        started_at: Timestamp::now().to_string(),
        runner: "server".to_owned(),
        attempt: Some(held),
    };
    fs::write(
        &env.app.reencode.lock,
        serde_json::to_string(&running).expect("json"),
    )
    .expect("lock");
    let (tools, _) = env.tools(Ok(()), writes(hevc(), 4000));
    let refusal = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect_err("refused");
    assert_eq!(refusal.reason, "Es läuft bereits ein Encode: Film (2019)");
    assert!(work.exists());
    assert_eq!(env.db().open_reencodes().await.expect("open").len(), 1);

    // A lock not touched for five minutes is left over from a crash: it is taken over.
    let old = SystemTime::now() - STALE - Duration::from_secs(1);
    fs::OpenOptions::new()
        .write(true)
        .open(&env.app.reencode.lock)
        .expect("lock")
        .set_modified(old)
        .expect("age");
    let report = env
        .attempt(&tools, Target::Movie(id))
        .await
        .expect("started");
    assert_eq!(report.outcome, ReencodeOutcome::Replaced);
    assert!(env.db().open_reencodes().await.expect("open").is_empty());
    assert_eq!(
        env.files(),
        [
            "filme/Film/Film.mkv",
            "papierkorb/Film/Film (1).mp4",
            "papierkorb/Film/Film.mp4"
        ]
    );
}

#[tokio::test]
async fn the_slot_shows_the_running_encode() {
    // AC-02 (part): what `GET /api/reencode/encode` reads.
    let env = Env::new("slot", "live").await;
    let (id, video) = env
        .movie("Film", "Film.mkv", &info(Container::Matroska, "h264"))
        .await;
    let seen = Arc::new(Mutex::new(None));
    let lock = env.app.reencode.lock.clone();
    let inside = Arc::clone(&seen);
    let (tools, _) = env.tools(
        Ok(()),
        Box::new(move |job: &Job| {
            *inside.lock().expect("seen") = running(&lock);
            write_fake(&job.output, &hevc(), 4000);
            Ok(())
        }),
    );
    env.attempt(&tools, Target::Movie(id))
        .await
        .expect("started");
    let seen = seen
        .lock()
        .expect("seen")
        .clone()
        .expect("the lock while encoding");
    assert_eq!(seen.title, "Film (2019)");
    assert_eq!(seen.path, video.to_string_lossy());
    assert_eq!(seen.runner, "cli");
    assert!(seen.attempt.is_some());
    assert_eq!(running(&env.app.reencode.lock), None);
}
