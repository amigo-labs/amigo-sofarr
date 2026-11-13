//! Failure handling (spec fail-handling): what decisions must not take again, how a
//! failed grab is recorded, and the re-search after failures.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use jiff::Timestamp;
use sofarr_db::{Episode, GrabStatus, MediaRef, NewBlock};
use sofarr_indexer::IndexerRelease;

use crate::{
    app::App,
    search::{Scope, SearchReport},
    series,
};

/// Movies or series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Movies,
    Series,
}

/// How handing a release to the download client ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrabOutcome {
    /// Sent to the download client, or recorded as a shadow grab.
    Taken,
    /// Broken NZB: the grab failed and the release is blocklisted.
    Broken,
    /// Indexer or download client unavailable: the grab failed, nothing blocklisted.
    Outage,
}

/// Why sending a release failed.
#[derive(Debug)]
pub enum SendError {
    Broken(String),
    Outage(String),
}

/// Reason of a grab whose download disappeared from the client.
pub const VANISHED: &str =
    "Download im Download-Client verschwunden (24 Stunden lang nicht mehr gefunden)";

/// Hours a download must be missing before its grab counts as failed.
pub const VANISH_HOURS: i64 = 24;

/// Releases a decision must not take for a movie or series: the GUIDs of its earlier
/// grabs that did not fail (shadow grabs included), the names of its imported releases,
/// and its blocklist (by GUID or normalized title).
#[derive(Debug, Default)]
pub struct Exclusions {
    held: HashSet<(MediaRef, String)>,
    imported: HashSet<(MediaRef, String)>,
    blocked_guids: HashSet<(MediaRef, String)>,
    blocked_titles: HashSet<(MediaRef, String)>,
}

impl Exclusions {
    pub async fn load(db: &sofarr_db::Db) -> anyhow::Result<Self> {
        let mut out = Self::default();
        let movies = db.grabs(None, u32::MAX).await?.into_iter().map(|g| {
            (
                MediaRef::Movie(g.movie_id),
                g.status,
                g.guid,
                g.title.to_lowercase(),
            )
        });
        let shows = db.series_grabs(None).await?.into_iter().map(|g| {
            (
                MediaRef::Series(g.series_id),
                g.status,
                g.guid,
                g.title.to_lowercase(),
            )
        });
        for (media, status, guid, title) in movies.chain(shows) {
            if status == GrabStatus::Imported {
                out.imported.insert((media, title));
            }
            if status != GrabStatus::Failed {
                out.held.insert((media, guid));
            }
        }
        for block in db.blocks().await? {
            out.blocked_guids.insert((block.media, block.guid));
            out.blocked_titles
                .insert((block.media, block.normalized_title));
        }
        Ok(out)
    }

    /// Whether the release must not be taken for this movie or series.
    pub fn excludes(&self, media: MediaRef, release: &IndexerRelease) -> bool {
        let guid = (media, release.guid.clone());
        self.held.contains(&guid)
            || self.blocked_guids.contains(&guid)
            || self
                .imported
                .contains(&(media, release.title.to_lowercase()))
            || self
                .blocked_titles
                .contains(&(media, sofarr_core::normalize_release_title(&release.title)))
    }
}

/// A grab to record as failed.
#[derive(Debug, Clone, Copy)]
pub struct FailedGrab<'a> {
    /// The grab's id in the table of its media kind.
    pub id: i64,
    pub media: MediaRef,
    pub title: &'a str,
    pub guid: &'a str,
    pub indexer: &'a str,
}

/// The seasons and episodes of one series to search again.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SeriesResearch {
    seasons: BTreeSet<u16>,
    /// Episode id → season.
    episodes: BTreeMap<i64, u16>,
}

impl SeriesResearch {
    /// Adds a failed grab: each season of a season pack, otherwise each covered
    /// episode (`(episode id, season)`).
    pub fn add(&mut self, season_pack: bool, covered: &[(i64, u16)]) {
        for (id, season) in covered {
            if season_pack {
                self.seasons.insert(*season);
            } else {
                self.episodes.insert(*id, *season);
            }
        }
    }

    pub fn seasons(&self) -> Vec<u16> {
        self.seasons.iter().copied().collect()
    }

    /// Episodes to search one by one: those not in a season that is searched anyway.
    pub fn episodes(&self) -> Vec<i64> {
        self.episodes
            .iter()
            .filter(|(_, season)| !self.seasons.contains(season))
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn scopes(&self) -> Vec<Scope> {
        self.seasons()
            .into_iter()
            .map(Scope::Season)
            .chain(self.episodes().into_iter().map(Scope::Episode))
            .collect()
    }
}

/// What a job run found to search again after failures; each item once.
#[derive(Debug, Default)]
pub struct Research {
    movies: BTreeSet<i64>,
    series: BTreeMap<i64, SeriesResearch>,
}

impl Research {
    pub fn movie(&mut self, movie_id: i64) {
        self.movies.insert(movie_id);
    }

    /// A failed series grab, by its release title and covered episodes.
    pub fn series(
        &mut self,
        series_id: i64,
        title: &str,
        episode_ids: &[i64],
        episodes: &[Episode],
    ) {
        let pack = sofarr_parser::parse_episode(title).is_some_and(|i| i.full_season);
        let covered: Vec<(i64, u16)> = episodes
            .iter()
            .filter(|e| episode_ids.contains(&e.id))
            .map(|e| (e.id, e.data.season))
            .collect();
        self.series
            .entry(series_id)
            .or_default()
            .add(pack, &covered);
    }

    fn is_empty(&self) -> bool {
        self.movies.is_empty() && self.series.is_empty()
    }
}

impl App {
    /// Whether a failed grab of this kind is searched again: `sofarr.toml` first, then
    /// the `autoRedownloadFailed` imported from Radarr or Sonarr, otherwise off.
    pub async fn redownload_failed(&self, kind: MediaKind) -> anyhow::Result<bool> {
        let (configured, key) = match kind {
            MediaKind::Movies => (
                self.config.download.redownload_failed_movies,
                "config_downloadclient".to_owned(),
            ),
            MediaKind::Series => (
                self.config.download.redownload_failed_series,
                format!("{}config_downloadclient", series::PREFIX),
            ),
        };
        if let Some(value) = configured {
            return Ok(value);
        }
        Ok(self
            .db
            .setting(&key)
            .await?
            .and_then(|v| v["autoRedownloadFailed"].as_bool())
            .unwrap_or(false))
    }

    /// Sets a grab to failed with the reason; `blocklist` also puts its release on the
    /// blocklist of its movie or series.
    pub async fn fail_grab(
        &self,
        grab: FailedGrab<'_>,
        reason: &str,
        blocklist: bool,
        now: Timestamp,
    ) -> anyhow::Result<()> {
        tracing::warn!(release = grab.title, reason, blocklist, "grab failed");
        match grab.media {
            MediaRef::Movie(_) => {
                self.db
                    .update_grab(grab.id, GrabStatus::Failed, None, Some(reason), now)
                    .await?;
            }
            MediaRef::Series(_) => {
                self.db
                    .update_series_grab(grab.id, GrabStatus::Failed, None, Some(reason), now)
                    .await?;
            }
        }
        if blocklist {
            self.db
                .add_block(
                    &NewBlock {
                        media: grab.media,
                        title: grab.title.to_owned(),
                        normalized_title: sofarr_core::normalize_release_title(grab.title),
                        guid: grab.guid.to_owned(),
                        indexer: grab.indexer.to_owned(),
                        reason: reason.to_owned(),
                    },
                    now,
                )
                .await?;
        }
        Ok(())
    }

    /// Searches what failed again where re-search is on for its kind and the movie or
    /// series is still in the library and monitored. Errors are only logged: the items
    /// stay wanted for the next RSS sync.
    pub async fn run_research(&self, plan: Research) {
        if plan.is_empty() {
            return;
        }
        if let Err(e) = self.research(plan).await {
            tracing::warn!(error = format!("{e:#}"), "re-search failed");
        }
    }

    async fn research(&self, plan: Research) -> anyhow::Result<()> {
        if !plan.movies.is_empty() && self.redownload_failed(MediaKind::Movies).await? {
            for id in plan.movies {
                let Some(movie) = self.db.movie(id).await?.filter(|m| m.monitored) else {
                    continue;
                };
                let report = self.search_movie(&movie, false).await;
                log_research(&movie.title, report);
            }
        }
        if !plan.series.is_empty() && self.redownload_failed(MediaKind::Series).await? {
            let all = self.db.all_series().await?;
            for (id, scope) in plan.series {
                let Some(show) = all.iter().find(|s| s.id == id && s.data.monitored) else {
                    continue;
                };
                let report = self.search_series(show, &scope.scopes(), None).await;
                log_research(&show.data.title, report);
            }
        }
        Ok(())
    }
}

fn log_research(title: &str, report: anyhow::Result<SearchReport>) {
    match report {
        Ok(r) => tracing::info!(
            item = title,
            releases = r.releases,
            grabs = r.grabs,
            errors = ?r.errors,
            "re-search done"
        ),
        Err(e) => tracing::warn!(item = title, error = format!("{e:#}"), "re-search failed"),
    }
}
