//! The subtitle endpoints (AC-11, AC-12, AC-13, AC-15): the overview of needed and
//! recently found subtitles, the subtitles of one file, and the search for one file.

use axum::{
    Json,
    extract::{Path, State, rejection::PathRejection},
    http::StatusCode,
};
use jiff::{SignedDuration, Timestamp};
use serde::Serialize;
use sofarr_db::{FileRef, SubtitleKind, SubtitleNeed};

use super::{Item, Scope, files, library, needs};
use crate::{api::ApiError, app::SharedApp};

/// What a subtitle belongs to: the library's movie or episode kind.
pub use crate::library_edit::ItemKind;

/// The movie or episode of a subtitle, with the ids for links.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SubtitleItem {
    pub kind: ItemKind,
    pub movie_id: Option<i64>,
    pub series_id: Option<i64>,
    pub episode_id: Option<i64>,
    /// German display name, e.g. `Der Film (2019)` or `Kleine Insel – S01E02`.
    pub title: String,
}

impl SubtitleItem {
    fn of(item: &Item) -> Self {
        Self {
            kind: if item.movie_id.is_some() {
                ItemKind::Movie
            } else {
                ItemKind::Episode
            },
            movie_id: item.movie_id,
            series_id: item.series_id,
            episode_id: item.episode_id,
            title: item.title.clone(),
        }
    }
}

/// A needed subtitle with its schedule.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct NeededSubtitle {
    pub item: SubtitleItem,
    /// ISO 639-1.
    pub language: String,
    /// `normal` or `forced`.
    pub kind: String,
    pub last_search: Option<String>,
    /// `null`: due at the next run.
    pub next_search: Option<String>,
    /// Shadow mode: the release that would have been downloaded (AC-13).
    pub would_download: Option<String>,
}

/// An own subtitle found recently.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct FoundSubtitle {
    pub item: SubtitleItem,
    pub language: String,
    pub kind: String,
    pub hearing_impaired: bool,
    pub file_name: String,
    pub release: String,
    /// `hash`, `release:<n>` (shared release attributes) or `title`.
    pub match_quality: String,
    pub created_at: String,
}

/// `GET /api/subtitles` (AC-11, AC-13, AC-15).
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SubtitleOverview {
    pub enabled: bool,
    /// Why subtitles are off, in German; `null` when they are on.
    pub reason: Option<String>,
    /// Shadow mode: searched, nothing downloaded.
    pub shadow: bool,
    /// The download quota is used up until then.
    pub quota_until: Option<String>,
    pub needed: Vec<NeededSubtitle>,
    /// Own subtitles of the last 7 days, newest first.
    pub recent: Vec<FoundSubtitle>,
}

/// A subtitle track inside the video.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct EmbeddedSubtitle {
    /// ISO 639-1 where the language has one, else ISO 639-2 (`und`: unknown).
    pub language: String,
    pub forced: bool,
    pub hearing_impaired: bool,
    pub format: String,
}

/// A subtitle file next to the video.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ExternalSubtitle {
    pub file_name: String,
    /// ISO 639-1 where the language has one, else ISO 639-2; `null` without a language.
    pub language: Option<String>,
    pub forced: bool,
    pub hearing_impaired: bool,
    /// sofarr wrote it.
    pub own: bool,
    /// For own subtitles: `hash`, `release:<n>` or `title`.
    pub match_quality: Option<String>,
}

/// `GET …/subtitles` of a movie or episode file (AC-12).
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct FileSubtitles {
    pub enabled: bool,
    /// `null` while the file's media info is unknown.
    pub embedded: Option<Vec<EmbeddedSubtitle>>,
    pub external: Vec<ExternalSubtitle>,
    pub needed: Vec<NeededSubtitle>,
}

/// `POST …/subtitles/search` (AC-11).
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SubtitleSearchReport {
    pub searched: u32,
    /// File names of the stored subtitles.
    pub downloaded: Vec<String>,
    /// Shadow mode: releases that would have been downloaded.
    pub would_download: Vec<String>,
    pub not_found: u32,
    pub waiting_for_quota: u32,
    pub failed: u32,
}

fn time(t: Option<Timestamp>) -> Option<String> {
    t.map(|t| t.to_string())
}

fn language_text(code: &str) -> String {
    needs::alpha2(code).unwrap_or_else(|| code.to_owned())
}

fn needed(item: &Item, need: &SubtitleNeed) -> NeededSubtitle {
    NeededSubtitle {
        item: SubtitleItem::of(item),
        language: need.language.clone(),
        kind: need.kind.as_str().to_owned(),
        last_search: time(need.last_search),
        next_search: time(need.next_search),
        would_download: need.would_download.clone(),
    }
}

/// Listed as needed: every need but a forced one the provider has not (AC-05).
fn listed(need: &SubtitleNeed) -> bool {
    !(need.kind == SubtitleKind::Forced && need.found == Some(false))
}

/// `GET /api/subtitles`. Needed subtitles are those of monitored items (Terms): the
/// stored needs of an unmonitored item (a manual search, or monitored before) are not
/// listed, as no scheduled run searches them.
pub async fn overview(State(app): State<SharedApp>) -> Result<Json<SubtitleOverview>, ApiError> {
    let reason = super::off_reason(&app.config);
    let now = Timestamp::now();
    let items = library::items(&app).await?;
    let find = |file: FileRef| items.iter().find(|i| i.file == file);
    let needed = app
        .db
        .subtitle_needs(None)
        .await?
        .iter()
        .filter(|n| listed(n))
        .filter_map(|n| find(n.file).filter(|i| i.monitored).map(|i| needed(i, n)))
        .collect();
    let since = now - SignedDuration::from_hours(24 * 7);
    let recent = app
        .db
        .own_subtitles_since(since)
        .await?
        .into_iter()
        .filter_map(|s| {
            let item = find(s.file)?;
            Some(FoundSubtitle {
                item: SubtitleItem::of(item),
                language: s.language,
                kind: s.kind.as_str().to_owned(),
                hearing_impaired: s.hearing_impaired,
                file_name: s.file_name,
                release: s.release,
                match_quality: s.match_quality,
                created_at: s.created_at.to_string(),
            })
        })
        .collect();
    Ok(Json(SubtitleOverview {
        enabled: reason.is_none(),
        reason,
        shadow: app.config.automation.mode == crate::config::Mode::Shadow,
        quota_until: time(app.subtitles.quota_until(now)),
        needed,
        recent,
    }))
}

fn bad_id() -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "Ungültige ID")
}

async fn movie_item(
    app: &SharedApp,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Item, ApiError> {
    let Path(id) = id.map_err(|_| bad_id())?;
    library::items(app)
        .await?
        .into_iter()
        .find(|i| i.movie_id == Some(id))
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, format!("Film {id} hat keine Datei")))
}

async fn episode_item(
    app: &SharedApp,
    ids: Result<Path<(i64, i64)>, PathRejection>,
) -> Result<Item, ApiError> {
    let Path((series_id, episode_id)) = ids.map_err(|_| bad_id())?;
    let episodes = app.db.episodes(Some(series_id)).await?;
    let file_id = episodes
        .iter()
        .find(|e| e.id == episode_id)
        .and_then(|e| e.file_id)
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                format!("Episode {episode_id} der Serie {series_id} hat keine Datei"),
            )
        })?;
    library::items(app)
        .await?
        .into_iter()
        .find(|i| i.file == FileRef::Episode(file_id))
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                format!("Episode {episode_id} der Serie {series_id} hat keine Datei"),
            )
        })
}

async fn file_subtitles(app: &SharedApp, item: &Item) -> Result<Json<FileSubtitles>, ApiError> {
    let video = item.video.clone();
    let external = tokio::task::spawn_blocking(move || files::external_subtitles(&video))
        .await
        .map_err(anyhow::Error::from)?
        .unwrap_or_default();
    let own = app.db.own_subtitles(Some(item.file)).await?;
    let media = app
        .db
        .media_info(item.file)
        .await?
        .as_ref()
        .and_then(crate::media::parse);
    let embedded = media.map(|m| {
        m.subtitles
            .iter()
            .map(|s| EmbeddedSubtitle {
                language: language_text(&s.language),
                forced: s.forced,
                hearing_impaired: s.hearing_impaired,
                format: s.format.clone(),
            })
            .collect()
    });
    let external = external
        .into_iter()
        .map(|e| {
            let mine = own.iter().find(|o| o.file_name == e.file_name);
            ExternalSubtitle {
                language: e.language.as_deref().map(language_text),
                own: mine.is_some(),
                match_quality: mine.map(|o| o.match_quality.clone()),
                file_name: e.file_name,
                forced: e.forced,
                hearing_impaired: e.hearing_impaired,
            }
        })
        .collect();
    let needed = app
        .db
        .subtitle_needs(Some(item.file))
        .await?
        .iter()
        .filter(|n| listed(n))
        .map(|n| needed(item, n))
        .collect();
    Ok(Json(FileSubtitles {
        enabled: super::off_reason(&app.config).is_none(),
        embedded,
        external,
        needed,
    }))
}

/// `GET /api/movies/{id}/subtitles`.
pub async fn file_subtitles_movie(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Json<FileSubtitles>, ApiError> {
    let item = movie_item(&app, id).await?;
    file_subtitles(&app, &item).await
}

/// `GET /api/series/{id}/episodes/{episode_id}/subtitles`.
pub async fn file_subtitles_episode(
    State(app): State<SharedApp>,
    ids: Result<Path<(i64, i64)>, PathRejection>,
) -> Result<Json<FileSubtitles>, ApiError> {
    let item = episode_item(&app, ids).await?;
    file_subtitles(&app, &item).await
}

async fn search(app: &SharedApp, item: &Item) -> Result<Json<SubtitleSearchReport>, ApiError> {
    if let Some(reason) = super::off_reason(&app.config) {
        return Err(ApiError::new(StatusCode::CONFLICT, reason));
    }
    match super::run(app, Scope::Manual(item.file), Timestamp::now()).await {
        Ok(report) => Ok(Json(SubtitleSearchReport {
            searched: report.searched,
            downloaded: report.downloaded,
            would_download: report.would_download,
            not_found: report.not_found,
            waiting_for_quota: report.waiting_for_quota,
            failed: report.failed_candidates,
        })),
        Err(e) => {
            let message = format!("{e:#}");
            tracing::warn!(error = %message, "subtitle search stopped");
            // Provider failures name the provider and the HTTP status, nothing internal.
            let shown = if message.starts_with("OpenSubtitles") {
                message
            } else {
                "Untertitelsuche fehlgeschlagen – Details stehen im Log".to_owned()
            };
            Err(ApiError::new(StatusCode::BAD_GATEWAY, shown))
        }
    }
}

/// `POST /api/movies/{id}/subtitles/search`.
pub async fn search_movie(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Json<SubtitleSearchReport>, ApiError> {
    let item = movie_item(&app, id).await?;
    search(&app, &item).await
}

/// `POST /api/series/{id}/episodes/{episode_id}/subtitles/search`.
pub async fn search_episode(
    State(app): State<SharedApp>,
    ids: Result<Path<(i64, i64)>, PathRejection>,
) -> Result<Json<SubtitleSearchReport>, ApiError> {
    let item = episode_item(&app, ids).await?;
    search(&app, &item).await
}
