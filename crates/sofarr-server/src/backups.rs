//! Backups of the database (spec backups): scheduled, manual and pre-migration backups,
//! pruning, the backup list, choosing a restore and applying it at start.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context;
use jiff::{SignedDuration, Timestamp};
use serde::Serialize;

use crate::{
    app::App,
    backup_files::{self, BackupFile, Kind, PRE_MIGRATION_KEEP},
    config::Config,
};

/// How often the scheduler looks whether a scheduled backup is due.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(60);

/// The last failed backup operation, shown in the list until a backup succeeds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupError {
    #[serde(serialize_with = "backup_files::as_text")]
    pub time: Timestamp,
    pub message: String,
}

/// The answer of the backup list.
#[derive(Debug, Serialize)]
pub struct BackupList {
    pub backups: Vec<BackupFile>,
    pub pending_restore: Option<String>,
    pub last_error: Option<BackupError>,
}

/// Why a backup request was refused; the messages are German.
#[derive(Debug)]
pub enum BackupRequestError {
    /// No such backup in the folder (404).
    NotFound,
    /// Not a valid backup (422).
    Invalid(String),
    /// The backup folder cannot be read (503).
    Unreadable(String),
    /// Writing, deleting or recording failed (500).
    Failed(String),
}

/// Reserves a name, lets `write` fill `<name>.tmp`, then gives it the name; a failure
/// leaves neither file behind.
async fn write_with<F, Fut>(
    dir: &Path,
    kind: Kind,
    migration: Option<i64>,
    now: Timestamp,
    write: F,
) -> anyhow::Result<BackupFile>
where
    F: FnOnce(PathBuf) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<()>>,
{
    let path = {
        let target = dir.to_path_buf();
        tokio::task::spawn_blocking(move || backup_files::reserve(&target, kind, now, migration))
            .await?
            .with_context(|| format!("Sicherungsordner {} nicht beschreibbar", dir.display()))?
    };
    let tmp = backup_files::tmp_path(&path);
    let written = match write(tmp.clone()).await {
        Ok(()) => std::fs::rename(&tmp, &path).map_err(anyhow::Error::from),
        Err(e) => Err(e),
    };
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_file(&path);
        return Err(e.context("Sicherung nicht geschrieben"));
    }
    let size = std::fs::metadata(&path)?.len();
    Ok(BackupFile {
        name: path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned(),
        kind,
        size,
        time: now,
        valid: true,
    })
}

/// Deletes backups of `kind` beyond `keep` (never the pending restore).
async fn prune(
    dir: &Path,
    db: &Path,
    kind: Kind,
    keep: usize,
    now: Timestamp,
) -> anyhow::Result<()> {
    let files = backup_files::list(dir).await?;
    let pending = backup_files::read_marker(db).ok().flatten();
    let mut failed = Vec::new();
    for name in backup_files::to_prune(&files, kind, keep, now, pending.as_deref()) {
        match std::fs::remove_file(dir.join(&name)) {
            Ok(()) => tracing::info!(backup = %name, "old backup deleted"),
            Err(e) => failed.push(format!("{name}: {e}")),
        }
    }
    anyhow::ensure!(
        failed.is_empty(),
        "Alte Sicherungen nicht gelöscht: {}",
        failed.join(", ")
    );
    Ok(())
}

/// Before migrations run on an existing database: a pre-migration backup, then the
/// three newest of them are kept. Without pending migrations or a database: nothing.
/// A failed pruning does not stop the start; it is returned to be shown in the list.
pub async fn before_migrations(
    config: &Config,
    now: Timestamp,
) -> anyhow::Result<Option<BackupError>> {
    let db = &config.database.path;
    let pending = sofarr_db::pending_migrations(db)
        .await
        .with_context(|| format!("Datenbank {} nicht lesbar", db.display()))?;
    let Some(first) = pending.as_ref().and_then(|p| p.first()) else {
        return Ok(None);
    };
    let dir = &config.backup.dir;
    let file = write_with(
        dir,
        Kind::PreMigration,
        Some(first.version),
        now,
        |tmp| async move { Ok(sofarr_db::snapshot_file(db, &tmp).await?) },
    )
    .await
    .context(
        "Sicherung vor der Datenbank-Migration fehlgeschlagen; Migrationen nicht ausgeführt",
    )?;
    tracing::info!(backup = %file.name, "pre-migration backup written");
    match prune(dir, db, Kind::PreMigration, PRE_MIGRATION_KEEP, now).await {
        Ok(()) => Ok(None),
        Err(e) => {
            let message = format!("{e:#}");
            tracing::warn!(error = %message, "pruning pre-migration backups failed");
            Ok(Some(BackupError { time: now, message }))
        }
    }
}

/// Puts a pending restore in place before the database is opened; `None` if there is
/// none or it cannot be used (the reason is logged). The marker is cleared either way.
pub async fn apply_pending_restore(config: &Config, now: Timestamp) -> Option<backup_files::Swap> {
    let db = &config.database.path;
    let marker = match backup_files::read_marker(db) {
        Ok(None) => return None,
        Ok(Some(name)) => Some(name),
        Err(e) => {
            tracing::warn!(error = %e, "pending restore unreadable");
            None
        }
    };
    // Cleared first: a marker left behind would restore the same backup again at the
    // next start, over everything changed since.
    if let Err(e) = backup_files::clear_marker(db) {
        tracing::warn!(error = %e, "pending restore not cleared, not applied");
        return None;
    }
    match marker {
        Some(name) => restore_named(config, &name, now).await,
        None => None,
    }
}

async fn restore_named(config: &Config, name: &str, now: Timestamp) -> Option<backup_files::Swap> {
    if !backup_files::is_plain_name(name) {
        tracing::warn!(
            backup = name,
            "pending restore names a file outside the backup folder"
        );
        return None;
    }
    let path = config.backup.dir.join(name);
    if !path.is_file() {
        tracing::warn!(backup = name, "pending restore: backup no longer exists");
        return None;
    }
    if !sofarr_db::inspect(&path).await.valid {
        tracing::warn!(backup = name, "pending restore: not a valid backup");
        return None;
    }
    match backup_files::put_in_place(&path, &config.database.path, now).await {
        Ok(swap) => {
            tracing::info!(backup = name, replaced = ?swap.replaced, "backup restored");
            Some(swap)
        }
        Err(e) => {
            tracing::warn!(backup = name, error = %e, "restore failed, previous database kept");
            None
        }
    }
}

/// Opens the app for `serve`: a pending restore is put in place first (its file is the
/// snapshot, so no pre-migration backup); if the restored database cannot be opened
/// (migrations fail), the previous one goes back in place and sofarr starts with it.
pub async fn open_for_serve(config: Config, now: Timestamp) -> anyhow::Result<App> {
    let Some(swap) = apply_pending_restore(&config, now).await else {
        return App::new(config).await;
    };
    match App::open(config.clone(), false).await {
        Ok(app) => Ok(app),
        Err(e) => {
            tracing::warn!(
                error = format!("{e:#}"),
                "restored database unusable, previous database back"
            );
            swap.undo()
                .context("Bisherige Datenbank nicht zurückgelegt")?;
            App::new(config).await
        }
    }
}

/// Starts the scheduled backups (the first look right away); `None` while
/// `interval_hours = 0`.
pub fn spawn_schedule(app: &std::sync::Arc<App>) -> Option<tokio::task::JoinHandle<()>> {
    if app.config.backup.interval_hours == 0 {
        return None;
    }
    let job = std::sync::Arc::clone(app);
    Some(tokio::spawn(crate::app::every(
        "backup",
        CHECK_INTERVAL,
        move || {
            let job = std::sync::Arc::clone(&job);
            async move { job.backup_tick(Timestamp::now()).await.map(|_| ()) }
        },
    )))
}

impl App {
    /// Shows an error that happened before the app existed (pre-migration pruning).
    pub fn show_backup_error(&self, error: BackupError) {
        if let Ok(mut last) = self.backup_error.lock() {
            *last = Some(error);
        }
    }

    fn record_backup_error(&self, now: Timestamp, error: &anyhow::Error) {
        let message = format!("{error:#}");
        tracing::warn!(error = %message, "backup failed");
        if let Ok(mut last) = self.backup_error.lock() {
            *last = Some(BackupError { time: now, message });
        }
    }

    fn clear_backup_error(&self) {
        if let Ok(mut last) = self.backup_error.lock() {
            *last = None;
        }
    }

    /// Writes a backup of the running database; a scheduled one prunes afterwards. A
    /// failure is logged and shown in the list until a backup succeeds.
    pub async fn write_backup(&self, kind: Kind, now: Timestamp) -> anyhow::Result<BackupFile> {
        let _guard = self.backup_lock.lock().await;
        let dir = &self.config.backup.dir;
        let db = &self.db;
        let written = write_with(dir, kind, None, now, |tmp| async move {
            Ok(db.backup_into(&tmp).await?)
        })
        .await;
        let file = match written {
            Ok(file) => file,
            Err(e) => {
                self.record_backup_error(now, &e);
                return Err(e);
            }
        };
        tracing::info!(backup = %file.name, "backup written");
        self.clear_backup_error();
        if kind == Kind::Scheduled {
            let keep = usize::try_from(self.config.backup.keep).unwrap_or(usize::MAX);
            if let Err(e) = prune(dir, &self.config.database.path, Kind::Scheduled, keep, now).await
            {
                self.record_backup_error(now, &e);
            }
        }
        Ok(file)
    }

    /// Whether a scheduled backup is due: the interval has passed since the newest
    /// scheduled backup that does not lie in the future, or there is none.
    pub async fn backup_due(&self, now: Timestamp) -> anyhow::Result<bool> {
        let hours = self.config.backup.interval_hours;
        if hours == 0 {
            return Ok(false);
        }
        // Names are enough here: no file is opened for the look every minute.
        let newest = backup_files::scheduled_times(&self.config.backup.dir)?
            .into_iter()
            .filter(|t| *t <= now)
            .max();
        let interval = SignedDuration::from_hours(i64::try_from(hours).unwrap_or(i64::MAX));
        Ok(newest.is_none_or(|t| now.checked_sub(interval).is_ok_and(|due| due >= t)))
    }

    /// One look of the scheduler: writes a scheduled backup when one is due.
    pub async fn backup_tick(&self, now: Timestamp) -> anyhow::Result<Option<BackupFile>> {
        if !self.backup_due(now).await? {
            return Ok(None);
        }
        self.write_backup(Kind::Scheduled, now).await.map(Some)
    }

    pub async fn backup_list(&self) -> Result<BackupList, BackupRequestError> {
        let dir = &self.config.backup.dir;
        let backups = backup_files::list(dir).await.map_err(|e| {
            BackupRequestError::Unreadable(format!(
                "Sicherungsordner {} nicht lesbar: {e}",
                dir.display()
            ))
        })?;
        let pending_restore = backup_files::read_marker(&self.config.database.path)
            .ok()
            .flatten();
        let last_error = self.backup_error.lock().ok().and_then(|e| e.clone());
        Ok(BackupList {
            backups,
            pending_restore,
            last_error,
        })
    }

    /// The path of a backup in the folder; anything else (other names, files that are
    /// no sofarr database) is not found.
    pub async fn backup_path(&self, name: &str) -> Result<PathBuf, BackupRequestError> {
        if !backup_files::is_plain_name(name) || name.ends_with(".tmp") {
            return Err(BackupRequestError::NotFound);
        }
        let path = self.config.backup.dir.join(name);
        if !path.is_file() || !sofarr_db::inspect(&path).await.sofarr {
            return Err(BackupRequestError::NotFound);
        }
        Ok(path)
    }

    /// Deletes a backup; deleting the pending restore clears it.
    pub async fn delete_backup(&self, name: &str) -> Result<(), BackupRequestError> {
        let _guard = self.backup_lock.lock().await;
        let path = self.backup_path(name).await?;
        let db = &self.config.database.path;
        if let Err(e) = std::fs::remove_file(&path) {
            let error = anyhow::anyhow!("Sicherung {name} nicht gelöscht: {e}");
            self.record_backup_error(Timestamp::now(), &error);
            return Err(BackupRequestError::Failed(error.to_string()));
        }
        if backup_files::read_marker(db).ok().flatten().as_deref() == Some(name) {
            backup_files::clear_marker(db).map_err(|e| {
                let error = anyhow::anyhow!("Vorgemerkte Wiederherstellung nicht aufgehoben: {e}");
                self.record_backup_error(Timestamp::now(), &error);
                BackupRequestError::Failed(error.to_string())
            })?;
        }
        Ok(())
    }

    /// Records a valid backup as the pending restore, applied at the next start.
    pub async fn choose_restore(&self, name: &str) -> Result<(), BackupRequestError> {
        let _guard = self.backup_lock.lock().await;
        let path = self.backup_path(name).await?;
        if !sofarr_db::inspect(&path).await.valid {
            return Err(BackupRequestError::Invalid(format!(
                "{name} ist keine gültige Sicherung (beschädigt oder von einer neueren sofarr-Version)"
            )));
        }
        backup_files::write_marker(&self.config.database.path, name).map_err(|e| {
            BackupRequestError::Failed(format!("Wiederherstellung nicht vorgemerkt: {e}"))
        })
    }

    pub async fn cancel_restore(&self) -> Result<(), BackupRequestError> {
        let _guard = self.backup_lock.lock().await;
        backup_files::clear_marker(&self.config.database.path).map_err(|e| {
            BackupRequestError::Failed(format!(
                "Vorgemerkte Wiederherstellung nicht aufgehoben: {e}"
            ))
        })
    }
}
