//! sofarr's own JSON API (spec D8: no v3 compatibility unless an integration needs it).

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sofarr_db::GrabStatus;

use crate::{app::SharedApp, library};

pub fn routes(app: SharedApp) -> Router {
    Router::new()
        .route("/api/movies", get(movies))
        .route("/api/wanted", get(wanted))
        .route("/api/grabs", get(grabs))
        .route("/api/command/rss-sync", post(rss_sync))
        .with_state(app)
}

struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("{:#}", self.0) })),
        )
            .into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        Self(e.into())
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
        Some(s) => {
            Some(GrabStatus::parse(s).ok_or_else(|| anyhow::anyhow!("unbekannter Status {s}"))?)
        }
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
        assert_eq!(bad.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
