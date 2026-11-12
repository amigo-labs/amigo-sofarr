//! Persistence: SQLite via sqlx, migrations, repositories.

mod blocklist;
mod files;
mod series;
mod sessions;

use std::{path::Path, str::FromStr};

pub use blocklist::{BlockEntry, MediaRef, NewBlock};
pub use files::{
    Inspection, MIGRATOR, Pending, create_at_version, inspect, pending_migrations, snapshot_file,
};
pub use series::{Episode, NewEpisode, NewSeries, Series, SeriesGrab};
pub use sessions::Session;

use jiff::Timestamp;
use serde_json::Value;
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow},
};

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Datenbank: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("Datenbank-Migration: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("Datenbank: ungültiges JSON unter `{key}`: {message}")]
    Json { key: String, message: String },
}

pub type Result<T> = std::result::Result<T, DbError>;

/// A movie of the library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Movie {
    pub id: i64,
    pub tmdb_id: i64,
    pub imdb_id: Option<String>,
    pub title: String,
    pub original_title: Option<String>,
    pub title_de: Option<String>,
    pub year: Option<u16>,
    pub runtime_minutes: u32,
    /// English language name, e.g. `French`.
    pub original_language: Option<String>,
    pub monitored: bool,
    pub quality_profile_id: i64,
    /// Absolute folder of the movie.
    pub path: String,
    pub radarr_id: Option<i64>,
    pub alternative_titles: Vec<String>,
    pub availability: Availability,
    /// Tag ids (of the `tag` setting).
    pub tags: Vec<i64>,
    pub file: Option<MovieFile>,
}

/// When a movie counts as available (`minimumAvailability` and its release dates).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Availability {
    /// `announced`, `inCinemas` or `released`.
    pub minimum: String,
    /// Release status as Radarr or TMDB reports it (`announced`, `inCinemas`, `released`).
    pub status: Option<String>,
    /// Dates as `YYYY-MM-DD`.
    pub in_cinemas: Option<String>,
    pub digital_release: Option<String>,
    pub physical_release: Option<String>,
}

impl Default for Availability {
    fn default() -> Self {
        Self {
            minimum: "released".to_owned(),
            status: None,
            in_cinemas: None,
            digital_release: None,
            physical_release: None,
        }
    }
}

/// Input for [`Db::upsert_movie`]: everything but the id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMovie {
    pub tmdb_id: i64,
    pub imdb_id: Option<String>,
    pub title: String,
    pub original_title: Option<String>,
    pub title_de: Option<String>,
    pub year: Option<u16>,
    pub runtime_minutes: u32,
    pub original_language: Option<String>,
    pub monitored: bool,
    pub quality_profile_id: i64,
    pub path: String,
    pub radarr_id: Option<i64>,
    pub alternative_titles: Vec<String>,
    pub availability: Availability,
    pub tags: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovieFile {
    /// Relative to the movie folder.
    pub relative_path: String,
    pub size: u64,
    /// Quality name, e.g. `Bluray-1080p`.
    pub quality: String,
    pub revision_version: u8,
    pub revision_real: u8,
    pub release_group: Option<String>,
    pub scene_name: Option<String>,
    pub custom_format_score: i32,
}

/// State of a grab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrabStatus {
    /// Shadow mode: only logged.
    Shadow,
    /// Sent to the download client.
    Queued,
    Downloading,
    Completed,
    Imported,
    Failed,
}

impl GrabStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shadow => "shadow",
            Self::Queued => "queued",
            Self::Downloading => "downloading",
            Self::Completed => "completed",
            Self::Imported => "imported",
            Self::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "shadow" => Self::Shadow,
            "queued" => Self::Queued,
            "downloading" => Self::Downloading,
            "completed" => Self::Completed,
            "imported" => Self::Imported,
            "failed" => Self::Failed,
            _ => return None,
        })
    }

    /// Grabs that still wait for the download client or the import.
    pub fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Downloading | Self::Completed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grab {
    pub id: i64,
    pub movie_id: i64,
    pub title: String,
    pub guid: String,
    pub indexer: String,
    pub download_url: Option<String>,
    pub quality: String,
    pub custom_format_score: i32,
    pub size: u64,
    pub status: GrabStatus,
    pub download_id: Option<String>,
    pub message: Option<String>,
    pub grabbed_at: Timestamp,
    /// First successful download check that did not find the download.
    pub absent_since: Option<Timestamp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewGrab {
    pub movie_id: i64,
    pub title: String,
    pub guid: String,
    pub indexer: String,
    pub download_url: Option<String>,
    pub quality: String,
    pub custom_format_score: i32,
    pub size: u64,
    pub status: GrabStatus,
    pub download_id: Option<String>,
}

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

fn to_i64(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

pub(crate) fn ids_text(ids: &[i64]) -> String {
    ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
}

pub(crate) fn ids_from_text(raw: &str) -> Vec<i64> {
    raw.split(',')
        .filter_map(|i| i.trim().parse().ok())
        .collect()
}

fn ts(raw: &str) -> Timestamp {
    raw.parse().unwrap_or(Timestamp::UNIX_EPOCH)
}

impl Db {
    /// Opens (and creates) the database file and applies all migrations.
    pub async fn open(path: &Path) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);
        Self::connect(options, 4).await
    }

    /// A fresh in-memory database (tests).
    pub async fn in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        // One connection: every in-memory connection is its own database.
        Self::connect(options, 1).await
    }

    async fn connect(options: SqliteConnectOptions, max: u32) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(max)
            .connect_with(options)
            .await?;
        // Closed on failure: the files must be free again (a failed restore is undone).
        if let Err(e) = MIGRATOR.run(&pool).await {
            pool.close().await;
            return Err(e.into());
        }
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // --- settings -------------------------------------------------------------

    pub async fn put_setting(&self, key: &str, value: &Value) -> Result<()> {
        sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT (key) DO UPDATE SET value = excluded.value")
            .bind(key)
            .bind(value.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn setting(&self, key: &str) -> Result<Option<Value>> {
        let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|(raw,)| {
            serde_json::from_str(&raw).map_err(|e| DbError::Json {
                key: key.to_owned(),
                message: e.to_string(),
            })
        })
        .transpose()
    }

    // --- movies ---------------------------------------------------------------

    /// Inserts a movie or updates the one with the same TMDB id; returns its id.
    pub async fn upsert_movie(&self, movie: &NewMovie, now: Timestamp) -> Result<i64> {
        let mut tx = self.pool.begin().await?;
        let (id,): (i64,) = sqlx::query_as(
            "INSERT INTO movies (tmdb_id, imdb_id, title, original_title, title_de, year, runtime,
                original_language, monitored, quality_profile_id, path, radarr_id, added_at,
                minimum_availability, status, in_cinemas, digital_release, physical_release, tags)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (tmdb_id) DO UPDATE SET
                imdb_id = excluded.imdb_id, title = excluded.title,
                original_title = excluded.original_title, title_de = excluded.title_de,
                year = excluded.year, runtime = excluded.runtime,
                original_language = excluded.original_language, monitored = excluded.monitored,
                quality_profile_id = excluded.quality_profile_id, path = excluded.path,
                radarr_id = COALESCE(excluded.radarr_id, movies.radarr_id),
                minimum_availability = excluded.minimum_availability,
                status = excluded.status, in_cinemas = excluded.in_cinemas,
                digital_release = excluded.digital_release,
                physical_release = excluded.physical_release, tags = excluded.tags
             RETURNING id",
        )
        .bind(movie.tmdb_id)
        .bind(&movie.imdb_id)
        .bind(&movie.title)
        .bind(&movie.original_title)
        .bind(&movie.title_de)
        .bind(movie.year)
        .bind(movie.runtime_minutes)
        .bind(&movie.original_language)
        .bind(movie.monitored)
        .bind(movie.quality_profile_id)
        .bind(&movie.path)
        .bind(movie.radarr_id)
        .bind(now.to_string())
        .bind(&movie.availability.minimum)
        .bind(&movie.availability.status)
        .bind(&movie.availability.in_cinemas)
        .bind(&movie.availability.digital_release)
        .bind(&movie.availability.physical_release)
        .bind(ids_text(&movie.tags))
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM movie_titles WHERE movie_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for title in &movie.alternative_titles {
            sqlx::query("INSERT OR IGNORE INTO movie_titles (movie_id, title) VALUES (?, ?)")
                .bind(id)
                .bind(title)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    pub async fn set_monitored(&self, movie_id: i64, monitored: bool) -> Result<()> {
        sqlx::query("UPDATE movies SET monitored = ? WHERE id = ?")
            .bind(monitored)
            .bind(movie_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_file(&self, movie_id: i64, file: &MovieFile, now: Timestamp) -> Result<()> {
        sqlx::query(
            "INSERT INTO movie_files (movie_id, relative_path, size, quality, revision_version,
                revision_real, release_group, scene_name, custom_format_score, added_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (movie_id) DO UPDATE SET
                relative_path = excluded.relative_path, size = excluded.size,
                quality = excluded.quality, revision_version = excluded.revision_version,
                revision_real = excluded.revision_real, release_group = excluded.release_group,
                scene_name = excluded.scene_name,
                custom_format_score = excluded.custom_format_score, added_at = excluded.added_at",
        )
        .bind(movie_id)
        .bind(&file.relative_path)
        .bind(to_i64(file.size))
        .bind(&file.quality)
        .bind(file.revision_version)
        .bind(file.revision_real)
        .bind(&file.release_group)
        .bind(&file.scene_name)
        .bind(file.custom_format_score)
        .bind(now.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_file(&self, movie_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM movie_files WHERE movie_id = ?")
            .bind(movie_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn movies(&self) -> Result<Vec<Movie>> {
        let rows = sqlx::query(MOVIE_SELECT).fetch_all(&self.pool).await?;
        let mut movies: Vec<Movie> = rows.iter().map(movie_from_row).collect();
        let titles: Vec<(i64, String)> =
            sqlx::query_as("SELECT movie_id, title FROM movie_titles ORDER BY movie_id, rowid")
                .fetch_all(&self.pool)
                .await?;
        for (movie_id, title) in titles {
            if let Some(m) = movies.iter_mut().find(|m| m.id == movie_id) {
                m.alternative_titles.push(title);
            }
        }
        Ok(movies)
    }

    pub async fn movie(&self, id: i64) -> Result<Option<Movie>> {
        let row = sqlx::query(&format!("{MOVIE_SELECT} WHERE m.id = ?"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else { return Ok(None) };
        let mut movie = movie_from_row(&row);
        movie.alternative_titles =
            sqlx::query_scalar("SELECT title FROM movie_titles WHERE movie_id = ? ORDER BY rowid")
                .bind(id)
                .fetch_all(&self.pool)
                .await?;
        Ok(Some(movie))
    }

    pub async fn movie_by_tmdb(&self, tmdb_id: i64) -> Result<Option<Movie>> {
        let id: Option<i64> = sqlx::query_scalar("SELECT id FROM movies WHERE tmdb_id = ?")
            .bind(tmdb_id)
            .fetch_optional(&self.pool)
            .await?;
        match id {
            Some(id) => self.movie(id).await,
            None => Ok(None),
        }
    }

    // --- grabs ----------------------------------------------------------------

    pub async fn add_grab(&self, grab: &NewGrab, now: Timestamp) -> Result<i64> {
        let id = sqlx::query_scalar(
            "INSERT INTO grabs (movie_id, title, guid, indexer, download_url, quality,
                custom_format_score, size, status, download_id, grabbed_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(grab.movie_id)
        .bind(&grab.title)
        .bind(&grab.guid)
        .bind(&grab.indexer)
        .bind(&grab.download_url)
        .bind(&grab.quality)
        .bind(grab.custom_format_score)
        .bind(to_i64(grab.size))
        .bind(grab.status.as_str())
        .bind(&grab.download_id)
        .bind(now.to_string())
        .bind(now.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn update_grab(
        &self,
        id: i64,
        status: GrabStatus,
        download_id: Option<&str>,
        message: Option<&str>,
        now: Timestamp,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE grabs SET status = ?, download_id = COALESCE(?, download_id),
                message = ?, updated_at = ? WHERE id = ?",
        )
        .bind(status.as_str())
        .bind(download_id)
        .bind(message)
        .bind(now.to_string())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Grabs, newest first; `status` filters if given.
    pub async fn grabs(&self, status: Option<GrabStatus>, limit: u32) -> Result<Vec<Grab>> {
        let rows = sqlx::query(
            "SELECT id, movie_id, title, guid, indexer, download_url, quality, custom_format_score,
                size, status, download_id, message, grabbed_at, absent_since
             FROM grabs WHERE (? IS NULL OR status = ?) ORDER BY grabbed_at DESC, id DESC LIMIT ?",
        )
        .bind(status.map(GrabStatus::as_str))
        .bind(status.map(GrabStatus::as_str))
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(grab_from_row).collect())
    }

    /// Whether this release was already grabbed (in any state) for the movie.
    pub async fn was_grabbed(&self, movie_id: i64, guid: &str) -> Result<bool> {
        let n: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM grabs WHERE movie_id = ? AND guid = ?")
                .bind(movie_id)
                .bind(guid)
                .fetch_one(&self.pool)
                .await?;
        Ok(n > 0)
    }

    pub async fn grab(&self, id: i64) -> Result<Option<Grab>> {
        let row = sqlx::query("SELECT * FROM grabs WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(grab_from_row))
    }

    /// Records when the download of a grab was first missing (`None`: seen again).
    pub async fn set_grab_absent(&self, id: i64, since: Option<Timestamp>) -> Result<()> {
        sqlx::query("UPDATE grabs SET absent_since = ? WHERE id = ?")
            .bind(since.map(|t| t.to_string()))
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The newest grab of a movie that still waits for download or import.
    pub async fn active_grab(&self, movie_id: i64) -> Result<Option<Grab>> {
        let row = sqlx::query(
            "SELECT id, movie_id, title, guid, indexer, download_url, quality, custom_format_score,
                size, status, download_id, message, grabbed_at, absent_since
             FROM grabs WHERE movie_id = ? AND status IN ('queued', 'downloading', 'completed')
             ORDER BY id DESC LIMIT 1",
        )
        .bind(movie_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.as_ref().map(grab_from_row))
    }
}

const MOVIE_SELECT: &str =
    "SELECT m.id, m.tmdb_id, m.imdb_id, m.title, m.original_title, m.title_de, m.year,
        m.runtime, m.original_language, m.monitored, m.quality_profile_id, m.path, m.radarr_id,
        m.minimum_availability, m.status, m.in_cinemas, m.digital_release, m.physical_release,
        m.tags,
        f.relative_path, f.size, f.quality, f.revision_version, f.revision_real,
        f.release_group, f.scene_name, f.custom_format_score
    FROM movies m LEFT JOIN movie_files f ON f.movie_id = m.id";

fn movie_from_row(row: &SqliteRow) -> Movie {
    let file = row
        .get::<Option<String>, _>("relative_path")
        .map(|relative_path| MovieFile {
            relative_path,
            size: u64::try_from(row.get::<i64, _>("size")).unwrap_or(0),
            quality: row.get("quality"),
            revision_version: row.get("revision_version"),
            revision_real: row.get("revision_real"),
            release_group: row.get("release_group"),
            scene_name: row.get("scene_name"),
            custom_format_score: row.get("custom_format_score"),
        });
    Movie {
        id: row.get("id"),
        tmdb_id: row.get("tmdb_id"),
        imdb_id: row.get("imdb_id"),
        title: row.get("title"),
        original_title: row.get("original_title"),
        title_de: row.get("title_de"),
        year: row.get("year"),
        runtime_minutes: row.get("runtime"),
        original_language: row.get("original_language"),
        monitored: row.get("monitored"),
        quality_profile_id: row.get("quality_profile_id"),
        path: row.get("path"),
        radarr_id: row.get("radarr_id"),
        alternative_titles: Vec::new(),
        availability: Availability {
            minimum: row.get("minimum_availability"),
            status: row.get("status"),
            in_cinemas: row.get("in_cinemas"),
            digital_release: row.get("digital_release"),
            physical_release: row.get("physical_release"),
        },
        tags: ids_from_text(row.get("tags")),
        file,
    }
}

fn grab_from_row(row: &SqliteRow) -> Grab {
    Grab {
        id: row.get("id"),
        movie_id: row.get("movie_id"),
        title: row.get("title"),
        guid: row.get("guid"),
        indexer: row.get("indexer"),
        download_url: row.get("download_url"),
        quality: row.get("quality"),
        custom_format_score: row.get("custom_format_score"),
        size: u64::try_from(row.get::<i64, _>("size")).unwrap_or(0),
        status: GrabStatus::parse(row.get("status")).unwrap_or(GrabStatus::Failed),
        download_id: row.get("download_id"),
        message: row.get("message"),
        grabbed_at: ts(row.get("grabbed_at")),
        absent_since: row
            .get::<Option<String>, _>("absent_since")
            .map(|raw| ts(&raw)),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn now() -> Timestamp {
        "2026-11-01T12:00:00Z".parse().expect("ts")
    }

    fn new_movie(tmdb_id: i64) -> NewMovie {
        NewMovie {
            tmdb_id,
            imdb_id: Some("tt0000042".to_owned()),
            title: "Der kleine Garten".to_owned(),
            original_title: Some("Le Petit Jardin".to_owned()),
            title_de: Some("Der kleine Garten".to_owned()),
            year: Some(2019),
            runtime_minutes: 104,
            original_language: Some("French".to_owned()),
            monitored: true,
            quality_profile_id: 4,
            path: "/data/media/movies/Der kleine Garten (2019)".to_owned(),
            radarr_id: Some(7),
            alternative_titles: vec!["Small Garden".to_owned(), "Kleiner Garten".to_owned()],
            availability: Availability {
                minimum: "inCinemas".to_owned(),
                in_cinemas: Some("2019-05-01".to_owned()),
                ..Availability::default()
            },
            tags: vec![1, 3],
        }
    }

    #[tokio::test]
    async fn settings_round_trip() {
        let db = Db::in_memory().await.expect("db");
        assert_eq!(db.setting("x").await.expect("read"), None);
        db.put_setting("x", &json!({"a": 1})).await.expect("write");
        db.put_setting("x", &json!({"a": 2}))
            .await
            .expect("overwrite");
        assert_eq!(db.setting("x").await.expect("read"), Some(json!({"a": 2})));
    }

    #[tokio::test]
    async fn movies_and_files() {
        let db = Db::in_memory().await.expect("db");
        let id = db
            .upsert_movie(&new_movie(42), now())
            .await
            .expect("insert");
        let mut changed = new_movie(42);
        changed.monitored = false;
        changed.alternative_titles = vec!["Small Garden".to_owned()];
        changed.radarr_id = None;
        assert_eq!(db.upsert_movie(&changed, now()).await.expect("update"), id);
        db.upsert_movie(&new_movie(43), now())
            .await
            .expect("second");

        let movie = db.movie(id).await.expect("read").expect("exists");
        assert!(!movie.monitored);
        assert_eq!(movie.radarr_id, Some(7), "radarr id is kept");
        assert_eq!(movie.alternative_titles, ["Small Garden"]);
        assert_eq!(movie.file, None);
        assert_eq!(movie.tags, [1, 3]);
        assert_eq!(movie.availability.minimum, "inCinemas");
        assert_eq!(movie.availability.in_cinemas.as_deref(), Some("2019-05-01"));

        let file = MovieFile {
            relative_path: "Der kleine Garten (2019) Bluray-1080p.mkv".to_owned(),
            size: 8_000_000_000,
            quality: "Bluray-1080p".to_owned(),
            revision_version: 1,
            revision_real: 0,
            release_group: Some("GRP".to_owned()),
            scene_name: None,
            custom_format_score: 25,
        };
        db.set_file(id, &file, now()).await.expect("file");
        let movies = db.movies().await.expect("list");
        assert_eq!(movies.len(), 2);
        assert_eq!(movies[0].file.as_ref(), Some(&file));
        assert_eq!(movies[1].alternative_titles.len(), 2);
        assert_eq!(
            db.movie_by_tmdb(43)
                .await
                .expect("by tmdb")
                .map(|m| m.tmdb_id),
            Some(43)
        );
        db.remove_file(id).await.expect("remove");
        assert_eq!(db.movie(id).await.expect("read").and_then(|m| m.file), None);
    }

    #[tokio::test]
    async fn grabs_lifecycle() {
        let db = Db::in_memory().await.expect("db");
        let movie = db.upsert_movie(&new_movie(42), now()).await.expect("movie");
        let grab = NewGrab {
            movie_id: movie,
            title: "Der.kleine.Garten.2019.German.1080p.BluRay.x264-GRP".to_owned(),
            guid: "g1".to_owned(),
            indexer: "idx".to_owned(),
            download_url: Some("https://idx.example/g1.nzb".to_owned()),
            quality: "Bluray-1080p".to_owned(),
            custom_format_score: 100,
            size: 9_000_000_000,
            status: GrabStatus::Queued,
            download_id: None,
        };
        let id = db.add_grab(&grab, now()).await.expect("grab");
        assert!(db.was_grabbed(movie, "g1").await.expect("check"));
        assert!(!db.was_grabbed(movie, "g2").await.expect("check"));
        assert_eq!(
            db.active_grab(movie).await.expect("active").map(|g| g.id),
            Some(id)
        );
        let absent: Timestamp = "2026-11-01T13:00:00Z".parse().expect("ts");
        db.set_grab_absent(id, Some(absent)).await.expect("absent");
        let read = db.grab(id).await.expect("read").expect("grab");
        assert_eq!(read.absent_since, Some(absent));
        db.set_grab_absent(id, None).await.expect("seen");
        assert_eq!(
            db.grab(id)
                .await
                .expect("read")
                .and_then(|g| g.absent_since),
            None
        );
        assert_eq!(db.grab(id + 1).await.expect("unknown"), None);
        db.update_grab(id, GrabStatus::Imported, Some("nzb-7"), None, now())
            .await
            .expect("update");
        assert_eq!(db.active_grab(movie).await.expect("active"), None);
        let all = db
            .grabs(Some(GrabStatus::Imported), 10)
            .await
            .expect("list");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].download_id.as_deref(), Some("nzb-7"));
        assert_eq!(all[0].grabbed_at, now());
        assert!(
            db.grabs(Some(GrabStatus::Shadow), 10)
                .await
                .expect("list")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn opens_a_file_database() {
        let dir = std::env::temp_dir().join(format!("sofarr-db-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("sofarr.db");
        {
            let db = Db::open(&path).await.expect("open");
            db.put_setting("k", &json!(1)).await.expect("write");
        }
        let db = Db::open(&path).await.expect("reopen");
        assert_eq!(db.setting("k").await.expect("read"), Some(json!(1)));
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
