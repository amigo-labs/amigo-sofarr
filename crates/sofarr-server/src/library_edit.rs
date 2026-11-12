//! Changes from the library pages that only touch the library (spec web-ui-library
//! AC-10, AC-11): monitoring (also in bulk, all or nothing), quality profile and minimum
//! availability with a preview of what becomes missing or cutoff unmet.

use axum::{
    Json,
    extract::{
        Path, State,
        rejection::{JsonRejection, PathRejection},
    },
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use sofarr_db::{Episode, MonitorTarget, Movie, Series};

use crate::{
    api::{ApiError, bad_id},
    app::SharedApp,
    library_detail::{
        MovieDetail, SeriesDetail, find_movie, find_series, movie_detail, series_detail,
    },
    library_list::{movie_judge, series_judge},
    library_view::Judge,
};

/// One item whose monitoring is switched.
#[derive(Debug, Clone, Copy, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MonitorItem {
    Movie {
        id: i64,
    },
    /// The series and its episodes.
    Series {
        id: i64,
    },
    /// The episodes of a season.
    Season {
        series_id: i64,
        season: u16,
    },
    Episode {
        id: i64,
    },
}

/// Body of `POST /api/library/monitor`.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct MonitorBody {
    pub monitored: bool,
    pub items: Vec<MonitorItem>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct MonitorAnswer {
    /// Items switched (all of the request).
    pub changed: u32,
}

const BAD_MONITOR: &str = "Ungültige Anfrage: monitored (true/false) und mindestens ein Eintrag in \
     items (kind movie, series, season oder episode) angeben";

impl MonitorItem {
    fn target(self) -> MonitorTarget {
        match self {
            Self::Movie { id } => MonitorTarget::Movie(id),
            Self::Series { id } => MonitorTarget::Series(id),
            Self::Season { series_id, season } => MonitorTarget::Season { series_id, season },
            Self::Episode { id } => MonitorTarget::Episode(id),
        }
    }
}

fn unknown(target: MonitorTarget) -> String {
    let what = match target {
        MonitorTarget::Movie(id) => format!("Film {id}"),
        MonitorTarget::Series(id) => format!("Serie {id}"),
        MonitorTarget::Season { series_id, season } => {
            format!("Staffel {season} der Serie {series_id}")
        }
        MonitorTarget::Episode(id) => format!("Episode {id}"),
    };
    format!("{what} ist nicht in der Bibliothek – nichts wurde geändert")
}

pub async fn monitor(
    State(app): State<SharedApp>,
    body: Result<Json<MonitorBody>, JsonRejection>,
) -> Result<Json<MonitorAnswer>, ApiError> {
    let Json(body) = body
        .ok()
        .filter(|Json(b)| !b.items.is_empty())
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, BAD_MONITOR))?;
    let targets: Vec<MonitorTarget> = body.items.iter().map(|i| i.target()).collect();
    match app.db.set_monitored_all(&targets, body.monitored).await? {
        Some(missing) => Err(ApiError::new(StatusCode::NOT_FOUND, unknown(missing))),
        None => Ok(Json(MonitorAnswer {
            changed: u32::try_from(targets.len()).unwrap_or(u32::MAX),
        })),
    }
}

/// Body of `PATCH /api/library/movies/{id}` and its preview.
#[derive(Debug, Default, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct MovieChange {
    pub quality_profile_id: Option<i64>,
    /// `announced`, `inCinemas` or `released`.
    pub minimum_availability: Option<String>,
}

/// Body of `PATCH /api/library/series/{id}` and its preview.
#[derive(Debug, Default, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct SeriesChange {
    pub quality_profile_id: Option<i64>,
}

/// A movie or episode in a preview or a wanted list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Movie,
    Episode,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PreviewItem {
    pub kind: ItemKind,
    pub id: i64,
    /// E.g. `Zweiter Film (2002)` or `Kleine Insel S01E02 – Sturm`.
    pub label: String,
}

/// What a change would do to the wanted lists.
#[derive(Debug, Default, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ChangePreview {
    pub becomes_missing: Vec<PreviewItem>,
    pub becomes_cutoff_unmet: Vec<PreviewItem>,
    pub no_longer_missing: Vec<PreviewItem>,
    pub no_longer_cutoff_unmet: Vec<PreviewItem>,
}

impl ChangePreview {
    fn add(&mut self, item: impl Fn() -> PreviewItem, before: (bool, bool), after: (bool, bool)) {
        match (before.0, after.0) {
            (false, true) => self.becomes_missing.push(item()),
            (true, false) => self.no_longer_missing.push(item()),
            _ => {}
        }
        match (before.1, after.1) {
            (false, true) => self.becomes_cutoff_unmet.push(item()),
            (true, false) => self.no_longer_cutoff_unmet.push(item()),
            _ => {}
        }
    }
}

const AVAILABILITY: [&str; 3] = ["announced", "inCinemas", "released"];

const BAD_CHANGE: &str = "Ungültige Änderung: quality_profile_id (ein vorhandenes Profil) und/oder \
     minimum_availability (announced, inCinemas, released) angeben";

pub fn movie_label(m: &Movie) -> String {
    match m.year {
        Some(y) => format!("{} ({y})", m.title),
        None => m.title.clone(),
    }
}

pub fn episode_label(show: &Series, e: &Episode) -> String {
    let code = format!(
        "{} S{:02}E{:02}",
        show.data.title, e.data.season, e.data.number
    );
    match &e.data.title {
        Some(t) => format!("{code} – {t}"),
        None => code,
    }
}

fn checked_profile(judge: &Judge, id: Option<i64>) -> Result<Option<i64>, ApiError> {
    match id {
        Some(id) if judge.profile(id).is_none() => {
            Err(ApiError::new(StatusCode::BAD_REQUEST, BAD_CHANGE))
        }
        other => Ok(other),
    }
}

fn movie_change(
    judge: &Judge,
    body: Result<Json<MovieChange>, JsonRejection>,
) -> Result<MovieChange, ApiError> {
    let bad = || ApiError::new(StatusCode::BAD_REQUEST, BAD_CHANGE);
    let Json(change) = body.map_err(|_| bad())?;
    if change.quality_profile_id.is_none() && change.minimum_availability.is_none() {
        return Err(bad());
    }
    if change
        .minimum_availability
        .as_deref()
        .is_some_and(|a| !AVAILABILITY.contains(&a))
    {
        return Err(bad());
    }
    checked_profile(judge, change.quality_profile_id)?;
    Ok(change)
}

fn series_change(
    judge: &Judge,
    body: Result<Json<SeriesChange>, JsonRejection>,
) -> Result<i64, ApiError> {
    let Json(change) = body.map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, BAD_CHANGE))?;
    checked_profile(judge, change.quality_profile_id)?
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, BAD_CHANGE))
}

fn changed_movie(movie: &Movie, change: &MovieChange) -> Movie {
    let mut after = movie.clone();
    if let Some(p) = change.quality_profile_id {
        after.quality_profile_id = p;
    }
    if let Some(a) = &change.minimum_availability {
        after.availability.minimum.clone_from(a);
    }
    after
}

pub async fn movie_preview(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
    body: Result<Json<MovieChange>, JsonRejection>,
) -> Result<Json<ChangePreview>, ApiError> {
    let Path(id) = id.map_err(|_| bad_id())?;
    let judge = movie_judge(&app).await;
    let change = movie_change(&judge, body)?;
    let movie = find_movie(&app, id).await?;
    let before = judge.movie(&movie);
    let after = judge.movie(&changed_movie(&movie, &change));
    let mut preview = ChangePreview::default();
    preview.add(
        || PreviewItem {
            kind: ItemKind::Movie,
            id: movie.id,
            label: movie_label(&movie),
        },
        (before.missing, before.cutoff_unmet),
        (after.missing, after.cutoff_unmet),
    );
    Ok(Json(preview))
}

pub async fn change_movie(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
    body: Result<Json<MovieChange>, JsonRejection>,
) -> Result<Json<MovieDetail>, ApiError> {
    let Path(id) = id.map_err(|_| bad_id())?;
    let judge = movie_judge(&app).await;
    let change = movie_change(&judge, body)?;
    find_movie(&app, id).await?;
    app.db
        .set_movie_settings(
            id,
            change.quality_profile_id,
            change.minimum_availability.as_deref(),
        )
        .await?;
    let movie = find_movie(&app, id).await?;
    Ok(Json(movie_detail(&app, movie).await?))
}

pub async fn series_preview(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
    body: Result<Json<SeriesChange>, JsonRejection>,
) -> Result<Json<ChangePreview>, ApiError> {
    let Path(id) = id.map_err(|_| bad_id())?;
    let judge = series_judge(&app).await;
    let profile = series_change(&judge, body)?;
    let show = find_series(&app, id).await?;
    let mut changed = show.clone();
    changed.data.quality_profile_id = profile;
    let today = judge.today_text();
    let mut preview = ChangePreview::default();
    for e in app.db.episodes(Some(id)).await? {
        let before = judge.episode(&show, &e, &today);
        let after = judge.episode(&changed, &e, &today);
        preview.add(
            || PreviewItem {
                kind: ItemKind::Episode,
                id: e.id,
                label: episode_label(&show, &e),
            },
            (before.missing, before.cutoff_unmet),
            (after.missing, after.cutoff_unmet),
        );
    }
    Ok(Json(preview))
}

pub async fn change_series(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
    body: Result<Json<SeriesChange>, JsonRejection>,
) -> Result<Json<SeriesDetail>, ApiError> {
    let Path(id) = id.map_err(|_| bad_id())?;
    let judge = series_judge(&app).await;
    let profile = series_change(&judge, body)?;
    find_series(&app, id).await?;
    app.db.set_series_profile(id, profile).await?;
    let show = find_series(&app, id).await?;
    Ok(Json(series_detail(&app, show).await?))
}
