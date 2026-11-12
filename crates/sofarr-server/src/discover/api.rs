//! The discover routes: status, shelves, result list, genres, search, title states,
//! previews, people, library cast, and the add actions (spec discover).

use axum::{
    Json, Router,
    extract::{Query, State, rejection::JsonRejection, rejection::QueryRejection},
    http::StatusCode,
    routing::{get, post},
};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use sofarr_metadata::browse::SearchHit;

use super::{
    NO_TMDB,
    cache::Freshness,
    cards::{Cards, LibraryState, Rating, TitleCard, TitleKind, image},
    failure, people, shelves, titles, tmdb_client,
};
use crate::{api::ApiError, app::SharedApp};

pub fn routes(app: SharedApp) -> Router {
    Router::new()
        .route("/api/discover/status", get(status))
        .route("/api/discover/shelves", get(shelves::shelves))
        .route("/api/discover/shelves/{id}", get(shelves::shelf))
        .route("/api/discover/list", get(shelves::list))
        .route("/api/discover/genres", get(shelves::genres))
        .route("/api/discover/search", get(search))
        .route("/api/discover/states", post(states))
        .route("/api/discover/titles/{kind}/{id}", get(titles::preview))
        .route("/api/discover/people/{id}", get(people::page))
        .route(
            "/api/discover/people/{id}/import-list",
            post(crate::add_api::create_import_list),
        )
        .route(
            "/api/discover/add/{kind}/options",
            get(crate::add_api::options),
        )
        .route("/api/discover/add/movie", post(crate::add_api::add_movie))
        .route("/api/discover/add/series", post(crate::add_api::add_series))
        .route("/api/library/movies/{id}/cast", get(titles::movie_cast))
        .route("/api/library/series/{id}/cast", get(titles::series_cast))
        .route(
            "/api/images/tmdb/{size}/{file}",
            get(crate::images::tmdb_image),
        )
        .with_state(app)
}

/// `GET /api/discover/status`: whether the discover pages can work (AC-15).
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct DiscoverStatus {
    /// A TMDB key is configured; without it search, previews, people, cast links and add
    /// actions are not offered.
    pub available: bool,
    /// German explanation while not available.
    pub message: Option<String>,
    /// IMDb ratings are on: the rating filter and sorting are offered.
    pub ratings: bool,
}

async fn status(State(app): State<SharedApp>) -> Json<DiscoverStatus> {
    let available = app.config.tmdb.is_some();
    Json(DiscoverStatus {
        available,
        message: (!available).then(|| NO_TMDB.to_owned()),
        ratings: app.config.imdb.enabled,
    })
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SearchQuery {
    pub q: String,
    pub page: Option<u32>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PersonCard {
    pub tmdb_id: i64,
    pub name: String,
    pub photo: Option<String>,
    pub department: Option<String>,
    pub known_for: Vec<String>,
}

/// `GET /api/discover/search?q=…`: movies, series and people in three groups (AC-03).
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SearchAnswer {
    pub movies: Vec<TitleCard>,
    pub series: Vec<TitleCard>,
    pub people: Vec<PersonCard>,
    pub next_page: Option<u32>,
    pub freshness: Freshness,
}

const BAD_SEARCH: &str = "Ungültige Suche: q mit 1 bis 200 Zeichen, page von 1 bis 500";

async fn search(
    State(app): State<SharedApp>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<Json<SearchAnswer>, ApiError> {
    let Query(q) = query.map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, BAD_SEARCH))?;
    let text = q.q.trim().to_owned();
    let page = q.page.unwrap_or(1);
    if text.is_empty() || text.chars().count() > 200 || !(1..=500).contains(&page) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, BAD_SEARCH));
    }
    let tmdb = tmdb_client(&app)?;
    let now = Timestamp::now();
    let key = format!("search/{}?page={page}", text.to_lowercase());
    let cached = app
        .discover
        .search
        .load(&key, now, || tmdb.search(&text, page))
        .await
        .map_err(|f| failure(f, "TMDB kennt diese Suche nicht"))?;
    let mut cards = Cards::new(&app).await?;
    let mut answer = SearchAnswer {
        movies: Vec::new(),
        series: Vec::new(),
        people: Vec::new(),
        next_page: cached.value.has_more().then_some(page + 1),
        freshness: Freshness::one(&cached, now),
    };
    for hit in &cached.value.results {
        match hit {
            SearchHit::Title(title) => {
                let card = cards.card(title).await?;
                match card.kind {
                    TitleKind::Movie => answer.movies.push(card),
                    TitleKind::Series => answer.series.push(card),
                }
            }
            SearchHit::Person(p) => answer.people.push(PersonCard {
                tmdb_id: p.tmdb_id,
                name: p.name.clone(),
                photo: image(p.profile_path.as_deref(), "w185"),
                department: p.department.clone(),
                known_for: p.known_for.clone(),
            }),
        }
    }
    cards.finish(&app);
    Ok(Json(answer))
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct TitleRef {
    pub kind: TitleKind,
    pub tmdb_id: i64,
}

/// `POST /api/discover/states`: the current library state and rating of titles already
/// shown, for ratings that were still loading and titles added meanwhile.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct StatesBody {
    /// At most 500.
    pub titles: Vec<TitleRef>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct TitleState {
    pub kind: TitleKind,
    pub tmdb_id: i64,
    pub library: LibraryState,
    pub rating: Rating,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct StatesAnswer {
    pub titles: Vec<TitleState>,
}

async fn states(
    State(app): State<SharedApp>,
    body: Result<Json<StatesBody>, JsonRejection>,
) -> Result<Json<StatesAnswer>, ApiError> {
    let Json(body) = body.map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "Ungültige Anfrage: titles mit kind und tmdb_id angeben",
        )
    })?;
    if body.titles.len() > 500 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Höchstens 500 Titel auf einmal",
        ));
    }
    let mut cards = Cards::new(&app).await?;
    let mut titles = Vec::new();
    for t in body.titles {
        let (library, rating) = cards.state(t.kind.tmdb(), t.tmdb_id).await?;
        titles.push(TitleState {
            kind: t.kind,
            tmdb_id: t.tmdb_id,
            library,
            rating,
        });
    }
    cards.finish(&app);
    Ok(Json(StatesAnswer { titles }))
}
