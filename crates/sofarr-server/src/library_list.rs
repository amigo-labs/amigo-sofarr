//! The movie and series lists (spec web-ui-library AC-02 to AC-04, AC-06): filtered,
//! searched, sorted and paged on the server, and the options the filters offer.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
    http::StatusCode,
};
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sofarr_core::normalize_title;
use sofarr_db::{Episode, ImdbRating, Movie, Presentation, Series};

use crate::{
    api::ApiError,
    app::{App, SharedApp},
    library,
    library_view::{Judge, today},
    series,
};

/// A yes/no/any filter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Tri {
    Yes,
    No,
    #[default]
    Any,
}

impl Tri {
    fn admits(self, value: bool) -> bool {
        match self {
            Self::Yes => value,
            Self::No => !value,
            Self::Any => true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    #[default]
    Title,
    Year,
    Added,
    Size,
    /// IMDb rating, higher first (votes break ties); titles without one last.
    Rating,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Order {
    #[default]
    Asc,
    Desc,
}

/// Query of `GET /api/library/movies`; all filters are combined with AND.
#[derive(Debug, Default, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(default, deny_unknown_fields)]
pub struct MovieListQuery {
    pub monitored: Tri,
    pub missing: Tri,
    pub cutoff_unmet: Tri,
    pub has_file: Tri,
    /// Quality profile id.
    pub profile: Option<i64>,
    /// Tag id.
    pub tag: Option<i64>,
    /// Part of the title, original, German or an alternative title.
    pub q: Option<String>,
    /// Lowest IMDb rating (0 to 10); hides titles without a rating.
    pub min_rating: Option<f32>,
    /// Lowest number of IMDb votes; hides titles without a rating.
    pub min_votes: Option<u32>,
    pub sort: Sort,
    pub order: Order,
    /// From 1.
    pub page: Option<u32>,
    /// 1 to 500, default 50.
    pub page_size: Option<u32>,
}

/// Query of `GET /api/library/series`.
#[derive(Debug, Default, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(default, deny_unknown_fields)]
pub struct SeriesListQuery {
    pub monitored: Tri,
    pub missing: Tri,
    pub cutoff_unmet: Tri,
    pub complete: Tri,
    pub profile: Option<i64>,
    pub tag: Option<i64>,
    pub q: Option<String>,
    pub min_rating: Option<f32>,
    pub min_votes: Option<u32>,
    pub sort: Sort,
    pub order: Order,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct MovieListItem {
    pub id: i64,
    pub tmdb_id: i64,
    pub title: String,
    pub title_de: Option<String>,
    pub original_title: Option<String>,
    pub year: Option<u16>,
    pub monitored: bool,
    /// Quality of the file; `null`: no file.
    pub quality: Option<String>,
    pub size: Option<u64>,
    pub missing: bool,
    pub cutoff_unmet: bool,
    pub available: bool,
    /// Whether the provider offers a poster (`/api/images/movie/{id}/list`).
    pub has_poster: bool,
    pub quality_profile_id: i64,
    pub tags: Vec<i64>,
    pub added_at: String,
    /// IMDb rating; `null`: no IMDb id, no rating in the data, or ratings switched off.
    pub imdb_rating: Option<f32>,
    pub imdb_votes: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SeriesListItem {
    pub id: i64,
    pub tvdb_id: i64,
    pub title: String,
    pub title_de: Option<String>,
    pub year: Option<u16>,
    pub monitored: bool,
    pub series_type: String,
    pub episodes: u32,
    pub monitored_episodes: u32,
    /// Episodes with a file.
    pub files: u32,
    pub missing: u32,
    pub cutoff_unmet: u32,
    pub complete: bool,
    pub size: u64,
    pub has_poster: bool,
    pub quality_profile_id: i64,
    pub tags: Vec<i64>,
    pub added_at: String,
    pub imdb_rating: Option<f32>,
    pub imdb_votes: Option<u32>,
}

/// One page of a list.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Page<T> {
    pub page: u32,
    pub page_size: u32,
    /// Matching items over all pages.
    pub total: u64,
    pub items: Vec<T>,
}

pub type MoviePage = Page<MovieListItem>;
pub type SeriesPage = Page<SeriesListItem>;

const BAD_LIST: &str = "Ungültige Listenanfrage: Filter yes/no/any, sort title/year/added/size/rating, \
     order asc/desc, min_rating 0 bis 10, min_votes ab 0, page ab 1, page_size von 1 bis 500";

/// Sorting or filtering by rating while `[imdb] enabled = false` (spec imdb-ratings AC-09).
const RATINGS_OFF: &str = "IMDb-Bewertungen sind ausgeschaltet ([imdb] enabled = false)";

/// The IMDb rating of a list item: rating and votes.
fn rated(rating: Option<f32>, votes: Option<u32>) -> Option<(f32, u32)> {
    rating.zip(votes)
}

/// Whether a rated item reaches the minimum rating and votes; an unrated one never
/// does while either is set (spec imdb-ratings AC-07).
fn reaches(rating: Option<(f32, u32)>, min_rating: Option<f32>, min_votes: Option<u32>) -> bool {
    if min_rating.is_none() && min_votes.is_none() {
        return true;
    }
    rating
        .is_some_and(|(r, v)| min_rating.is_none_or(|m| r >= m) && min_votes.is_none_or(|m| v >= m))
}

/// Page number and size of a request (checked, German message).
pub fn paging(page: Option<u32>, size: Option<u32>) -> Result<(u32, u32), ApiError> {
    let page = page.unwrap_or(1);
    let size = size.unwrap_or(50);
    if page == 0 || !(1..=500).contains(&size) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, BAD_LIST));
    }
    Ok((page, size))
}

/// The items of one page.
pub fn page_of<T>(items: Vec<T>, page: u32, size: u32) -> Page<T> {
    let total = items.len() as u64;
    let skip = usize::try_from(u64::from(page - 1) * u64::from(size)).unwrap_or(usize::MAX);
    Page {
        page,
        page_size: size,
        total,
        items: items
            .into_iter()
            .skip(skip)
            .take(usize::try_from(size).unwrap_or(usize::MAX))
            .collect(),
    }
}

/// A search over titles: normalized (case, punctuation) substring.
struct Needle(Option<String>);

impl Needle {
    fn new(q: Option<&str>) -> Self {
        Self(q.map(normalize_title).filter(|n| !n.is_empty()))
    }

    fn matches(&self, keys: &[String]) -> bool {
        self.0
            .as_ref()
            .is_none_or(|needle| keys.iter().any(|k| k.contains(needle.as_str())))
    }
}

/// Normalized search keys of titles.
fn keys<'a>(titles: impl IntoIterator<Item = &'a String>) -> Vec<String> {
    let mut keys: Vec<String> = titles.into_iter().map(|t| normalize_title(t)).collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// A list item with what filtering and sorting need, computed once per library version.
#[derive(Debug, Clone)]
pub struct Row<T> {
    pub item: T,
    keys: Vec<String>,
    sort_title: String,
    added: i64,
}

/// The computed list of a library version and day.
#[derive(Debug)]
pub struct Snapshot<T> {
    version: i64,
    today: Date,
    /// When the ratings data in the rows was loaded.
    ratings: Option<jiff::Timestamp>,
    rows: Vec<Row<T>>,
}

/// The last computed movie and series lists (spec web-ui-library AC-04): a filter or
/// search change only filters these again until the library changes.
#[derive(Debug, Default)]
pub struct ListCache {
    movies: Mutex<Option<Arc<Snapshot<MovieListItem>>>>,
    series: Mutex<Option<Arc<Snapshot<SeriesListItem>>>>,
}

fn cached<T>(
    slot: &Mutex<Option<Arc<Snapshot<T>>>>,
    version: i64,
    today: Date,
    ratings: Option<jiff::Timestamp>,
) -> Option<Arc<Snapshot<T>>> {
    slot.lock()
        .ok()?
        .as_ref()
        .filter(|s| s.version == version && s.today == today && s.ratings == ratings)
        .cloned()
}

fn keep<T>(slot: &Mutex<Option<Arc<Snapshot<T>>>>, snapshot: &Arc<Snapshot<T>>) {
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(Arc::clone(snapshot));
    }
}

/// How a page is ordered: by the chosen key, then by title and id so it is stable.
struct Ordering<T> {
    sort: Sort,
    order: Order,
    id: fn(&T) -> i64,
    year: fn(&T) -> Option<u16>,
    bytes: fn(&T) -> u64,
    rating: fn(&T) -> Option<(f32, u32)>,
}

impl<T: Clone> Ordering<T> {
    fn primary(&self, a: &Row<T>, b: &Row<T>) -> std::cmp::Ordering {
        match self.sort {
            Sort::Title => a.sort_title.cmp(&b.sort_title),
            Sort::Year => (self.year)(&a.item)
                .unwrap_or(0)
                .cmp(&(self.year)(&b.item).unwrap_or(0)),
            Sort::Added => a.added.cmp(&b.added),
            Sort::Size => (self.bytes)(&a.item).cmp(&(self.bytes)(&b.item)),
            // Higher first in the default order (spec imdb-ratings AC-06).
            Sort::Rating => {
                let key = |i: &T| (self.rating)(i).unwrap_or((0.0, 0));
                let (ra, va) = key(&b.item);
                let (rb, vb) = key(&a.item);
                ra.total_cmp(&rb).then(va.cmp(&vb))
            }
        }
    }

    /// One page of the filtered rows.
    fn page(&self, mut rows: Vec<&Row<T>>, page: u32, size: u32) -> Page<T> {
        rows.sort_by(|a, b| {
            // Titles without a rating come last in either order.
            let unrated = |r: &Row<T>| (self.rating)(&r.item).is_none();
            if self.sort == Sort::Rating && unrated(a) != unrated(b) {
                return unrated(a).cmp(&unrated(b));
            }
            let primary = match self.order {
                Order::Asc => self.primary(a, b),
                Order::Desc => self.primary(b, a),
            };
            primary
                .then_with(|| a.sort_title.cmp(&b.sort_title))
                .then_with(|| (self.id)(&a.item).cmp(&(self.id)(&b.item)))
        });
        let page = page_of(rows, page, size);
        Page {
            page: page.page,
            page_size: page.page_size,
            total: page.total,
            items: page.items.into_iter().map(|r| r.item.clone()).collect(),
        }
    }
}

pub async fn movie_judge(app: &App) -> Judge {
    Judge::new(
        library::load_rules(&app.db).await.ok(),
        today(jiff::Timestamp::now()),
    )
}

pub async fn series_judge(app: &App) -> Judge {
    Judge::new(
        series::load_rules(&app.db).await.ok(),
        today(jiff::Timestamp::now()),
    )
}

fn list_query<T>(query: Result<Query<T>, QueryRejection>) -> Result<T, ApiError> {
    query
        .map(|Query(q)| q)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, BAD_LIST))
}

pub fn movie_item(
    movie: &Movie,
    judge: &Judge,
    p: Option<&Presentation>,
    rating: Option<&ImdbRating>,
) -> MovieListItem {
    let state = judge.movie(movie);
    MovieListItem {
        id: movie.id,
        tmdb_id: movie.tmdb_id,
        title: movie.title.clone(),
        title_de: movie.title_de.clone(),
        original_title: movie.original_title.clone(),
        year: movie.year,
        monitored: movie.monitored,
        quality: movie.file.as_ref().map(|f| f.quality.clone()),
        size: movie.file.as_ref().map(|f| f.size),
        missing: state.missing,
        cutoff_unmet: state.cutoff_unmet,
        available: state.available,
        has_poster: p.is_some_and(|p| p.poster_path.is_some()),
        quality_profile_id: movie.quality_profile_id,
        tags: movie.tags.clone(),
        added_at: p.map(|p| p.added_at.to_string()).unwrap_or_default(),
        imdb_rating: rating.map(|r| r.rating),
        imdb_votes: rating.map(|r| r.votes),
    }
}

/// The rows of every movie.
pub fn movie_rows(
    movies: &[Movie],
    presentation: &HashMap<i64, Presentation>,
    judge: &Judge,
    ratings: &HashMap<String, ImdbRating>,
) -> Vec<Row<MovieListItem>> {
    movies
        .iter()
        .map(|m| {
            let p = presentation.get(&m.id);
            Row {
                item: movie_item(
                    m,
                    judge,
                    p,
                    m.imdb_id.as_ref().and_then(|id| ratings.get(id)),
                ),
                keys: keys(
                    std::iter::once(&m.title)
                        .chain(&m.original_title)
                        .chain(&m.title_de)
                        .chain(&m.alternative_titles),
                ),
                sort_title: m.title.to_lowercase(),
                added: p.map_or(0, |p| p.added_at.as_second()),
            }
        })
        .collect()
}

/// Filters, searches, sorts and pages the movie rows.
pub fn movie_page(
    rows: &[Row<MovieListItem>],
    q: &MovieListQuery,
    page: u32,
    size: u32,
) -> MoviePage {
    let needle = Needle::new(q.q.as_deref());
    let matching: Vec<&Row<MovieListItem>> = rows
        .iter()
        .filter(|r| {
            let i = &r.item;
            q.monitored.admits(i.monitored)
                && q.has_file.admits(i.quality.is_some())
                && q.missing.admits(i.missing)
                && q.cutoff_unmet.admits(i.cutoff_unmet)
                && q.profile.is_none_or(|p| i.quality_profile_id == p)
                && q.tag.is_none_or(|t| i.tags.contains(&t))
                && reaches(
                    rated(i.imdb_rating, i.imdb_votes),
                    q.min_rating,
                    q.min_votes,
                )
                && needle.matches(&r.keys)
        })
        .collect();
    Ordering {
        sort: q.sort,
        order: q.order,
        id: |i: &MovieListItem| i.id,
        year: |i: &MovieListItem| i.year,
        bytes: |i: &MovieListItem| i.size.unwrap_or(0),
        rating: |i: &MovieListItem| rated(i.imdb_rating, i.imdb_votes),
    }
    .page(matching, page, size)
}

pub fn series_item(
    show: &Series,
    episodes: &[&Episode],
    judge: &Judge,
    p: Option<&Presentation>,
    rating: Option<&ImdbRating>,
) -> SeriesListItem {
    let counts = judge.series(show, episodes);
    SeriesListItem {
        id: show.id,
        tvdb_id: show.data.tvdb_id,
        title: show.data.title.clone(),
        title_de: show.data.title_de.clone(),
        year: show.data.year,
        monitored: show.data.monitored,
        series_type: show.data.series_type.clone(),
        episodes: counts.episodes,
        monitored_episodes: counts.monitored,
        files: counts.files,
        missing: counts.missing,
        cutoff_unmet: counts.cutoff_unmet,
        complete: counts.complete(),
        size: counts.size,
        has_poster: p.is_some_and(|p| p.poster_path.is_some()),
        quality_profile_id: show.data.quality_profile_id,
        tags: show.tags.clone(),
        added_at: p.map(|p| p.added_at.to_string()).unwrap_or_default(),
        imdb_rating: rating.map(|r| r.rating),
        imdb_votes: rating.map(|r| r.votes),
    }
}

/// The rows of every series.
pub fn series_rows(
    shows: &[Series],
    episodes: &[Episode],
    presentation: &HashMap<i64, Presentation>,
    judge: &Judge,
    ratings: &HashMap<String, ImdbRating>,
) -> Vec<Row<SeriesListItem>> {
    let mut by_series: HashMap<i64, Vec<&Episode>> = HashMap::new();
    for e in episodes {
        by_series.entry(e.series_id).or_default().push(e);
    }
    shows
        .iter()
        .map(|s| {
            let p = presentation.get(&s.id);
            let eps = by_series.get(&s.id).map_or(&[][..], Vec::as_slice);
            Row {
                item: series_item(
                    s,
                    eps,
                    judge,
                    p,
                    s.data.imdb_id.as_ref().and_then(|id| ratings.get(id)),
                ),
                keys: keys(
                    std::iter::once(&s.data.title)
                        .chain(&s.data.title_de)
                        .chain(&s.data.alternative_titles),
                ),
                sort_title: s.data.title.to_lowercase(),
                added: p.map_or(0, |p| p.added_at.as_second()),
            }
        })
        .collect()
}

pub fn series_page(
    rows: &[Row<SeriesListItem>],
    q: &SeriesListQuery,
    page: u32,
    size: u32,
) -> SeriesPage {
    let needle = Needle::new(q.q.as_deref());
    let matching: Vec<&Row<SeriesListItem>> = rows
        .iter()
        .filter(|r| {
            let i = &r.item;
            q.monitored.admits(i.monitored)
                && q.missing.admits(i.missing > 0)
                && q.cutoff_unmet.admits(i.cutoff_unmet > 0)
                && q.complete.admits(i.complete)
                && q.profile.is_none_or(|p| i.quality_profile_id == p)
                && q.tag.is_none_or(|t| i.tags.contains(&t))
                && reaches(
                    rated(i.imdb_rating, i.imdb_votes),
                    q.min_rating,
                    q.min_votes,
                )
                && needle.matches(&r.keys)
        })
        .collect();
    Ordering {
        sort: q.sort,
        order: q.order,
        id: |i: &SeriesListItem| i.id,
        year: |i: &SeriesListItem| i.year,
        bytes: |i: &SeriesListItem| i.size,
        rating: |i: &SeriesListItem| rated(i.imdb_rating, i.imdb_votes),
    }
    .page(matching, page, size)
}

impl App {
    /// When the ratings data was loaded and the ratings of the library; nothing while
    /// `[imdb] enabled = false`. The ratings are only read when the lists are computed.
    async fn library_ratings(
        &self,
    ) -> anyhow::Result<(Option<jiff::Timestamp>, HashMap<String, ImdbRating>)> {
        if !self.config.imdb.enabled {
            return Ok((None, HashMap::new()));
        }
        let at = self.db.imdb_loaded_at().await?;
        let ratings = match at {
            Some(_) => self.db.library_imdb_ratings().await?,
            None => HashMap::new(),
        };
        Ok((at, ratings))
    }

    /// The movie rows of the current library version (computed again when it changed).
    pub async fn movie_snapshot(&self) -> anyhow::Result<Arc<Snapshot<MovieListItem>>> {
        let version = self.db.library_version().await?;
        let today = today(jiff::Timestamp::now());
        let (ratings_at, ratings) = self.library_ratings().await?;
        if let Some(snapshot) = cached(&self.lists.movies, version, today, ratings_at) {
            return Ok(snapshot);
        }
        let movies = self.db.movies().await?;
        let presentation = self.db.movie_presentation().await?;
        let judge = movie_judge(self).await;
        let snapshot = Arc::new(Snapshot {
            version,
            today,
            ratings: ratings_at,
            rows: movie_rows(&movies, &presentation, &judge, &ratings),
        });
        keep(&self.lists.movies, &snapshot);
        Ok(snapshot)
    }

    /// The series rows of the current library version.
    pub async fn series_snapshot(&self) -> anyhow::Result<Arc<Snapshot<SeriesListItem>>> {
        let version = self.db.library_version().await?;
        let today = today(jiff::Timestamp::now());
        let (ratings_at, ratings) = self.library_ratings().await?;
        if let Some(snapshot) = cached(&self.lists.series, version, today, ratings_at) {
            return Ok(snapshot);
        }
        let shows = self.db.all_series().await?;
        let episodes = self.db.episodes(None).await?;
        let presentation = self.db.series_presentation().await?;
        let judge = series_judge(self).await;
        let snapshot = Arc::new(Snapshot {
            version,
            today,
            ratings: ratings_at,
            rows: series_rows(&shows, &episodes, &presentation, &judge, &ratings),
        });
        keep(&self.lists.series, &snapshot);
        Ok(snapshot)
    }
}

/// Rating sort and filters need ratings switched on and a minimum within range.
fn check_ratings(
    app: &App,
    sort: Sort,
    min_rating: Option<f32>,
    min_votes: Option<u32>,
) -> Result<(), ApiError> {
    let by_rating = sort == Sort::Rating || min_rating.is_some() || min_votes.is_some();
    if by_rating && !app.config.imdb.enabled {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, RATINGS_OFF));
    }
    if min_rating.is_some_and(|r| !(0.0..=10.0).contains(&r)) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, BAD_LIST));
    }
    Ok(())
}

pub async fn movies(
    State(app): State<SharedApp>,
    query: Result<Query<MovieListQuery>, QueryRejection>,
) -> Result<Json<MoviePage>, ApiError> {
    let q = list_query(query)?;
    let (page, size) = paging(q.page, q.page_size)?;
    check_ratings(&app, q.sort, q.min_rating, q.min_votes)?;
    let snapshot = app.movie_snapshot().await?;
    Ok(Json(movie_page(&snapshot.rows, &q, page, size)))
}

pub async fn shows(
    State(app): State<SharedApp>,
    query: Result<Query<SeriesListQuery>, QueryRejection>,
) -> Result<Json<SeriesPage>, ApiError> {
    let q = list_query(query)?;
    let (page, size) = paging(q.page, q.page_size)?;
    check_ratings(&app, q.sort, q.min_rating, q.min_votes)?;
    let snapshot = app.series_snapshot().await?;
    Ok(Json(series_page(&snapshot.rows, &q, page, size)))
}

/// A quality profile or tag to choose from.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Choice {
    pub id: i64,
    pub name: String,
}

/// What the filters and settings dialogs offer.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct LibraryOptions {
    pub movie_profiles: Vec<Choice>,
    pub series_profiles: Vec<Choice>,
    pub movie_tags: Vec<Choice>,
    pub series_tags: Vec<Choice>,
    /// `announced`, `inCinemas`, `released`.
    pub minimum_availability: Vec<String>,
    /// Whether file actions (rename, move, delete files) can run (live mode).
    pub file_actions: bool,
}

fn profiles(judge: &Judge) -> Vec<Choice> {
    judge.rules.as_ref().map_or_else(Vec::new, |r| {
        r.config
            .profiles
            .iter()
            .map(|p| Choice {
                id: i64::from(p.id.0),
                name: p.name.clone(),
            })
            .collect()
    })
}

fn tags(setting: Option<&Value>) -> Vec<Choice> {
    setting
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|t| {
            Some(Choice {
                id: t["id"].as_i64()?,
                name: t["label"].as_str()?.to_owned(),
            })
        })
        .collect()
}

pub async fn options(State(app): State<SharedApp>) -> Result<Json<LibraryOptions>, ApiError> {
    let movie_tags = app.db.setting("tag").await?;
    let series_tags = app.db.setting(&format!("{}tag", series::PREFIX)).await?;
    Ok(Json(LibraryOptions {
        movie_profiles: profiles(&movie_judge(&app).await),
        series_profiles: profiles(&series_judge(&app).await),
        movie_tags: tags(movie_tags.as_ref()),
        series_tags: tags(series_tags.as_ref()),
        minimum_availability: ["announced", "inCinemas", "released"]
            .map(str::to_owned)
            .to_vec(),
        file_actions: app.config.automation.mode == crate::config::Mode::Live,
    }))
}
