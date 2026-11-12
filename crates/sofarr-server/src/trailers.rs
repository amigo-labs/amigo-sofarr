//! Trailers (spec trailers): one trailer per movie and series from TMDB's YouTube videos,
//! downloaded with yt-dlp into the item's `trailers` folder for Jellyfin. A daily run
//! works through eligible items, newest added first, within the daily budget; a manual
//! download works outside it.

use std::{
    collections::HashSet,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::Context;
use jiff::{SignedDuration, Timestamp};
use sofarr_db::{MediaRef, TrailerItem, TrailerRecord, TrailerState};
use sofarr_metadata::{MetadataError, Video, VideoOwner, trailer_candidates};

use crate::{
    app::{App, SharedApp},
    config::Mode,
    importer,
    status::Job,
};

pub mod api;
pub mod ytdlp;

use ytdlp::{Downloader, VideoId};

/// Attempts before an item is left alone (AC-05).
pub const MAX_ATTEMPTS: u32 = 4;
/// Wait after an attempt without trailer (AC-05).
const RETRY: SignedDuration = SignedDuration::from_hours(7 * 24);
const DAY: SignedDuration = SignedDuration::from_hours(24);
/// When the daily run last ran (a setting, so a restart does not run it again).
const RAN_AT: &str = "trailers_ran_at";
/// The folder Jellyfin reads local trailers from.
pub const FOLDER: &str = "trailers";

/// The German name of a trailer's language (AC-03, AC-10); `Original` without one.
pub fn language_name(code: Option<&str>) -> String {
    let Some(code) = code else {
        return "Original".to_owned();
    };
    let name = match code {
        "de" => "Deutsch",
        "en" => "Englisch",
        "fr" => "Französisch",
        "es" => "Spanisch",
        "it" => "Italienisch",
        "pt" => "Portugiesisch",
        "nl" => "Niederländisch",
        "da" => "Dänisch",
        "sv" => "Schwedisch",
        "no" | "nb" | "nn" => "Norwegisch",
        "fi" => "Finnisch",
        "is" => "Isländisch",
        "pl" => "Polnisch",
        "cs" => "Tschechisch",
        "hu" => "Ungarisch",
        "ro" => "Rumänisch",
        "bg" => "Bulgarisch",
        "el" => "Griechisch",
        "tr" => "Türkisch",
        "ru" => "Russisch",
        "uk" => "Ukrainisch",
        "lt" => "Litauisch",
        "he" => "Hebräisch",
        "ar" => "Arabisch",
        "hi" => "Hindi",
        "th" => "Thailändisch",
        "vi" => "Vietnamesisch",
        "ja" => "Japanisch",
        "ko" => "Koreanisch",
        "zh" => "Chinesisch",
        other => return other.to_ascii_uppercase(),
    };
    name.to_owned()
}

/// `<item folder name> - Trailer (<language name>).mkv` (AC-03).
pub fn file_name(folder_name: &str, language: Option<&str>) -> String {
    format!("{folder_name} - Trailer ({}).mkv", language_name(language))
}

/// Whether a folder is Jellyfin's trailer folder.
pub fn is_trailer_folder(name: &str) -> bool {
    name.eq_ignore_ascii_case(FOLDER)
}

/// Whether a file name marks a trailer: `-trailer`, `.trailer` or `_trailer` at the end
/// of its stem (Jellyfin's rule).
pub fn has_trailer_suffix(path: &Path) -> bool {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|stem| {
            ["-trailer", ".trailer", "_trailer"]
                .iter()
                .any(|suffix| stem.ends_with(suffix))
        })
}

/// The trailers of an item folder, relative to it (`/`-separated): videos in its
/// `trailers` folder and videos with a trailer suffix next to the item's files.
pub fn trailer_files(folder: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(folder) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if kind.is_dir() && is_trailer_folder(&name) {
            for inner in fs::read_dir(&path).into_iter().flatten().flatten() {
                let inner_path = inner.path();
                if inner.file_type().is_ok_and(|t| t.is_file()) && importer::is_video(&inner_path) {
                    found.push(importer::relative_path(folder, &inner_path));
                }
            }
        } else if kind.is_file() && importer::is_video(&path) && has_trailer_suffix(&path) {
            found.push(name);
        }
    }
    found.sort();
    found
}

/// Whether an item's trailer may be tried now (AC-05): fewer than four attempts and no
/// later retry date.
pub fn due(record: Option<&TrailerRecord>, now: Timestamp) -> bool {
    record.is_none_or(|r| r.attempts < MAX_ATTEMPTS && r.next_attempt.is_none_or(|t| t <= now))
}

/// The items the daily run tries, in order (AC-01, AC-02, AC-05): eligible (a video, a
/// TMDB id) and due; items an import made eligible first, then newest added.
pub fn queue(items: Vec<TrailerItem>, now: Timestamp) -> Vec<TrailerItem> {
    let mut queue: Vec<TrailerItem> = items
        .into_iter()
        .filter(|i| i.has_video && i.tmdb_id.is_some() && due(i.trailer.as_ref(), now))
        .collect();
    queue.sort_by(|a, b| {
        let first = |i: &TrailerItem| i.trailer.as_ref().and_then(|t| t.first_in_line);
        first(b)
            .cmp(&first(a))
            .then_with(|| b.added_at.cmp(&a.added_at))
    });
    queue
}

/// How one try of an item ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Downloaded {
        language: Option<String>,
        key: String,
        /// Relative to the item folder.
        file: String,
    },
    NoCandidate,
    /// Every candidate failed; the reasons.
    Failed(String),
}

/// The stored state after a try (AC-05, AC-09): success clears the attempts; otherwise an
/// automatic try counts one more attempt and a manual one starts over at one, with a
/// retry seven days later while attempts are left. A trailer sofarr wrote before stays
/// recorded until a new one replaces it.
pub fn after(
    previous: Option<&TrailerRecord>,
    outcome: &Outcome,
    manual: bool,
    now: Timestamp,
) -> TrailerRecord {
    let previous = previous.cloned().unwrap_or_default();
    match outcome {
        Outcome::Downloaded {
            language,
            key,
            file,
        } => TrailerRecord {
            state: TrailerState::Downloaded,
            language: language.clone(),
            video_key: Some(key.clone()),
            file: Some(file.clone()),
            message: None,
            attempts: 0,
            next_attempt: None,
            first_in_line: None,
        },
        Outcome::NoCandidate | Outcome::Failed(_) => {
            let attempts = if manual { 1 } else { previous.attempts + 1 };
            let (state, message) = match outcome {
                Outcome::Failed(reason) => (TrailerState::Failed, Some(reason.clone())),
                _ => (TrailerState::NoCandidate, None),
            };
            TrailerRecord {
                state,
                message,
                attempts,
                next_attempt: (attempts < MAX_ATTEMPTS)
                    .then(|| now.checked_add(RETRY).ok())
                    .flatten(),
                first_in_line: None,
                ..previous
            }
        }
    }
}

/// Runtime state of the trailer feature.
pub struct Runtime {
    pub downloader: Arc<dyn Downloader>,
    /// Result of the last tool check; `None` before the first (AC-12).
    tools: Mutex<Option<Result<(), String>>>,
    /// Items whose trailer is being downloaded.
    busy: Mutex<HashSet<MediaRef>>,
}

/// Marks an item as busy until dropped.
pub struct Claim<'a> {
    runtime: &'a Runtime,
    media: MediaRef,
}

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        if let Ok(mut busy) = self.runtime.busy.lock() {
            busy.remove(&self.media);
        }
    }
}

impl Runtime {
    pub fn new(downloader: Arc<dyn Downloader>) -> Self {
        Self {
            downloader,
            tools: Mutex::new(None),
            busy: Mutex::new(HashSet::new()),
        }
    }

    /// Claims an item for a download; `None` while another download of it runs.
    pub fn claim(&self, media: MediaRef) -> Option<Claim<'_>> {
        let mut busy = self.busy.lock().ok()?;
        busy.insert(media).then_some(Claim {
            runtime: self,
            media,
        })
    }

    /// The last tool check.
    pub fn tools(&self) -> Option<Result<(), String>> {
        self.tools.lock().ok().and_then(|t| t.clone())
    }

    /// Checks yt-dlp and ffmpeg again and keeps the result.
    pub async fn check_tools(&self) -> Result<(), String> {
        let downloader = Arc::clone(&self.downloader);
        let result = tokio::task::spawn_blocking(move || downloader.check())
            .await
            .unwrap_or_else(|e| Err(format!("Prüfung abgebrochen: {e}")));
        if let Ok(mut tools) = self.tools.lock() {
            *tools = Some(result.clone());
        }
        result
    }
}

/// Whether sofarr downloads trailers at all; `Err` holds the German reason for the
/// system page (AC-11, AC-12).
pub fn support(app: &App) -> Result<(), String> {
    configured(app)?;
    match app.trailers.tools() {
        None => Err("yt-dlp und ffmpeg werden noch geprüft".to_owned()),
        Some(result) => result,
    }
}

/// [`support`] without the tools: switched on, live mode, TMDB configured.
fn configured(app: &App) -> Result<(), String> {
    if !app.config.trailers.enabled {
        return Err("ausgeschaltet ([trailers] enabled = false)".to_owned());
    }
    if app.config.automation.mode == Mode::Shadow {
        return Err("Schattenmodus: sofarr lädt keine Trailer".to_owned());
    }
    if app.config.tmdb.is_none() {
        return Err("[tmdb] fehlt in der Konfiguration".to_owned());
    }
    Ok(())
}

/// Where downloads are made before they move into the library (under `/config/cache`).
pub fn work_folder(app: &App) -> PathBuf {
    app.config
        .database
        .path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("cache")
        .join(FOLDER)
}

/// The free space on the item's file system is below the import's minimum (AC-15);
/// returns the German reason.
async fn low_space(app: &App, item: &TrailerItem) -> anyhow::Result<Option<String>> {
    let key = match item.media {
        MediaRef::Movie(_) => "config_mediamanagement".to_owned(),
        MediaRef::Series(_) => format!("{}config_mediamanagement", crate::series::PREFIX),
    };
    let settings = importer::ImportSettings::from_json(app.db.setting(&key).await?.as_ref());
    if settings.skip_free_space_check || settings.minimum_free_space_mb == 0 {
        return Ok(None);
    }
    Ok(importer::free_space_mb(Path::new(&item.path))
        .filter(|free| *free < settings.minimum_free_space_mb)
        .map(|free| {
            format!(
                "zu wenig Speicherplatz: {free} MB frei, verlangt sind {} MB",
                settings.minimum_free_space_mb
            )
        }))
}

/// How a download of one item ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetched {
    /// Tried: downloaded, no candidate, or every candidate failed.
    Tried(Outcome),
    /// Too little free space; nothing was tried (AC-15).
    LowSpace,
    /// TMDB could not be asked; nothing was stored.
    TmdbDown(String),
}

/// Downloads the trailer of one item (AC-03, AC-04, AC-15) and stores the new state.
/// The caller checked that sofarr may write there.
pub async fn fetch(
    app: &App,
    item: &TrailerItem,
    manual: bool,
    now: Timestamp,
) -> anyhow::Result<Fetched> {
    if let Some(message) = low_space(app, item).await? {
        tracing::info!(item = %item.title, %message, "no trailer download");
        let record = TrailerRecord {
            state: TrailerState::LowSpace,
            message: Some(message),
            ..item.trailer.clone().unwrap_or_default()
        };
        app.db.put_trailer(item.media, &record, now).await?;
        return Ok(Fetched::LowSpace);
    }
    let tmdb_id = item.tmdb_id.context("ohne TMDB-ID")?;
    let owner = match item.media {
        MediaRef::Movie(_) => VideoOwner::Movie(tmdb_id),
        MediaRef::Series(_) => VideoOwner::Series(tmdb_id),
    };
    let languages = &app.config.trailers.languages;
    let videos = match crate::metadata::tmdb(app)?.videos(owner, languages).await {
        Ok(videos) => videos,
        Err(MetadataError::NotFound { .. }) => Vec::new(),
        Err(e) => return Ok(Fetched::TmdbDown(e.to_string())),
    };
    let candidates = trailer_candidates(&videos, languages);
    let outcome = if candidates.is_empty() {
        Outcome::NoCandidate
    } else {
        try_candidates(app, item, &candidates, now).await?
    };
    match &outcome {
        Outcome::Downloaded { file, .. } => {
            tracing::info!(item = %item.title, %file, "trailer downloaded");
        }
        Outcome::NoCandidate => tracing::info!(item = %item.title, "no trailer candidate"),
        Outcome::Failed(reasons) => {
            tracing::warn!(item = %item.title, %reasons, "trailer download failed");
        }
    }
    let record = after(item.trailer.as_ref(), &outcome, manual, now);
    app.db.put_trailer(item.media, &record, now).await?;
    Ok(Fetched::Tried(outcome))
}

/// Tries the candidates in order until one is downloaded and in place (AC-04).
async fn try_candidates(
    app: &App,
    item: &TrailerItem,
    candidates: &[Video],
    now: Timestamp,
) -> anyhow::Result<Outcome> {
    let mut reasons = Vec::new();
    for candidate in candidates {
        let Some(id) = VideoId::parse(&candidate.key) else {
            let shown: String = candidate.key.chars().take(20).collect();
            reasons.push(format!("{shown}: keine gültige YouTube-ID"));
            continue;
        };
        let work = work_folder(app).join(format!(
            "{}-{}",
            match item.media {
                MediaRef::Movie(id) => format!("movie-{id}"),
                MediaRef::Series(id) => format!("series-{id}"),
            },
            now.as_millisecond()
        ));
        let result = download_and_place(app, item, candidate, &id, &work).await;
        // Whatever happened, nothing stays in the work folder (AC-04).
        if let Err(e) = fs::remove_dir_all(&work)
            && e.kind() != ErrorKind::NotFound
        {
            tracing::warn!(folder = %work.display(), error = %e, "work folder not removed");
        }
        match result {
            Ok(file) => {
                return Ok(Outcome::Downloaded {
                    language: candidate.language.clone(),
                    key: id.as_str().to_owned(),
                    file,
                });
            }
            Err(reason) => reasons.push(format!("{}: {reason}", id.as_str())),
        }
    }
    Ok(Outcome::Failed(reasons.join("; ")))
}

/// Downloads one candidate into `work` and moves it into the item's `trailers` folder;
/// returns the trailer relative to the item folder.
async fn download_and_place(
    app: &App,
    item: &TrailerItem,
    candidate: &Video,
    id: &VideoId,
    work: &Path,
) -> Result<String, String> {
    fs::create_dir_all(work).map_err(|e| format!("Arbeitsordner nicht anlegbar: {e}"))?;
    let downloader = Arc::clone(&app.trailers.downloader);
    let (id_owned, work_owned) = (id.clone(), work.to_path_buf());
    let video = tokio::task::spawn_blocking(move || downloader.download(&id_owned, &work_owned))
        .await
        .map_err(|e| format!("Download abgebrochen: {e}"))??;
    // The folder may have moved meanwhile: the trailer goes to where the item is now.
    let current = app
        .db
        .trailer_item(item.media)
        .await
        .map_err(|e| format!("Bibliothek nicht lesbar: {e}"))?
        .ok_or("nicht mehr in der Bibliothek")?;
    let folder = PathBuf::from(&current.path);
    let own = current.trailer.and_then(|t| t.file);
    let language = candidate.language.clone();
    tokio::task::spawn_blocking(move || place(&video, &folder, language.as_deref(), own))
        .await
        .map_err(|e| format!("Verschieben abgebrochen: {e}"))?
}

/// Moves a downloaded trailer into `folder/trailers/` under its name (AC-03, AC-06): never
/// over a file sofarr did not write; a previous own trailer with another name goes.
fn place(
    video: &Path,
    folder: &Path,
    language: Option<&str>,
    own: Option<String>,
) -> Result<String, String> {
    if !folder.is_dir() {
        return Err("der Ordner ist nicht mehr vorhanden".to_owned());
    }
    let folder_name = folder
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Ordner ohne Namen")?;
    let name = file_name(folder_name, language);
    let relative = format!("{FOLDER}/{name}");
    let target_dir = folder.join(FOLDER);
    let target = target_dir.join(&name);
    if target.exists() && own.as_deref() != Some(relative.as_str()) {
        return Err(format!("{relative} gibt es schon (nicht von sofarr)"));
    }
    fs::create_dir_all(&target_dir).map_err(|e| format!("{FOLDER} nicht anlegbar: {e}"))?;
    move_into(video, &target).map_err(|e| format!("nicht verschiebbar: {e}"))?;
    if let Some(old) = own.filter(|old| *old != relative) {
        let old_path = folder.join(&old);
        if let Err(e) = fs::remove_file(&old_path)
            && e.kind() != ErrorKind::NotFound
        {
            tracing::warn!(file = %old_path.display(), error = %e, "old trailer not removed");
        }
    }
    Ok(relative)
}

/// Moves a file; across file systems it is copied under a hidden name next to the target
/// first, so the target only ever appears complete.
fn move_into(from: &Path, to: &Path) -> std::io::Result<()> {
    match fs::rename(from, to) {
        Err(e) if e.kind() == ErrorKind::CrossesDevices => {
            let name = to
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default();
            let partial = to.with_file_name(format!(".{name}.partial"));
            let copied = fs::copy(from, &partial).and_then(|_| fs::rename(&partial, to));
            if copied.is_err() {
                let _ = fs::remove_file(&partial);
            }
            copied?;
            fs::remove_file(from)
        }
        other => other,
    }
}

/// What one daily run did.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RunReport {
    pub downloaded: usize,
    pub failed: usize,
    pub no_candidate: usize,
    pub low_space: usize,
}

/// The daily run (AC-01, AC-05, AC-06, AC-15): eligible items in queue order until the
/// budget is used; an item counts once it was tried with candidates.
pub async fn run(app: &App, now: Timestamp) -> anyhow::Result<RunReport> {
    configured(app).map_err(anyhow::Error::msg)?;
    app.trailers
        .check_tools()
        .await
        .map_err(anyhow::Error::msg)?;
    let mut budget = app.config.trailers.per_day;
    let mut report = RunReport::default();
    for item in queue(app.db.trailer_items().await?, now) {
        if budget == 0 {
            break;
        }
        let folder = PathBuf::from(&item.path);
        // The job never downloads for an item that has a trailer (AC-06).
        if !folder.is_dir() || !trailer_files(&folder).is_empty() {
            continue;
        }
        let Some(_claim) = app.trailers.claim(item.media) else {
            continue;
        };
        match fetch(app, &item, false, now).await? {
            Fetched::Tried(Outcome::Downloaded { .. }) => {
                report.downloaded += 1;
                budget -= 1;
            }
            Fetched::Tried(Outcome::Failed(_)) => {
                report.failed += 1;
                budget -= 1;
            }
            Fetched::Tried(Outcome::NoCandidate) => report.no_candidate += 1,
            Fetched::LowSpace => report.low_space += 1,
            Fetched::TmdbDown(e) => anyhow::bail!("TMDB nicht erreichbar: {e}"),
        }
    }
    tracing::info!(?report, "trailer run done");
    Ok(report)
}

/// When the daily run is next: a day after the last, never in the past.
pub fn next_run(ran_at: Option<Timestamp>, now: Timestamp) -> Timestamp {
    ran_at
        .and_then(|t| t.checked_add(DAY).ok())
        .map_or(now, |t| t.max(now))
}

/// Checks the tools (also for the system page) and starts the daily run where it is on.
pub fn spawn_schedule(app: &SharedApp) -> Option<tokio::task::JoinHandle<()>> {
    if !app.config.trailers.enabled {
        return None;
    }
    let app = Arc::clone(app);
    Some(tokio::spawn(async move {
        if let Err(reason) = app.trailers.check_tools().await {
            tracing::warn!(%reason, "trailers: tools missing");
        }
        if Job::Trailers.interval(&app).is_none() {
            return;
        }
        // Leftovers of a run a crash or restart interrupted.
        let work = work_folder(&app);
        if let Err(e) = fs::remove_dir_all(&work)
            && e.kind() != ErrorKind::NotFound
        {
            tracing::warn!(folder = %work.display(), error = %e, "trailer work folder not cleaned");
        }
        loop {
            let now = Timestamp::now();
            let ran_at = app
                .db
                .setting(RAN_AT)
                .await
                .ok()
                .flatten()
                .and_then(|v| v.as_str().and_then(|s| s.parse().ok()));
            let next = next_run(ran_at, now);
            let wait = std::time::Duration::try_from(next.duration_since(now)).unwrap_or_default();
            app.status.job_waits(Job::Trailers, now, wait);
            tokio::time::sleep(wait).await;
            let at = Timestamp::now();
            let result = run(&app, at).await;
            if let Err(e) = app
                .db
                .put_setting(RAN_AT, &serde_json::Value::String(at.to_string()))
                .await
            {
                tracing::warn!(error = %e, "trailer run time not stored");
            }
            if let Err(e) = &result {
                tracing::warn!(error = format!("{e:#}"), "trailer run failed");
            }
            app.status.job_ran(
                Job::Trailers,
                at,
                &result.map(|_| ()).map_err(|e| format!("{e:#}")),
            );
        }
    }))
}

#[cfg(test)]
mod tests;
