//! The discover page: shelves (spec discover AC-01) and, once a filter is set, one
//! result list (AC-02).

use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
    http::StatusCode,
};
use jiff::{Timestamp, ToSpan};
use serde::{Deserialize, Serialize};
use sofarr_metadata::{
    Tmdb,
    browse::{Filter, Kind, MovieList, Page, TitleSummary, Window},
};

use super::{
    Failure,
    cache::{Cached, Freshness},
    cards::{Cards, TitleCard, TitleKind},
    failure, tmdb_client,
};
use crate::{
    api::ApiError,
    app::{App, SharedApp},
};

/// Titles per shelf and the least a result list shows per request (AC-01, AC-02).
pub const VISIBLE: usize = 20;
/// TMDB pages one result list request reads at most; the next request goes on.
const MAX_PAGES: u32 = 10;
/// „Neu als Stream oder Disc“ looks this many days back.
const NEW_AT_HOME_DAYS: i64 = 42;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ShelfId {
    TrendingDay,
    TrendingWeek,
    PopularMovies,
    Upcoming,
    NowPlaying,
    NewAtHome,
    PopularSeries,
}

/// The shelves in the order the page shows them.
pub const SHELVES: [ShelfId; 7] = [
    ShelfId::TrendingDay,
    ShelfId::TrendingWeek,
    ShelfId::PopularMovies,
    ShelfId::Upcoming,
    ShelfId::NowPlaying,
    ShelfId::NewAtHome,
    ShelfId::PopularSeries,
];

impl ShelfId {
    pub fn title(self) -> &'static str {
        match self {
            ShelfId::TrendingDay => "Trending heute",
            ShelfId::TrendingWeek => "Trending diese Woche",
            ShelfId::PopularMovies => "Beliebte Filme",
            ShelfId::Upcoming => "Demnächst im Kino",
            ShelfId::NowPlaying => "Jetzt im Kino",
            ShelfId::NewAtHome => "Neu als Stream oder Disc",
            ShelfId::PopularSeries => "Beliebte Serien",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::from(text)).ok()
    }
}

/// One page of a shelf from the cache (6 hours) or TMDB.
async fn shelf_page(
    app: &App,
    tmdb: &Tmdb,
    shelf: ShelfId,
    page: u32,
    now: Timestamp,
) -> Result<Cached<Page<TitleSummary>>, Failure> {
    let today = now.to_zoned(jiff::tz::TimeZone::UTC).date();
    let from = today
        .checked_sub(NEW_AT_HOME_DAYS.days())
        .unwrap_or(today)
        .to_string();
    let to = today.to_string();
    let id = serde_json::to_value(shelf)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    let key = match shelf {
        ShelfId::NewAtHome => format!("shelf/{id}/{to}?page={page}"),
        _ => format!("shelf/{id}?page={page}"),
    };
    app.discover
        .lists
        .load(&key, now, || async {
            match shelf {
                ShelfId::TrendingDay => tmdb.trending(Window::Day, page).await,
                ShelfId::TrendingWeek => tmdb.trending(Window::Week, page).await,
                ShelfId::PopularMovies => tmdb.movie_list(MovieList::Popular, page).await,
                ShelfId::Upcoming => tmdb.movie_list(MovieList::Upcoming, page).await,
                ShelfId::NowPlaying => tmdb.movie_list(MovieList::NowPlaying, page).await,
                ShelfId::NewAtHome => tmdb.new_at_home(&from, &to, page).await,
                ShelfId::PopularSeries => tmdb.popular_series(page).await,
            }
        })
        .await
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Shelf {
    pub id: ShelfId,
    pub title: String,
    /// Up to 20 titles.
    pub items: Vec<TitleCard>,
    /// More titles via `GET /api/discover/shelves/{id}?page=2`.
    pub has_more: bool,
    /// German message when this shelf could not be loaded.
    pub error: Option<String>,
}

/// `GET /api/discover/shelves`.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ShelvesAnswer {
    pub shelves: Vec<Shelf>,
    pub freshness: Freshness,
}

/// A page of titles: a shelf's further pages, the filtered result list.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ListAnswer {
    pub items: Vec<TitleCard>,
    /// The `page` that continues the list; `null` when TMDB has no more.
    pub next_page: Option<u32>,
    pub freshness: Freshness,
}

pub async fn shelves(State(app): State<SharedApp>) -> Result<Json<ShelvesAnswer>, ApiError> {
    let tmdb = tmdb_client(&app)?;
    let now = Timestamp::now();
    let mut cards = Cards::new(&app).await?;
    let mut shelves = Vec::new();
    let mut times = Vec::new();
    let mut first_failure = None;
    for id in SHELVES {
        let mut shelf = Shelf {
            id,
            title: id.title().to_owned(),
            items: Vec::new(),
            has_more: false,
            error: None,
        };
        match shelf_page(&app, &tmdb, id, 1, now).await {
            Ok(page) => {
                times.push((page.fetched_at, page.stale));
                for title in page.value.results.iter().take(VISIBLE) {
                    shelf.items.push(cards.card(title).await?);
                }
                shelf.has_more = page.value.has_more() || page.value.results.len() > VISIBLE;
            }
            Err(f) => {
                let error = failure(f, "TMDB kennt diese Liste nicht");
                shelf.error = Some(error.message().to_owned());
                first_failure.get_or_insert(error);
            }
        }
        shelves.push(shelf);
    }
    cards.finish(&app);
    if times.is_empty()
        && let Some(error) = first_failure
    {
        return Err(error);
    }
    Ok(Json(ShelvesAnswer {
        shelves,
        freshness: Freshness::of(times, now),
    }))
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ShelfQuery {
    pub page: Option<u32>,
}

/// `GET /api/discover/shelves/{id}?page=n`: more titles of a shelf.
pub async fn shelf(
    State(app): State<SharedApp>,
    Path(id): Path<String>,
    query: Result<Query<ShelfQuery>, QueryRejection>,
) -> Result<Json<ListAnswer>, ApiError> {
    let shelf = ShelfId::parse(&id)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, format!("Unbekanntes Regal {id}")))?;
    let page = query
        .ok()
        .map_or(Some(1), |Query(q)| q.page.or(Some(1)))
        .filter(|p| (1..=500).contains(p))
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, BAD_PAGE))?;
    let tmdb = tmdb_client(&app)?;
    let now = Timestamp::now();
    let cached = shelf_page(&app, &tmdb, shelf, page, now)
        .await
        .map_err(|f| failure(f, "TMDB kennt diese Liste nicht"))?;
    let mut cards = Cards::new(&app).await?;
    let mut items = Vec::new();
    for title in &cached.value.results {
        items.push(cards.card(title).await?);
    }
    cards.finish(&app);
    Ok(Json(ListAnswer {
        items,
        next_page: cached.value.has_more().then_some(page + 1),
        freshness: Freshness::one(&cached, now),
    }))
}

const BAD_PAGE: &str = "Ungültige Seite: page von 1 bis 500";

/// `GET /api/discover/list`: titles matching the filters, most popular first.
#[derive(Debug, Default, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct ListQuery {
    /// Movies or series only; both merged by popularity without it.
    pub kind: Option<TitleKind>,
    /// TMDB genre id (`GET /api/discover/genres`).
    pub genre: Option<i64>,
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    /// Original language, ISO 639-1 (`de`, `en`, …).
    pub language: Option<String>,
    /// Minimum IMDb rating; only while `[imdb] enabled`.
    pub min_rating: Option<f32>,
    pub page: Option<u32>,
}

fn bad(message: &str) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, message)
}

/// The TMDB filter of a query, checked.
fn checked(app: &App, q: &ListQuery) -> Result<(Filter, u32), ApiError> {
    let years = 1870..=2100;
    if q.year_from.is_some_and(|y| !years.contains(&y))
        || q.year_to.is_some_and(|y| !years.contains(&y))
        || matches!((q.year_from, q.year_to), (Some(f), Some(t)) if f > t)
    {
        return Err(bad(
            "Ungültiger Zeitraum: Jahre von 1870 bis 2100, das erste nicht nach dem zweiten",
        ));
    }
    let language = q
        .language
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty());
    if language.is_some_and(|l| l.len() != 2 || !l.bytes().all(|b| b.is_ascii_lowercase())) {
        return Err(bad(
            "Ungültige Originalsprache: zwei Kleinbuchstaben nach ISO 639-1, z. B. de",
        ));
    }
    if let Some(minimum) = q.min_rating {
        if !app.config.imdb.enabled {
            return Err(bad(
                "Der Bewertungsfilter braucht IMDb-Bewertungen ([imdb] enabled)",
            ));
        }
        if !(0.0..=10.0).contains(&minimum) {
            return Err(bad("Ungültige Mindestbewertung: 0 bis 10"));
        }
    }
    let page = q.page.unwrap_or(1);
    if !(1..=500).contains(&page) {
        return Err(bad(BAD_PAGE));
    }
    Ok((
        Filter {
            genre: q.genre,
            year_from: q.year_from,
            year_to: q.year_to,
            language: language.map(str::to_owned),
        },
        page,
    ))
}

fn filter_key(kind: Kind, filter: &Filter, page: u32) -> String {
    format!(
        "discover/{}?genre={:?}&from={:?}&to={:?}&language={:?}&page={page}",
        kind.path(),
        filter.genre,
        filter.year_from,
        filter.year_to,
        filter.language
    )
}

pub async fn list(
    State(app): State<SharedApp>,
    query: Result<Query<ListQuery>, QueryRejection>,
) -> Result<Json<ListAnswer>, ApiError> {
    let Query(q) = query.map_err(|_| bad("Ungültige Filter"))?;
    let (filter, first) = checked(&app, &q)?;
    let tmdb = tmdb_client(&app)?;
    let now = Timestamp::now();
    let kinds = match q.kind {
        Some(kind) => vec![kind.tmdb()],
        None => vec![Kind::Movie, Kind::Series],
    };
    let mut cards = Cards::new(&app).await?;
    let mut items = Vec::new();
    let mut times = Vec::new();
    let mut page = first;
    let mut more = true;
    // The rating filter thins the list out: keep reading until enough are visible.
    while more && items.len() < VISIBLE && page < first + MAX_PAGES && page <= 500 {
        more = false;
        let mut batch: Vec<TitleSummary> = Vec::new();
        for kind in &kinds {
            let cached = app
                .discover
                .lists
                .load(&filter_key(*kind, &filter, page), now, || {
                    tmdb.discover(*kind, &filter, page)
                })
                .await
                .map_err(|f| failure(f, "TMDB kennt diese Filter nicht"))?;
            times.push((cached.fetched_at, cached.stale));
            more |= cached.value.has_more();
            batch.extend(cached.value.results.iter().cloned());
        }
        batch.sort_by(|a, b| b.popularity.total_cmp(&a.popularity));
        for title in &batch {
            let card = cards.card(title).await?;
            if q.min_rating.is_none_or(|m| card.rating.reaches(m)) {
                items.push(card);
            }
        }
        page += 1;
    }
    cards.finish(&app);
    Ok(Json(ListAnswer {
        items,
        next_page: (more && page <= 500).then_some(page),
        freshness: Freshness::of(times, now),
    }))
}

/// A genre to filter by; TMDB's movie and series genres share ids where they match.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct GenreChoice {
    pub id: i64,
    pub name: String,
    pub movie: bool,
    pub series: bool,
}

/// `GET /api/discover/genres`.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct GenresAnswer {
    /// By name.
    pub genres: Vec<GenreChoice>,
    pub freshness: Freshness,
}

pub async fn genres(State(app): State<SharedApp>) -> Result<Json<GenresAnswer>, ApiError> {
    let tmdb = tmdb_client(&app)?;
    let now = Timestamp::now();
    let mut genres: Vec<GenreChoice> = Vec::new();
    let mut times = Vec::new();
    for kind in [Kind::Movie, Kind::Series] {
        let cached = app
            .discover
            .genres
            .load(&format!("genres/{}", kind.path()), now, || {
                tmdb.genres(kind)
            })
            .await
            .map_err(|f| failure(f, "TMDB kennt keine Genres"))?;
        times.push((cached.fetched_at, cached.stale));
        for genre in cached.value.iter() {
            match genres.iter_mut().find(|g| g.id == genre.id) {
                Some(known) => known.series |= kind == Kind::Series,
                None => genres.push(GenreChoice {
                    id: genre.id,
                    name: genre.name.clone(),
                    movie: kind == Kind::Movie,
                    series: kind == Kind::Series,
                }),
            }
        }
    }
    genres.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(GenresAnswer {
        genres,
        freshness: Freshness::of(times, now),
    }))
}
