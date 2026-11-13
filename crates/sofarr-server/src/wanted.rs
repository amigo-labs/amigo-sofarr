//! The missing page (spec web-ui-library AC-08, AC-09): missing and cutoff-unmet movies
//! and episodes with their reason, and searches for one, several or all of a list.

use std::collections::BTreeMap;

use axum::{
    Json,
    extract::{
        Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use sofarr_db::Episode;

use crate::{
    api::ApiError,
    app::{App, SharedApp},
    library_edit::{ItemKind, episode_label, movie_label},
    library_list::{Page, movie_judge, page_of, paging, series_judge},
    search::{Scope, SearchError, SearchReport, SearchTarget},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum WantedList {
    #[default]
    Missing,
    CutoffUnmet,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum WantedMedia {
    #[default]
    Movies,
    Episodes,
}

/// Query of `GET /api/library/wanted`.
#[derive(Debug, Default, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(default, deny_unknown_fields)]
pub struct WantedQuery {
    pub list: WantedList,
    pub media: WantedMedia,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct WantedItem {
    pub kind: ItemKind,
    /// Movie id or episode id.
    pub id: i64,
    pub series_id: Option<i64>,
    pub season: Option<u16>,
    pub episode: Option<u16>,
    /// E.g. `Erster Film (2001)` or `Kleine Insel S01E02 – Sturm`.
    pub label: String,
    pub profile: String,
    /// `fehlt`, or the file's quality and the cutoff it misses.
    pub reason: String,
    pub quality: Option<String>,
    /// Movies: none; episodes: the air date.
    pub air_date: Option<String>,
}

const BAD_WANTED: &str = "Ungültige Anfrage: list missing/cutoff_unmet, media movies/episodes, \
     page ab 1, page_size von 1 bis 500";

const MISSING: &str = "fehlt";

/// Every item of one list, in page order (movies by title, episodes by series and number).
pub async fn wanted_items(
    app: &App,
    list: WantedList,
    media: WantedMedia,
) -> anyhow::Result<Vec<WantedItem>> {
    let mut items = Vec::new();
    match media {
        WantedMedia::Movies => {
            let judge = movie_judge(app).await;
            let mut movies = app.db.movies().await?;
            movies.sort_by_cached_key(|m| (m.title.to_lowercase(), m.id));
            for m in &movies {
                let state = judge.movie(m);
                let reason = match (list, &m.file) {
                    (WantedList::Missing, _) if state.missing => MISSING.to_owned(),
                    (WantedList::CutoffUnmet, Some(f)) if state.cutoff_unmet => {
                        judge.cutoff_reason(m.quality_profile_id, f)
                    }
                    _ => continue,
                };
                items.push(WantedItem {
                    kind: ItemKind::Movie,
                    id: m.id,
                    series_id: None,
                    season: None,
                    episode: None,
                    label: movie_label(m),
                    profile: judge.profile_name(m.quality_profile_id),
                    reason,
                    quality: m.file.as_ref().map(|f| f.quality.clone()),
                    air_date: None,
                });
            }
        }
        WantedMedia::Episodes => {
            let judge = series_judge(app).await;
            let today = judge.today_text();
            let mut shows = app.db.all_series().await?;
            shows.sort_by_cached_key(|s| (s.data.title.to_lowercase(), s.id));
            let episodes = app.db.episodes(None).await?;
            let mut by_series: BTreeMap<i64, Vec<&Episode>> = BTreeMap::new();
            for e in &episodes {
                by_series.entry(e.series_id).or_default().push(e);
            }
            for show in &shows {
                for e in by_series.get(&show.id).map_or(&[][..], Vec::as_slice) {
                    let state = judge.episode(show, e, &today);
                    let reason = match (list, &e.file) {
                        (WantedList::Missing, _) if state.missing => MISSING.to_owned(),
                        (WantedList::CutoffUnmet, Some(f)) if state.cutoff_unmet => {
                            judge.cutoff_reason(show.data.quality_profile_id, f)
                        }
                        _ => continue,
                    };
                    items.push(WantedItem {
                        kind: ItemKind::Episode,
                        id: e.id,
                        series_id: Some(show.id),
                        season: Some(e.data.season),
                        episode: Some(e.data.number),
                        label: episode_label(show, e),
                        profile: judge.profile_name(show.data.quality_profile_id),
                        reason,
                        quality: e.file.as_ref().map(|f| f.quality.clone()),
                        air_date: e.data.air_date.clone(),
                    });
                }
            }
        }
    }
    Ok(items)
}

pub async fn wanted(
    State(app): State<SharedApp>,
    query: Result<Query<WantedQuery>, QueryRejection>,
) -> Result<Json<Page<WantedItem>>, ApiError> {
    let Query(q) = query.map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, BAD_WANTED))?;
    let (page, size) = paging(q.page, q.page_size)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, BAD_WANTED))?;
    let items = wanted_items(&app, q.list, q.media).await?;
    Ok(Json(page_of(items, page, size)))
}

/// One target of a bulk search, like the body of `POST /api/command/search`.
#[derive(Debug, Clone, Copy, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct SearchItem {
    pub movie_id: Option<i64>,
    pub series_id: Option<i64>,
    pub season: Option<u16>,
    pub episode: Option<u16>,
}

/// A whole wanted list.
#[derive(Debug, Clone, Copy, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct WantedSelector {
    pub list: WantedList,
    pub media: WantedMedia,
}

/// Body of `POST /api/command/search-bulk`: either `items` or `all`.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct BulkSearchBody {
    pub items: Option<Vec<SearchItem>>,
    pub all: Option<WantedSelector>,
}

/// Outcome of a bulk search.
#[derive(Debug, Default, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct BulkSearchReport {
    /// Searches run (one per movie, one per series with its seasons and episodes).
    pub searched: u32,
    pub releases: u32,
    pub grabs: u32,
    pub errors: Vec<String>,
    pub notes: Vec<String>,
}

impl BulkSearchReport {
    fn add(&mut self, report: SearchReport) {
        self.searched += 1;
        self.releases += u32::try_from(report.releases).unwrap_or(u32::MAX);
        self.grabs += u32::try_from(report.grabs).unwrap_or(u32::MAX);
        self.errors.extend(report.errors);
        self.notes.extend(report.note);
    }
}

const BAD_BULK: &str = "Ungültige Suchanfrage: entweder items (Liste von Zielen wie bei \
     /api/command/search) oder all (list und media) angeben";

impl SearchItem {
    fn target(self) -> Option<SearchTarget> {
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

/// The searches for a set of episodes: per series, a season search where two or more
/// episodes of a season are wanted, else one per episode.
fn episode_scopes(items: &[WantedItem]) -> BTreeMap<i64, Vec<Scope>> {
    let mut seasons: BTreeMap<(i64, u16), Vec<i64>> = BTreeMap::new();
    for i in items {
        if let (Some(series), Some(season)) = (i.series_id, i.season) {
            seasons.entry((series, season)).or_default().push(i.id);
        }
    }
    let mut out: BTreeMap<i64, Vec<Scope>> = BTreeMap::new();
    for ((series, season), episodes) in seasons {
        let scopes = out.entry(series).or_default();
        if episodes.len() >= 2 {
            scopes.push(Scope::Season(season));
        } else {
            scopes.extend(episodes.into_iter().map(Scope::Episode));
        }
    }
    out
}

impl App {
    /// Searches every item of a wanted list, under one jobs lock.
    pub async fn search_wanted(
        &self,
        list: WantedList,
        media: WantedMedia,
    ) -> anyhow::Result<BulkSearchReport> {
        let items = wanted_items(self, list, media).await?;
        let _guard = self.jobs.lock().await;
        let mut report = BulkSearchReport::default();
        match media {
            WantedMedia::Movies => {
                for item in &items {
                    if let Some(movie) = self.db.movie(item.id).await? {
                        report.add(self.search_movie(&movie, true).await?);
                    }
                }
            }
            WantedMedia::Episodes => {
                let shows = self.db.all_series().await?;
                for (series_id, scopes) in episode_scopes(&items) {
                    if let Some(show) = shows.iter().find(|s| s.id == series_id) {
                        report.add(self.search_series(show, &scopes, None).await?);
                    }
                }
            }
        }
        Ok(report)
    }

    /// Searches the given targets one after the other, under one jobs lock; a target
    /// that is not in the library becomes an error of the report.
    pub async fn search_targets(
        &self,
        targets: &[SearchTarget],
    ) -> anyhow::Result<BulkSearchReport> {
        let _guard = self.jobs.lock().await;
        let mut report = BulkSearchReport::default();
        for target in targets {
            match self.search_locked(*target).await {
                Ok(r) => report.add(r),
                Err(SearchError::NotFound(message)) => report.errors.push(message),
                Err(SearchError::Failed(e)) => return Err(e),
            }
        }
        Ok(report)
    }
}

pub async fn search_bulk(
    State(app): State<SharedApp>,
    body: Result<Json<BulkSearchBody>, JsonRejection>,
) -> Result<Json<BulkSearchReport>, ApiError> {
    let bad = || ApiError::new(StatusCode::BAD_REQUEST, BAD_BULK);
    let Json(body) = body.map_err(|_| bad())?;
    let report = match (body.items, body.all) {
        (Some(items), None) if !items.is_empty() => {
            let targets: Vec<SearchTarget> = items
                .iter()
                .map(|i| i.target())
                .collect::<Option<_>>()
                .ok_or_else(bad)?;
            app.search_targets(&targets).await?
        }
        (None, Some(all)) => app.search_wanted(all.list, all.media).await?,
        _ => return Err(bad()),
    };
    Ok(Json(report))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn episode(id: i64, series: i64, season: u16) -> WantedItem {
        WantedItem {
            kind: ItemKind::Episode,
            id,
            series_id: Some(series),
            season: Some(season),
            episode: Some(1),
            label: String::new(),
            profile: String::new(),
            reason: MISSING.to_owned(),
            quality: None,
            air_date: None,
        }
    }

    #[test]
    fn seasons_with_several_wanted_episodes_are_searched_as_a_season() {
        let items = [
            episode(1, 7, 1),
            episode(2, 7, 1),
            episode(3, 7, 2),
            episode(4, 8, 1),
        ];
        let scopes = episode_scopes(&items);
        assert_eq!(scopes[&7], [Scope::Season(1), Scope::Episode(3)]);
        assert_eq!(scopes[&8], [Scope::Episode(4)]);
    }
}
