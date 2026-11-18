//! One re-encode of one library file (spec reencode-file): the checks, the encode slot,
//! the clean-up after an interrupted attempt, the encode, the check of the result and
//! the swap with the original.

use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use sofarr_db::{Db, FileRef, NewReencode, ReencodeOutcome};
use sofarr_mediainfo::{Container, MediaInfo, ReadStatus};

use super::ffmpeg::{EncodeError, Encoder, Ffmpeg, Job, Settings};
use crate::{
    app::App,
    config::Mode,
    importer::{self, ImportSettings},
    media::{self, FileMedia},
    process::Limits,
};

/// A lock not touched for this long is left over from a crash.
pub const STALE: Duration = Duration::from_secs(5 * 60);
/// How often a running encode touches its lock.
const HEARTBEAT: Duration = Duration::from_secs(60);
const WORK_SUFFIX: &str = ".reencode-partial";
const ASIDE_SUFFIX: &str = ".reencode-original";
const MB: u64 = 1024 * 1024;

/// What to re-encode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Movie(i64),
    /// Any episode of the file.
    Episode(i64),
}

/// Who runs the encode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runner {
    Server,
    Cli,
}

impl Runner {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Server => "server",
            Self::Cli => "cli",
        }
    }
}

/// Reads the media info of a file (blocking); `None` when it is gone.
pub type ReadMedia = dyn Fn(&Path) -> Option<FileMedia> + Send + Sync;

/// The encoder and the media info reader; tests put fakes here.
pub struct Tools {
    pub encoder: Arc<dyn Encoder>,
    pub read: Arc<ReadMedia>,
}

impl Tools {
    /// ffmpeg with the configured settings and sofarr's own media info reader.
    pub fn real(app: &App) -> Self {
        let config = &app.config.reencode;
        Self {
            encoder: Arc::new(Ffmpeg::new(Settings {
                crf: config.crf,
                preset: config.preset.clone(),
                threads: config.threads,
            })),
            read: Arc::new(media::read_path),
        }
    }
}

/// The re-encode state of the server: where the encode slot's lock lives, and the tools
/// (built at first use).
pub struct State {
    pub lock: PathBuf,
    pub tools: std::sync::Mutex<Option<Arc<Tools>>>,
    /// Set when sofarr stops: running encoders stop (spec reencode-job AC-06).
    pub stopping: Arc<AtomicBool>,
    /// Encodes of this process that have not ended yet.
    pub in_flight: Arc<std::sync::atomic::AtomicUsize>,
    /// The result of the job's last check of ffmpeg (spec reencode-job AC-07, AC-08).
    pub ffmpeg: std::sync::Mutex<Option<Result<(), String>>>,
}

impl State {
    pub fn new(database: &Path) -> Self {
        Self {
            lock: database
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("reencode.lock"),
            tools: std::sync::Mutex::new(None),
            stopping: Arc::new(AtomicBool::new(false)),
            in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            ffmpeg: std::sync::Mutex::new(None),
        }
    }
}

/// Counts an encode of this process while it runs.
struct InFlight(Arc<std::sync::atomic::AtomicUsize>);

impl InFlight {
    fn new(count: &Arc<std::sync::atomic::AtomicUsize>) -> Self {
        count.fetch_add(1, Ordering::SeqCst);
        Self(Arc::clone(count))
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// sofarr stops: running encoders are stopped and their attempts recorded; waits up to
/// `wait` for them (spec reencode-job AC-06).
pub async fn stop(app: &App, wait: Duration) {
    app.reencode.stopping.store(true, Ordering::SeqCst);
    let deadline = std::time::Instant::now() + wait;
    while app.reencode.in_flight.load(Ordering::SeqCst) > 0 && std::time::Instant::now() < deadline
    {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// The tools of this app (real ones unless a test set others).
pub fn tools(app: &App) -> Arc<Tools> {
    let mut slot = app
        .reencode
        .tools
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Arc::clone(slot.get_or_insert_with(|| Arc::new(Tools::real(app))))
}

/// The library file an attempt works on.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Subject {
    file: FileRef,
    title: String,
    /// The movie or series folder.
    folder: PathBuf,
    /// Below the folder, `/`-separated.
    relative_path: String,
    series: bool,
}

impl Subject {
    fn video(&self) -> PathBuf {
        self.folder.join(&self.relative_path)
    }

    /// The item folder's name, under which the recycle bin keeps its files.
    fn folder_name(&self) -> String {
        self.folder
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// The relative path with the extension `.mkv`.
    fn final_relative(&self) -> String {
        let (dir, name) = match self.relative_path.rsplit_once('/') {
            Some((dir, name)) => (Some(dir), name),
            None => (None, self.relative_path.as_str()),
        };
        let stem = Path::new(name)
            .file_stem()
            .map_or_else(|| name.to_owned(), |s| s.to_string_lossy().into_owned());
        match dir {
            Some(dir) => format!("{dir}/{stem}.mkv"),
            None => format!("{stem}.mkv"),
        }
    }
}

async fn resolve(db: &Db, target: Target) -> anyhow::Result<Option<Subject>> {
    match target {
        Target::Movie(id) => {
            let Some(movie) = db.movie(id).await? else {
                return Ok(None);
            };
            let Some(file) = movie.file else {
                return Ok(None);
            };
            let name = movie.title_de.as_ref().unwrap_or(&movie.title);
            Ok(Some(Subject {
                file: FileRef::Movie(id),
                title: movie
                    .year
                    .map_or_else(|| name.clone(), |year| format!("{name} ({year})")),
                folder: PathBuf::from(&movie.path),
                relative_path: file.relative_path,
                series: false,
            }))
        }
        Target::Episode(id) => {
            let episodes = db.episodes(None).await?;
            let Some(episode) = episodes.iter().find(|e| e.id == id) else {
                return Ok(None);
            };
            let (Some(file_id), Some(file)) = (episode.file_id, &episode.file) else {
                return Ok(None);
            };
            let Some(show) = db
                .all_series()
                .await?
                .into_iter()
                .find(|s| s.id == episode.series_id)
            else {
                return Ok(None);
            };
            let mut numbers: Vec<(u16, u16)> = episodes
                .iter()
                .filter(|e| e.file_id == Some(file_id))
                .map(|e| (e.data.season, e.data.number))
                .collect();
            numbers.sort_unstable();
            let name = show.data.title_de.as_ref().unwrap_or(&show.data.title);
            let (season, first) = numbers.first().copied().unwrap_or_default();
            let last = numbers.last().map_or(first, |(_, n)| *n);
            let mut title = format!("{name} – S{season:02}E{first:02}");
            if last != first {
                let _ = write!(title, "-E{last:02}");
            }
            Ok(Some(Subject {
                file: FileRef::Episode(file_id),
                title,
                folder: PathBuf::from(&show.data.path),
                relative_path: file.relative_path.clone(),
                series: true,
            }))
        }
    }
}

/// The path the library records for a file now.
async fn recorded_path(db: &Db, file: FileRef) -> anyhow::Result<Option<String>> {
    Ok(match file {
        FileRef::Movie(id) => db
            .movie(id)
            .await?
            .and_then(|movie| movie.file)
            .map(|file| file.relative_path),
        FileRef::Episode(file_id) => db
            .episodes(None)
            .await?
            .into_iter()
            .find(|e| e.file_id == Some(file_id))
            .and_then(|e| e.file)
            .map(|file| file.relative_path),
    })
}

/// Why an attempt did not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// No such movie or episode, or it has no library file (nothing was recorded).
    pub not_found: bool,
    pub reason: String,
}

impl Refusal {
    fn not_found() -> Self {
        Self {
            not_found: true,
            reason: "Kein Film und keine Episode mit Datei unter dieser ID".to_owned(),
        }
    }
}

/// The running encode as its lock says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Running {
    pub title: String,
    /// The original's absolute path.
    pub path: String,
    pub started_at: String,
    /// `server` or `cli`.
    pub runner: String,
    #[serde(default)]
    pub attempt: Option<i64>,
}

fn is_fresh(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age < STALE)
}

/// The encode that holds the slot, if one does (a stale lock holds nothing).
pub fn running(lock: &Path) -> Option<Running> {
    if !is_fresh(lock) {
        return None;
    }
    serde_json::from_str(&fs::read_to_string(lock).ok()?).ok()
}

/// The held encode slot: its lock is touched every minute and removed when it is dropped.
pub struct Slot {
    lock: PathBuf,
    running: Running,
    stop: Arc<AtomicBool>,
}

impl Slot {
    /// Takes the slot, or answers who holds it (`Err(None)`: the lock cannot be written).
    fn take(lock: &Path, running: Running) -> Result<Self, Option<Running>> {
        for _ in 0..2 {
            let file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(lock);
            match file {
                Ok(_) => {
                    let slot = Self {
                        lock: lock.to_owned(),
                        running,
                        stop: Arc::new(AtomicBool::new(false)),
                    };
                    slot.write();
                    slot.beat();
                    return Ok(slot);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if let Some(holder) = self::running(lock) {
                        return Err(Some(holder));
                    }
                    if is_fresh(lock) {
                        // Fresh but unreadable: someone is writing it right now.
                        return Err(None);
                    }
                    tracing::warn!(lock = %lock.display(), "taking over a stale re-encode lock");
                    let _ = fs::remove_file(lock);
                }
                Err(_) => return Err(None),
            }
        }
        Err(None)
    }

    fn write(&self) {
        if let Ok(json) = serde_json::to_string(&self.running) {
            let _ = fs::write(&self.lock, json);
        }
    }

    fn set_attempt(&mut self, id: i64) {
        self.running.attempt = Some(id);
        self.write();
    }

    /// Touches the lock every minute until the slot is dropped.
    fn beat(&self) {
        let lock = self.lock.clone();
        let stop = Arc::clone(&self.stop);
        std::thread::spawn(move || {
            let mut waited = Duration::ZERO;
            while !stop.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_secs(1));
                waited += Duration::from_secs(1);
                if waited >= HEARTBEAT {
                    waited = Duration::ZERO;
                    if let Ok(file) = fs::OpenOptions::new().write(true).open(&lock) {
                        let _ = file.set_modified(SystemTime::now());
                    }
                }
            }
        });
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = fs::remove_file(&self.lock);
    }
}

/// An attempt that passed its checks and holds the slot.
pub struct Prepared {
    subject: Subject,
    slot: Slot,
    attempt: i64,
    original: FileMedia,
    /// Most bytes a good result may have.
    limit: u64,
}

impl Prepared {
    pub fn title(&self) -> &str {
        &self.subject.title
    }

    pub fn path(&self) -> PathBuf {
        self.subject.video()
    }

    pub fn size_before(&self) -> u64 {
        self.original.size
    }
}

/// How an attempt ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub outcome: ReencodeOutcome,
    pub reason: Option<String>,
    pub title: String,
    pub path: PathBuf,
    pub size_before: u64,
    pub size_after: Option<u64>,
}

async fn import_settings(db: &Db, series: bool) -> anyhow::Result<ImportSettings> {
    let key = if series {
        format!("{}config_mediamanagement", crate::series::PREFIX)
    } else {
        "config_mediamanagement".to_owned()
    };
    Ok(ImportSettings::from_json(db.setting(&key).await?.as_ref()))
}

/// The hidden name of a file in its folder: `.<name><suffix>`.
fn hidden(video: &Path, suffix: &str) -> PathBuf {
    let name = video
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    video.with_file_name(format!(".{name}{suffix}"))
}

/// Moves a file into the recycle bin under `<item folder>/<relative path>`, or deletes it
/// without one. Returns the German reason of a failure.
fn recycle(file: &Path, bin: Option<&Path>, folder_name: &str, label: &str) -> Result<(), String> {
    let (_, failure) =
        crate::file_ops::remove_files(&[(file.to_owned(), label.to_owned())], bin, folder_name);
    failure.map_or(Ok(()), Err)
}

/// What an interrupted attempt on this file left behind (AC-12): a hidden original goes
/// back to its path where that is empty, else (the swap was complete) to the recycle
/// bin; work files go. A file the running encode holds is left alone.
async fn clean_up(db: &Db, subject: &Subject, held: Option<&Running>) -> anyhow::Result<()> {
    let video = subject.video();
    if held.is_some_and(|r| Path::new(&r.path) == video) {
        return Ok(());
    }
    let Some(folder) = video.parent() else {
        return Ok(());
    };
    let name = video
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = video
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let prefix = format!(".{stem}.");
    let Ok(entries) = fs::read_dir(folder) else {
        return Ok(());
    };
    let settings = import_settings(db, subject.series).await?;
    for entry in entries.flatten() {
        let entry_name = entry.file_name().to_string_lossy().into_owned();
        if !entry_name.starts_with(&prefix) {
            continue;
        }
        let path = entry.path();
        if entry_name.ends_with(WORK_SUFFIX) {
            tracing::info!(file = %path.display(), "removing a left-over re-encode work file");
            let _ = fs::remove_file(&path);
        } else if let Some(original) = entry_name
            .strip_suffix(ASIDE_SUFFIX)
            .and_then(|n| n.strip_prefix('.'))
        {
            if original == name && !video.exists() {
                tracing::info!(file = %video.display(), "restoring the original of an interrupted re-encode");
                fs::rename(&path, &video)?;
            } else {
                let label = match subject.relative_path.rsplit_once('/') {
                    Some((dir, _)) => format!("{dir}/{original}"),
                    None => original.to_owned(),
                };
                if let Err(reason) = recycle(
                    &path,
                    settings.recycle_bin.as_deref(),
                    &subject.folder_name(),
                    &label,
                ) {
                    tracing::warn!(file = %path.display(), %reason, "left-over original not removed");
                }
            }
        }
    }
    Ok(())
}

/// The library file of a record, as an attempt sees it.
async fn subject_of(db: &Db, file: FileRef) -> anyhow::Result<Option<Subject>> {
    match file {
        FileRef::Movie(id) => resolve(db, Target::Movie(id)).await,
        FileRef::Episode(file_id) => {
            let episode = db
                .episodes(None)
                .await?
                .into_iter()
                .find(|e| e.file_id == Some(file_id));
            match episode {
                Some(episode) => resolve(db, Target::Episode(episode.id)).await,
                None => Ok(None),
            }
        }
    }
}

/// Repairs every interrupted attempt (spec reencode-job AC-05): what an attempt on its
/// file does first (AC-12), for all of them at once.
pub async fn repair_all(app: &App) -> anyhow::Result<usize> {
    let held = running(&app.reencode.lock);
    let open: Vec<_> = app
        .db
        .open_reencodes()
        .await?
        .into_iter()
        .filter(|a| !held.as_ref().is_some_and(|r| r.attempt == Some(a.id)))
        .collect();
    for attempt in &open {
        if let Some(file) = attempt.file
            && let Some(subject) = subject_of(&app.db, file).await?
        {
            clean_up(&app.db, &subject, held.as_ref()).await?;
        }
    }
    close_interrupted(&app.db, held.as_ref()).await?;
    Ok(open.len())
}

/// Closes the recorded attempts no running encode holds as interrupted (AC-12).
async fn close_interrupted(db: &Db, held: Option<&Running>) -> anyhow::Result<()> {
    for attempt in db.open_reencodes().await? {
        if held.is_some_and(|r| r.attempt == Some(attempt.id)) {
            continue;
        }
        db.finish_reencode(
            attempt.id,
            ReencodeOutcome::Failed,
            Some("abgebrochen"),
            None,
            Timestamp::now(),
        )
        .await?;
    }
    Ok(())
}

async fn refuse(
    db: &Db,
    subject: &Subject,
    runner: Runner,
    size: u64,
    reason: String,
) -> anyhow::Result<Refusal> {
    let now = Timestamp::now();
    let new = NewReencode {
        file: subject.file,
        title: subject.title.clone(),
        path: subject.video().to_string_lossy().into_owned(),
        runner: runner.as_str().to_owned(),
        size_before: size,
    };
    let id = db.start_reencode(&new, now).await?;
    db.finish_reencode(id, ReencodeOutcome::Refused, Some(&reason), None, now)
        .await?;
    tracing::info!(title = %subject.title, %reason, "re-encode refused");
    Ok(Refusal {
        not_found: false,
        reason,
    })
}

/// Checks a request and takes the encode slot (AC-10, AC-12). Every refusal of a known
/// library file is recorded.
pub async fn prepare(
    app: &App,
    tools: &Tools,
    target: Target,
    runner: Runner,
) -> anyhow::Result<Result<Prepared, Refusal>> {
    let Some(subject) = resolve(&app.db, target).await? else {
        return Ok(Err(Refusal::not_found()));
    };
    let held = running(&app.reencode.lock);
    clean_up(&app.db, &subject, held.as_ref()).await?;
    close_interrupted(&app.db, held.as_ref()).await?;

    let video = subject.video();
    let size = fs::metadata(&video).map_or(0, |m| m.len());
    let refused = |reason: String| refuse(&app.db, &subject, runner, size, reason);
    if app.config.automation.mode == Mode::Shadow {
        return Ok(Err(refused(
            "Im Shadow-Mode encodiert sofarr nichts".to_owned(),
        )
        .await?));
    }
    let encoder = Arc::clone(&tools.encoder);
    let checked = tokio::task::spawn_blocking(move || encoder.check()).await?;
    if let Err(reason) = checked {
        return Ok(Err(refused(reason).await?));
    }
    let read = Arc::clone(&tools.read);
    let path = video.clone();
    let Some(original) = tokio::task::spawn_blocking(move || read(&path)).await? else {
        return Ok(Err(
            refused(format!("Datei fehlt: {}", video.display())).await?
        ));
    };
    let info = &original.info;
    let sized = info
        .video
        .as_ref()
        .is_some_and(|v| v.width.is_some() && v.height.is_some());
    if info.status != ReadStatus::Complete || !sized {
        return Ok(Err(refused(
            "Media-Infos des Originals nicht vollständig lesbar".to_owned(),
        )
        .await?));
    }
    if info
        .video
        .as_ref()
        .is_some_and(|v| v.dynamic_range.dolby_vision || v.dynamic_range.hdr10_plus)
    {
        return Ok(Err(refused(
            "Dolby Vision/HDR10+ würde verloren gehen".to_owned(),
        )
        .await?));
    }
    let keep = 100 - u64::from(app.config.reencode.min_saving_percent);
    let limit = original.size / 100 * keep + original.size % 100 * keep / 100;
    let settings = import_settings(&app.db, subject.series).await?;
    let minimum = if settings.skip_free_space_check {
        0
    } else {
        settings.minimum_free_space_mb
    };
    let needed = limit.div_ceil(MB) + minimum;
    if let Some(free) = video.parent().and_then(importer::free_space_mb)
        && free < needed
    {
        return Ok(Err(refused(format!(
            "zu wenig Speicherplatz: {free} MB frei, verlangt sind {needed} MB"
        ))
        .await?));
    }
    let running = Running {
        title: subject.title.clone(),
        path: video.to_string_lossy().into_owned(),
        started_at: Timestamp::now().to_string(),
        runner: runner.as_str().to_owned(),
        attempt: None,
    };
    let mut slot = match Slot::take(&app.reencode.lock, running) {
        Ok(slot) => slot,
        Err(holder) => {
            let reason = holder.map_or_else(
                || "Es läuft bereits ein Encode".to_owned(),
                |h| format!("Es läuft bereits ein Encode: {}", h.title),
            );
            return Ok(Err(refused(reason).await?));
        }
    };
    let new = NewReencode {
        file: subject.file,
        title: subject.title.clone(),
        path: video.to_string_lossy().into_owned(),
        runner: runner.as_str().to_owned(),
        size_before: original.size,
    };
    let attempt = app.db.start_reencode(&new, Timestamp::now()).await?;
    slot.set_attempt(attempt);
    tracing::info!(title = %subject.title, file = %video.display(), "re-encode started");
    Ok(Ok(Prepared {
        subject,
        slot,
        attempt,
        original,
        limit,
    }))
}

/// Whether a finished result is good apart from its size; `Err` is the German reason.
pub fn check_result(
    original: &MediaInfo,
    result: &MediaInfo,
    audio: &[super::audio::AudioPlan],
) -> Result<(), String> {
    let bad = |what: &str| Err(format!("Ergebnis unbrauchbar: {what}"));
    if result.status != ReadStatus::Complete {
        return bad("nicht vollständig lesbar");
    }
    let (Some(before), Some(after)) = (&original.video, &result.video) else {
        return bad("kein Video");
    };
    if after.codec != "h265" {
        return bad("Video ist nicht HEVC");
    }
    if (before.width, before.height) != (after.width, after.height) {
        return bad("andere Auflösung");
    }
    if original.audio.len() != result.audio.len() {
        return bad("andere Zahl an Tonspuren");
    }
    // Each audio track as planned (spec reencode-audio AC-05).
    for (n, (before, after)) in original.audio.iter().zip(&result.audio).enumerate() {
        let plan = audio
            .get(n)
            .copied()
            .unwrap_or(super::audio::AudioPlan::Copy);
        let fits = match plan {
            super::audio::AudioPlan::Copy => {
                after.codec == before.codec && after.channels == before.channels
            }
            super::audio::AudioPlan::Convert { .. } => {
                after.codec == "eac3"
                    && after
                        .channels
                        .is_none_or(|c| c <= super::audio::MAX_CHANNELS)
                    && after.language == before.language
            }
        };
        if !fits {
            return bad(&format!("Tonspur {} nicht wie geplant", n + 1));
        }
    }
    if original.subtitles.len() != result.subtitles.len() {
        return bad("andere Zahl an Untertitelspuren");
    }
    if let Some(expected) = original.duration_ms {
        match result.duration_ms {
            Some(got) if got.abs_diff(expected) <= 1000 => {}
            Some(_) => return bad("andere Laufzeit"),
            None => return bad("keine Laufzeit"),
        }
    }
    let (b, a) = (before.dynamic_range, after.dynamic_range);
    if (b.hdr10 && !a.hdr10) || (b.hlg && !a.hlg) {
        return bad("HDR ging verloren");
    }
    Ok(())
}

/// Runs a prepared attempt to its end and records its outcome (AC-03 … AC-09, AC-11).
pub async fn run(app: &App, tools: &Tools, prepared: Prepared) -> anyhow::Result<Report> {
    let Prepared {
        subject,
        slot,
        attempt,
        original,
        limit,
    } = prepared;
    let _in_flight = InFlight::new(&app.reencode.in_flight);
    let video = subject.video();
    let work = hidden(&video, WORK_SUFFIX);
    let mut report = Report {
        outcome: ReencodeOutcome::Failed,
        reason: None,
        title: subject.title.clone(),
        path: video.clone(),
        size_before: original.size,
        size_after: None,
    };
    let finish = |report: Report| async move {
        app.db
            .finish_reencode(
                attempt,
                report.outcome,
                report.reason.as_deref(),
                report.size_after,
                Timestamp::now(),
            )
            .await?;
        tracing::info!(
            title = %report.title,
            outcome = report.outcome.as_str(),
            reason = report.reason.as_deref().unwrap_or(""),
            "re-encode finished"
        );
        anyhow::Ok(report)
    };

    let audio = super::audio::plan(
        &original.info,
        super::audio::AudioSettings {
            convert: app.config.reencode.convert_lossless_audio,
            bitrate_kbps: app.config.reencode.audio_bitrate_kbps,
        },
    );
    let copy_video = super::audio::copies_video(&original.info, &audio);
    let job = Job {
        input: video.clone(),
        output: work.clone(),
        mp4: matches!(
            original.info.container,
            Some(Container::Mp4 | Container::QuickTime)
        ),
        audio: audio.clone(),
        copy_video,
        limits: Limits {
            timeout: Duration::from_secs(u64::from(app.config.reencode.timeout_hours) * 3600),
            max_bytes: limit,
        },
        cancel: Some(Arc::clone(&app.reencode.stopping)),
    };
    let encoder = Arc::clone(&tools.encoder);
    let encoded = tokio::task::spawn_blocking(move || encoder.encode(&job)).await?;
    let discard = || {
        let _ = fs::remove_file(&work);
    };
    match encoded {
        Ok(()) => {}
        Err(EncodeError::TooLarge) => {
            discard();
            report.outcome = ReencodeOutcome::NoGain;
            report.reason = Some(format!(
                "Ergebnis wäre größer als {} MB geworden",
                limit.div_ceil(MB)
            ));
            return finish(report).await;
        }
        Err(EncodeError::Failed(reason)) => {
            discard();
            report.reason = Some(reason);
            return finish(report).await;
        }
        Err(EncodeError::Cancelled) => {
            discard();
            report.reason = Some("abgebrochen: sofarr wurde beendet".to_owned());
            return finish(report).await;
        }
    }

    let read = Arc::clone(&tools.read);
    let path = work.clone();
    let result = tokio::task::spawn_blocking(move || read(&path)).await?;
    let Some(result) = result else {
        report.reason = Some("Ergebnis unbrauchbar: keine Datei geschrieben".to_owned());
        return finish(report).await;
    };
    report.size_after = Some(result.size);
    if result.size > limit {
        discard();
        report.outcome = ReencodeOutcome::NoGain;
        report.reason = Some(format!(
            "Ergebnis nur {} % kleiner",
            original.size.saturating_sub(result.size) * 100 / original.size.max(1)
        ));
        return finish(report).await;
    }
    if let Err(reason) = check_result(&original.info, &result.info, &audio) {
        discard();
        report.reason = Some(reason);
        return finish(report).await;
    }

    // The swap happens while no other job of this process touches the library.
    let _jobs = app.jobs.lock().await;
    let unchanged = fs::metadata(&video).is_ok_and(|meta| {
        meta.len() == original.size && media::modified(&meta) == original.modified
    });
    let recorded =
        recorded_path(&app.db, subject.file).await? == Some(subject.relative_path.clone());
    let final_relative = subject.final_relative();
    let final_path = subject.folder.join(&final_relative);
    let reason = if !unchanged {
        Some("Original hat sich während des Encodes geändert".to_owned())
    } else if !recorded {
        Some("Die Bibliothek führt das Original nicht mehr unter diesem Pfad".to_owned())
    } else if final_path != video && final_path.exists() {
        Some(format!(
            "Zieldatei existiert bereits: {}",
            final_path.display()
        ))
    } else {
        None
    };
    if let Some(reason) = reason {
        discard();
        report.reason = Some(reason);
        return finish(report).await;
    }

    let aside = hidden(&video, ASIDE_SUFFIX);
    if let Err(e) = fs::rename(&video, &aside) {
        discard();
        report.reason = Some(format!("Original nicht beiseitelegbar: {e}"));
        return finish(report).await;
    }
    let restore = |placed: bool| {
        if placed {
            let _ = fs::remove_file(&final_path);
        }
        let _ = fs::remove_file(&work);
        let _ = fs::rename(&aside, &video);
    };
    if let Err(e) = fs::rename(&work, &final_path) {
        restore(false);
        report.reason = Some(format!("Ergebnis nicht einsetzbar: {e}"));
        return finish(report).await;
    }
    if let Err(e) = app
        .db
        .replace_file_on_disk(subject.file, &final_relative, result.size)
        .await
    {
        restore(true);
        report.reason = Some(format!("Bibliothek nicht aktualisierbar: {e}"));
        return finish(report).await;
    }
    let placed = FileMedia {
        modified: fs::metadata(&final_path)
            .map(|meta| media::modified(&meta))
            .unwrap_or_default(),
        ..result
    };
    if let Err(e) = media::store(&app.db, subject.file, &placed).await {
        tracing::warn!(
            error = format!("{e:#}"),
            "media info of the re-encoded file not stored"
        );
    }
    report.outcome = ReencodeOutcome::Replaced;
    let settings = import_settings(&app.db, subject.series).await?;
    let original_name = video
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Err(reason) = recycle(
        &aside,
        settings.recycle_bin.as_deref(),
        &subject.folder_name(),
        &subject.relative_path,
    ) {
        report.reason = Some(format!(
            "Original liegt noch als .{original_name}{ASIDE_SUFFIX} im Ordner: {reason}"
        ));
    }
    drop(slot);
    finish(report).await
}

/// A whole attempt: prepare, then run (the CLI and the API print or answer in between).
#[cfg(test)]
pub async fn attempt(
    app: &App,
    tools: &Tools,
    target: Target,
    runner: Runner,
) -> anyhow::Result<Result<Report, Refusal>> {
    match prepare(app, tools, target, runner).await? {
        Ok(prepared) => Ok(Ok(run(app, tools, prepared).await?)),
        Err(refusal) => Ok(Err(refusal)),
    }
}

#[cfg(test)]
pub(crate) mod tests;
