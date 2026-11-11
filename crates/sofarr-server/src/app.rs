//! Application state and the jobs that run on it.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use anyhow::Context;
use jiff::Timestamp;
use sofarr_db::{Db, GrabStatus, NewGrab};
use sofarr_download::{DownloadClient, DownloadState, Nzbget, NzbgetConfig};
use sofarr_indexer::{Newznab, NewznabConfig};
use tokio::sync::Mutex;

use crate::{
    config::{Config, Mode},
    import_lists, importer, library, metadata,
    rescan::{self, RescanAfterRefresh, ScanReport, ScanSettings},
    rss::{self, Selected, SyncReport},
    series,
};

pub struct App {
    pub config: Config,
    pub db: Db,
    pub http: reqwest::Client,
    pub indexers: Vec<Newznab>,
    pub nzbget: Option<Nzbget>,
    /// Serialises the jobs (RSS sync, download check, import lists).
    pub jobs: Mutex<()>,
}

pub type SharedApp = Arc<App>;

const USER_AGENT: &str = concat!("amigo-sofarr/", env!("CARGO_PKG_VERSION"));

impl App {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        if let Some(parent) = config.database.path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Verzeichnis {} nicht anlegbar", parent.display()))?;
        }
        let db = Db::open(&config.database.path).await.with_context(|| {
            format!("Datenbank {} nicht nutzbar", config.database.path.display())
        })?;
        Self::with_db(config, db)
    }

    pub fn with_db(config: Config, db: Db) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(60))
            .build()
            .context("HTTP-Client nicht erzeugbar")?;
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
        Ok(Self {
            config,
            db,
            http,
            indexers,
            nzbget,
            jobs: Mutex::new(()),
        })
    }

    /// One RSS sync: fetch, decide, grab (or, in shadow mode, only record).
    pub async fn rss_sync(&self) -> anyhow::Result<SyncReport> {
        let _guard = self.jobs.lock().await;
        let mut report = SyncReport::default();
        let mut releases = Vec::new();
        for indexer in &self.indexers {
            match indexer.rss().await {
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
        self.decide_and_grab(&movies, None, &mut report).await?;
        let episodes: Vec<_> = episodes
            .into_iter()
            .filter_map(|(r, info)| Some((r, info?)))
            .collect();
        report.series_grabs = self.decide_and_grab_series(&episodes).await?;
        tracing::info!(
            releases = report.releases,
            matched = report.matched,
            approved = report.approved,
            "rss sync done"
        );
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
    pub async fn grab(&self, pick: Selected, now: Timestamp) -> anyhow::Result<()> {
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
        match self.config.automation.mode {
            Mode::Shadow => tracing::info!(
                movie = %pick.movie.title,
                release = %pick.release.title,
                quality = %pick.decision.quality.name,
                score = pick.decision.score,
                "would grab (shadow mode)"
            ),
            Mode::Live => self.send_to_download_client(id, &pick, now).await?,
        }
        Ok(())
    }

    /// Series part of the RSS sync; returns how many releases were taken.
    async fn decide_and_grab_series(
        &self,
        releases: &[(sofarr_indexer::IndexerRelease, sofarr_parser::EpisodeInfo)],
    ) -> anyhow::Result<usize> {
        let all_series = self.db.all_series().await?;
        if all_series.is_empty() || releases.is_empty() {
            return Ok(0);
        }
        let rules = series::load_rules(&self.db).await?;
        let episodes = self.db.episodes(None).await?;
        let grabs = self.db.series_grabs(None).await?;
        let pending: Vec<_> = grabs
            .iter()
            .filter(|g| g.status == GrabStatus::Shadow || g.status.is_active())
            .cloned()
            .collect();
        let seen: std::collections::HashSet<(i64, String)> = grabs
            .iter()
            .map(|g| (g.series_id, g.guid.clone()))
            .collect();
        let imported: std::collections::HashSet<(i64, String)> = grabs
            .iter()
            .filter(|g| g.status == GrabStatus::Imported)
            .map(|g| (g.series_id, g.title.to_lowercase()))
            .collect();
        let tags = self.db.setting(&format!("{}tag", series::PREFIX)).await?;
        let now = Timestamp::now();
        let picks = series::decide(
            releases,
            &all_series,
            &episodes,
            &pending,
            &rules,
            &|s, release| {
                seen.contains(&(s.id, release.guid.clone()))
                    || imported.contains(&(s.id, release.title.to_lowercase()))
                    || !self.indexer_allows(
                        &release.indexer,
                        &library::tag_labels(tags.as_ref(), &s.tags),
                    )
            },
            now,
        );
        let n = picks.len();
        for pick in picks {
            self.grab_series(&pick, now).await?;
        }
        Ok(n)
    }

    async fn grab_series(
        &self,
        pick: &series::SelectedEpisodes,
        now: Timestamp,
    ) -> anyhow::Result<()> {
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
            return Ok(());
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
            }
            Err(e) => {
                let message = format!("{e:#}");
                tracing::warn!(release = %pick.release.title, error = %message, "grab failed");
                self.db
                    .update_series_grab(id, GrabStatus::Failed, None, Some(&message), now)
                    .await?;
            }
        }
        Ok(())
    }

    /// Loads the NZB of a release and hands it to nzbget; returns the download id.
    async fn fetch_and_send(
        &self,
        release: &sofarr_indexer::IndexerRelease,
        category: &str,
    ) -> anyhow::Result<String> {
        let client = self
            .nzbget
            .as_ref()
            .context("kein Download-Client konfiguriert ([nzbget])")?;
        let url = release
            .download_url
            .as_deref()
            .context("Release ohne Download-Link")?;
        let nzb = self
            .http
            .get(url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(reqwest::Error::without_url)
            .context("NZB nicht ladbar")?
            .bytes()
            .await
            .map_err(reqwest::Error::without_url)?;
        Ok(client.add_nzb(&release.title, &nzb, category).await?)
    }

    /// Series part of the download check.
    async fn check_series_downloads(
        &self,
        items: &[sofarr_download::DownloadItem],
        settings: &importer::ImportSettings,
    ) -> anyhow::Result<usize> {
        let mut imported = 0;
        let mut naming = None;
        for grab in self.db.series_grabs(None).await? {
            if !grab.status.is_active() {
                continue;
            }
            let Some(item) = grab
                .download_id
                .as_deref()
                .and_then(|id| items.iter().find(|i| i.id == id))
            else {
                continue;
            };
            let now = Timestamp::now();
            match item.state {
                DownloadState::Queued | DownloadState::Paused => {}
                DownloadState::Downloading => {
                    if grab.status != GrabStatus::Downloading {
                        self.db
                            .update_series_grab(grab.id, GrabStatus::Downloading, None, None, now)
                            .await?;
                    }
                }
                DownloadState::Failed => {
                    self.db
                        .update_series_grab(
                            grab.id,
                            GrabStatus::Failed,
                            None,
                            item.message.as_deref(),
                            now,
                        )
                        .await?;
                }
                DownloadState::Completed => {
                    if naming.is_none() {
                        naming = Some(series::naming_config(&self.db).await?);
                    }
                    let naming = naming.as_ref().context("naming")?;
                    match self.import_episodes(&grab, item, naming, settings).await {
                        Ok(n) => imported += n,
                        Err(e) => {
                            let message = format!("{e:#}");
                            tracing::warn!(release = %grab.title, error = %message, "import failed");
                            self.db
                                .update_series_grab(
                                    grab.id,
                                    GrabStatus::Completed,
                                    None,
                                    Some(&message),
                                    now,
                                )
                                .await?;
                        }
                    }
                }
            }
        }
        Ok(imported)
    }

    async fn import_episodes(
        &self,
        grab: &sofarr_db::SeriesGrab,
        item: &sofarr_download::DownloadItem,
        naming: &sofarr_core::naming::NamingConfig,
        settings: &importer::ImportSettings,
    ) -> anyhow::Result<usize> {
        let all = self.db.all_series().await?;
        let show = all
            .iter()
            .find(|s| s.id == grab.series_id)
            .context("Serie nicht mehr in der Bibliothek")?;
        let episodes = self.db.episodes(Some(show.id)).await?;
        let remote = item
            .output_path
            .as_deref()
            .context("Download-Client meldet keinen Ordner")?;
        let download = self.config.download.map_path(remote);
        let plans = series::plan_episode_import(show, grab, &episodes, &download, naming)?;
        let media = self
            .db
            .setting(&format!("{}config_mediamanagement", series::PREFIX))
            .await?;
        let required = media
            .as_ref()
            .and_then(|m| m["episodeTitleRequired"].as_str());
        let pattern = match show.data.series_type.as_str() {
            "daily" => &naming.daily_episode_format,
            "anime" => &naming.anime_episode_format,
            _ => &naming.standard_episode_format,
        };
        let season_pack = parse_episode_pack(&grab.title);
        let covered: Vec<&sofarr_db::Episode> = episodes
            .iter()
            .filter(|e| plans.iter().any(|(_, ids)| ids.contains(&e.id)))
            .collect();
        if naming.rename_episodes
            && series::waits_for_titles(required, pattern, &covered, season_pack, Timestamp::now())
        {
            tracing::info!(release = %grab.title, "import waits for episode titles");
            self.db
                .update_series_grab(
                    grab.id,
                    GrabStatus::Completed,
                    None,
                    Some(
                        "Import wartet auf Episodentitel (höchstens 48 Stunden nach Ausstrahlung)",
                    ),
                    Timestamp::now(),
                )
                .await?;
            return Ok(0);
        }
        let quality = grab.quality.as_str();
        let mut n = 0;
        for (plan, episode_ids) in plans {
            let settings = settings.clone();
            let p = plan.clone();
            let size = tokio::task::spawn_blocking(move || importer::execute(&p, &settings))
                .await
                .context("Import abgebrochen")??;
            let mut file = importer::imported_file(
                &plan,
                size,
                &grab.title,
                quality,
                grab.custom_format_score,
            );
            file.relative_path = series::relative_to(show, &plan.destination);
            self.db
                .set_episode_file(show.id, &episode_ids, &file, Timestamp::now())
                .await?;
            tracing::info!(series = %show.data.title, file = %plan.destination.display(), "imported");
            n += 1;
        }
        self.db
            .update_series_grab(grab.id, GrabStatus::Imported, None, None, Timestamp::now())
            .await?;
        self.remove_completed(item, &download).await;
        Ok(n)
    }

    /// "Remove Completed": the imported download leaves the client's history and its
    /// folder is deleted if no video is left. Failures are only logged.
    async fn remove_completed(
        &self,
        item: &sofarr_download::DownloadItem,
        download: &std::path::Path,
    ) {
        if !self.config.download.remove_completed {
            return;
        }
        if let Some(client) = &self.nzbget
            && let Err(e) = client.remove(&item.id).await
        {
            tracing::warn!(download = %item.name, error = %e, "removing from history failed");
        }
        let dir = download.to_path_buf();
        match tokio::task::spawn_blocking(move || importer::remove_leftovers(&dir)).await {
            Ok(Ok(true)) => tracing::info!(folder = %download.display(), "download folder removed"),
            Ok(Ok(false)) => {}
            Ok(Err(e)) => tracing::warn!(error = format!("{e:#}"), "download folder not removed"),
            Err(e) => tracing::warn!(error = %e, "download folder not removed"),
        }
    }

    /// Live mode: fetches the NZB and hands it to nzbget.
    async fn send_to_download_client(
        &self,
        id: i64,
        pick: &Selected,
        now: Timestamp,
    ) -> anyhow::Result<()> {
        let category = self
            .config
            .nzbget
            .as_ref()
            .map_or("movies", |c| c.category.as_str());
        let result = self.fetch_and_send(&pick.release, category).await;
        match result {
            Ok(download_id) => {
                tracing::info!(movie = %pick.movie.title, release = %pick.release.title, %download_id, "grabbed");
                self.db
                    .update_grab(id, GrabStatus::Queued, Some(&download_id), None, now)
                    .await?;
            }
            Err(e) => {
                let message = format!("{e:#}");
                tracing::warn!(release = %pick.release.title, error = %message, "grab failed");
                self.db
                    .update_grab(id, GrabStatus::Failed, None, Some(&message), now)
                    .await?;
            }
        }
        Ok(())
    }

    /// Searches all indexers for one movie and grabs the best release.
    pub async fn search_movie(&self, movie_id: i64) -> anyhow::Result<SyncReport> {
        let _guard = self.jobs.lock().await;
        self.search_movie_locked(movie_id).await
    }

    async fn search_movie_locked(&self, movie_id: i64) -> anyhow::Result<SyncReport> {
        let movie = self
            .db
            .movie(movie_id)
            .await?
            .with_context(|| format!("Film {movie_id} nicht gefunden"))?;
        let query = match movie.year {
            Some(y) => format!("{} {y}", movie.title),
            None => movie.title.clone(),
        };
        let mut report = SyncReport::default();
        let mut releases = Vec::new();
        for indexer in &self.indexers {
            match indexer.search_movie(movie.imdb_id.as_deref(), &query).await {
                Ok(mut batch) => releases.append(&mut batch),
                Err(e) => report.errors.push(e.to_string()),
            }
        }
        // Search results belong to this movie even if the indexer sends no IMDb id.
        for release in &mut releases {
            release.imdb_id = None;
        }
        self.decide_and_grab(&releases, Some(movie_id), &mut report)
            .await?;
        Ok(report)
    }

    async fn decide_and_grab(
        &self,
        releases: &[sofarr_indexer::IndexerRelease],
        only: Option<i64>,
        report: &mut SyncReport,
    ) -> anyhow::Result<()> {
        let mut movies = self.db.movies().await?;
        if let Some(id) = only {
            movies.retain(|m| m.id == id);
        }
        // A setup without movies (series only) needs no movie rules.
        if movies.is_empty() {
            report.releases += releases.len();
            return Ok(());
        }
        let rules = library::load_rules(&self.db).await?;
        let tags = self.db.setting("tag").await?;
        let mut pending = BTreeMap::new();
        let mut seen = std::collections::HashSet::new();
        let mut imported = std::collections::HashSet::new();
        for grab in self.db.grabs(None, u32::MAX).await? {
            seen.insert((grab.movie_id, grab.guid.clone()));
            if grab.status == GrabStatus::Imported {
                imported.insert((grab.movie_id, grab.title.to_lowercase()));
            }
            let open = grab.status == GrabStatus::Shadow || grab.status.is_active();
            if open && !pending.contains_key(&grab.movie_id) {
                pending.insert(grab.movie_id, grab);
            }
        }
        let now = Timestamp::now();
        let selected = rss::decide(
            releases,
            &movies,
            &rules,
            &pending,
            &|movie, release| {
                seen.contains(&(movie.id, release.guid.clone()))
                    || imported.contains(&(movie.id, release.title.to_lowercase()))
                    || !self.indexer_allows(
                        &release.indexer,
                        &library::tag_labels(tags.as_ref(), &movie.tags),
                    )
            },
            now,
            report,
        );
        for pick in selected {
            self.grab(pick, now).await?;
        }
        Ok(())
    }

    /// Checks the download client: updates grab states and imports finished downloads.
    pub async fn check_downloads(&self) -> anyhow::Result<usize> {
        let _guard = self.jobs.lock().await;
        let Some(client) = &self.nzbget else {
            return Ok(0);
        };
        let items = client.items().await?;
        let series_settings = importer::ImportSettings::from_json(
            self.db
                .setting(&format!("{}config_mediamanagement", series::PREFIX))
                .await?
                .as_ref(),
        );
        let mut imported = self
            .check_series_downloads(&items, &series_settings)
            .await?;
        let mut rules = None;
        let naming = sofarr_core::naming::NamingConfig::from_json(
            &self.db.setting("config_naming").await?.unwrap_or_default(),
        );
        let settings = importer::ImportSettings::from_json(
            self.db.setting("config_mediamanagement").await?.as_ref(),
        );
        for status in [
            GrabStatus::Queued,
            GrabStatus::Downloading,
            GrabStatus::Completed,
        ] {
            for grab in self.db.grabs(Some(status), u32::MAX).await? {
                let Some(item) = grab
                    .download_id
                    .as_deref()
                    .and_then(|id| items.iter().find(|i| i.id == id))
                else {
                    continue;
                };
                let now = Timestamp::now();
                match item.state {
                    DownloadState::Queued | DownloadState::Paused => {}
                    DownloadState::Downloading => {
                        if grab.status != GrabStatus::Downloading {
                            self.db
                                .update_grab(grab.id, GrabStatus::Downloading, None, None, now)
                                .await?;
                        }
                    }
                    DownloadState::Failed => {
                        tracing::warn!(release = %grab.title, status = ?item.message, "download failed");
                        self.db
                            .update_grab(
                                grab.id,
                                GrabStatus::Failed,
                                None,
                                item.message.as_deref(),
                                now,
                            )
                            .await?;
                    }
                    DownloadState::Completed => {
                        if rules.is_none() {
                            rules = Some(library::load_rules(&self.db).await?);
                        }
                        let rules = rules.as_ref().context("rules")?;
                        match self.import(&grab, item, rules, &naming, &settings).await {
                            Ok(()) => imported += 1,
                            Err(e) => {
                                let message = format!("{e:#}");
                                tracing::warn!(release = %grab.title, error = %message, "import failed");
                                self.db
                                    .update_grab(
                                        grab.id,
                                        GrabStatus::Completed,
                                        None,
                                        Some(&message),
                                        now,
                                    )
                                    .await?;
                            }
                        }
                    }
                }
            }
        }
        Ok(imported)
    }

    async fn import(
        &self,
        grab: &sofarr_db::Grab,
        item: &sofarr_download::DownloadItem,
        rules: &library::Rules,
        naming: &sofarr_core::naming::NamingConfig,
        settings: &importer::ImportSettings,
    ) -> anyhow::Result<()> {
        let movie = self
            .db
            .movie(grab.movie_id)
            .await?
            .context("Film nicht mehr in der Bibliothek")?;
        let remote = item
            .output_path
            .as_deref()
            .context("Download-Client meldet keinen Ordner")?;
        let download = self.config.download.map_path(remote);
        let quality = grab.quality.as_str();
        let plan = importer::plan(&movie, &grab.title, quality, &download, naming)?;
        let settings = settings.clone();
        let plan_clone = plan.clone();
        let size = tokio::task::spawn_blocking(move || importer::execute(&plan_clone, &settings))
            .await
            .context("Import abgebrochen")??;
        let score = rules.score(
            movie.quality_profile_id,
            &grab.title,
            library::original_language(&movie),
        );
        let now = Timestamp::now();
        self.db
            .set_file(
                movie.id,
                &importer::imported_file(&plan, size, &grab.title, quality, score),
                now,
            )
            .await?;
        self.db
            .update_grab(grab.id, GrabStatus::Imported, None, None, now)
            .await?;
        tracing::info!(movie = %movie.title, file = %plan.destination.display(), "imported");
        self.remove_completed(item, &download).await;
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
                let input =
                    importer::naming_input(&metadata::as_library_movie(&movie), "", "Unknown");
                movie.path = importer::movie_folder(
                    std::path::Path::new(&list.root_folder),
                    &input,
                    &naming,
                )
                .to_string_lossy()
                .into_owned();
                let id = self.db.upsert_movie(&movie, Timestamp::now()).await?;
                tracing::info!(list = %list.name, movie = %movie.title, "added from import list");
                added.push(movie.title.clone());
                // Like an RSS grab, the search on add waits until the movie is available.
                let today = Timestamp::now().to_zoned(jiff::tz::TimeZone::UTC).date();
                let available = match (&rules, self.db.movie(id).await?) {
                    (Ok(rules), Some(stored)) => library::is_available(&stored, rules, today),
                    _ => true,
                };
                if list.search_on_add
                    && list.monitored
                    && available
                    && let Err(e) = self.search_movie_locked(id).await
                {
                    tracing::warn!(movie = %movie.title, error = format!("{e:#}"), "search failed");
                }
            }
        }
        Ok(added)
    }
}

/// Whether a release title is a season pack.
fn parse_episode_pack(title: &str) -> bool {
    sofarr_parser::parse_episode(title).is_some_and(|i| i.full_season)
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
                for show in self.db.all_series().await? {
                    match series::refresh(&self.db, &tvdb, &show, Timestamp::now()).await {
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

/// Runs a job every `interval` until the process ends.
pub async fn every<F, Fut>(name: &'static str, interval: Duration, job: F)
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<()>>,
{
    run_every(name, tokio::time::interval(interval), job).await;
}

/// Like [`every`], but the first run is one `interval` after the start (jobs that are
/// too expensive for every container start).
pub async fn every_later<F, Fut>(name: &'static str, interval: Duration, job: F)
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<()>>,
{
    let start = tokio::time::Instant::now() + interval;
    run_every(name, tokio::time::interval_at(start, interval), job).await;
}

async fn run_every<F, Fut>(name: &'static str, mut ticker: tokio::time::Interval, job: F)
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<()>>,
{
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticker.tick().await;
        if let Err(e) = job().await {
            tracing::error!(job = name, error = format!("{e:#}"), "job failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::{Json, Router, extract::State, routing::get, routing::post};
    use serde_json::{Value, json};

    use super::*;
    use crate::library::{import_documents, tests::documents};

    #[derive(Default)]
    struct Mock {
        /// nzbget history entries to report.
        history: Vec<Value>,
        appended: Vec<String>,
    }

    type Shared = Arc<Mutex<Mock>>;

    async fn feed() -> &'static str {
        r#"<rss><channel>
          <item><title>Erster.Film.2001.German.1080p.BluRay.x264-GRP</title><guid>g1</guid>
            <enclosure url="http://HOST/nzb/g1" length="5000000000"/></item>
          <item><title>Erster.Film.2001.720p.HDTV.x264-LOW</title><guid>g2</guid>
            <enclosure url="http://HOST/nzb/g2" length="1000000000"/></item>
        </channel></rss>"#
    }

    async fn rpc(State(mock): State<Shared>, Json(body): Json<Value>) -> Json<Value> {
        let mut mock = mock.lock().expect("lock");
        Json(match body["method"].as_str().unwrap_or_default() {
            "append" => {
                mock.appended
                    .push(body["params"][0].as_str().unwrap_or_default().to_owned());
                json!({"result": 77})
            }
            "listgroups" => json!({"result": []}),
            "history" => json!({"result": mock.history}),
            _ => json!({"result": true}),
        })
    }

    #[tokio::test]
    async fn live_pipeline_grabs_and_imports() {
        let root = std::env::temp_dir().join(format!("sofarr-live-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let library_dir = root.join("movies/Erster Film (2001)");
        let download_dir = root.join("downloads/Erster.Film.2001.German.1080p.BluRay.x264-GRP");
        std::fs::create_dir_all(&download_dir).expect("dirs");
        std::fs::write(download_dir.join("erster.mkv"), vec![1u8; 4096]).expect("video");

        let mock: Shared = Arc::default();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let feed_body = feed().await.replace("HOST", &addr.to_string());
        let app = Router::new()
            .route("/api", get(move || async move { feed_body }))
            .route("/nzb/{id}", get(|| async { "<nzb/>" }))
            .route("/jsonrpc", post(rpc))
            .with_state(Arc::clone(&mock));
        tokio::spawn(async move { axum::serve(listener, app).await });

        let config = Config::parse(&format!(
            "[automation]\nmode = \"live\"\n\
             [[indexer]]\nname = \"idx\"\nurl = \"http://{addr}/api\"\napi_key = \"k\"\n\
             [nzbget]\nurl = \"http://{addr}\"\nusername = \"u\"\npassword = \"p\"\n\
             [[download.path_map]]\nremote = \"/remote\"\nlocal = \"{}\"\n",
            root.join("downloads").display()
        ))
        .expect("config");
        let db = Db::in_memory().await.expect("db");
        let mut docs = documents();
        docs.insert(
            "config_naming".to_owned(),
            json!({"renameMovies": true, "standardMovieFormat": "{Movie Title} ({Release Year}) {Quality Full}{-Release Group}"}),
        );
        docs.get_mut("movie").expect("movies")[0]["path"] = json!(library_dir.to_string_lossy());
        import_documents(&db, &docs, Timestamp::now())
            .await
            .expect("import");
        let mut app = App::with_db(config, db).expect("app");
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("client");
        app.http = client.clone();
        app.indexers = vec![Newznab::new(
            NewznabConfig {
                name: "idx".to_owned(),
                url: format!("http://{addr}/api"),
                api_key: "k".to_owned(),
                categories: vec![2000],
                limit: 100,
            },
            client.clone(),
        )];
        app.nzbget = Some(Nzbget::new(
            client,
            NzbgetConfig {
                url: format!("http://{addr}"),
                username: "u".to_owned(),
                password: "p".to_owned(),
            },
        ));

        let report = app.rss_sync().await.expect("rss");
        assert_eq!(report.approved, 1, "{report:?}");
        assert_eq!(
            mock.lock().expect("lock").appended,
            ["Erster.Film.2001.German.1080p.BluRay.x264-GRP.nzb"]
        );
        let queued = app
            .db
            .grabs(Some(GrabStatus::Queued), 10)
            .await
            .expect("grabs");
        assert_eq!(queued[0].download_id.as_deref(), Some("77"));

        // A second sync must not grab the same release again.
        let again = app.rss_sync().await.expect("rss");
        assert_eq!(again.approved, 0);

        mock.lock().expect("lock").history = vec![json!({
            "NZBID": 77, "Name": "Erster.Film.2001.German.1080p.BluRay.x264-GRP",
            "Status": "SUCCESS/ALL",
            "DestDir": "/remote/Erster.Film.2001.German.1080p.BluRay.x264-GRP"
        })];
        assert_eq!(app.check_downloads().await.expect("check"), 1);
        let target = library_dir.join("Erster Film (2001) Bluray-1080p-GRP.mkv");
        assert!(target.exists(), "{}", target.display());
        let movie = app.db.movie_by_tmdb(101).await.expect("db").expect("movie");
        let file = movie.file.expect("file recorded");
        assert_eq!(file.quality, "Bluray-1080p");
        assert_eq!(file.custom_format_score, 10);
        assert_eq!(
            app.db
                .grabs(Some(GrabStatus::Imported), 10)
                .await
                .expect("grabs")
                .len(),
            1
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn live_series_pipeline() {
        let root = std::env::temp_dir().join(format!("sofarr-series-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let show_dir = root.join("tv/Kleine Insel");
        let download_dir = root.join("downloads/Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP");
        std::fs::create_dir_all(&download_dir).expect("dirs");
        std::fs::write(
            download_dir.join("kleine.insel.s01e02.mkv"),
            vec![1u8; 1024],
        )
        .expect("video");

        let mock: Shared = Arc::default();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let feed = format!(
            r#"<rss><channel>
              <item><title>Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP</title><guid>s1</guid>
                <enclosure url="http://{addr}/nzb/s1" length="2000000000"/></item>
            </channel></rss>"#
        );
        let app = Router::new()
            .route("/api", get(move || async move { feed }))
            .route("/nzb/{id}", get(|| async { "<nzb/>" }))
            .route("/jsonrpc", post(rpc))
            .with_state(Arc::clone(&mock));
        tokio::spawn(async move { axum::serve(listener, app).await });

        let config = Config::parse(&format!(
            "[automation]\nmode = \"live\"\n\
             [nzbget]\nurl = \"http://{addr}\"\nusername = \"u\"\npassword = \"p\"\n\
             [[download.path_map]]\nremote = \"/remote\"\nlocal = \"{}\"\n",
            root.join("downloads").display()
        ))
        .expect("config");
        let db = Db::in_memory().await.expect("db");
        let (mut docs, episodes) = crate::series::tests::sonarr_documents();
        docs.get_mut("series").expect("series")[0]["path"] = json!(show_dir.to_string_lossy());
        crate::series::import_documents(&db, &docs, &episodes, Timestamp::now())
            .await
            .expect("import");
        let mut app = App::with_db(config, db).expect("app");
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("client");
        app.http = client.clone();
        app.indexers = vec![Newznab::new(
            NewznabConfig {
                name: "idx".to_owned(),
                url: format!("http://{addr}/api"),
                api_key: "k".to_owned(),
                categories: vec![5000],
                limit: 100,
            },
            client.clone(),
        )];
        app.nzbget = Some(Nzbget::new(
            client,
            NzbgetConfig {
                url: format!("http://{addr}"),
                username: "u".to_owned(),
                password: "p".to_owned(),
            },
        ));

        let report = app.rss_sync().await.expect("rss");
        assert_eq!(report.series_grabs, 1, "{report:?}");
        mock.lock().expect("lock").history = vec![json!({
            "NZBID": 77, "Name": "Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP",
            "Status": "SUCCESS/ALL",
            "DestDir": "/remote/Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP"
        })];
        assert_eq!(app.check_downloads().await.expect("check"), 1);
        let target = show_dir.join("Staffel 01/Kleine Insel - S01E02 - Sturm WEBDL-1080p.mkv");
        assert!(target.exists(), "{}", target.display());
        let eps = app.db.episodes(None).await.expect("episodes");
        let e2 = eps.iter().find(|e| e.data.number == 2).expect("e2");
        assert_eq!(
            e2.file.as_ref().map(|f| f.relative_path.as_str()),
            Some("Staffel 01/Kleine Insel - S01E02 - Sturm WEBDL-1080p.mkv")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn indexer_tags_restrict_indexers() {
        let db = Db::in_memory().await.expect("db");
        let config = Config::parse(
            "[[indexer]]\nname = \"a\"\nurl = \"u\"\napi_key = \"k\"\ntags = [\"Kids\"]\n\
             [[indexer]]\nname = \"b\"\nurl = \"u\"\napi_key = \"k\"\n",
        )
        .expect("config");
        let app = App::with_db(config, db).expect("app");
        assert!(app.indexer_allows("a", &["kids".to_owned()]));
        assert!(!app.indexer_allows("a", &[]));
        assert!(app.indexer_allows("b", &[]));
    }
}
