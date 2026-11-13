//! Title previews (spec discover AC-04) and the cast of library titles (AC-06).

use axum::{
    Json,
    extract::{Path, State, rejection::PathRejection},
    http::StatusCode,
};
use jiff::Timestamp;
use serde::Serialize;
use sofarr_metadata::browse::{Kind, TitleDetail};

use super::{
    Failure,
    cache::{Cached, Freshness},
    cards::{Cards, LibraryState, Rating, TitleKind, image},
    failure, tmdb_client,
};
use crate::{
    api::ApiError,
    app::{App, SharedApp},
    library_detail::{find_movie, find_series},
};

/// Library pages show this many cast members (AC-06).
pub const LIBRARY_CAST: usize = 10;
/// A preview shows this many cast members.
const PREVIEW_CAST: usize = 20;
/// Crew jobs a preview names.
const PREVIEW_JOBS: &[&str] = &[
    "Director",
    "Creator",
    "Screenplay",
    "Writer",
    "Novel",
    "Story",
    "Producer",
    "Original Music Composer",
    "Director of Photography",
    "Editor",
];

pub fn title_key(kind: Kind, tmdb_id: i64) -> String {
    format!("{}/{tmdb_id}", kind.path())
}

/// A title's detail from the cache (24 hours) or TMDB.
pub async fn detail(
    app: &App,
    kind: Kind,
    tmdb_id: i64,
    now: Timestamp,
) -> Result<Cached<TitleDetail>, ApiError> {
    let tmdb = tmdb_client(app)?;
    let key = title_key(kind, tmdb_id);
    let loaded = app
        .discover
        .titles
        .load(&key, now, || tmdb.title(kind, tmdb_id))
        .await;
    if loaded.as_ref().err() == Some(&Failure::NotFound)
        && let Ok(mut unknown) = app.discover.unknown.lock()
    {
        unknown.insert(key);
    }
    loaded.map_err(|f| failure(f, "TMDB kennt diesen Titel nicht"))
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct CastCard {
    pub person_id: i64,
    pub name: String,
    pub character: Option<String>,
    /// sofarr's image URL of the photo (D24).
    pub photo: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct CrewCard {
    pub person_id: i64,
    pub name: String,
    pub job: String,
    pub department: String,
    pub photo: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SeasonInfo {
    pub number: u16,
    pub name: String,
    pub episode_count: u32,
    pub air_date: Option<String>,
}

/// `GET /api/discover/titles/{kind}/{tmdb_id}`: the preview of a title; one in the
/// library carries its library id, and the UI opens its library page (AC-04).
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct TitlePreview {
    pub kind: TitleKind,
    pub tmdb_id: i64,
    pub title: String,
    pub original_title: String,
    pub year: Option<u16>,
    /// First release or first air date, `YYYY-MM-DD`.
    pub date: Option<String>,
    pub overview: Option<String>,
    pub poster: Option<String>,
    pub backdrop: Option<String>,
    pub genres: Vec<String>,
    pub runtime_minutes: Option<u32>,
    /// Series only.
    pub seasons: Vec<SeasonInfo>,
    /// TMDB's status, e.g. `Released`, `Returning Series`.
    pub status: Option<String>,
    pub imdb_id: Option<String>,
    pub rating: Rating,
    pub cast: Vec<CastCard>,
    pub crew: Vec<CrewCard>,
    /// YouTube link of the trailer.
    pub trailer: Option<String>,
    pub library: LibraryState,
    pub freshness: Freshness,
}

fn cast(app: &App, detail: &TitleDetail, n: usize) -> Vec<CastCard> {
    detail
        .cast
        .iter()
        .take(n)
        .map(|c| CastCard {
            person_id: c.person_id,
            name: c.name.clone(),
            character: c.character.clone(),
            photo: image(app, c.profile_path.as_deref(), "w185"),
        })
        .collect()
}

fn crew(app: &App, detail: &TitleDetail) -> Vec<CrewCard> {
    let mut out: Vec<CrewCard> = Vec::new();
    for job in PREVIEW_JOBS {
        for c in detail.crew.iter().filter(|c| c.job == *job) {
            if !out
                .iter()
                .any(|o| o.person_id == c.person_id && o.job == c.job)
            {
                out.push(CrewCard {
                    person_id: c.person_id,
                    name: c.name.clone(),
                    job: c.job.clone(),
                    department: c.department.clone(),
                    photo: image(app, c.profile_path.as_deref(), "w185"),
                });
            }
        }
    }
    out
}

pub fn kind_and_id(
    path: Result<Path<(String, i64)>, PathRejection>,
) -> Result<(TitleKind, i64), ApiError> {
    path.ok()
        .and_then(|Path((kind, id))| Some((TitleKind::parse(&kind)?, id)))
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "Ungültiger Titel: movie oder series und eine TMDB-ID angeben",
            )
        })
}

pub async fn preview(
    State(app): State<SharedApp>,
    path: Result<Path<(String, i64)>, PathRejection>,
) -> Result<Json<TitlePreview>, ApiError> {
    let (kind, tmdb_id) = kind_and_id(path)?;
    let now = Timestamp::now();
    let cached = detail(&app, kind.tmdb(), tmdb_id, now).await?;
    let d = cached.value.as_ref();
    let mut cards = Cards::new(&app).await?;
    let (library, rating) = cards.state(kind.tmdb(), tmdb_id).await?;
    cards.finish(&app);
    Ok(Json(TitlePreview {
        kind,
        tmdb_id,
        title: d.summary.title.clone(),
        original_title: d.summary.original_title.clone(),
        year: d.summary.year,
        date: d.summary.date.clone(),
        overview: d.overview.clone(),
        poster: image(&app, d.summary.poster_path.as_deref(), "w780"),
        backdrop: image(&app, d.backdrop_path.as_deref(), "w1280"),
        genres: d.genres.clone(),
        runtime_minutes: d.runtime_minutes,
        seasons: d
            .seasons
            .iter()
            .map(|s| SeasonInfo {
                number: s.number,
                name: s.name.clone(),
                episode_count: s.episode_count,
                air_date: s.air_date.clone(),
            })
            .collect(),
        status: d.status.clone(),
        imdb_id: d.imdb_id.clone(),
        rating,
        cast: cast(&app, d, PREVIEW_CAST),
        crew: crew(&app, d),
        trailer: d.trailer.clone(),
        library,
        freshness: Freshness::one(&cached, now),
    }))
}

/// The cast of a library movie or series (AC-06); a series without TMDB id has none.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct CastAnswer {
    /// The first 10 billed cast members.
    pub cast: Vec<CastCard>,
    /// `null` when there is no TMDB data for the title.
    pub freshness: Option<Freshness>,
}

async fn library_cast(app: &App, kind: Kind, tmdb_id: Option<i64>) -> Result<CastAnswer, ApiError> {
    let Some(tmdb_id) = tmdb_id.filter(|id| *id > 0) else {
        return Ok(CastAnswer {
            cast: Vec::new(),
            freshness: None,
        });
    };
    let now = Timestamp::now();
    let cached = detail(app, kind, tmdb_id, now).await?;
    Ok(CastAnswer {
        cast: cast(app, &cached.value, LIBRARY_CAST),
        freshness: Some(Freshness::one(&cached, now)),
    })
}

fn library_id(id: Result<Path<i64>, PathRejection>) -> Result<i64, ApiError> {
    id.map(|Path(id)| id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Ungültige ID"))
}

/// `GET /api/library/movies/{id}/cast`.
pub async fn movie_cast(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Json<CastAnswer>, ApiError> {
    let movie = find_movie(&app, library_id(id)?).await?;
    Ok(Json(
        library_cast(&app, Kind::Movie, Some(movie.tmdb_id)).await?,
    ))
}

/// `GET /api/library/series/{id}/cast`.
pub async fn series_cast(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Json<CastAnswer>, ApiError> {
    let show = find_series(&app, library_id(id)?).await?;
    let tmdb_id = app.db.series_tmdb_id(show.id).await?;
    Ok(Json(library_cast(&app, Kind::Series, tmdb_id).await?))
}
