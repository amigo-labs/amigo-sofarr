//! Trailer API (spec trailers AC-09, AC-10, AC-12): the trailer state of a movie or
//! series and the manual download.
//!
//! - `GET /api/movies/{id}/trailer`, `GET /api/series/{id}/trailer` → [`TrailerInfo`]
//! - `POST` on the same paths starts a download at once and answers with the new
//!   [`TrailerInfo`]; a refusal is `409` with a German message.

use std::path::{Path, PathBuf};

use axum::{
    Json,
    extract::{Path as UrlPath, State, rejection::PathRejection},
    http::StatusCode,
};
use jiff::Timestamp;
use serde::Serialize;
use sofarr_db::{MediaRef, TrailerItem, TrailerState};

use super::{Fetched, MAX_ATTEMPTS, VideoId, fetch, language_name, support, trailer_files};
use crate::{
    api::ApiError,
    app::{App, SharedApp},
};

/// Where the trailer of an item stands.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum TrailerStatus {
    /// sofarr's trailer is in place.
    Downloaded,
    /// A trailer sofarr did not write is in place.
    Foreign,
    /// Eligible, waits for the daily run and its budget.
    Waiting,
    /// TMDB listed no candidate.
    NoCandidate,
    /// Every candidate failed (`message` says why).
    Failed,
    /// Too little free space on the item's file system.
    LowSpace,
    /// Not eligible: no video in the library, or a series without TMDB id.
    NotEligible,
}

/// The trailer of a movie or series (AC-10).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct TrailerInfo {
    pub status: TrailerStatus,
    /// The trailers in the item folder, relative to it.
    pub files: Vec<String>,
    /// German name of the language of sofarr's trailer, e.g. `Deutsch` or `Original`.
    pub language: Option<String>,
    /// YouTube page of sofarr's trailer.
    pub link: Option<String>,
    /// Why there is no trailer (German).
    pub message: Option<String>,
    pub attempts: u32,
    /// When the daily run tries again; `null` when it does not (any more).
    pub next_attempt: Option<String>,
    /// Why a manual download is refused (German); `null` when it can be started.
    pub blocked: Option<String>,
}

/// Whether sofarr downloads trailers, for the system page (AC-12).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct TrailerSupport {
    pub active: bool,
    /// Why not (German), e.g. switched off, shadow mode, yt-dlp or ffmpeg missing.
    pub message: Option<String>,
}

pub fn support_answer(app: &App) -> TrailerSupport {
    match support(app) {
        Ok(()) => TrailerSupport {
            active: true,
            message: None,
        },
        Err(message) => TrailerSupport {
            active: false,
            message: Some(message),
        },
    }
}

/// Why a manual download of the item is refused (AC-09, AC-11, AC-12); `files` are the
/// trailers in its folder.
pub fn refusal(app: &App, item: &TrailerItem, files: &[String]) -> Option<String> {
    if let Err(reason) = support(app) {
        return Some(format!("Keine Trailer-Downloads: {reason}"));
    }
    if item.path.is_empty() || !Path::new(&item.path).is_dir() {
        return Some(format!(
            "Der Ordner „{}“ existiert nicht – erst wenn er angelegt ist, gibt es einen Trailer",
            item.path
        ));
    }
    let own = item.trailer.as_ref().and_then(|t| t.file.as_deref());
    if let Some(foreign) = files.iter().find(|f| Some(f.as_str()) != own) {
        return Some(format!(
            "Im Ordner liegt schon ein Trailer, den sofarr nicht geschrieben hat: {foreign}"
        ));
    }
    if item.tmdb_id.is_none() {
        return Some(
            "Die Serie hat noch keine TMDB-ID; die nächste Metadaten-Aktualisierung sucht sie"
                .to_owned(),
        );
    }
    None
}

/// The trailer state of an item from its record and the trailers in its folder.
pub fn describe(item: &TrailerItem, files: Vec<String>, blocked: Option<String>) -> TrailerInfo {
    let record = item.trailer.clone().unwrap_or_default();
    let own = record
        .file
        .as_deref()
        .filter(|f| files.iter().any(|x| x == f));
    let exhausted = record.attempts >= MAX_ATTEMPTS;
    let (status, message) = if own.is_some() {
        (TrailerStatus::Downloaded, None)
    } else if !files.is_empty() {
        (TrailerStatus::Foreign, None)
    } else if !item.has_video {
        (
            TrailerStatus::NotEligible,
            Some("Keine Videodatei in der Bibliothek – ein Trailer nur von Hand".to_owned()),
        )
    } else if item.tmdb_id.is_none() {
        (
            TrailerStatus::NotEligible,
            Some("Die Serie hat noch keine TMDB-ID".to_owned()),
        )
    } else {
        let status = match record.state {
            TrailerState::NoCandidate => TrailerStatus::NoCandidate,
            TrailerState::Failed => TrailerStatus::Failed,
            TrailerState::LowSpace => TrailerStatus::LowSpace,
            TrailerState::Waiting | TrailerState::Downloaded => TrailerStatus::Waiting,
        };
        let text = match status {
            TrailerStatus::NoCandidate => {
                "TMDB listet keinen passenden Trailer auf YouTube".to_owned()
            }
            TrailerStatus::Waiting => {
                "Wartet auf den täglichen Trailer-Lauf und sein Tagesbudget".to_owned()
            }
            _ => record
                .message
                .clone()
                .unwrap_or_else(|| "Download fehlgeschlagen".to_owned()),
        };
        let text = if exhausted && status != TrailerStatus::Waiting {
            format!("{text} – nach {MAX_ATTEMPTS} Versuchen nur noch von Hand")
        } else {
            text
        };
        (status, Some(text))
    };
    let mine = status == TrailerStatus::Downloaded;
    TrailerInfo {
        status,
        language: mine.then(|| language_name(record.language.as_deref())),
        link: record
            .video_key
            .as_deref()
            .and_then(VideoId::parse)
            .filter(|_| mine)
            .map(|id| id.url()),
        message,
        attempts: record.attempts,
        next_attempt: record.next_attempt.filter(|_| !mine).map(|t| t.to_string()),
        blocked,
        files,
    }
}

async fn item(app: &App, media: MediaRef) -> Result<TrailerItem, ApiError> {
    app.db.trailer_item(media).await?.ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            match media {
                MediaRef::Movie(id) => format!("Film {id} nicht gefunden"),
                MediaRef::Series(id) => format!("Serie {id} nicht gefunden"),
            },
        )
    })
}

async fn files_of(item: &TrailerItem) -> Vec<String> {
    let folder = PathBuf::from(&item.path);
    tokio::task::spawn_blocking(move || trailer_files(&folder))
        .await
        .unwrap_or_default()
}

async fn info(app: &App, media: MediaRef) -> Result<Json<TrailerInfo>, ApiError> {
    let item = item(app, media).await?;
    let files = files_of(&item).await;
    let blocked = refusal(app, &item, &files);
    Ok(Json(describe(&item, files, blocked)))
}

/// A manual download (AC-09): at once, outside the daily budget.
async fn download(app: &App, media: MediaRef) -> Result<Json<TrailerInfo>, ApiError> {
    let item = item(app, media).await?;
    if app.trailers.tools().is_none() {
        let _ = app.trailers.check_tools().await;
    }
    let files = files_of(&item).await;
    if let Some(reason) = refusal(app, &item, &files) {
        return Err(ApiError::new(StatusCode::CONFLICT, reason));
    }
    let Some(_claim) = app.trailers.claim(media) else {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "Für diesen Titel läuft schon ein Trailer-Download",
        ));
    };
    if let Fetched::TmdbDown(reason) = fetch(app, &item, true, Timestamp::now()).await? {
        return Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            format!("TMDB nicht erreichbar: {reason}"),
        ));
    }
    info(app, media).await
}

fn bad_id() -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "Ungültige ID")
}

/// `GET /api/movies/{id}/trailer`.
pub async fn movie(
    State(app): State<SharedApp>,
    id: Result<UrlPath<i64>, PathRejection>,
) -> Result<Json<TrailerInfo>, ApiError> {
    let UrlPath(id) = id.map_err(|_| bad_id())?;
    info(&app, MediaRef::Movie(id)).await
}

/// `GET /api/series/{id}/trailer`.
pub async fn series(
    State(app): State<SharedApp>,
    id: Result<UrlPath<i64>, PathRejection>,
) -> Result<Json<TrailerInfo>, ApiError> {
    let UrlPath(id) = id.map_err(|_| bad_id())?;
    info(&app, MediaRef::Series(id)).await
}

/// `POST /api/movies/{id}/trailer`.
pub async fn download_movie(
    State(app): State<SharedApp>,
    id: Result<UrlPath<i64>, PathRejection>,
) -> Result<Json<TrailerInfo>, ApiError> {
    let UrlPath(id) = id.map_err(|_| bad_id())?;
    download(&app, MediaRef::Movie(id)).await
}

/// `POST /api/series/{id}/trailer`.
pub async fn download_series(
    State(app): State<SharedApp>,
    id: Result<UrlPath<i64>, PathRejection>,
) -> Result<Json<TrailerInfo>, ApiError> {
    let UrlPath(id) = id.map_err(|_| bad_id())?;
    download(&app, MediaRef::Series(id)).await
}
