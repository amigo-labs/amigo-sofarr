//! Application state and the jobs that run on it.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use jiff::Timestamp;
use sofarr_db::{Db, GrabStatus, MediaRef, NewGrab};
use sofarr_download::{DownloadClient, DownloadError, Nzbget, NzbgetConfig};
use sofarr_indexer::{Newznab, NewznabConfig, NzbError, TvCaps};
use tokio::sync::Mutex;

use crate::{
    config::{Config, Mode},
    failures::{Exclusions, FailedGrab, GrabOutcome, Research, SendError},
    import_lists, importer, library, metadata,
    rescan::{self, RescanAfterRefresh, ScanReport, ScanSettings},
    rss::{self, Selected, SyncReport},
    series,
    status::{Job, Peer},
};

pub struct App {
    pub config: Config,
    pub db: Db,
    pub http: reqwest::Client,
    pub indexers: Vec<Newznab>,
    pub nzbget: Option<Nzbget>,
    /// Serialises the jobs (RSS sync, download check, import lists, searches, marking).
    pub jobs: Mutex<()>,
    /// TV search capabilities per indexer name, asked once.
    pub tv_caps: Mutex<HashMap<String, TvCaps>>,
    /// Serialises backup operations (writing, pruning, deleting, choosing a restore).
    pub backup_lock: Mutex<()>,
    /// The last failed backup operation (spec backups AC-07).
    pub backup_error: std::sync::Mutex<Option<crate::backups::BackupError>>,
    /// Trusted networks and proxies, login throttling (spec auth).
    pub auth: crate::auth::AuthState,
    /// Job runs and contacts for the system page (spec web-ui-shell).
    pub status: crate::status::Status,
    /// The computed movie and series lists (spec web-ui-library).
    pub lists: crate::library_list::ListCache,
    /// The download client's last answer, shown while it cannot be reached.
    pub queue_cache: crate::overview::QueueCache,
    /// Poster sources and cache (spec web-ui-library).
    pub images: crate::images::Images,
    /// The subtitle provider; `None` while subtitles are off (spec subtitles).
    pub opensubtitles: Option<sofarr_subtitles::OpenSubtitles>,
    /// Run state of the subtitle search.
    pub subtitles: crate::subtitles::SubtitleState,
}

pub type SharedApp = Arc<App>;

const USER_AGENT: &str = concat!("amigo-sofarr/", env!("CARGO_PKG_VERSION"));

impl App {
    /// Opens the database; before pending migrations a pre-migration backup is written.
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        Self::open(config, true).await
    }

    /// Opens the database, with or without the pre-migration backup (a restored backup
    /// is that snapshot itself).
    pub async fn open(config: Config, pre_migration_backup: bool) -> anyhow::Result<Self> {
        if let Some(parent) = config.database.path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Verzeichnis {} nicht anlegbar", parent.display()))?;
        }
        let pruning = if pre_migration_backup {
            crate::backups::before_migrations(&config, Timestamp::now()).await?
        } else {
            None
        };
        let db = Db::open(&config.database.path).await.with_context(|| {
            format!("Datenbank {} nicht nutzbar", config.database.path.display())
        })?;
        let app = Self::with_db(config, db)?;
        if let Some(error) = pruning {
            app.show_backup_error(error);
        }
        Ok(app)
    }

    pub fn with_db(config: Config, db: Db) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(60))
            .build()
            .context("HTTP-Client nicht erzeugbar")?;
        let (indexers, nzbget) = Self::build_clients(&config, &http);
        let auth = crate::auth::AuthState::new(&config.auth);
        let images = crate::images::Images::new(&config.database.path);
        let opensubtitles = crate::subtitles::client(&config, &http);
        Ok(Self {
            config,
            db,
            http,
            indexers,
            nzbget,
            jobs: Mutex::new(()),
            tv_caps: Mutex::new(HashMap::new()),
            backup_lock: Mutex::new(()),
            backup_error: std::sync::Mutex::new(None),
            auth,
            status: crate::status::Status::default(),
            lists: crate::library_list::ListCache::default(),
            queue_cache: crate::overview::QueueCache::default(),
            images,
            opensubtitles,
            subtitles: crate::subtitles::SubtitleState::default(),
        })
    }

    /// The indexer and download clients of a configuration on an HTTP client.
    pub fn build_clients(
        config: &Config,
        http: &reqwest::Client,
    ) -> (Vec<Newznab>, Option<Nzbget>) {
        let indexers = config
            .indexers
            .iter()
            .map(|i| {
                Newznab::new(
                    NewznabConfig {
                        name: i.name.clone(),
                        url: i.url.clone(),
                        api_key: i.api_key.clone(),
                        categories: {
                            let mut c = i.categories.clone();
                            c.extend(&i.tv_categories);
                            c.dedup();
                            c
                        },
                        limit: i.limit,
                    },
                    http.clone(),
                )
            })
            .collect();
        let nzbget = config.nzbget.as_ref().map(|c| {
            Nzbget::new(
                http.clone(),
                NzbgetConfig {
                    url: c.url.clone(),
                    username: c.username.clone(),
                    password: c.password.clone(),
                },
            )
        });
        (indexers, nzbget)
    }

    /// One RSS sync: fetch, decide, grab (or, in shadow mode, only record); afterwards
    /// the re-search for grabs that failed with a broken NZB.
    pub async fn rss_sync(&self) -> anyhow::Result<SyncReport> {
        let _guard = self.jobs.lock().await;
        let mut report = SyncReport::default();
        let mut releases = Vec::new();
        for indexer in &self.indexers {
            let fetched = indexer.rss().await;
            self.status.contact(
                Peer::Indexer(indexer.name().to_owned()),
                Timestamp::now(),
                &fetched,
            );
            match fetched {
                Ok(mut batch) => releases.append(&mut batch),
                Err(e) => {
                    tracing::warn!(indexer = indexer.name(), error = %e, "rss fetch failed");
                    report.errors.push(e.to_string());
                }
            }
        }
        // Series releases go to the series pipeline, the rest to the movies.
        let (episodes, movies): (Vec<_>, Vec<_>) = releases
            .into_iter()
            .map(|r| {
                let info = sofarr_parser::parse_episode(&r.title);
                (r, info)
            })
            .partition(|(_, info)| info.is_some());
        let movies: Vec<_> = movies.into_iter().map(|(r, _)| r).collect();
        let mut research = Research::default();
        self.decide_and_grab(&movies, &mut report, &mut research)
            .await?;
        let episodes: Vec<_> = episodes
            .into_iter()
            .filter_map(|(r, info)| Some((r, info?)))
            .collect();
        report.series_grabs = self
            .decide_and_grab_series(&episodes, &mut research)
            .await?;
        tracing::info!(
            releases = report.releases,
            matched = report.matched,
            approved = report.approved,
            "rss sync done"
        );
        self.run_research(research).await;
        Ok(report)
    }

    /// Indexer tags: an indexer with tags serves only movies and series that share one
    /// of them (labels, case-insensitive); an indexer without tags serves everything.
    pub fn indexer_allows(&self, indexer: &str, labels: &[String]) -> bool {
        self.config
            .indexers
            .iter()
            .find(|i| i.name == indexer)
            .is_none_or(|i| {
                i.tags.is_empty() || i.tags.iter().any(|t| labels.contains(&t.to_lowercase()))
            })
    }

    /// Takes a release: shadow mode records it, live mode hands it to the download client.
    /// A broken NZB fails the grab and blocklists the release, an outage only fails it.
    pub async fn grab(&self, pick: Selected, now: Timestamp) -> anyhow::Result<GrabOutcome> {
        let status = match self.config.automation.mode {
            Mode::Shadow => GrabStatus::Shadow,
            Mode::Live => GrabStatus::Queued,
        };
        let grab = NewGrab {
            movie_id: pick.movie.id,
            title: pick.release.title.clone(),
            guid: pick.release.guid.clone(),
            indexer: pick.release.indexer.clone(),
            download_url: pick.release.download_url.clone(),
            quality: pick.decision.quality.name.clone(),
            custom_format_score: pick.decision.score,
            size: pick.release.size_bytes,
            status,
            download_id: None,
        };
        let id = self.db.add_grab(&grab, now).await?;
        if self.config.automation.mode == Mode::Shadow {
            tracing::info!(
                movie = %pick.movie.title,
                release = %pick.release.title,
                quality = %pick.decision.quality.name,
                score = pick.decision.score,
                "would grab (shadow mode)"
            );
            return Ok(GrabOutcome::Taken);
        }
        let category = self
            .config
            .nzbget
            .as_ref()
            .map_or("movies", |c| c.category.as_str());
        match self.fetch_and_send(&pick.release, category).await {
            Ok(download_id) => {
                tracing::info!(movie = %pick.movie.title, release = %pick.release.title, %download_id, "grabbed");
                self.db
                    .update_grab(id, GrabStatus::Queued, Some(&download_id), None, now)
                    .await?;
                Ok(GrabOutcome::Taken)
            }
            Err(e) => {
                let failed = FailedGrab {
                    id,
                    media: MediaRef::Movie(pick.movie.id),
                    title: &pick.release.title,
                    guid: &pick.release.guid,
                    indexer: &pick.release.indexer,
                };
                self.record_send_error(failed, e, now).await
            }
        }
    }

    async fn record_send_error(
        &self,
        failed: FailedGrab<'_>,
        error: SendError,
        now: Timestamp,
    ) -> anyhow::Result<GrabOutcome> {
        let (reason, blocklist, outcome) = match error {
            SendError::Broken(reason) => (reason, true, GrabOutcome::Broken),
            SendError::Outage(reason) => (reason, false, GrabOutcome::Outage),
        };
        self.fail_grab(failed, &reason, blocklist, now).await?;
        Ok(outcome)
    }

    /// Series part of the RSS sync; returns how many releases were taken.
    async fn decide_and_grab_series(
        &self,
        releases: &[(sofarr_indexer::IndexerRelease, sofarr_parser::EpisodeInfo)],
        research: &mut Research,
    ) -> anyhow::Result<usize> {
        let all_series = self.db.all_series().await?;
        if all_series.is_empty() || releases.is_empty() {
            return Ok(0);
        }
        let rules = series::load_rules(&self.db).await?;
        let episodes = self.db.episodes(None).await?;
        let pending: Vec<_> = self
            .db
            .series_grabs(None)
            .await?
            .into_iter()
            .filter(|g| g.status == GrabStatus::Shadow || g.status.is_active())
            .collect();
        let exclusions = Exclusions::load(&self.db).await?;
        let tags = self.db.setting(&format!("{}tag", series::PREFIX)).await?;
        let now = Timestamp::now();
        let picks = series::decide(
            releases,
            &all_series,
            &episodes,
            &pending,
            &rules,
            &|s, release| {
                exclusions.excludes(MediaRef::Series(s.id), release)
                    || !self.indexer_allows(
                        &release.indexer,
                        &library::tag_labels(tags.as_ref(), &s.tags),
                    )
            },
            now,
        );
        let mut taken = 0;
        for pick in picks {
            match self.grab_series(&pick, now).await? {
                GrabOutcome::Taken => taken += 1,
                GrabOutcome::Broken => research.series(
                    pick.series.id,
                    &pick.release.title,
                    &pick.decision.episodes,
                    &episodes,
                ),
                GrabOutcome::Outage => {}
            }
        }
        Ok(taken)
    }

    /// Takes a series release like [`App::grab`].
    pub async fn grab_series(
        &self,
        pick: &series::SelectedEpisodes,
        now: Timestamp,
    ) -> anyhow::Result<GrabOutcome> {
        let live = self.config.automation.mode == Mode::Live;
        let grab = NewGrab {
            movie_id: 0,
            title: pick.release.title.clone(),
            guid: pick.release.guid.clone(),
            indexer: pick.release.indexer.clone(),
            download_url: pick.release.download_url.clone(),
            quality: pick.decision.decision.quality.name.clone(),
            custom_format_score: pick.decision.decision.score,
            size: pick.release.size_bytes,
            status: if live {
                GrabStatus::Queued
            } else {
                GrabStatus::Shadow
            },
            download_id: None,
        };
        let id = self
            .db
            .add_series_grab(pick.series.id, &pick.decision.episodes, &grab, now)
            .await?;
        if !live {
            tracing::info!(series = %pick.series.data.title, release = %pick.release.title,
                quality = %pick.decision.decision.quality.name, "would grab (shadow mode)");
            return Ok(GrabOutcome::Taken);
        }
        let category = self
            .config
            .nzbget
            .as_ref()
            .map_or("tv", |c| c.tv_category.as_str());
        match self.fetch_and_send(&pick.release, category).await {
            Ok(download_id) => {
                tracing::info!(series = %pick.series.data.title, release = %pick.release.title, %download_id, "grabbed");
                self.db
                    .update_series_grab(id, GrabStatus::Queued, Some(&download_id), None, now)
                    .await?;
                Ok(GrabOutcome::Taken)
            }
            Err(e) => {
                let failed = FailedGrab {
                    id,
                    media: MediaRef::Series(pick.series.id),
                    title: &pick.release.title,
                    guid: &pick.release.guid,
                    indexer: &pick.release.indexer,
                };
                self.record_send_error(failed, e, now).await
            }
        }
    }

    /// Loads the NZB of a release and hands it to nzbget; returns the download id. A
    /// missing NZB or one the client rejects is broken, anything else an outage.
    async fn fetch_and_send(
        &self,
        release: &sofarr_indexer::IndexerRelease,
        category: &str,
    ) -> Result<String, SendError> {
        let client = self.nzbget.as_ref().ok_or_else(|| {
            SendError::Outage("kein Download-Client konfiguriert ([nzbget])".to_owned())
        })?;
        let url = release
            .download_url
            .as_deref()
            .ok_or_else(|| SendError::Broken("Release ohne Download-Link".to_owned()))?;
        let nzb = sofarr_indexer::fetch_nzb(&self.http, url)
            .await
            .map_err(|e| match e {
                NzbError::Broken(_) => SendError::Broken(e.to_string()),
                NzbError::Unavailable(_) => SendError::Outage(e.to_string()),
            })?;
        let added = client.add_nzb(&release.title, &nzb, category).await;
        self.status
            .contact(Peer::DownloadClient, Timestamp::now(), &added);
        added.map_err(|e| match e {
            DownloadError::Rejected { .. } => SendError::Broken(e.to_string()),
            _ => SendError::Outage(e.to_string()),
        })
    }

    async fn decide_and_grab(
        &self,
        releases: &[sofarr_indexer::IndexerRelease],
        report: &mut SyncReport,
        research: &mut Research,
    ) -> anyhow::Result<()> {
        let movies = self.db.movies().await?;
        // A setup without movies (series only) needs no movie rules.
        if movies.is_empty() {
            report.releases += releases.len();
            return Ok(());
        }
        let rules = library::load_rules(&self.db).await?;
        let tags = self.db.setting("tag").await?;
        let mut pending = BTreeMap::new();
        for grab in self.db.grabs(None, u32::MAX).await? {
            let open = grab.status == GrabStatus::Shadow || grab.status.is_active();
            if open && !pending.contains_key(&grab.movie_id) {
                pending.insert(grab.movie_id, grab);
            }
        }
        let exclusions = Exclusions::load(&self.db).await?;
        let now = Timestamp::now();
        let selected = rss::decide(
            releases,
            &movies,
            &rules,
            &pending,
            &|movie, release| {
                exclusions.excludes(MediaRef::Movie(movie.id), release)
                    || !self.indexer_allows(
                        &release.indexer,
                        &library::tag_labels(tags.as_ref(), &movie.tags),
                    )
            },
            now,
            report,
        );
        for pick in selected {
            let movie_id = pick.movie.id;
            if self.grab(pick, now).await? == GrabOutcome::Broken {
                research.movie(movie_id);
            }
        }
        Ok(())
    }

    /// Syncs the TMDb person lists: adds new movies, searches them if the list says so.
    pub async fn sync_import_lists(&self) -> anyhow::Result<Vec<String>> {
        let _guard = self.jobs.lock().await;
        let Some(settings) = self.db.setting("importlist").await? else {
            return Ok(Vec::new());
        };
        let (lists, warnings) = import_lists::person_lists(&settings);
        for w in &warnings {
            tracing::warn!("{w}");
        }
        if lists.is_empty() {
            return Ok(Vec::new());
        }
        let tmdb = metadata::tmdb(self)?;
        let excluded = import_lists::exclusions(self.db.setting("exclusions").await?.as_ref());
        let naming = sofarr_core::naming::NamingConfig::from_json(
            &self.db.setting("config_naming").await?.unwrap_or_default(),
        );
        let rules = library::load_rules(&self.db).await;
        let mut added = Vec::new();
        for list in &lists {
            let credits = tmdb.person_movies(list.person_id).await?;
            for credit in import_lists::select(list, &credits) {
                if excluded.contains(&credit.tmdb_id)
                    || self.db.movie_by_tmdb(credit.tmdb_id).await?.is_some()
                {
                    continue;
                }
                let meta = match tmdb.movie(credit.tmdb_id).await {
                    Ok(meta) => meta,
                    Err(e) => {
                        tracing::warn!(tmdb = credit.tmdb_id, error = %e, "metadata failed");
                        continue;
                    }
                };
                let mut movie = metadata::new_movie(
                    &meta,
                    list.monitored,
                    list.quality_profile_id,
                    &list.minimum_availability,
                    list.tags.clone(),
                );
                let input = importer::naming_input(
                    &metadata::as_library_movie(&movie),
                    "",
                    "Unknown",
                    None,
                );
                movie.path = importer::movie_folder(
                    std::path::Path::new(&list.root_folder),
                    &input,
                    &naming,
                )
                .to_string_lossy()
                .into_owned();
                let id = self.db.upsert_movie(&movie, Timestamp::now()).await?;
                metadata::store_presentation(self, id, &meta).await?;
                tracing::info!(list = %list.name, movie = %movie.title, "added from import list");
                added.push(movie.title.clone());
                // Like an RSS grab, the search on add waits until the movie is available.
                let today = Timestamp::now().to_zoned(jiff::tz::TimeZone::UTC).date();
                let stored = self.db.movie(id).await?;
                let available = match (&rules, &stored) {
                    (Ok(rules), Some(stored)) => library::is_available(stored, rules, today),
                    _ => true,
                };
                if list.search_on_add
                    && list.monitored
                    && available
                    && let Some(stored) = stored
                    && let Err(e) = self.search_movie(&stored, false).await
                {
                    tracing::warn!(movie = %movie.title, error = format!("{e:#}"), "search failed");
                }
            }
        }
        Ok(added)
    }
}

/// Outcome of a metadata refresh with its disk scans.
#[derive(Debug, Default, serde::Serialize)]
pub struct RefreshReport {
    pub movies: usize,
    pub episodes: usize,
    pub movie_scan: Option<ScanReport>,
    pub series_scan: Option<ScanReport>,
}

impl App {
    /// Disk scan of movies and series (see [`rescan`]).
    pub async fn rescan(&self) -> anyhow::Result<(Option<ScanReport>, Option<ScanReport>)> {
        let _guard = self.jobs.lock().await;
        Ok((self.rescan_movies().await?, self.rescan_series().await?))
    }

    /// Shadow mode leaves `/data` alone: no folder is deleted.
    fn scan_settings(&self, mut settings: ScanSettings) -> ScanSettings {
        if self.config.automation.mode == Mode::Shadow {
            settings.delete_empty_folders = false;
        }
        settings
    }

    async fn rescan_movies(&self) -> anyhow::Result<Option<ScanReport>> {
        if self.db.movies().await?.is_empty() {
            return Ok(None);
        }
        let rules = library::load_rules(&self.db).await?;
        let media = self.db.setting("config_mediamanagement").await?;
        let settings = self.scan_settings(ScanSettings::movies(media.as_ref()));
        Ok(Some(
            rescan::movies(&self.db, &rules, settings, Timestamp::now()).await?,
        ))
    }

    async fn rescan_series(&self) -> anyhow::Result<Option<ScanReport>> {
        if self.db.all_series().await?.is_empty() {
            return Ok(None);
        }
        let rules = series::load_rules(&self.db).await?;
        let media = self
            .db
            .setting(&format!("{}config_mediamanagement", series::PREFIX))
            .await?;
        let settings = self.scan_settings(ScanSettings::series(media.as_ref()));
        Ok(Some(
            rescan::all_series(&self.db, &rules, settings, Timestamp::now()).await?,
        ))
    }

    /// Refreshes movies from TMDB and series from TVDB (where configured), then scans
    /// the disk as `rescanAfterRefresh` says; `manual` marks a refresh from the CLI.
    /// The remote requests run without the jobs lock (RSS and imports go on); only the
    /// disk scans take it.
    pub async fn refresh(
        &self,
        movies: bool,
        shows: bool,
        manual: bool,
    ) -> anyhow::Result<RefreshReport> {
        let mut report = RefreshReport::default();
        if movies {
            if self.config.tmdb.is_some() {
                report.movies = metadata::refresh_all(self).await?;
            }
            let media = self.db.setting("config_mediamanagement").await?;
            if RescanAfterRefresh::from_setting(media.as_ref()).applies(manual) {
                let _guard = self.jobs.lock().await;
                report.movie_scan = self.rescan_movies().await?;
            }
        }
        if shows {
            if let Some(config) = &self.config.tvdb {
                let tvdb = sofarr_metadata::tvdb::Tvdb::new(
                    self.http.clone(),
                    &config.api_key,
                    config.pin.clone(),
                );
                let tmdb = self
                    .config
                    .tmdb
                    .as_ref()
                    .map(|c| sofarr_metadata::Tmdb::new(self.http.clone(), &c.api_key));
                for show in self.db.all_series().await? {
                    match series::refresh(&self.db, &tvdb, tmdb.as_ref(), &show, Timestamp::now())
                        .await
                    {
                        Ok(n) => report.episodes += n,
                        Err(e) => {
                            tracing::warn!(series = %show.data.title, error = format!("{e:#}"), "series refresh failed");
                        }
                    }
                }
            }
            let media = self
                .db
                .setting(&format!("{}config_mediamanagement", series::PREFIX))
                .await?;
            if RescanAfterRefresh::from_setting(media.as_ref()).applies(manual) {
                let _guard = self.jobs.lock().await;
                report.series_scan = self.rescan_series().await?;
            }
        }
        Ok(report)
    }
}

/// Runs a job every `interval` until the process ends; the first run is right away, or
/// one `interval` after the start when `delayed` (jobs too expensive for every container
/// start). A run that takes longer than `interval` is followed by the next at once.
pub async fn every<F, Fut>(app: SharedApp, job: Job, interval: Duration, delayed: bool, run: F)
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<()>>,
{
    let mut wait = if delayed { interval } else { Duration::ZERO };
    loop {
        app.status.job_waits(job, Timestamp::now(), wait);
        tokio::time::sleep(wait).await;
        let started = tokio::time::Instant::now();
        let at = Timestamp::now();
        let result = run().await.map_err(|e| format!("{e:#}"));
        if let Err(e) = &result {
            tracing::error!(job = job.name(), error = %e, "job failed");
        }
        app.status.job_ran(job, at, &result);
        wait = interval.saturating_sub(started.elapsed());
    }
}

#[cfg(test)]
mod tests;
