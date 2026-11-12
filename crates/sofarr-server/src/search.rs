//! Searches through the indexers: manual searches for a movie, series, season or
//! episode, and the re-search after failures (spec fail-handling).

use std::collections::{BTreeMap, HashSet};

use jiff::Timestamp;
use serde::Serialize;
use sofarr_db::{Episode, GrabStatus, MediaRef, Movie, Series};
use sofarr_indexer::{IndexerRelease, Newznab, TvCaps, TvItem, TvRequest};

use crate::{
    app::App,
    failures::{Exclusions, GrabOutcome},
    library,
    rss::{self, SyncReport},
    series,
};

/// What a manual search looks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchTarget {
    Movie(i64),
    Series(i64),
    Season {
        series_id: i64,
        season: u16,
    },
    Episode {
        series_id: i64,
        season: u16,
        episode: u16,
    },
}

/// Outcome of a search.
#[derive(Debug, Default, Serialize)]
pub struct SearchReport {
    /// Distinct releases (by GUID) the indexers returned.
    pub releases: usize,
    /// Releases taken (sent to the download client or recorded in shadow mode).
    pub grabs: usize,
    /// Indexer errors.
    pub errors: Vec<String>,
    /// Why nothing was searched, if so.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug)]
pub enum SearchError {
    /// The target is not in the library (German message).
    NotFound(String),
    Failed(anyhow::Error),
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(message) => f.write_str(message),
            Self::Failed(e) => write!(f, "{e:#}"),
        }
    }
}

impl std::error::Error for SearchError {}

impl From<anyhow::Error> for SearchError {
    fn from(e: anyhow::Error) -> Self {
        Self::Failed(e)
    }
}

impl From<sofarr_db::DbError> for SearchError {
    fn from(e: sofarr_db::DbError) -> Self {
        Self::Failed(e.into())
    }
}

/// Part of a series to search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Season(u16),
    /// An episode, by id.
    Episode(i64),
}

/// A release of a series search with its parsed episodes; `by_id` when the indexer was
/// asked by TVDB id.
struct Candidate {
    release: IndexerRelease,
    info: sofarr_parser::EpisodeInfo,
    by_id: bool,
}

fn distinct(guids: impl IntoIterator<Item = String>) -> usize {
    guids.into_iter().collect::<HashSet<_>>().len()
}

impl App {
    /// A manual search (API, CLI); takes the jobs lock.
    pub async fn search(&self, target: SearchTarget) -> Result<SearchReport, SearchError> {
        let _guard = self.jobs.lock().await;
        match target {
            SearchTarget::Movie(id) => {
                let movie = self.db.movie(id).await?.ok_or_else(|| {
                    SearchError::NotFound(format!("Film {id} ist nicht in der Bibliothek"))
                })?;
                Ok(self.search_movie(&movie, true).await?)
            }
            SearchTarget::Series(id) => {
                let show = self.series_or_not_found(id).await?;
                let episodes = self.db.episodes(Some(id)).await?;
                let rules = series::load_rules(&self.db).await?;
                let today = series::today(Timestamp::now());
                let seasons: std::collections::BTreeSet<u16> = episodes
                    .iter()
                    .filter(|e| show.data.monitored && e.data.monitored)
                    .filter(|e| {
                        e.data
                            .air_date
                            .as_deref()
                            .is_some_and(|d| d <= today.as_str())
                    })
                    .filter(|e| {
                        e.file.as_ref().is_none_or(|f| {
                            rules.profile(show.data.quality_profile_id).is_none_or(|p| {
                                !p.cutoff_met(rules.quality_id(&f.quality), f.custom_format_score)
                            })
                        })
                    })
                    .map(|e| e.data.season)
                    .collect();
                let scopes: Vec<Scope> = seasons.into_iter().map(Scope::Season).collect();
                Ok(self.search_series(&show, &scopes, None).await?)
            }
            SearchTarget::Season { series_id, season } => {
                let show = self.series_or_not_found(series_id).await?;
                let episodes = self.db.episodes(Some(series_id)).await?;
                if !episodes.iter().any(|e| e.data.season == season) {
                    return Err(SearchError::NotFound(format!(
                        "Staffel {season} von „{}“ ist nicht in der Bibliothek",
                        show.data.title
                    )));
                }
                Ok(self
                    .search_series(&show, &[Scope::Season(season)], None)
                    .await?)
            }
            SearchTarget::Episode {
                series_id,
                season,
                episode,
            } => {
                let show = self.series_or_not_found(series_id).await?;
                let episodes = self.db.episodes(Some(series_id)).await?;
                let found = episodes
                    .iter()
                    .find(|e| e.data.season == season && e.data.number == episode)
                    .ok_or_else(|| {
                        SearchError::NotFound(format!(
                            "Episode S{season:02}E{episode:02} von „{}“ ist nicht in der Bibliothek",
                            show.data.title
                        ))
                    })?;
                Ok(self
                    .search_series(&show, &[Scope::Episode(found.id)], Some(found.id))
                    .await?)
            }
        }
    }

    async fn series_or_not_found(&self, id: i64) -> Result<Series, SearchError> {
        self.db
            .all_series()
            .await?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| SearchError::NotFound(format!("Serie {id} ist nicht in der Bibliothek")))
    }

    /// The indexers allowed for an item with these tag labels.
    fn allowed_indexers(&self, labels: &[String]) -> Vec<&Newznab> {
        self.indexers
            .iter()
            .filter(|i| self.indexer_allows(i.name(), labels))
            .collect()
    }

    /// Searches the allowed indexers for one movie and takes the best release; a broken
    /// NZB falls back to the next candidate of the same results, an outage ends it.
    /// `explicit`: the user named the movie, so it counts even when unmonitored.
    pub async fn search_movie(
        &self,
        movie: &Movie,
        explicit: bool,
    ) -> anyhow::Result<SearchReport> {
        let mut report = SearchReport::default();
        let tags = self.db.setting("tag").await?;
        let allowed = self.allowed_indexers(&library::tag_labels(tags.as_ref(), &movie.tags));
        if allowed.is_empty() {
            report.note = Some(format!(
                "Kein Indexer ist für „{}“ zuständig (Tags)",
                movie.title
            ));
            return Ok(report);
        }
        let query = match movie.year {
            Some(y) => format!("{} {y}", movie.title),
            None => movie.title.clone(),
        };
        let mut releases = Vec::new();
        for indexer in allowed {
            let found = indexer.search_movie(movie.imdb_id.as_deref(), &query).await;
            self.status.contact(
                crate::status::Peer::Indexer(indexer.name().to_owned()),
                jiff::Timestamp::now(),
                &found,
            );
            match found {
                Ok(mut batch) => releases.append(&mut batch),
                Err(e) => {
                    tracing::warn!(indexer = indexer.name(), error = %e, "search failed");
                    report.errors.push(e.to_string());
                }
            }
        }
        report.releases = distinct(releases.iter().map(|r| r.guid.clone()));
        // Search results belong to this movie even if the indexer sends no IMDb id.
        for release in &mut releases {
            release.imdb_id = None;
        }
        let mut candidate = movie.clone();
        candidate.monitored = movie.monitored || explicit;
        let rules = library::load_rules(&self.db).await?;
        let mut tried = HashSet::new();
        loop {
            let exclusions = Exclusions::load(&self.db).await?;
            let pending: BTreeMap<i64, sofarr_db::Grab> = self
                .db
                .grabs(None, u32::MAX)
                .await?
                .into_iter()
                .filter(|g| g.movie_id == movie.id)
                .find(|g| g.status == GrabStatus::Shadow || g.status.is_active())
                .map(|g| (movie.id, g))
                .into_iter()
                .collect();
            let now = Timestamp::now();
            let picks = rss::decide(
                &releases,
                std::slice::from_ref(&candidate),
                &rules,
                &pending,
                &|m, r| tried.contains(&r.guid) || exclusions.excludes(MediaRef::Movie(m.id), r),
                now,
                &mut SyncReport::default(),
            );
            let Some(pick) = picks.into_iter().next() else {
                break;
            };
            tried.insert(pick.release.guid.clone());
            match self.grab(pick, now).await? {
                GrabOutcome::Taken => {
                    report.grabs += 1;
                    break;
                }
                GrabOutcome::Broken => {}
                GrabOutcome::Outage => break,
            }
        }
        Ok(report)
    }

    /// What an indexer's TV search supports; asked once per process.
    async fn tv_caps(&self, indexer: &Newznab) -> Result<TvCaps, sofarr_indexer::IndexerError> {
        if let Some(caps) = self.tv_caps.lock().await.get(indexer.name()) {
            return Ok(*caps);
        }
        let caps = indexer.tv_caps().await?;
        self.tv_caps
            .lock()
            .await
            .insert(indexer.name().to_owned(), caps);
        Ok(caps)
    }

    /// Searches seasons and episodes of a series and takes the best releases by the
    /// rules of the RSS sync. `explicit`: an episode the user named, searched even when
    /// unmonitored; otherwise only monitored episodes of a monitored series count.
    pub async fn search_series(
        &self,
        show: &Series,
        scopes: &[Scope],
        explicit: Option<i64>,
    ) -> anyhow::Result<SearchReport> {
        let mut report = SearchReport::default();
        let tags = self.db.setting(&format!("{}tag", series::PREFIX)).await?;
        let allowed = self.allowed_indexers(&library::tag_labels(tags.as_ref(), &show.tags));
        if allowed.is_empty() {
            report.note = Some(format!(
                "Kein Indexer ist für „{}“ zuständig (Tags)",
                show.data.title
            ));
            return Ok(report);
        }
        let episodes = self.db.episodes(Some(show.id)).await?;
        let requests: Vec<TvRequest<'_>> = scopes
            .iter()
            .filter_map(|scope| tv_request(show, &episodes, *scope))
            .collect();
        let mut candidates = Vec::new();
        // Every release returned counts, also those without an episode in their name.
        let mut returned = HashSet::new();
        for indexer in allowed {
            let caps = match self.tv_caps(indexer).await {
                Ok(caps) => caps,
                Err(e) => {
                    tracing::warn!(indexer = indexer.name(), error = %e, "caps failed");
                    report.errors.push(e.to_string());
                    continue;
                }
            };
            for request in &requests {
                let found = indexer.search_tv(&caps, request).await;
                self.status.contact(
                    crate::status::Peer::Indexer(indexer.name().to_owned()),
                    jiff::Timestamp::now(),
                    &found,
                );
                match found {
                    Ok((batch, by_id)) => {
                        returned.extend(batch.iter().map(|r| r.guid.clone()));
                        candidates.extend(batch.into_iter().filter_map(|release| {
                            let info = sofarr_parser::parse_episode(&release.title)?;
                            Some(Candidate {
                                release,
                                info,
                                by_id,
                            })
                        }));
                    }
                    Err(e) => {
                        tracing::warn!(indexer = indexer.name(), error = %e, "search failed");
                        report.errors.push(e.to_string());
                    }
                }
            }
        }
        report.releases = returned.len();
        report.grabs = self
            .grab_series_candidates(show, &episodes, scopes, explicit, &candidates)
            .await?;
        Ok(report)
    }

    /// Decides over the candidates of a series search and grabs; a broken NZB falls back
    /// to the next candidate for the same episodes, an outage ends them.
    async fn grab_series_candidates(
        &self,
        show: &Series,
        episodes: &[Episode],
        scopes: &[Scope],
        explicit: Option<i64>,
        candidates: &[Candidate],
    ) -> anyhow::Result<usize> {
        let rules = series::load_rules(&self.db).await?;
        let keys = series::title_keys(show);
        let by_id: HashSet<&str> = candidates
            .iter()
            .filter(|c| c.by_id)
            .map(|c| c.release.guid.as_str())
            .collect();
        let releases: Vec<(IndexerRelease, sofarr_parser::EpisodeInfo)> = candidates
            .iter()
            .map(|c| (c.release.clone(), c.info.clone()))
            .collect();
        let mut target = show.clone();
        target.data.monitored = true;
        let mut open: HashSet<i64> = episodes
            .iter()
            .filter(|e| {
                scopes.iter().any(|s| match s {
                    Scope::Season(n) => e.data.season == *n,
                    Scope::Episode(id) => e.id == *id,
                })
            })
            .map(|e| e.id)
            .collect();
        let mut tried = HashSet::new();
        let mut grabs = 0;
        while !open.is_empty() {
            let scoped: Vec<Episode> = episodes
                .iter()
                .map(|e| {
                    let mut e = e.clone();
                    e.data.monitored = open.contains(&e.id)
                        && (explicit == Some(e.id) || (show.data.monitored && e.data.monitored));
                    e
                })
                .collect();
            let exclusions = Exclusions::load(&self.db).await?;
            let pending: Vec<sofarr_db::SeriesGrab> = self
                .db
                .series_grabs(None)
                .await?
                .into_iter()
                .filter(|g| g.series_id == show.id)
                .filter(|g| g.status == GrabStatus::Shadow || g.status.is_active())
                .collect();
            let now = Timestamp::now();
            let picks = series::decide_with(
                &releases,
                &|release, info| {
                    let belongs = by_id.contains(release.guid.as_str())
                        || keys.contains(&sofarr_core::normalize_title(&info.series_title));
                    if belongs { vec![&target] } else { Vec::new() }
                },
                &scoped,
                &pending,
                &rules,
                &|s, r| tried.contains(&r.guid) || exclusions.excludes(MediaRef::Series(s.id), r),
                now,
            );
            if picks.is_empty() {
                break;
            }
            let mut retry = HashSet::new();
            for pick in picks {
                tried.insert(pick.release.guid.clone());
                match self.grab_series(&pick, now).await? {
                    GrabOutcome::Taken => grabs += 1,
                    GrabOutcome::Broken => retry.extend(pick.decision.episodes.iter().copied()),
                    GrabOutcome::Outage => {}
                }
            }
            open.retain(|id| retry.contains(id));
        }
        Ok(grabs)
    }
}

/// The TV search for a scope in the numbering the series uses.
fn tv_request<'a>(show: &'a Series, episodes: &[Episode], scope: Scope) -> Option<TvRequest<'a>> {
    let item = match scope {
        Scope::Season(n) => TvItem::Season(n),
        Scope::Episode(id) => {
            let e = episodes.iter().find(|e| e.id == id)?;
            let (season, episode) = (e.data.season, e.data.number);
            match (
                show.data.series_type.as_str(),
                &e.data.air_date,
                e.data.absolute,
            ) {
                ("daily", Some(date), _) => TvItem::Daily { date: date.clone() },
                ("anime", _, Some(absolute)) => TvItem::Absolute {
                    season,
                    episode,
                    absolute,
                },
                _ => TvItem::Episode { season, episode },
            }
        }
    };
    Some(TvRequest {
        tvdb_id: Some(show.data.tvdb_id).filter(|id| *id > 0),
        title: &show.data.title,
        item,
    })
}
