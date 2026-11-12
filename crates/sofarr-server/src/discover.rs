//! Discover and people pages (spec discover, D19): shelves, filtered lists and search
//! from TMDB with each title's library state and IMDb rating, title previews, person
//! pages, and the cast of library titles. TMDB answers are cached (AC-13) and served
//! stale while TMDB cannot be reached (AC-14).

pub mod api;
mod background;
pub mod cache;
pub mod cards;
pub mod people;
pub mod shelves;
pub mod titles;

use axum::http::StatusCode;
use sofarr_metadata::{
    Tmdb,
    browse::{Genre, Page, PersonDetail, SearchHit, TitleDetail, TitleSummary},
};

pub use api::routes;
pub use cache::{Cache, Failure};

use crate::{api::ApiError, app::App};

/// Answer while no TMDB key is configured (AC-15).
pub const NO_TMDB: &str = "Entdecken braucht einen TMDB-Zugang: [tmdb] mit api_key in sofarr.toml eintragen und sofarr neu starten.";

/// TMDB answers of the discover and people pages.
#[derive(Debug)]
pub struct Discover {
    pub lists: Cache<Page<TitleSummary>>,
    pub search: Cache<Page<SearchHit>>,
    pub genres: Cache<Vec<Genre>>,
    pub titles: Cache<TitleDetail>,
    pub people: Cache<PersonDetail>,
    /// Titles TMDB answered „not found“ for: shown without rating instead of waiting.
    pub unknown: std::sync::Mutex<std::collections::HashSet<String>>,
    pub queue: background::Queue,
}

impl Default for Discover {
    fn default() -> Self {
        Self {
            lists: Cache::new(cache::LIST_TTL, 1_000),
            search: Cache::new(cache::LIST_TTL, 500),
            genres: Cache::new(cache::LIST_TTL, 4),
            titles: Cache::new(cache::DETAIL_TTL, 5_000),
            people: Cache::new(cache::DETAIL_TTL, 500),
            unknown: std::sync::Mutex::default(),
            queue: background::Queue::default(),
        }
    }
}

/// The TMDB client; without `[tmdb]` an answer the UI shows (AC-15).
pub fn tmdb_client(app: &App) -> Result<Tmdb, ApiError> {
    crate::metadata::tmdb(app).map_err(|_| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, NO_TMDB))
}

/// A missing TMDB answer as an API error (AC-14: never an internal error).
pub fn failure(failure: Failure, not_found: &str) -> ApiError {
    match failure {
        Failure::NotFound => ApiError::new(StatusCode::NOT_FOUND, not_found),
        Failure::Unreachable => ApiError::new(
            StatusCode::BAD_GATEWAY,
            "TMDB ist nicht erreichbar, und es liegen noch keine zwischengespeicherten Daten vor. Bitte später erneut versuchen.",
        ),
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod title_tests;
