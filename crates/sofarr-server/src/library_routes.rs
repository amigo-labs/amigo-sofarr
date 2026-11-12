//! The routes of the library pages (spec web-ui-library).

use axum::{
    Router,
    routing::{get, post},
};

use crate::{app::SharedApp, library_detail, library_edit, library_list, wanted};

pub fn routes(app: SharedApp) -> Router {
    Router::new()
        .route("/api/library/options", get(library_list::options))
        .route("/api/library/movies", get(library_list::movies))
        .route("/api/library/series", get(library_list::shows))
        .route(
            "/api/library/movies/{id}",
            get(library_detail::movie).patch(library_edit::change_movie),
        )
        .route(
            "/api/library/movies/{id}/change-preview",
            post(library_edit::movie_preview),
        )
        .route(
            "/api/library/series/{id}",
            get(library_detail::show).patch(library_edit::change_series),
        )
        .route(
            "/api/library/series/{id}/change-preview",
            post(library_edit::series_preview),
        )
        .route("/api/library/monitor", post(library_edit::monitor))
        .route("/api/library/wanted", get(wanted::wanted))
        .route("/api/command/search-bulk", post(wanted::search_bulk))
        .route(
            "/api/library/series/{id}/seasons/{season}",
            get(library_detail::season),
        )
        .route(
            "/api/library/series/{id}/episodes/{episode_id}",
            get(library_detail::episode),
        )
        .with_state(app)
}
