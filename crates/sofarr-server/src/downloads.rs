//! The download check: grab states from the download client, failures and vanished
//! downloads (spec fail-handling), imports of finished downloads.

use anyhow::Context;
use jiff::{SignedDuration, Timestamp};
use sofarr_db::{GrabStatus, MediaRef};
use sofarr_download::{DownloadClient, DownloadItem, DownloadState, Failure};

use crate::{
    app::App,
    failures::{FailedGrab, Research, VANISH_HOURS, VANISHED},
    importer, library, series,
};

/// What the client says about the download of an active grab.
enum Sighting<'a> {
    Seen(&'a DownloadItem),
    /// Missing; the grab's download vanished if it has been missing long enough.
    Absent {
        vanished: bool,
    },
}

fn sighting<'a>(
    items: &'a [DownloadItem],
    download_id: &str,
    absent_since: Option<Timestamp>,
    now: Timestamp,
) -> Sighting<'a> {
    match items.iter().find(|i| i.id == download_id) {
        Some(item) => Sighting::Seen(item),
        None => Sighting::Absent {
            vanished: absent_since
                .is_some_and(|since| now - SignedDuration::from_hours(VANISH_HOURS) >= since),
        },
    }
}

impl App {
    /// Checks the download client: updates grab states, records failures and vanished
    /// downloads, imports finished downloads, then searches again what failed.
    pub async fn check_downloads(&self) -> anyhow::Result<usize> {
        self.check_downloads_at(Timestamp::now()).await
    }

    /// [`App::check_downloads`] at a given time. A client that cannot be reached
    /// changes nothing: the check counts neither as an absence nor as a sighting.
    pub async fn check_downloads_at(&self, now: Timestamp) -> anyhow::Result<usize> {
        let _guard = self.jobs.lock().await;
        let Some(client) = &self.nzbget else {
            return Ok(0);
        };
        let items = client.items().await?;
        let mut research = Research::default();
        let mut imported = self
            .check_series_downloads(&items, now, &mut research)
            .await?;
        imported += self
            .check_movie_downloads(&items, now, &mut research)
            .await?;
        self.run_research(research).await;
        Ok(imported)
    }

    async fn check_movie_downloads(
        &self,
        items: &[DownloadItem],
        now: Timestamp,
        research: &mut Research,
    ) -> anyhow::Result<usize> {
        let mut imported = 0;
        let mut rules = None;
        let naming = sofarr_core::naming::NamingConfig::from_json(
            &self.db.setting("config_naming").await?.unwrap_or_default(),
        );
        let settings = importer::ImportSettings::from_json(
            self.db.setting("config_mediamanagement").await?.as_ref(),
        );
        for grab in self.db.grabs(None, u32::MAX).await? {
            if !grab.status.is_active() {
                continue;
            }
            let Some(download_id) = grab.download_id.as_deref() else {
                continue;
            };
            let failed = FailedGrab {
                id: grab.id,
                media: MediaRef::Movie(grab.movie_id),
                title: &grab.title,
                guid: &grab.guid,
                indexer: &grab.indexer,
            };
            let item = match sighting(items, download_id, grab.absent_since, now) {
                Sighting::Absent { vanished: true } => {
                    self.fail_grab(failed, VANISHED, false, now).await?;
                    research.movie(grab.movie_id);
                    continue;
                }
                Sighting::Absent { vanished: false } => {
                    if grab.absent_since.is_none() {
                        self.db.set_grab_absent(grab.id, Some(now)).await?;
                    }
                    continue;
                }
                Sighting::Seen(item) => item,
            };
            if grab.absent_since.is_some() {
                self.db.set_grab_absent(grab.id, None).await?;
            }
            match item.state {
                DownloadState::Queued | DownloadState::Paused => {}
                DownloadState::Downloading => {
                    if grab.status != GrabStatus::Downloading {
                        self.db
                            .update_grab(grab.id, GrabStatus::Downloading, None, None, now)
                            .await?;
                    }
                }
                DownloadState::Failed(kind) => {
                    let reason = item.message.as_deref().unwrap_or("Download fehlgeschlagen");
                    self.fail_grab(failed, reason, kind == Failure::Release, now)
                        .await?;
                    research.movie(grab.movie_id);
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
        Ok(imported)
    }

    /// Series part of the download check.
    async fn check_series_downloads(
        &self,
        items: &[DownloadItem],
        now: Timestamp,
        research: &mut Research,
    ) -> anyhow::Result<usize> {
        let settings = importer::ImportSettings::from_json(
            self.db
                .setting(&format!("{}config_mediamanagement", series::PREFIX))
                .await?
                .as_ref(),
        );
        let mut imported = 0;
        let mut naming = None;
        let mut episodes = None;
        for grab in self.db.series_grabs(None).await? {
            if !grab.status.is_active() {
                continue;
            }
            let Some(download_id) = grab.download_id.as_deref() else {
                continue;
            };
            let failed = FailedGrab {
                id: grab.id,
                media: MediaRef::Series(grab.series_id),
                title: &grab.title,
                guid: &grab.guid,
                indexer: &grab.indexer,
            };
            let failure = match sighting(items, download_id, grab.absent_since, now) {
                Sighting::Absent { vanished: true } => Some((VANISHED, false)),
                Sighting::Absent { vanished: false } => {
                    if grab.absent_since.is_none() {
                        self.db.set_series_grab_absent(grab.id, Some(now)).await?;
                    }
                    continue;
                }
                Sighting::Seen(item) => {
                    if grab.absent_since.is_some() {
                        self.db.set_series_grab_absent(grab.id, None).await?;
                    }
                    match item.state {
                        DownloadState::Queued | DownloadState::Paused => None,
                        DownloadState::Downloading => {
                            if grab.status != GrabStatus::Downloading {
                                self.db
                                    .update_series_grab(
                                        grab.id,
                                        GrabStatus::Downloading,
                                        None,
                                        None,
                                        now,
                                    )
                                    .await?;
                            }
                            None
                        }
                        DownloadState::Failed(kind) => Some((
                            item.message.as_deref().unwrap_or("Download fehlgeschlagen"),
                            kind == Failure::Release,
                        )),
                        DownloadState::Completed => {
                            if naming.is_none() {
                                naming = Some(series::naming_config(&self.db).await?);
                            }
                            let naming = naming.as_ref().context("naming")?;
                            match self.import_episodes(&grab, item, naming, &settings).await {
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
                            None
                        }
                    }
                }
            };
            if let Some((reason, blocklist)) = failure {
                self.fail_grab(failed, reason, blocklist, now).await?;
                if episodes.is_none() {
                    episodes = Some(self.db.episodes(None).await?);
                }
                let all = episodes.as_deref().unwrap_or_default();
                research.series(grab.series_id, &grab.title, &grab.episode_ids, all);
            }
        }
        Ok(imported)
    }

    async fn import_episodes(
        &self,
        grab: &sofarr_db::SeriesGrab,
        item: &DownloadItem,
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
        let season_pack = sofarr_parser::parse_episode(&grab.title).is_some_and(|i| i.full_season);
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
    async fn remove_completed(&self, item: &DownloadItem, download: &std::path::Path) {
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

    async fn import(
        &self,
        grab: &sofarr_db::Grab,
        item: &DownloadItem,
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
}

/// A grab, by its id in the table of its media kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrabRef {
    Movie(i64),
    Series(i64),
}

/// Why marking a grab as failed was refused; the messages are German.
#[derive(Debug)]
pub enum MarkError {
    NotFound(String),
    NotActive(String),
    /// The download client cannot be reached (or is not configured).
    Unavailable(String),
    Failed(anyhow::Error),
}

impl From<anyhow::Error> for MarkError {
    fn from(e: anyhow::Error) -> Self {
        Self::Failed(e)
    }
}

impl From<sofarr_db::DbError> for MarkError {
    fn from(e: sofarr_db::DbError) -> Self {
        Self::Failed(e.into())
    }
}

/// Reason of a grab the user marked as failed.
pub const MARKED: &str = "Manuell als fehlgeschlagen markiert";

/// The download folder sofarr may delete for a download: it must be named after the
/// download, so a client reporting a category or root folder never deletes more.
fn own_folder(dir: &std::path::Path, item: &DownloadItem) -> bool {
    !item.name.is_empty()
        && dir.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
            // nzbget's intermediate folder is `<name>.#<id>`.
            n == item.name
                || n.strip_prefix(item.name.as_str())
                    .and_then(|rest| rest.strip_prefix(".#"))
                    .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
        })
}

impl App {
    /// Marks an active grab as failed: removes its download and files from the
    /// download client (if it still has it), fails the grab, blocklists the release
    /// and searches again as after any failure.
    pub async fn mark_failed(&self, grab: GrabRef) -> Result<(), MarkError> {
        let _guard = self.jobs.lock().await;
        let (id, media, title, guid, indexer, status, download_id, episode_ids) = match grab {
            GrabRef::Movie(id) => {
                let g = self
                    .db
                    .grab(id)
                    .await?
                    .ok_or_else(|| MarkError::NotFound(format!("Grab {id} nicht gefunden")))?;
                let media = MediaRef::Movie(g.movie_id);
                (
                    g.id,
                    media,
                    g.title,
                    g.guid,
                    g.indexer,
                    g.status,
                    g.download_id,
                    Vec::new(),
                )
            }
            GrabRef::Series(id) => {
                let g = self.db.series_grab(id).await?.ok_or_else(|| {
                    MarkError::NotFound(format!("Serien-Grab {id} nicht gefunden"))
                })?;
                let media = MediaRef::Series(g.series_id);
                (
                    g.id,
                    media,
                    g.title,
                    g.guid,
                    g.indexer,
                    g.status,
                    g.download_id,
                    g.episode_ids,
                )
            }
        };
        if !status.is_active() {
            return Err(MarkError::NotActive(format!(
                "Grab {id} ist nicht aktiv (Status {}) und kann nicht als fehlgeschlagen markiert werden",
                status.as_str()
            )));
        }
        let client = self.nzbget.as_ref().ok_or_else(|| {
            MarkError::Unavailable("Kein Download-Client konfiguriert".to_owned())
        })?;
        let unavailable = |e: sofarr_download::DownloadError| {
            MarkError::Unavailable(format!("Download-Client nicht erreichbar: {e}"))
        };
        let items = client.items().await.map_err(unavailable)?;
        if let Some(item) = download_id
            .as_deref()
            .and_then(|d| items.iter().find(|i| i.id == d))
        {
            client.remove(&item.id).await.map_err(unavailable)?;
            self.delete_download_files(item).await;
        }
        let now = Timestamp::now();
        let failed = FailedGrab {
            id,
            media,
            title: &title,
            guid: &guid,
            indexer: &indexer,
        };
        self.fail_grab(failed, MARKED, true, now).await?;
        let mut research = Research::default();
        match media {
            MediaRef::Movie(movie_id) => research.movie(movie_id),
            MediaRef::Series(series_id) => {
                let episodes = self.db.episodes(Some(series_id)).await?;
                research.series(series_id, &title, &episode_ids, &episodes);
            }
        }
        self.run_research(research).await;
        Ok(())
    }

    /// Deletes the folder of a removed download; failures are only logged.
    async fn delete_download_files(&self, item: &DownloadItem) {
        let Some(remote) = item.output_path.as_deref() else {
            return;
        };
        let dir = self.config.download.map_path(remote);
        if !own_folder(&dir, item) {
            tracing::warn!(folder = %dir.display(), download = %item.name, "folder not deleted: not named after the download");
            return;
        }
        let target = dir.clone();
        let removed = tokio::task::spawn_blocking(move || {
            if target.is_dir() {
                std::fs::remove_dir_all(&target)
            } else {
                Ok(())
            }
        })
        .await;
        match removed {
            Ok(Ok(())) => tracing::info!(folder = %dir.display(), "download files removed"),
            Ok(Err(e)) => {
                tracing::warn!(folder = %dir.display(), error = %e, "download files not removed")
            }
            Err(e) => tracing::warn!(error = %e, "download files not removed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_folders_named_after_the_download_are_deleted() {
        let item = DownloadItem {
            id: "1".to_owned(),
            name: "Film.2020.1080p-GRP".to_owned(),
            category: "movies".to_owned(),
            state: DownloadState::Completed,
            size_bytes: 0,
            remaining_bytes: 0,
            output_path: None,
            message: None,
        };
        let path = std::path::Path::new;
        assert!(own_folder(
            path("/data/usenet/movies/Film.2020.1080p-GRP"),
            &item
        ));
        assert!(own_folder(
            path("/data/usenet/inter/Film.2020.1080p-GRP.#1"),
            &item
        ));
        assert!(!own_folder(path("/data/usenet/movies"), &item));
        assert!(!own_folder(
            path("/data/usenet/movies/Film.2020.1080p-GRP-other"),
            &item
        ));
        assert!(!own_folder(path("/"), &item));
        let unnamed = DownloadItem {
            name: String::new(),
            ..item
        };
        assert!(!own_folder(path("/data/x"), &unnamed));
    }
}
