//! Subtitles from OpenSubtitles.com (spec subtitles, D17, D25): which subtitles each file
//! needs, the scheduled and the import-triggered search within the download quota, own
//! subtitles stored by Jellyfin's naming rules and kept with their video.

pub mod api;
mod files;
mod library;
mod needs;

#[cfg(test)]
mod api_tests;
#[cfg(test)]
mod fake;
#[cfg(test)]
mod import_tests;
#[cfg(test)]
mod rename_tests;
#[cfg(test)]
mod schedule_tests;
#[cfg(test)]
mod tests;

use std::{path::PathBuf, sync::Arc};

use anyhow::Context;
use jiff::{SignedDuration, Timestamp};
use sofarr_db::{FileRef, NewOwnSubtitle, SearchOutcome, SubtitleKind, SubtitleNeed};
use sofarr_subtitles::{DownloadError, OpenSubtitles, ProviderError, SearchQuery, Video, Wanted};

pub use api::{
    file_subtitles_episode, file_subtitles_movie, overview, search_episode, search_movie,
};
pub use library::Item;

use crate::{
    app::{App, SharedApp},
    config::{Config, Mode},
    status::Job,
};

const DAY: SignedDuration = SignedDuration::from_hours(24);
const WEEK: SignedDuration = SignedDuration::from_hours(24 * 7);

/// Run state of the subtitle search, shared by the schedule, imports and the API.
#[derive(Debug, Default)]
pub struct SubtitleState {
    /// One run at a time.
    pub lock: tokio::sync::Mutex<()>,
    /// The quota is used up until then (AC-07).
    pub quota_until: std::sync::Mutex<Option<Timestamp>>,
    /// Files imported since the last run (AC-01).
    pub pending: std::sync::Mutex<Vec<FileRef>>,
    /// Wakes the schedule after an import.
    pub wake: tokio::sync::Notify,
}

impl SubtitleState {
    fn quota_until(&self, now: Timestamp) -> Option<Timestamp> {
        self.quota_until
            .lock()
            .ok()
            .and_then(|q| *q)
            .filter(|until| *until > now)
    }

    fn set_quota_until(&self, until: Timestamp) {
        if let Ok(mut slot) = self.quota_until.lock() {
            *slot = Some(until);
        }
    }
}

/// Why subtitles are off (AC-15), in German; `None` when they are on.
pub fn off_reason(config: &Config) -> Option<String> {
    if config.subtitles.enabled == Some(false) {
        return Some("Untertitel sind abgeschaltet ([subtitles] enabled = false)".to_owned());
    }
    if config.opensubtitles.is_none() {
        return Some(
            "Kein OpenSubtitles-Zugang eingetragen ([opensubtitles] fehlt in der Konfiguration)"
                .to_owned(),
        );
    }
    None
}

/// The provider client of a configuration; `None` while subtitles are off.
pub fn client(config: &Config, http: &reqwest::Client) -> Option<OpenSubtitles> {
    if off_reason(config).is_some() {
        return None;
    }
    let account = config.opensubtitles.as_ref()?;
    Some(OpenSubtitles::new(
        http.clone(),
        sofarr_subtitles::ProviderConfig {
            url: account.url.clone(),
            api_key: account.api_key.clone(),
            username: account.username.clone(),
            password: account.password.clone(),
            user_agent: concat!("amigo-sofarr v", env!("CARGO_PKG_VERSION")).to_owned(),
            pause: std::time::Duration::from_millis(account.pause_ms),
        },
    ))
}

/// Which files a run looks at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Every monitored file; the due needs (AC-02).
    All,
    /// The files imported since the last run (AC-01).
    Pending,
    /// One file, monitored or not, every need at once (manual search, AC-11).
    Manual(FileRef),
}

/// What a run did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RunReport {
    pub searched: u32,
    /// File names of the stored subtitles.
    pub downloaded: Vec<String>,
    /// Shadow mode: releases that would have been downloaded (AC-13).
    pub would_download: Vec<String>,
    pub not_found: u32,
    /// Candidates exist, but the quota is used up (AC-07).
    pub waiting_for_quota: u32,
    /// Candidates whose download or conversion failed (AC-14).
    pub failed_candidates: u32,
    /// Needs that turned out to be covered by a file in the target's place (AC-06).
    pub covered: u32,
}

/// Records an imported file: its needs start afresh and the schedule searches them soon
/// (AC-01, AC-10).
pub async fn imported(app: &App, file: FileRef) {
    if app.opensubtitles.is_none() {
        return;
    }
    if let Err(e) = app.db.reset_subtitle_needs(file).await {
        tracing::warn!(error = %e, "subtitle needs not reset");
    }
    if let Ok(mut pending) = app.subtitles.pending.lock()
        && !pending.contains(&file)
    {
        pending.push(file);
    }
    app.subtitles.wake.notify_one();
}

/// When a need without any candidate is searched again (AC-05): a day after the search
/// in the first 7 days after the import, a week after it later.
pub fn next_search(added: Timestamp, now: Timestamp) -> Option<Timestamp> {
    let fresh = added.checked_add(WEEK).is_ok_and(|end| now < end);
    now.checked_add(if fresh { DAY } else { WEEK }).ok()
}

/// The movie hash of a video, if it can be read (a video that cannot is searched
/// without it).
async fn hash_of(video: PathBuf) -> Option<String> {
    tokio::task::spawn_blocking(move || {
        let mut file = std::fs::File::open(video).ok()?;
        sofarr_subtitles::movie_hash(&mut file).ok().flatten()
    })
    .await
    .ok()
    .flatten()
}

/// Determines the needs of the items' files: drops records of own subtitles whose file
/// is gone (the owner deleted them: needed again), then stores what each file needs.
/// Returns the items whose video is there; the others are left alone (a missing video is
/// the disk scan's business, an unreadable folder may be an unmounted share).
async fn refresh_needs(app: &App, items: Vec<Item>) -> anyhow::Result<Vec<Item>> {
    let wanted = app.config.subtitles.languages.clone();
    let mut present = Vec::with_capacity(items.len());
    for item in items {
        let video = item.video.clone();
        let Some(external) = tokio::task::spawn_blocking(move || files::external_subtitles(&video))
            .await
            .context("subtitle folder scan stopped")?
        else {
            continue;
        };
        for own in app.db.own_subtitles(Some(item.file)).await? {
            if !external.iter().any(|e| e.file_name == own.file_name) {
                app.db.remove_own_subtitle(own.id).await?;
            }
        }
        let media = app
            .db
            .media_info(item.file)
            .await?
            .as_ref()
            .and_then(crate::media::parse);
        let needed = needs::needs(&wanted, media.as_ref(), &external);
        app.db.set_subtitle_needs(item.file, &needed).await?;
        present.push(item);
    }
    Ok(present)
}

/// One run over `scope`. A provider failure stops it (subtitles stored so far stay) and
/// is the error (AC-08).
pub async fn run(app: &App, scope: Scope, now: Timestamp) -> anyhow::Result<RunReport> {
    let Some(client) = &app.opensubtitles else {
        return Ok(RunReport::default());
    };
    let _guard = app.subtitles.lock.lock().await;
    let mut items = library::items(app).await?;
    match &scope {
        Scope::All => items.retain(|i| i.monitored),
        Scope::Pending => {
            let pending: Vec<FileRef> = app
                .subtitles
                .pending
                .lock()
                .map(|mut p| std::mem::take(&mut *p))
                .unwrap_or_default();
            items.retain(|i| i.monitored && pending.contains(&i.file));
        }
        Scope::Manual(file) => items.retain(|i| i.file == *file),
    }
    let mut items = refresh_needs(app, items).await?;
    // Newest imported files first (AC-02, AC-07).
    items.sort_by_key(|i| std::cmp::Reverse(i.added_at));
    let mut report = RunReport::default();
    for item in &items {
        let due: Vec<SubtitleNeed> = app
            .db
            .subtitle_needs(Some(item.file))
            .await?
            .into_iter()
            .filter(|n| scope != Scope::All || n.next_search.is_none_or(|t| t <= now))
            .collect();
        if due.is_empty() {
            continue;
        }
        let hash = hash_of(item.video.clone()).await;
        let stored = report.downloaded.len() + report.covered as usize;
        for need in &due {
            search_one(app, client, item, need, hash.clone(), now, &mut report).await?;
        }
        // What a stored subtitle covers is no longer needed.
        if report.downloaded.len() + report.covered as usize > stored {
            refresh_needs(app, vec![item.clone()]).await?;
        }
    }
    Ok(report)
}

/// Searches one need and downloads the best candidate that works.
async fn search_one(
    app: &App,
    client: &OpenSubtitles,
    item: &Item,
    need: &SubtitleNeed,
    moviehash: Option<String>,
    now: Timestamp,
    report: &mut RunReport,
) -> anyhow::Result<()> {
    let forced = need.kind == SubtitleKind::Forced;
    let query = SearchQuery {
        feature: item.feature.clone(),
        moviehash,
        language: need.language.clone(),
        forced,
    };
    let candidates = client.search(&query).await.map_err(stop)?;
    report.searched += 1;
    let ranked = sofarr_subtitles::rank(
        &candidates,
        Wanted {
            language: &need.language,
            forced,
        },
        Video {
            release: &item.release,
            episode: item.episode,
        },
    );
    let mut outcome = SearchOutcome {
        at: now,
        next_search: next_search(item.added_at, now),
        found: !ranked.is_empty(),
        would_download: None,
    };
    if ranked.is_empty() {
        report.not_found += 1;
    } else if app.config.automation.mode == Mode::Shadow {
        let (quality, best) = ranked[0];
        let release = if best.release.is_empty() {
            format!("OpenSubtitles {}", best.subtitle_id)
        } else {
            best.release.clone()
        };
        tracing::info!(file = %item.video.display(), language = %need.language, kind = need.kind.as_str(),
            release = %release, quality = %quality.label(), "would download subtitle (shadow mode)");
        report.would_download.push(release.clone());
        outcome.would_download = Some(release);
        outcome.next_search = None;
    } else if app.subtitles.quota_until(now).is_some() {
        report.waiting_for_quota += 1;
        outcome.next_search = None;
    } else {
        for (quality, candidate) in ranked {
            match fetch(app, client, item, need, quality, candidate, now).await? {
                Fetched::Stored(name) => {
                    report.downloaded.push(name);
                    outcome.next_search = None;
                    break;
                }
                Fetched::Covered => {
                    report.covered += 1;
                    outcome.next_search = None;
                    break;
                }
                Fetched::Quota => {
                    report.waiting_for_quota += 1;
                    outcome.next_search = None;
                    break;
                }
                Fetched::Failed => report.failed_candidates += 1,
            }
        }
    }
    app.db
        .record_subtitle_search(item.file, &need.language, need.kind, &outcome)
        .await?;
    Ok(())
}

enum Fetched {
    Stored(String),
    Covered,
    Quota,
    Failed,
}

/// A provider failure as the error that stops the run (AC-08).
fn stop(e: ProviderError) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

/// Downloads one candidate, converts it to UTF-8 and stores it as an own subtitle.
async fn fetch(
    app: &App,
    client: &OpenSubtitles,
    item: &Item,
    need: &SubtitleNeed,
    quality: sofarr_subtitles::MatchQuality,
    candidate: &sofarr_subtitles::Candidate,
    now: Timestamp,
) -> anyhow::Result<Fetched> {
    let downloaded = match client.download(candidate.file_id).await {
        Ok(d) => d,
        Err(DownloadError::Quota { reset }) => {
            let until = reset.unwrap_or_else(|| now.checked_add(DAY).unwrap_or(now));
            tracing::info!(until = %until, "subtitle download quota used up");
            app.subtitles.set_quota_until(until);
            return Ok(Fetched::Quota);
        }
        Err(DownloadError::Candidate(reason)) => {
            tracing::warn!(subtitle = %candidate.subtitle_id, reason = %reason, "subtitle not downloadable");
            return Ok(Fetched::Failed);
        }
        Err(DownloadError::Provider(e)) => return Err(stop(e)),
    };
    if downloaded.remaining.is_some_and(|r| r <= 0) {
        let until = downloaded
            .reset
            .unwrap_or_else(|| now.checked_add(DAY).unwrap_or(now));
        app.subtitles.set_quota_until(until);
    }
    let text = match sofarr_subtitles::to_utf8(&downloaded.content) {
        Ok(text) => text,
        Err(reason) => {
            tracing::warn!(subtitle = %candidate.subtitle_id, reason = %reason, "subtitle discarded");
            return Ok(Fetched::Failed);
        }
    };
    let Some((_, stem)) = files::video_parts(&item.video) else {
        return Ok(Fetched::Failed);
    };
    let forced = need.kind == SubtitleKind::Forced;
    let name =
        sofarr_subtitles::sidecar_name(&stem, &need.language, forced, candidate.hearing_impaired);
    let video = item.video.clone();
    let target = name.clone();
    let written =
        tokio::task::spawn_blocking(move || files::write_subtitle(&video, &target, &text))
            .await
            .context("subtitle write stopped")?;
    match written {
        Ok(files::Written::Stored) => {}
        Ok(files::Written::Taken) => return Ok(Fetched::Covered),
        Err(e) => {
            tracing::warn!(file = %name, error = %e, "subtitle not written");
            return Ok(Fetched::Failed);
        }
    }
    app.db
        .add_own_subtitle(
            &NewOwnSubtitle {
                file: item.file,
                language: need.language.clone(),
                kind: need.kind,
                hearing_impaired: candidate.hearing_impaired,
                file_name: name.clone(),
                provider_id: candidate.file_id.to_string(),
                release: candidate.release.clone(),
                match_quality: quality.label(),
            },
            now,
        )
        .await?;
    tracing::info!(file = %name, quality = %quality.label(), "subtitle stored");
    Ok(Fetched::Stored(name))
}

/// After a video was renamed or moved: its own subtitles get the matching names in the
/// new place, and the library records the new names (spec subtitles AC-09). A failure is
/// only logged: the rename of the videos goes on.
pub async fn follow_rename(app: &App, file: FileRef, old_video: PathBuf, new_video: PathBuf) {
    if let Err(e) = move_own(app, file, old_video, new_video).await {
        tracing::warn!(
            error = format!("{e:#}"),
            "own subtitles did not follow the video"
        );
    }
}

async fn move_own(
    app: &App,
    file: FileRef,
    old_video: PathBuf,
    new_video: PathBuf,
) -> anyhow::Result<()> {
    let own = app.db.own_subtitles(Some(file)).await?;
    if own.is_empty() {
        return Ok(());
    }
    let names: Vec<String> = own.iter().map(|s| s.file_name.clone()).collect();
    let moved = tokio::task::spawn_blocking(move || {
        sofarr_subtitles::follow_video(&old_video, &new_video, &names)
    })
    .await?;
    for (old, new) in moved {
        if let Some(sub) = own.iter().find(|s| s.file_name == old) {
            app.db.rename_own_subtitle(sub.id, &new).await?;
        }
    }
    Ok(())
}

/// Own subtitles of a video that is replaced or removed go with it (AC-10): into the
/// recycle bin or deleted, and their records go too.
pub async fn remove_with_video(
    app: &App,
    file: FileRef,
    folder: PathBuf,
    recycle_bin: Option<PathBuf>,
) -> anyhow::Result<()> {
    let own = app.db.own_subtitles(Some(file)).await?;
    if own.is_empty() {
        return Ok(());
    }
    let subtitles = own.clone();
    tokio::task::spawn_blocking(move || {
        files::remove_own(&folder, &subtitles, recycle_bin.as_deref())
    })
    .await
    .context("subtitle removal stopped")?
    .context("own subtitles not removable")?;
    for subtitle in own {
        app.db.remove_own_subtitle(subtitle.id).await?;
    }
    Ok(())
}

/// How long the schedule sleeps before its next full run: `interval` after the last one
/// (at once before the first), or until the quota's reset when that comes earlier
/// (AC-07); `None` without an interval and without a used-up quota, when only imports
/// wake it.
fn next_wait(
    app: &App,
    last: Option<tokio::time::Instant>,
    interval: Option<std::time::Duration>,
    now: Timestamp,
) -> Option<std::time::Duration> {
    let by_interval = interval.map(|every| {
        last.map_or(std::time::Duration::ZERO, |l| {
            every.saturating_sub(l.elapsed())
        })
    });
    let by_quota = app
        .subtitles
        .quota_until(now)
        .and_then(|until| std::time::Duration::try_from(until.duration_since(now)).ok());
    match (by_interval, by_quota) {
        (Some(interval), Some(quota)) => Some(interval.min(quota)),
        (interval, quota) => interval.or(quota),
    }
}

/// Starts the subtitle schedule (AC-02, AC-07) and the search after imports (AC-01);
/// `None` while subtitles are off (AC-15).
pub fn spawn_schedule(app: &SharedApp) -> Option<tokio::task::JoinHandle<()>> {
    app.opensubtitles.as_ref()?;
    let interval = Job::Subtitles.interval(app);
    let app = Arc::clone(app);
    Some(tokio::spawn(async move {
        let mut last_full: Option<tokio::time::Instant> = None;
        loop {
            let now = Timestamp::now();
            let wait = next_wait(&app, last_full, interval, now);
            if let Some(wait) = wait {
                app.status.job_waits(Job::Subtitles, now, wait);
            }
            let full = tokio::select! {
                () = async {
                    match wait {
                        Some(wait) => tokio::time::sleep(wait).await,
                        None => std::future::pending::<()>().await,
                    }
                } => true,
                () = app.subtitles.wake.notified() => false,
            };
            let at = Timestamp::now();
            let scope = if full { Scope::All } else { Scope::Pending };
            if full {
                last_full = Some(tokio::time::Instant::now());
            }
            let result = run(&app, scope, at).await.map_err(|e| format!("{e:#}"));
            match &result {
                Ok(report) => tracing::info!(
                    searched = report.searched,
                    downloaded = report.downloaded.len(),
                    "subtitle run done"
                ),
                Err(e) => tracing::warn!(error = %e, "subtitle run stopped"),
            }
            app.status.job_ran(Job::Subtitles, at, &result.map(|_| ()));
        }
    }))
}
