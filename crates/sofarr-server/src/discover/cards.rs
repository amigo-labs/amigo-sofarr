//! Titles as the discover pages show them: poster, title, year, IMDb rating and library
//! state (spec discover AC-01, AC-03, AC-05). A title whose IMDb id or TVDB id is not
//! known yet gets its detail loaded in the background.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use sofarr_db::{MovieKey, SeriesKey};
use sofarr_metadata::browse::{Kind, TitleDetail, TitleSummary};

use super::titles::title_key;
use crate::{app::App, import_lists, series};

/// Movie or series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum TitleKind {
    Movie,
    Series,
}

impl TitleKind {
    pub fn tmdb(self) -> Kind {
        match self {
            TitleKind::Movie => Kind::Movie,
            TitleKind::Series => Kind::Series,
        }
    }

    pub fn of(kind: Kind) -> Self {
        match kind {
            Kind::Movie => TitleKind::Movie,
            Kind::Series => TitleKind::Series,
        }
    }

    /// From the path segment `movie` or `series`.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "movie" => Some(TitleKind::Movie),
            "series" => Some(TitleKind::Series),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum LibraryStatus {
    None,
    InLibrary,
    /// On the import list exclusion list.
    Excluded,
}

/// Whether a title is in the library (with its library id) or excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct LibraryState {
    pub status: LibraryStatus,
    /// The id of the library movie or series.
    pub id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum RatingState {
    /// `[imdb] enabled = false`.
    Off,
    /// The IMDb id is still loading (placeholder).
    Pending,
    /// No IMDb id or no rating („–“).
    None,
    Rated,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Rating {
    pub state: RatingState,
    pub rating: Option<f32>,
    pub votes: Option<u32>,
}

impl Rating {
    fn of(state: RatingState) -> Self {
        Self {
            state,
            rating: None,
            votes: None,
        }
    }

    /// Whether a minimum rating keeps the title: a known rating reaching it, or one
    /// still loading (AC-02).
    pub fn reaches(&self, minimum: f32) -> bool {
        match self.state {
            RatingState::Pending => true,
            RatingState::Rated => self.rating.is_some_and(|r| r >= minimum),
            RatingState::Off | RatingState::None => false,
        }
    }
}

/// A movie or series of a shelf, list, search or person page.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct TitleCard {
    pub kind: TitleKind,
    pub tmdb_id: i64,
    /// German title where TMDB has one, else the original one.
    pub title: String,
    pub original_title: String,
    pub year: Option<u16>,
    /// sofarr's image URL of the poster (D24).
    pub poster: Option<String>,
    /// TMDB's popularity value.
    pub popularity: f64,
    pub library: LibraryState,
    pub rating: Rating,
}

/// The library as the discover pages look it up.
#[derive(Debug, Default)]
pub struct Library {
    movies: HashMap<i64, MovieKey>,
    series_by_tmdb: HashMap<i64, SeriesKey>,
    series_by_tvdb: HashMap<i64, SeriesKey>,
    excluded_movies: BTreeSet<i64>,
    /// TVDB ids.
    excluded_series: BTreeSet<i64>,
    ratings: bool,
}

/// TVDB ids on Sonarr's import list exclusion list.
pub fn series_exclusions(setting: Option<&serde_json::Value>) -> BTreeSet<i64> {
    setting
        .and_then(serde_json::Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|e| e["tvdbId"].as_i64())
        .collect()
}

impl Library {
    pub async fn load(app: &App) -> anyhow::Result<Self> {
        let series = app.db.series_keys().await?;
        Ok(Self {
            movies: app
                .db
                .movie_keys()
                .await?
                .into_iter()
                .map(|m| (m.tmdb_id, m))
                .collect(),
            series_by_tmdb: series
                .iter()
                .filter_map(|s| Some((s.tmdb_id?, s.clone())))
                .collect(),
            series_by_tvdb: series.into_iter().map(|s| (s.tvdb_id, s)).collect(),
            excluded_movies: import_lists::exclusions(app.db.setting("exclusions").await?.as_ref()),
            excluded_series: series_exclusions(
                app.db
                    .setting(&format!("{}importlistexclusion", series::PREFIX))
                    .await?
                    .as_ref(),
            ),
            ratings: app.config.imdb.enabled,
        })
    }

    /// The library state and the library's IMDb id of a title; a series is found by its
    /// TMDB id or, once its detail is known, by its TVDB id.
    pub fn state(
        &self,
        kind: Kind,
        tmdb_id: i64,
        detail: Option<&TitleDetail>,
    ) -> (LibraryState, Option<String>) {
        let found = |id: i64| LibraryState {
            status: LibraryStatus::InLibrary,
            id: Some(id),
        };
        let none = LibraryState {
            status: LibraryStatus::None,
            id: None,
        };
        let excluded = LibraryState {
            status: LibraryStatus::Excluded,
            id: None,
        };
        match kind {
            Kind::Movie => match self.movies.get(&tmdb_id) {
                Some(m) => (found(m.id), m.imdb_id.clone()),
                None if self.excluded_movies.contains(&tmdb_id) => (excluded, None),
                None => (none, None),
            },
            Kind::Series => {
                let tvdb = detail.and_then(|d| d.tvdb_id);
                let known = self
                    .series_by_tmdb
                    .get(&tmdb_id)
                    .or_else(|| tvdb.and_then(|id| self.series_by_tvdb.get(&id)));
                match known {
                    Some(s) => (found(s.id), s.imdb_id.clone()),
                    None if tvdb.is_some_and(|id| self.excluded_series.contains(&id)) => {
                        (excluded, None)
                    }
                    None => (none, None),
                }
            }
        }
    }
}

/// Builds cards and collects the titles whose detail must be loaded.
pub struct Cards<'a> {
    app: &'a App,
    pub library: Library,
    /// Titles to load in the background.
    pub missing: Vec<(Kind, i64)>,
}

/// sofarr's URL of a TMDB image in one size (D24).
pub fn image(app: &App, path: Option<&str>, size: &str) -> Option<String> {
    app.images.tmdb_url(size, path?)
}

impl<'a> Cards<'a> {
    pub async fn new(app: &'a App) -> anyhow::Result<Self> {
        Ok(Self {
            app,
            library: Library::load(app).await?,
            missing: Vec::new(),
        })
    }

    /// Library state and rating of a title.
    pub async fn state(
        &mut self,
        kind: Kind,
        tmdb_id: i64,
    ) -> anyhow::Result<(LibraryState, Rating)> {
        let key = title_key(kind, tmdb_id);
        let detail = self.app.discover.titles.any(&key);
        let detail = detail.as_ref().map(|c| c.value.as_ref());
        // TMDB knows no detail: nothing to wait for.
        let unknown = detail.is_none()
            && self
                .app
                .discover
                .unknown
                .lock()
                .is_ok_and(|u| u.contains(&key));
        let (library, library_imdb) = self.library.state(kind, tmdb_id, detail);
        let imdb_id = library_imdb.or_else(|| detail.and_then(|d| d.imdb_id.clone()));
        let unknown_series = kind == Kind::Series && library.status == LibraryStatus::None;
        if detail.is_none()
            && !unknown
            && ((self.library.ratings && imdb_id.is_none()) || unknown_series)
        {
            self.missing.push((kind, tmdb_id));
        }
        let rating = if !self.library.ratings {
            Rating::of(RatingState::Off)
        } else if let Some(id) = imdb_id {
            match self.app.db.imdb_rating(&id).await? {
                Some(r) => Rating {
                    state: RatingState::Rated,
                    rating: Some(r.rating),
                    votes: Some(r.votes),
                },
                None => Rating::of(RatingState::None),
            }
        } else if detail.is_some() || unknown {
            Rating::of(RatingState::None)
        } else {
            Rating::of(RatingState::Pending)
        };
        Ok((library, rating))
    }

    pub async fn card(&mut self, title: &TitleSummary) -> anyhow::Result<TitleCard> {
        let (library, rating) = self.state(title.kind, title.tmdb_id).await?;
        Ok(TitleCard {
            kind: TitleKind::of(title.kind),
            tmdb_id: title.tmdb_id,
            title: title.title.clone(),
            original_title: title.original_title.clone(),
            year: title.year,
            poster: image(self.app, title.poster_path.as_deref(), "w342"),
            popularity: title.popularity,
            library,
            rating,
        })
    }

    /// Starts the background loads of the titles seen without detail.
    pub fn finish(self, app: &crate::app::SharedApp) {
        super::background::enqueue(app, self.missing);
    }
}
