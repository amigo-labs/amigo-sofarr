//! Database files as a whole: consistent copies (`VACUUM INTO`), inspection of backup
//! files and pending migrations (spec backups).

use std::{borrow::Cow, path::Path};

use sqlx::{
    Connection, SqliteConnection,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
};

use crate::{Db, Result};

/// sofarr's schema migrations.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// What a database file is, judged by opening it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inspection {
    /// Its migration table starts with sofarr's first migration.
    pub sofarr: bool,
    /// A sofarr database that passes the integrity check and has no migration applied
    /// that this version does not know.
    pub valid: bool,
}

const FOREIGN: Inspection = Inspection {
    sofarr: false,
    valid: false,
};

/// A migration this version would run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub version: i64,
    pub description: String,
}

async fn open_read_only(path: &Path) -> Result<SqliteConnection> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .immutable(true);
    Ok(SqliteConnection::connect_with(&options).await?)
}

/// Applied migrations (version, description); `None` without a migration table.
async fn applied(conn: &mut SqliteConnection) -> Result<Option<Vec<(i64, String)>>> {
    let exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(&mut *conn)
    .await?;
    if exists == 0 {
        return Ok(None);
    }
    Ok(Some(
        sqlx::query_as(
            "SELECT version, description FROM _sqlx_migrations WHERE success = 1 ORDER BY version",
        )
        .fetch_all(&mut *conn)
        .await?,
    ))
}

/// Opens a file read-only and tells whether it is a (valid) sofarr database; a file
/// SQLite cannot read is none.
pub async fn inspect(path: &Path) -> Inspection {
    try_inspect(path).await.unwrap_or(FOREIGN)
}

async fn try_inspect(path: &Path) -> Result<Inspection> {
    let mut conn = open_read_only(path).await?;
    let Some(applied) = applied(&mut conn).await? else {
        return Ok(FOREIGN);
    };
    let sofarr = match (applied.first(), MIGRATOR.iter().next()) {
        (Some((version, description)), Some(first)) => {
            *version == first.version && *description == first.description
        }
        _ => false,
    };
    if !sofarr {
        return Ok(FOREIGN);
    }
    let known = applied
        .iter()
        .all(|(v, _)| MIGRATOR.iter().any(|m| m.version == *v));
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&mut conn)
        .await?;
    conn.close().await?;
    Ok(Inspection {
        sofarr: true,
        valid: known && integrity == "ok",
    })
}

/// Migrations this version would run on the database file; `None` if there is no file.
pub async fn pending_migrations(path: &Path) -> Result<Option<Vec<Pending>>> {
    if !path.exists() {
        return Ok(None);
    }
    let options = SqliteConnectOptions::new().filename(path).read_only(true);
    let mut conn = SqliteConnection::connect_with(&options).await?;
    let applied = applied(&mut conn).await?.unwrap_or_default();
    conn.close().await?;
    Ok(Some(
        MIGRATOR
            .iter()
            .filter(|m| !applied.iter().any(|(v, _)| *v == m.version))
            .map(|m| Pending {
                version: m.version,
                description: m.description.to_string(),
            })
            .collect(),
    ))
}

async fn vacuum_into(conn: &mut SqliteConnection, target: &Path) -> Result<()> {
    sqlx::query("VACUUM INTO ?")
        .bind(target.to_string_lossy().into_owned())
        .execute(conn)
        .await?;
    Ok(())
}

/// Writes a consistent copy of the database file at `source` to `target` (which must
/// not exist) without running migrations.
pub async fn snapshot_file(source: &Path, target: &Path) -> Result<()> {
    let options = SqliteConnectOptions::new()
        .filename(source)
        .journal_mode(SqliteJournalMode::Wal);
    let mut conn = SqliteConnection::connect_with(&options).await?;
    let result = vacuum_into(&mut conn, target).await;
    conn.close().await?;
    result
}

/// Creates a database with the migrations up to `version` only, as an older sofarr
/// version left it (for tests of upgrades and restores).
pub async fn create_at_version(path: &Path, version: i64) -> Result<()> {
    let migrator = Migrator {
        migrations: Cow::Owned(
            MIGRATOR
                .iter()
                .filter(|m| m.version <= version)
                .cloned()
                .collect(),
        ),
        ignore_missing: false,
        locking: true,
        no_tx: false,
    };
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true);
    let mut conn = SqliteConnection::connect_with(&options).await?;
    let result = migrator.run(&mut conn).await;
    conn.close().await?;
    Ok(result?)
}

impl Db {
    /// Writes a consistent copy of the live database to `target` (which must not exist).
    pub async fn backup_into(&self, target: &Path) -> Result<()> {
        let mut conn = self.pool().acquire().await?;
        vacuum_into(&mut conn, target).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("sofarr-db-files-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    #[tokio::test]
    async fn copies_are_valid_sofarr_databases() {
        let dir = dir("copies");
        let path = dir.join("sofarr.db");
        let db = Db::open(&path).await.expect("open");
        db.put_setting("k", &serde_json::json!(1))
            .await
            .expect("write");
        let live = dir.join("live.db");
        db.backup_into(&live).await.expect("backup");
        drop(db);
        let file = dir.join("file.db");
        snapshot_file(&path, &file).await.expect("snapshot");
        for copy in [&live, &file] {
            assert_eq!(
                inspect(copy).await,
                Inspection {
                    sofarr: true,
                    valid: true
                }
            );
            let reopened = Db::open(copy).await.expect("reopen");
            assert_eq!(
                reopened.setting("k").await.expect("read"),
                Some(serde_json::json!(1))
            );
        }
        // The target must not exist.
        assert!(snapshot_file(&path, &file).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn tells_foreign_newer_and_broken_files_apart() {
        let dir = dir("inspect");
        let text = dir.join("notes.db");
        std::fs::write(&text, b"no database").expect("text");
        assert_eq!(inspect(&text).await, FOREIGN);
        assert_eq!(inspect(&dir.join("missing.db")).await, FOREIGN);
        // Another SQLite database.
        let other = dir.join("other.db");
        {
            let options = SqliteConnectOptions::new()
                .filename(&other)
                .create_if_missing(true);
            let mut conn = SqliteConnection::connect_with(&options)
                .await
                .expect("other");
            sqlx::query("CREATE TABLE t (x)")
                .execute(&mut conn)
                .await
                .expect("table");
            conn.close().await.expect("close");
        }
        assert_eq!(inspect(&other).await, FOREIGN);
        // A database of a newer sofarr version.
        let newer = dir.join("newer.db");
        create_at_version(&newer, i64::MAX).await.expect("current");
        {
            let options = SqliteConnectOptions::new().filename(&newer);
            let mut conn = SqliteConnection::connect_with(&options)
                .await
                .expect("newer");
            sqlx::query(
                "INSERT INTO _sqlx_migrations (version, description, success, checksum,
                    execution_time) VALUES (9999, 'future', 1, x'00', 0)",
            )
            .execute(&mut conn)
            .await
            .expect("future");
            conn.close().await.expect("close");
        }
        assert_eq!(
            inspect(&newer).await,
            Inspection {
                sofarr: true,
                valid: false
            }
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn reports_pending_migrations() {
        let dir = dir("pending");
        let path = dir.join("sofarr.db");
        assert_eq!(pending_migrations(&path).await.expect("none"), None);
        create_at_version(&path, 3).await.expect("old");
        let pending = pending_migrations(&path).await.expect("old").expect("file");
        assert_eq!(pending.first().map(|p| p.version), Some(4));
        assert!(
            inspect(&path).await.valid,
            "an older database is a valid backup"
        );
        drop(Db::open(&path).await.expect("migrate"));
        assert_eq!(
            pending_migrations(&path).await.expect("current"),
            Some(Vec::new())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
