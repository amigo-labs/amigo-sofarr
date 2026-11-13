//! The movie, series, season and episode pages (spec web-ui-library AC-05, AC-06, AC-15,
//! AC-17): everything one item shows, and whether file actions can run for it.

use axum::{
    Json,
    extract::{Path, State, rejection::PathRejection},
    http::StatusCode,
};
use serde::Serialize;
use serde_json::Value;
use sofarr_db::{
    BlockEntry, Episode, FileRef, Grab, MediaRef, Movie, MovieFile, Series, SeriesGrab,
};

use crate::{
    api::{ApiError, bad_id},
    app::{App, SharedApp},
    config::Mode,
    folders,
    library_list::{Choice, movie_judge, series_judge},
    library_view::{Judge, ReleaseStage},
    media::MediaInfoAnswer,
    series,
};

/// Whether file actions (rename, move folder, remove with files) can run now.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct FileActions {
    pub available: bool,
    /// Why not, in German.
    pub reason: Option<String>,
}

pub const NEEDS_LIVE: &str = "Dateiaktionen (Umbenennen, Ordner verschieben, Dateien löschen) \
     gibt es nur im Live-Modus ([automation] mode = \"live\")";

pub const MOVIE_BUSY: &str =
    "Für diesen Film läuft ein Download oder Import – Dateiaktionen sind erst danach möglich";
pub const SERIES_BUSY: &str =
    "Für diese Serie läuft ein Download oder Import – Dateiaktionen sind erst danach möglich";

impl FileActions {
    pub fn of(app: &App, busy: bool, busy_reason: &str) -> Self {
        let reason = if app.config.automation.mode == Mode::Shadow {
            Some(NEEDS_LIVE.to_owned())
        } else if busy {
            Some(busy_reason.to_owned())
        } else {
            None
        };
        Self {
            available: reason.is_none(),
            reason,
        }
    }
}

/// The folder an item has and the one its naming format gives it today.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct FolderState {
    pub current: String,
    /// `null` when no naming format is imported.
    pub expected: Option<String>,
    pub differs: bool,
}

impl FolderState {
    pub fn new(current: &str, expected: Option<String>) -> Self {
        let differs = expected
            .as_deref()
            .is_some_and(|e| !folders::same_path(e, current));
        Self {
            current: current.to_owned(),
            expected,
            differs,
        }
    }
}

/// A library file with its details.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct FileDetail {
    /// Relative to the item's folder, `/`-separated.
    pub relative_path: String,
    pub size: u64,
    pub quality: String,
    pub release_group: Option<String>,
    pub scene_name: Option<String>,
    pub custom_format_score: i32,
    /// Read from the file (spec mediainfo); `null` when not read yet.
    #[cfg_attr(test, ts(type = "Record<string, unknown> | null"))]
    pub media_info: Option<MediaInfoAnswer>,
}

/// A grab of the item.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct GrabEntry {
    pub id: i64,
    pub title: String,
    pub indexer: String,
    pub quality: String,
    pub size: u64,
    pub status: String,
    pub message: Option<String>,
    pub grabbed_at: String,
    /// Waits for the download client or the import (can be marked as failed).
    pub active: bool,
    /// Series grabs: the episodes covered.
    pub episode_ids: Vec<i64>,
}

/// A blocklist entry of the item.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct BlockedRelease {
    pub id: i64,
    pub title: String,
    pub indexer: String,
    pub reason: String,
    pub blocklisted_at: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ReleaseDates {
    pub in_cinemas: Option<String>,
    pub digital: Option<String>,
    pub physical: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct MovieDetail {
    pub id: i64,
    pub tmdb_id: i64,
    pub imdb_id: Option<String>,
    /// IMDb rating and votes; `null` without one (spec imdb-ratings AC-04).
    pub imdb_rating: Option<f32>,
    pub imdb_votes: Option<u32>,
    pub title: String,
    pub original_title: Option<String>,
    pub title_de: Option<String>,
    pub alternative_titles: Vec<String>,
    pub year: Option<u16>,
    pub runtime_minutes: u32,
    pub overview: Option<String>,
    pub has_poster: bool,
    pub monitored: bool,
    pub release_dates: ReleaseDates,
    pub stage: ReleaseStage,
    /// Counts as available by its minimum availability today.
    pub available: bool,
    /// `announced`, `inCinemas` or `released`.
    pub minimum_availability: String,
    pub quality_profile: Choice,
    pub tags: Vec<Choice>,
    pub folder: FolderState,
    pub file: Option<FileDetail>,
    pub missing: bool,
    pub cutoff_unmet: bool,
    /// Why the file misses the cutoff, e.g. `WEBDL-1080p statt Bluray-1080p`.
    pub cutoff_reason: Option<String>,
    pub grabs: Vec<GrabEntry>,
    pub blocklist: Vec<BlockedRelease>,
    pub file_actions: FileActions,
    pub added_at: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct EpisodeAnswer {
    pub id: i64,
    pub season: u16,
    pub number: u16,
    pub absolute: Option<u16>,
    pub title: Option<String>,
    pub title_de: Option<String>,
    pub air_date: Option<String>,
    pub monitored: bool,
    pub has_file: bool,
    pub quality: Option<String>,
    pub size: Option<u64>,
    pub relative_path: Option<String>,
    pub missing: bool,
    pub cutoff_unmet: bool,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SeasonAnswer {
    pub number: u16,
    /// Every episode of the season is monitored.
    pub monitored: bool,
    pub episodes_total: u32,
    pub episodes_with_file: u32,
    pub episodes_monitored: u32,
    pub missing: u32,
    pub cutoff_unmet: u32,
    pub episodes: Vec<EpisodeAnswer>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SeriesDetail {
    pub id: i64,
    pub tvdb_id: i64,
    pub imdb_id: Option<String>,
    pub imdb_rating: Option<f32>,
    pub imdb_votes: Option<u32>,
    pub title: String,
    pub title_de: Option<String>,
    pub alternative_titles: Vec<String>,
    pub year: Option<u16>,
    pub overview: Option<String>,
    pub series_type: String,
    pub has_poster: bool,
    pub monitored: bool,
    pub quality_profile: Choice,
    pub tags: Vec<Choice>,
    pub folder: FolderState,
    pub episodes: u32,
    pub monitored_episodes: u32,
    pub files: u32,
    pub missing: u32,
    pub cutoff_unmet: u32,
    pub complete: bool,
    pub size: u64,
    pub seasons: Vec<SeasonAnswer>,
    pub grabs: Vec<GrabEntry>,
    pub blocklist: Vec<BlockedRelease>,
    pub file_actions: FileActions,
    pub added_at: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct EpisodeDetail {
    pub series_id: i64,
    pub series_title: String,
    pub episode: EpisodeAnswer,
    pub file: Option<FileDetail>,
    pub cutoff_reason: Option<String>,
}

fn not_found(what: &str) -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, what)
}

fn movie_grab(g: Grab) -> GrabEntry {
    GrabEntry {
        id: g.id,
        title: g.title,
        indexer: g.indexer,
        quality: g.quality,
        size: g.size,
        status: g.status.as_str().to_owned(),
        message: g.message,
        grabbed_at: g.grabbed_at.to_string(),
        active: g.status.is_active(),
        episode_ids: Vec::new(),
    }
}

fn series_grab(g: SeriesGrab) -> GrabEntry {
    GrabEntry {
        id: g.id,
        title: g.title,
        indexer: g.indexer,
        quality: g.quality,
        size: g.size,
        status: g.status.as_str().to_owned(),
        message: g.message,
        grabbed_at: g.grabbed_at.to_string(),
        active: g.status.is_active(),
        episode_ids: g.episode_ids,
    }
}

fn blocked(b: BlockEntry) -> BlockedRelease {
    BlockedRelease {
        id: b.id,
        title: b.title,
        indexer: b.indexer,
        reason: b.reason,
        blocklisted_at: b.blocklisted_at.to_string(),
    }
}

fn tag_choices(setting: Option<&Value>, ids: &[i64]) -> Vec<Choice> {
    ids.iter()
        .map(|id| Choice {
            id: *id,
            name: setting
                .and_then(Value::as_array)
                .and_then(|tags| tags.iter().find(|t| t["id"].as_i64() == Some(*id)))
                .and_then(|t| t["label"].as_str())
                .map_or_else(|| format!("Tag {id}"), str::to_owned),
        })
        .collect()
}

async fn file_detail(app: &App, file: FileRef, recorded: &MovieFile) -> anyhow::Result<FileDetail> {
    let media_info = app
        .db
        .media_info(file)
        .await?
        .as_ref()
        .and_then(crate::media::parse)
        .map(MediaInfoAnswer::of);
    Ok(FileDetail {
        relative_path: recorded.relative_path.clone(),
        size: recorded.size,
        quality: recorded.quality.clone(),
        release_group: recorded.release_group.clone(),
        scene_name: recorded.scene_name.clone(),
        custom_format_score: recorded.custom_format_score,
        media_info,
    })
}

/// Whether a movie has a grab that waits for the download client or the import.
pub async fn movie_busy(app: &App, movie_id: i64) -> anyhow::Result<bool> {
    Ok(app.db.active_grab(movie_id).await?.is_some())
}

/// Whether a series has a grab that waits for the download client or the import.
pub async fn series_busy(app: &App, series_id: i64) -> anyhow::Result<bool> {
    Ok(app
        .db
        .grabs_of_series(series_id)
        .await?
        .iter()
        .any(|g| g.status.is_active()))
}

pub async fn find_movie(app: &App, id: i64) -> Result<Movie, ApiError> {
    app.db
        .movie(id)
        .await?
        .ok_or_else(|| not_found(&format!("Film {id} ist nicht in der Bibliothek")))
}

pub async fn find_series(app: &App, id: i64) -> Result<Series, ApiError> {
    app.db
        .all_series()
        .await?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| not_found(&format!("Serie {id} ist nicht in der Bibliothek")))
}

/// The IMDb rating of a title, none while ratings are switched off.
async fn imdb_rating(
    app: &App,
    imdb_id: Option<&str>,
) -> anyhow::Result<Option<sofarr_db::ImdbRating>> {
    match imdb_id {
        Some(id) if app.config.imdb.enabled => Ok(app.db.imdb_rating(id).await?),
        _ => Ok(None),
    }
}

pub async fn movie_detail(app: &App, movie: Movie) -> Result<MovieDetail, ApiError> {
    let judge = movie_judge(app).await;
    let state = judge.movie(&movie);
    let presentation = app.db.movie_presentation().await?.remove(&movie.id);
    let tags = app.db.setting("tag").await?;
    let file = match &movie.file {
        Some(f) => Some(file_detail(app, FileRef::Movie(movie.id), f).await?),
        None => None,
    };
    let busy = movie_busy(app, movie.id).await?;
    let expected = folders::movie_folder(app, &movie).await?;
    let a = &movie.availability;
    let rating = imdb_rating(app, movie.imdb_id.as_deref()).await?;
    Ok(MovieDetail {
        id: movie.id,
        tmdb_id: movie.tmdb_id,
        imdb_id: movie.imdb_id.clone(),
        imdb_rating: rating.map(|r| r.rating),
        imdb_votes: rating.map(|r| r.votes),
        title: movie.title.clone(),
        original_title: movie.original_title.clone(),
        title_de: movie.title_de.clone(),
        alternative_titles: movie.alternative_titles.clone(),
        year: movie.year,
        runtime_minutes: movie.runtime_minutes,
        overview: presentation.as_ref().and_then(|p| p.overview.clone()),
        has_poster: presentation
            .as_ref()
            .is_some_and(|p| p.poster_path.is_some()),
        monitored: movie.monitored,
        release_dates: ReleaseDates {
            in_cinemas: a.in_cinemas.clone(),
            digital: a.digital_release.clone(),
            physical: a.physical_release.clone(),
        },
        stage: judge.stage(&movie),
        available: state.available,
        minimum_availability: a.minimum.clone(),
        quality_profile: Choice {
            id: movie.quality_profile_id,
            name: judge.profile_name(movie.quality_profile_id),
        },
        tags: tag_choices(tags.as_ref(), &movie.tags),
        folder: FolderState::new(&movie.path, expected),
        file,
        missing: state.missing,
        cutoff_unmet: state.cutoff_unmet,
        cutoff_reason: movie
            .file
            .as_ref()
            .filter(|_| state.cutoff_unmet)
            .map(|f| judge.cutoff_reason(movie.quality_profile_id, f)),
        grabs: app
            .db
            .movie_grabs(movie.id)
            .await?
            .into_iter()
            .map(movie_grab)
            .collect(),
        blocklist: app
            .db
            .blocks_of(MediaRef::Movie(movie.id))
            .await?
            .into_iter()
            .map(blocked)
            .collect(),
        file_actions: FileActions::of(app, busy, MOVIE_BUSY),
        added_at: presentation
            .map(|p| p.added_at.to_string())
            .unwrap_or_default(),
    })
}

pub fn episode_answer(judge: &Judge, show: &Series, e: &Episode, today: &str) -> EpisodeAnswer {
    let state = judge.episode(show, e, today);
    EpisodeAnswer {
        id: e.id,
        season: e.data.season,
        number: e.data.number,
        absolute: e.data.absolute,
        title: e.data.title.clone(),
        title_de: e.data.title_de.clone(),
        air_date: e.data.air_date.clone(),
        monitored: e.data.monitored,
        has_file: e.file.is_some(),
        quality: e.file.as_ref().map(|f| f.quality.clone()),
        size: e.file.as_ref().map(|f| f.size),
        relative_path: e.file.as_ref().map(|f| f.relative_path.clone()),
        missing: state.missing,
        cutoff_unmet: state.cutoff_unmet,
    }
}

/// The seasons of a series with their episodes, in order.
pub fn seasons(judge: &Judge, show: &Series, episodes: &[Episode]) -> Vec<SeasonAnswer> {
    let today = judge.today_text();
    let mut out: Vec<SeasonAnswer> = Vec::new();
    for e in episodes.iter().filter(|e| e.series_id == show.id) {
        let answer = episode_answer(judge, show, e, &today);
        let season = match out.iter_mut().find(|s| s.number == e.data.season) {
            Some(s) => s,
            None => {
                out.push(SeasonAnswer {
                    number: e.data.season,
                    monitored: true,
                    episodes_total: 0,
                    episodes_with_file: 0,
                    episodes_monitored: 0,
                    missing: 0,
                    cutoff_unmet: 0,
                    episodes: Vec::new(),
                });
                out.last_mut().expect("just pushed")
            }
        };
        season.monitored &= answer.monitored;
        season.episodes_total += 1;
        season.episodes_with_file += u32::from(answer.has_file);
        season.episodes_monitored += u32::from(answer.monitored);
        season.missing += u32::from(answer.missing);
        season.cutoff_unmet += u32::from(answer.cutoff_unmet);
        season.episodes.push(answer);
    }
    out.sort_by_key(|s| s.number);
    for s in &mut out {
        s.episodes.sort_by_key(|e| e.number);
    }
    out
}

pub async fn series_detail(app: &App, show: Series) -> Result<SeriesDetail, ApiError> {
    let judge = series_judge(app).await;
    let episodes = app.db.episodes(Some(show.id)).await?;
    let refs: Vec<&Episode> = episodes.iter().collect();
    let counts = judge.series(&show, &refs);
    let presentation = app.db.series_presentation().await?.remove(&show.id);
    let tags = app.db.setting(&format!("{}tag", series::PREFIX)).await?;
    let busy = series_busy(app, show.id).await?;
    let expected = folders::series_folder(app, &show).await?;
    let rating = imdb_rating(app, show.data.imdb_id.as_deref()).await?;
    Ok(SeriesDetail {
        id: show.id,
        tvdb_id: show.data.tvdb_id,
        imdb_id: show.data.imdb_id.clone(),
        imdb_rating: rating.map(|r| r.rating),
        imdb_votes: rating.map(|r| r.votes),
        title: show.data.title.clone(),
        title_de: show.data.title_de.clone(),
        alternative_titles: show.data.alternative_titles.clone(),
        year: show.data.year,
        overview: presentation.as_ref().and_then(|p| p.overview.clone()),
        series_type: show.data.series_type.clone(),
        has_poster: presentation
            .as_ref()
            .is_some_and(|p| p.poster_path.is_some()),
        monitored: show.data.monitored,
        quality_profile: Choice {
            id: show.data.quality_profile_id,
            name: judge.profile_name(show.data.quality_profile_id),
        },
        tags: tag_choices(tags.as_ref(), &show.tags),
        folder: FolderState::new(&show.data.path, expected),
        episodes: counts.episodes,
        monitored_episodes: counts.monitored,
        files: counts.files,
        missing: counts.missing,
        cutoff_unmet: counts.cutoff_unmet,
        complete: counts.complete(),
        size: counts.size,
        seasons: seasons(&judge, &show, &episodes),
        grabs: app
            .db
            .grabs_of_series(show.id)
            .await?
            .into_iter()
            .map(series_grab)
            .collect(),
        blocklist: app
            .db
            .blocks_of(MediaRef::Series(show.id))
            .await?
            .into_iter()
            .map(blocked)
            .collect(),
        file_actions: FileActions::of(app, busy, SERIES_BUSY),
        added_at: presentation
            .map(|p| p.added_at.to_string())
            .unwrap_or_default(),
    })
}

pub async fn movie(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Json<MovieDetail>, ApiError> {
    let Path(id) = id.map_err(|_| bad_id())?;
    let movie = find_movie(&app, id).await?;
    Ok(Json(movie_detail(&app, movie).await?))
}

pub async fn show(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Json<SeriesDetail>, ApiError> {
    let Path(id) = id.map_err(|_| bad_id())?;
    let show = find_series(&app, id).await?;
    Ok(Json(series_detail(&app, show).await?))
}

pub async fn season(
    State(app): State<SharedApp>,
    ids: Result<Path<(i64, u16)>, PathRejection>,
) -> Result<Json<SeasonAnswer>, ApiError> {
    let Path((id, number)) = ids.map_err(|_| bad_id())?;
    let show = find_series(&app, id).await?;
    let judge = series_judge(&app).await;
    let episodes = app.db.episodes(Some(id)).await?;
    seasons(&judge, &show, &episodes)
        .into_iter()
        .find(|s| s.number == number)
        .map(Json)
        .ok_or_else(|| {
            not_found(&format!(
                "Staffel {number} von „{}“ ist nicht in der Bibliothek",
                show.data.title
            ))
        })
}

pub async fn episode(
    State(app): State<SharedApp>,
    ids: Result<Path<(i64, i64)>, PathRejection>,
) -> Result<Json<EpisodeDetail>, ApiError> {
    let Path((id, episode_id)) = ids.map_err(|_| bad_id())?;
    let show = find_series(&app, id).await?;
    let judge = series_judge(&app).await;
    let episodes = app.db.episodes(Some(id)).await?;
    let e = episodes
        .iter()
        .find(|e| e.id == episode_id)
        .ok_or_else(|| {
            not_found(&format!(
                "Episode {episode_id} der Serie {id} ist nicht in der Bibliothek"
            ))
        })?;
    let answer = episode_answer(&judge, &show, e, &judge.today_text());
    let file = match (e.file_id, &e.file) {
        (Some(file_id), Some(f)) => Some(file_detail(&app, FileRef::Episode(file_id), f).await?),
        _ => None,
    };
    Ok(Json(EpisodeDetail {
        series_id: show.id,
        series_title: show.data.title.clone(),
        cutoff_reason: e
            .file
            .as_ref()
            .filter(|_| answer.cutoff_unmet)
            .map(|f| judge.cutoff_reason(show.data.quality_profile_id, f)),
        episode: answer,
        file,
    }))
}
