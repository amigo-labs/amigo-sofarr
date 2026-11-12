//! sofarr's own JSON API (spec D8: no v3 compatibility unless an integration needs it).

use axum::{
    Json, Router,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use sofarr_db::{GrabStatus, MediaRef};

use crate::{
    app::SharedApp,
    backup_files::{BackupFile, Kind},
    backups::{BackupList, BackupRequestError},
    downloads::{GrabRef, MarkError},
    library,
    search::{SearchError, SearchReport, SearchTarget},
};

/// Everything sofarr serves over HTTP, behind the authentication middleware.
pub fn app_router(app: SharedApp) -> Router {
    app_router_with(app, crate::ui::Files::Embedded)
}

/// [`app_router`] with the UI files from `ui`.
pub fn app_router_with(app: SharedApp, ui: crate::ui::Files) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/login", post(crate::auth::login))
        .route("/api/auth/logout", post(crate::auth::logout))
        .route("/api/auth/me", get(crate::auth::me))
        .with_state(std::sync::Arc::clone(&app))
        .merge(routes(std::sync::Arc::clone(&app)))
        .fallback(move |request| crate::ui::serve(ui.clone(), request))
        .layer(axum::middleware::from_fn_with_state(
            app,
            crate::auth::guard,
        ))
}

/// The answer of every failed API request: a German message.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ErrorBody {
    pub error: String,
}

impl ErrorBody {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            error: message.into(),
        }
    }
}

/// The answer of a request that only says it worked.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct OkBody {
    pub ok: bool,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Health {
    status: &'static str,
    version: &'static str,
}

pub async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

pub fn routes(app: SharedApp) -> Router {
    Router::new()
        .route("/api/system", get(crate::system::system))
        .route("/api/imdb", get(crate::imdb::info))
        .route("/api/movies", get(movies))
        .route("/api/movies/{id}/file", get(movie_file))
        .route(
            "/api/series/{id}/episodes/{episode_id}/file",
            get(episode_file),
        )
        .route("/api/subtitles", get(crate::subtitles::overview))
        .route(
            "/api/movies/{id}/subtitles",
            get(crate::subtitles::file_subtitles_movie),
        )
        .route(
            "/api/movies/{id}/subtitles/search",
            post(crate::subtitles::search_movie),
        )
        .route(
            "/api/series/{id}/episodes/{episode_id}/subtitles",
            get(crate::subtitles::file_subtitles_episode),
        )
        .route(
            "/api/series/{id}/episodes/{episode_id}/subtitles/search",
            post(crate::subtitles::search_episode),
        )
        .route("/api/wanted", get(wanted))
        .route("/api/grabs", get(grabs))
        .route("/api/grabs/movie/{id}/fail", post(fail_movie_grab))
        .route("/api/grabs/series/{id}/fail", post(fail_series_grab))
        .route("/api/blocklist", get(blocklist))
        .route("/api/blocklist/{id}", delete(unblock))
        .route("/api/command/rss-sync", post(rss_sync))
        .route("/api/command/search", post(search))
        .route("/api/backups", get(backup_list).post(backup_now))
        .route("/api/backups/restore", delete(cancel_restore))
        .route(
            "/api/backups/{name}",
            get(download_backup).delete(delete_backup),
        )
        .route("/api/backups/{name}/restore", post(choose_restore))
        .with_state(std::sync::Arc::clone(&app))
        .merge(crate::library_routes::routes(app))
}

/// An error answer: status and a German message.
pub(crate) struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub(crate) fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(ErrorBody::new(self.message))).into_response()
    }
}

/// Internal errors are logged; the answer names no internals.
impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        tracing::error!(error = format!("{:#}", e.into()), "api request failed");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Interner Fehler – Details stehen im Log",
        )
    }
}

#[derive(Serialize)]
struct MovieSummary {
    id: i64,
    tmdb_id: i64,
    title: String,
    year: Option<u16>,
    monitored: bool,
    quality: Option<String>,
    custom_format_score: Option<i32>,
}

async fn movies(State(app): State<SharedApp>) -> Result<Json<Vec<MovieSummary>>, ApiError> {
    let movies = app.db.movies().await?;
    Ok(Json(
        movies
            .into_iter()
            .map(|m| MovieSummary {
                id: m.id,
                tmdb_id: m.tmdb_id,
                title: m.title,
                year: m.year,
                monitored: m.monitored,
                quality: m.file.as_ref().map(|f| f.quality.clone()),
                custom_format_score: m.file.as_ref().map(|f| f.custom_format_score),
            })
            .collect(),
    ))
}

async fn wanted(State(app): State<SharedApp>) -> Result<Json<library::Wanted>, ApiError> {
    let rules = library::load_rules(&app.db).await?;
    let movies = app.db.movies().await?;
    Ok(Json(library::wanted(&movies, &rules)))
}

#[derive(Deserialize)]
struct GrabQuery {
    status: Option<String>,
    limit: Option<u32>,
}

#[derive(Serialize)]
struct GrabSummary {
    id: i64,
    movie_id: i64,
    title: String,
    indexer: String,
    quality: String,
    custom_format_score: i32,
    status: &'static str,
    message: Option<String>,
    grabbed_at: String,
}

async fn grabs(
    State(app): State<SharedApp>,
    Query(q): Query<GrabQuery>,
) -> Result<Json<Vec<GrabSummary>>, ApiError> {
    let status = match q.status.as_deref() {
        Some(s) => Some(GrabStatus::parse(s).ok_or_else(|| {
            ApiError::new(StatusCode::BAD_REQUEST, format!("unbekannter Status {s}"))
        })?),
        None => None,
    };
    let grabs = app.db.grabs(status, q.limit.unwrap_or(100)).await?;
    Ok(Json(
        grabs
            .into_iter()
            .map(|g| GrabSummary {
                id: g.id,
                movie_id: g.movie_id,
                title: g.title,
                indexer: g.indexer,
                quality: g.quality,
                custom_format_score: g.custom_format_score,
                status: g.status.as_str(),
                message: g.message,
                grabbed_at: g.grabbed_at.to_string(),
            })
            .collect(),
    ))
}

async fn rss_sync(State(app): State<SharedApp>) -> Result<Json<crate::rss::SyncReport>, ApiError> {
    Ok(Json(app.rss_sync().await?))
}

/// Body of `POST /api/command/search`: exactly one of `movie_id`; `series_id`;
/// `series_id` + `season`; `series_id` + `season` + `episode`.
#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub(crate) struct SearchBody {
    movie_id: Option<i64>,
    series_id: Option<i64>,
    season: Option<u16>,
    episode: Option<u16>,
}

impl SearchBody {
    fn target(&self) -> Option<SearchTarget> {
        Some(
            match (self.movie_id, self.series_id, self.season, self.episode) {
                (Some(id), None, None, None) => SearchTarget::Movie(id),
                (None, Some(id), None, None) => SearchTarget::Series(id),
                (None, Some(series_id), Some(season), None) => {
                    SearchTarget::Season { series_id, season }
                }
                (None, Some(series_id), Some(season), Some(episode)) => SearchTarget::Episode {
                    series_id,
                    season,
                    episode,
                },
                _ => return None,
            },
        )
    }
}

const BAD_SEARCH: &str = "Ungültige Suchanfrage: genau ein Ziel angeben (movie_id, series_id, series_id + season oder series_id + season + episode)";

async fn search(
    State(app): State<SharedApp>,
    body: Result<Json<SearchBody>, JsonRejection>,
) -> Result<Json<SearchReport>, ApiError> {
    let target = body
        .ok()
        .and_then(|Json(b)| b.target())
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, BAD_SEARCH))?;
    match app.search(target).await {
        Ok(report) => Ok(Json(report)),
        Err(SearchError::NotFound(message)) => Err(ApiError::new(StatusCode::NOT_FOUND, message)),
        Err(SearchError::Failed(e)) => Err(e.into()),
    }
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub(crate) struct PageQuery {
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub(crate) struct BlocklistPage {
    page: u32,
    page_size: u32,
    total: u64,
    entries: Vec<BlocklistEntry>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub(crate) struct BlocklistEntry {
    id: i64,
    movie_id: Option<i64>,
    series_id: Option<i64>,
    /// Title of the movie or series.
    media_title: String,
    title: String,
    indexer: String,
    reason: String,
    blocklisted_at: String,
}

const BAD_PAGE: &str = "Ungültige Seite: page ab 1, page_size von 1 bis 500";

async fn blocklist(
    State(app): State<SharedApp>,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> Result<Json<BlocklistPage>, ApiError> {
    let bad = || ApiError::new(StatusCode::BAD_REQUEST, BAD_PAGE);
    let Query(q) = query.map_err(|_| bad())?;
    let page = u32::try_from(q.page.unwrap_or(1))
        .ok()
        .filter(|p| *p >= 1)
        .ok_or_else(bad)?;
    let page_size = u32::try_from(q.page_size.unwrap_or(50))
        .ok()
        .filter(|s| (1..=500).contains(s))
        .ok_or_else(bad)?;
    let (entries, total) = app.db.block_page(page, page_size).await?;
    Ok(Json(BlocklistPage {
        page,
        page_size,
        total,
        entries: entries
            .into_iter()
            .map(|e| {
                let (movie_id, series_id) = match e.media {
                    MediaRef::Movie(id) => (Some(id), None),
                    MediaRef::Series(id) => (None, Some(id)),
                };
                BlocklistEntry {
                    id: e.id,
                    movie_id,
                    series_id,
                    media_title: e.media_title,
                    title: e.title,
                    indexer: e.indexer,
                    reason: e.reason,
                    blocklisted_at: e.blocklisted_at.to_string(),
                }
            })
            .collect(),
    }))
}

pub(crate) fn bad_id() -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "Ungültige ID")
}

async fn unblock(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<StatusCode, ApiError> {
    let Path(id) = id.map_err(|_| bad_id())?;
    if app.db.remove_block(id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::new(
            StatusCode::NOT_FOUND,
            format!("Blocklist-Eintrag {id} nicht gefunden"),
        ))
    }
}

async fn fail_movie_grab(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<StatusCode, ApiError> {
    let Path(id) = id.map_err(|_| bad_id())?;
    mark(&app, GrabRef::Movie(id)).await
}

async fn fail_series_grab(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<StatusCode, ApiError> {
    let Path(id) = id.map_err(|_| bad_id())?;
    mark(&app, GrabRef::Series(id)).await
}

async fn mark(app: &SharedApp, grab: GrabRef) -> Result<StatusCode, ApiError> {
    match app.mark_failed(grab).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(MarkError::NotFound(m)) => Err(ApiError::new(StatusCode::NOT_FOUND, m)),
        Err(MarkError::NotActive(m)) => Err(ApiError::new(StatusCode::CONFLICT, m)),
        Err(MarkError::Unavailable(m)) => Err(ApiError::new(StatusCode::SERVICE_UNAVAILABLE, m)),
        Err(MarkError::Failed(e)) => Err(e.into()),
    }
}

/// A library file with its media info (spec mediainfo AC-10).
#[derive(Serialize)]
struct FileAnswer {
    path: String,
    size: u64,
    quality: String,
    media_info: Option<crate::media::MediaInfoAnswer>,
}

async fn file_answer(
    app: &SharedApp,
    file: sofarr_db::FileRef,
    folder: &str,
    recorded: &sofarr_db::MovieFile,
) -> Result<Json<FileAnswer>, ApiError> {
    let media_info = app
        .db
        .media_info(file)
        .await?
        .as_ref()
        .and_then(crate::media::parse)
        .map(crate::media::MediaInfoAnswer::of);
    Ok(Json(FileAnswer {
        path: std::path::Path::new(folder)
            .join(&recorded.relative_path)
            .to_string_lossy()
            .into_owned(),
        size: recorded.size,
        quality: recorded.quality.clone(),
        media_info,
    }))
}

async fn movie_file(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Json<FileAnswer>, ApiError> {
    let Path(id) = id.map_err(|_| bad_id())?;
    let movie = app.db.movie(id).await?;
    let Some((movie, file)) = movie.and_then(|m| m.file.clone().map(|f| (m, f))) else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            format!("Film {id} hat keine Datei"),
        ));
    };
    file_answer(
        &app,
        sofarr_db::FileRef::Movie(movie.id),
        &movie.path,
        &file,
    )
    .await
}

async fn episode_file(
    State(app): State<SharedApp>,
    ids: Result<Path<(i64, i64)>, PathRejection>,
) -> Result<Json<FileAnswer>, ApiError> {
    let Path((series_id, episode_id)) = ids.map_err(|_| bad_id())?;
    let show = app
        .db
        .all_series()
        .await?
        .into_iter()
        .find(|s| s.id == series_id);
    let episode = match &show {
        Some(_) => app
            .db
            .episodes(Some(series_id))
            .await?
            .into_iter()
            .find(|e| e.id == episode_id),
        None => None,
    };
    let (Some(show), Some(episode)) = (show, episode) else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            format!("Episode {episode_id} der Serie {series_id} nicht gefunden"),
        ));
    };
    let (Some(file_id), Some(file)) = (episode.file_id, episode.file.clone()) else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            format!("Episode {episode_id} hat keine Datei"),
        ));
    };
    file_answer(
        &app,
        sofarr_db::FileRef::Episode(file_id),
        &show.data.path,
        &file,
    )
    .await
}

impl From<BackupRequestError> for ApiError {
    fn from(e: BackupRequestError) -> Self {
        match e {
            BackupRequestError::NotFound => {
                Self::new(StatusCode::NOT_FOUND, "Sicherung nicht gefunden")
            }
            BackupRequestError::Invalid(m) => Self::new(StatusCode::UNPROCESSABLE_ENTITY, m),
            BackupRequestError::Unreadable(m) => Self::new(StatusCode::SERVICE_UNAVAILABLE, m),
            BackupRequestError::Failed(m) => Self::new(StatusCode::INTERNAL_SERVER_ERROR, m),
        }
    }
}

async fn backup_list(State(app): State<SharedApp>) -> Result<Json<BackupList>, ApiError> {
    Ok(Json(app.backup_list().await?))
}

async fn backup_now(State(app): State<SharedApp>) -> Result<Json<BackupFile>, ApiError> {
    // The details (paths, OS errors) go to the log and the backup list, not the answer.
    app.write_backup(Kind::Manual, jiff::Timestamp::now())
        .await
        .map(Json)
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Sicherung fehlgeschlagen – Details stehen im Log",
            )
        })
}

/// The backup file as it is (read whole: backups are a few megabytes).
async fn download_backup(
    State(app): State<SharedApp>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    let path = app.backup_path(&name).await?;
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| ApiError::from(BackupRequestError::NotFound))?;
    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/octet-stream".to_owned(),
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
        ],
        bytes,
    )
        .into_response())
}

async fn delete_backup(
    State(app): State<SharedApp>,
    Path(name): Path<String>,
) -> Result<StatusCode, ApiError> {
    app.delete_backup(&name).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub(crate) struct RestoreAnswer {
    pending_restore: String,
    message: &'static str,
}

async fn choose_restore(
    State(app): State<SharedApp>,
    Path(name): Path<String>,
) -> Result<Json<RestoreAnswer>, ApiError> {
    app.choose_restore(&name).await?;
    Ok(Json(RestoreAnswer {
        pending_restore: name,
        message: "Die Sicherung wird beim nächsten Start eingespielt",
    }))
}

async fn cancel_restore(State(app): State<SharedApp>) -> Result<StatusCode, ApiError> {
    app.cancel_restore().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{app::App, config::Config, library::tests::documents};

    #[tokio::test]
    async fn serves_movies_and_wanted() {
        let db = sofarr_db::Db::in_memory().await.expect("db");
        library::import_documents(&db, &documents(), jiff::Timestamp::now())
            .await
            .expect("import");
        let app = Arc::new(App::with_db(Config::default(), db).expect("app"));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move { axum::serve(listener, routes(app)).await });
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("client");
        let movies: serde_json::Value = client
            .get(format!("http://{addr}/api/movies"))
            .send()
            .await
            .expect("get")
            .json()
            .await
            .expect("json");
        assert_eq!(movies.as_array().map(Vec::len), Some(4));
        let wanted: serde_json::Value = client
            .get(format!("http://{addr}/api/wanted"))
            .send()
            .await
            .expect("get")
            .json()
            .await
            .expect("json");
        assert_eq!(wanted["missing"], serde_json::json!([101]));
        let sync: serde_json::Value = client
            .post(format!("http://{addr}/api/command/rss-sync"))
            .send()
            .await
            .expect("post")
            .json()
            .await
            .expect("json");
        assert_eq!(sync["releases"], 0);
        let bad = client
            .get(format!("http://{addr}/api/grabs?status=kaputt"))
            .send()
            .await
            .expect("get");
        assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
    }
}
