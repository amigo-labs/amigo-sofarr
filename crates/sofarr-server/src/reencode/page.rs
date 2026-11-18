//! The page "Neu encodieren" (spec ui-reencode): one answer with the job, the candidates
//! and the history, and the exclusions from the background job.

use std::collections::HashSet;

use axum::{Json, extract::State, http::StatusCode};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use sofarr_db::{Db, FileRef};

use super::{
    job::{self, FileState, ReencodeSupport},
    run::{self, Running},
};
use crate::{
    api::ApiError,
    app::{App, SharedApp},
    config::Mode,
};

#[cfg(test)]
mod tests;

/// How many attempts the history shows.
pub const HISTORY: u32 = 50;

/// A candidate with its job state.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PageCandidate {
    /// `movie` or `episode`.
    pub kind: String,
    pub movie_id: Option<i64>,
    pub series_id: Option<i64>,
    pub episode_ids: Vec<i64>,
    pub title: String,
    pub path: String,
    /// Bytes.
    pub size: u64,
    pub mb_per_minute: f64,
    pub threshold: f64,
    pub resolution: String,
    pub video_codec: String,
    /// `running`, `excluded`, `done`, `no_gain`, `rests_until`, `rests_until_change`, `due`.
    pub state: String,
    /// When a resting file is due again (`rests_until`).
    pub rests_until: Option<String>,
}

/// An attempt in the history.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct HistoryEntry {
    pub title: String,
    pub started_at: String,
    /// `server` or `cli`.
    pub runner: String,
    /// `replaced`, `no_gain`, `failed`, `refused`; `null` while it runs.
    pub outcome: Option<String>,
    pub reason: Option<String>,
    pub size_before: u64,
    pub size_after: Option<u64>,
}

/// The excluded library files of `reencode-rule`, by reason.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RuleExclusions {
    pub no_media_info: u32,
    pub resolution_off: u32,
    pub unknown_duration: u32,
    pub dynamic_metadata: u32,
}

/// `GET /api/reencode`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ReencodePage {
    pub job: ReencodeSupport,
    pub running: Option<Running>,
    /// Files replaced and bytes saved in total.
    pub saved_files: u64,
    pub saved_bytes: u64,
    pub count: u32,
    /// Bytes of all candidates.
    pub total_size: u64,
    pub excluded: RuleExclusions,
    pub candidates: Vec<PageCandidate>,
    pub history: Vec<HistoryEntry>,
    /// Shadow mode: the page may change nothing.
    pub shadow: bool,
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The answer of the page.
pub async fn answer(app: &App, now: Timestamp) -> anyhow::Result<ReencodePage> {
    let thresholds = app.config.reencode.thresholds();
    let list = super::candidates(&app.db, &thresholds).await?;
    let attempts = app.db.all_reencodes().await?;
    let by_file = job::by_file(&attempts);
    let excluded: HashSet<FileRef> = app.db.reencode_exclusions().await?.into_iter().collect();
    let running = run::running(&app.reencode.lock);
    let candidates = list
        .candidates
        .iter()
        .map(|c| {
            let (state, until) = if running.as_ref().is_some_and(|r| r.path == c.path) {
                ("running", None)
            } else if excluded.contains(&c.file) {
                ("excluded", None)
            } else {
                let attempts = by_file.get(&c.file).map_or(&[][..], Vec::as_slice);
                match job::state_of(c.size, attempts, now) {
                    FileState::Done => ("done", None),
                    FileState::NoGain => ("no_gain", None),
                    FileState::RestsUntil(at) => ("rests_until", Some(at.to_string())),
                    FileState::RestsUntilChange => ("rests_until_change", None),
                    FileState::Due => ("due", None),
                }
            };
            PageCandidate {
                kind: c.kind.to_owned(),
                movie_id: c.movie_id,
                series_id: c.series_id,
                episode_ids: c.episode_ids.clone(),
                title: c.title.clone(),
                path: c.path.clone(),
                size: c.size,
                mb_per_minute: c.mb_per_minute,
                threshold: c.threshold,
                resolution: c.resolution.to_owned(),
                video_codec: c.video_codec.clone(),
                state: state.to_owned(),
                rests_until: until,
            }
        })
        .collect();
    let (saved_files, saved_bytes) = app.db.reencode_savings().await?;
    let history = app
        .db
        .recent_reencodes(HISTORY)
        .await?
        .into_iter()
        .map(|a| HistoryEntry {
            title: a.title,
            started_at: a.started_at.to_string(),
            runner: a.runner,
            outcome: a.outcome.map(|o| o.as_str().to_owned()),
            reason: a.reason,
            size_before: a.size_before,
            size_after: a.size_after,
        })
        .collect();
    Ok(ReencodePage {
        job: job::support_answer(app),
        running,
        saved_files,
        saved_bytes,
        count: count(list.count),
        total_size: list.total_size,
        excluded: RuleExclusions {
            no_media_info: count(list.excluded.no_media_info),
            resolution_off: count(list.excluded.resolution_off),
            unknown_duration: count(list.excluded.unknown_duration),
            dynamic_metadata: count(list.excluded.dynamic_metadata),
        },
        candidates,
        history,
        shadow: app.config.automation.mode == Mode::Shadow,
    })
}

/// `GET /api/reencode`.
pub async fn get(State(app): State<SharedApp>) -> Result<Json<ReencodePage>, ApiError> {
    Ok(Json(answer(&app, Timestamp::now()).await?))
}

/// `POST` and `DELETE /api/reencode/exclusions`: exactly one of the two ids; any
/// episode of a file names the file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExclusionRequest {
    pub movie_id: Option<i64>,
    pub episode_id: Option<i64>,
}

async fn file_of(db: &Db, request: &ExclusionRequest) -> Result<FileRef, ApiError> {
    let not_found = || {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "Kein Film und keine Episode mit Datei unter dieser ID",
        )
    };
    match (request.movie_id, request.episode_id) {
        (Some(id), None) => match db.movie(id).await? {
            Some(movie) if movie.file.is_some() => Ok(FileRef::Movie(id)),
            _ => Err(not_found()),
        },
        (None, Some(id)) => db
            .episodes(None)
            .await?
            .into_iter()
            .find(|e| e.id == id)
            .and_then(|e| e.file_id)
            .map(FileRef::Episode)
            .ok_or_else(not_found),
        _ => Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Genau eine von movie_id und episode_id angeben",
        )),
    }
}

async fn change(
    app: &App,
    request: &ExclusionRequest,
    exclude: bool,
) -> Result<StatusCode, ApiError> {
    if app.config.automation.mode == Mode::Shadow {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "Ausschluss nicht geändert: im Shadow-Mode",
        ));
    }
    let file = file_of(&app.db, request).await?;
    if exclude {
        app.db.exclude_from_reencode(file, Timestamp::now()).await?;
    } else {
        app.db.include_in_reencode(file).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/reencode/exclusions`: keeps a file out of the background job.
pub async fn exclude(
    State(app): State<SharedApp>,
    Json(request): Json<ExclusionRequest>,
) -> Result<StatusCode, ApiError> {
    change(&app, &request, true).await
}

/// `DELETE /api/reencode/exclusions`: takes it back.
pub async fn include(
    State(app): State<SharedApp>,
    Json(request): Json<ExclusionRequest>,
) -> Result<StatusCode, ApiError> {
    change(&app, &request, false).await
}
