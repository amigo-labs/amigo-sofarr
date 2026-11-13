//! What the library pages need beyond the decision data (spec web-ui-library): posters,
//! overview texts, monitoring in bulk, settings per item, removing items, new paths after
//! renames and moves, and the activity of grabs and imports.

use std::collections::HashMap;

use jiff::Timestamp;
use sqlx::{Row, sqlite::SqliteRow};

use crate::{BlockEntry, Db, Grab, GrabStatus, MediaRef, Result, SeriesGrab, ids_from_text};

/// Presentation data of a movie or series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presentation {
    pub added_at: Timestamp,
    /// The provider's poster path (TMDB file path, TVDB artwork path).
    pub poster_path: Option<String>,
    pub overview: Option<String>,
}

/// One item whose monitoring is switched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorTarget {
    Movie(i64),
    /// The series and all its episodes.
    Series(i64),
    /// The episodes of one season.
    Season {
        series_id: i64,
        season: u16,
    },
    Episode(i64),
}

/// A grab of a movie or series with the time of its last change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    pub media: MediaRef,
    /// The grab's id in the table of its media kind.
    pub grab_id: i64,
    /// Title of the movie or series.
    pub media_title: String,
    pub title: String,
    pub quality: String,
    pub size: u64,
    pub status: GrabStatus,
    pub message: Option<String>,
    pub download_id: Option<String>,
    /// Series grabs: the episodes covered.
    pub episode_ids: Vec<i64>,
    pub grabbed_at: Timestamp,
    pub updated_at: Timestamp,
}

const ACTIVITY: &str = "SELECT * FROM (
        SELECT g.id, g.movie_id, NULL AS series_id, '' AS episode_ids, g.title, g.quality, g.size,
            g.status, g.message, g.download_id, g.grabbed_at, g.updated_at,
            COALESCE(m.title, '') AS media_title
        FROM grabs g LEFT JOIN movies m ON m.id = g.movie_id
        UNION ALL
        SELECT g.id, NULL, g.series_id, g.episode_ids, g.title, g.quality, g.size, g.status,
            g.message, g.download_id, g.grabbed_at, g.updated_at,
            COALESCE(s.title, '') AS media_title
        FROM series_grabs g LEFT JOIN series s ON s.id = g.series_id)";

fn ts(raw: &str) -> Timestamp {
    raw.parse().unwrap_or(Timestamp::UNIX_EPOCH)
}

fn activity_from_row(row: &SqliteRow) -> Activity {
    let media = match row.get::<Option<i64>, _>("movie_id") {
        Some(id) => MediaRef::Movie(id),
        None => MediaRef::Series(row.get("series_id")),
    };
    Activity {
        media,
        grab_id: row.get("id"),
        media_title: row.get("media_title"),
        title: row.get("title"),
        quality: row.get("quality"),
        size: u64::try_from(row.get::<i64, _>("size")).unwrap_or(0),
        status: GrabStatus::parse(row.get("status")).unwrap_or(GrabStatus::Failed),
        message: row.get("message"),
        download_id: row.get("download_id"),
        episode_ids: ids_from_text(row.get("episode_ids")),
        grabbed_at: ts(row.get("grabbed_at")),
        updated_at: ts(row.get("updated_at")),
    }
}

async fn presentation(db: &Db, table: &str) -> Result<HashMap<i64, Presentation>> {
    let rows: Vec<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(&format!(
        "SELECT id, added_at, poster_path, overview FROM {table}"
    ))
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, added_at, poster_path, overview)| {
            (
                id,
                Presentation {
                    added_at: ts(&added_at),
                    poster_path,
                    overview,
                },
            )
        })
        .collect())
}

impl Db {
    /// A number that changes with every change of movies, series, episodes, their files
    /// and titles, or the settings.
    pub async fn library_version(&self) -> Result<i64> {
        Ok(
            sqlx::query_scalar("SELECT version FROM library_version WHERE id = 1")
                .fetch_one(self.pool())
                .await?,
        )
    }

    /// Added time, poster path and overview of every movie, by id.
    pub async fn movie_presentation(&self) -> Result<HashMap<i64, Presentation>> {
        presentation(self, "movies").await
    }

    /// Added time, poster path and overview of every series, by id.
    pub async fn series_presentation(&self) -> Result<HashMap<i64, Presentation>> {
        presentation(self, "series").await
    }

    /// The poster path of one movie or series.
    pub async fn poster_path(&self, media: MediaRef) -> Result<Option<String>> {
        let (table, id) = match media {
            MediaRef::Movie(id) => ("movies", id),
            MediaRef::Series(id) => ("series", id),
        };
        let path: Option<Option<String>> =
            sqlx::query_scalar(&format!("SELECT poster_path FROM {table} WHERE id = ?"))
                .bind(id)
                .fetch_optional(self.pool())
                .await?;
        Ok(path.flatten())
    }

    /// Stores what the provider says; `None` keeps the stored value.
    pub async fn set_movie_presentation(
        &self,
        id: i64,
        poster_path: Option<&str>,
        overview: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE movies SET poster_path = COALESCE(?, poster_path),
                overview = COALESCE(?, overview) WHERE id = ?",
        )
        .bind(poster_path)
        .bind(overview)
        .bind(id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Like [`Db::set_movie_presentation`] for a series.
    pub async fn set_series_presentation(
        &self,
        id: i64,
        poster_path: Option<&str>,
        overview: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE series SET poster_path = COALESCE(?, poster_path),
                overview = COALESCE(?, overview) WHERE id = ?",
        )
        .bind(poster_path)
        .bind(overview)
        .bind(id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Switches monitoring for all targets in one transaction. Returns the first target
    /// that does not exist; then nothing is changed. Switching a series on monitors its
    /// episodes except the specials (season 0), switching it off all of them.
    pub async fn set_monitored_all(
        &self,
        targets: &[MonitorTarget],
        monitored: bool,
    ) -> Result<Option<MonitorTarget>> {
        let mut tx = self.pool().begin().await?;
        for target in targets {
            let changed = match *target {
                MonitorTarget::Movie(id) => {
                    sqlx::query("UPDATE movies SET monitored = ? WHERE id = ?")
                        .bind(monitored)
                        .bind(id)
                        .execute(&mut *tx)
                        .await?
                        .rows_affected()
                }
                MonitorTarget::Series(id) => {
                    let n = sqlx::query("UPDATE series SET monitored = ? WHERE id = ?")
                        .bind(monitored)
                        .bind(id)
                        .execute(&mut *tx)
                        .await?
                        .rows_affected();
                    sqlx::query(
                        "UPDATE episodes SET monitored = ? WHERE series_id = ? AND (? = 0 OR season > 0)",
                    )
                    .bind(monitored)
                    .bind(id)
                    .bind(monitored)
                    .execute(&mut *tx)
                    .await?;
                    n
                }
                MonitorTarget::Season { series_id, season } => sqlx::query(
                    "UPDATE episodes SET monitored = ? WHERE series_id = ? AND season = ?",
                )
                .bind(monitored)
                .bind(series_id)
                .bind(season)
                .execute(&mut *tx)
                .await?
                .rows_affected(),
                MonitorTarget::Episode(id) => {
                    sqlx::query("UPDATE episodes SET monitored = ? WHERE id = ?")
                        .bind(monitored)
                        .bind(id)
                        .execute(&mut *tx)
                        .await?
                        .rows_affected()
                }
            };
            if changed == 0 {
                return Ok(Some(*target));
            }
        }
        tx.commit().await?;
        Ok(None)
    }

    /// Changes quality profile and/or minimum availability of a movie; false if unknown.
    pub async fn set_movie_settings(
        &self,
        id: i64,
        quality_profile_id: Option<i64>,
        minimum_availability: Option<&str>,
    ) -> Result<bool> {
        let done = sqlx::query(
            "UPDATE movies SET quality_profile_id = COALESCE(?, quality_profile_id),
                minimum_availability = COALESCE(?, minimum_availability) WHERE id = ?",
        )
        .bind(quality_profile_id)
        .bind(minimum_availability)
        .bind(id)
        .execute(self.pool())
        .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Changes the quality profile of a series; false if unknown.
    pub async fn set_series_profile(&self, id: i64, quality_profile_id: i64) -> Result<bool> {
        let done = sqlx::query("UPDATE series SET quality_profile_id = ? WHERE id = ?")
            .bind(quality_profile_id)
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Removes a movie with its file record, grabs, titles and blocklist entries.
    pub async fn delete_movie(&self, id: i64) -> Result<bool> {
        let done = sqlx::query("DELETE FROM movies WHERE id = ?")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Removes a series with its episodes, files, grabs, titles and blocklist entries.
    pub async fn delete_series(&self, id: i64) -> Result<bool> {
        let mut tx = self.pool().begin().await?;
        // Episodes point at their files: they go first.
        sqlx::query("DELETE FROM episodes WHERE series_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let done = sqlx::query("DELETE FROM series WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(done.rows_affected() > 0)
    }

    pub async fn set_movie_path(&self, id: i64, path: &str) -> Result<()> {
        sqlx::query("UPDATE movies SET path = ? WHERE id = ?")
            .bind(path)
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn set_series_path(&self, id: i64, path: &str) -> Result<()> {
        sqlx::query("UPDATE series SET path = ? WHERE id = ?")
            .bind(path)
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// The file of a movie got a new name (relative to the movie folder).
    pub async fn set_movie_file_path(&self, movie_id: i64, relative_path: &str) -> Result<()> {
        sqlx::query("UPDATE movie_files SET relative_path = ? WHERE movie_id = ?")
            .bind(relative_path)
            .bind(movie_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// An episode file got a new name (relative to the series folder).
    pub async fn set_episode_file_path(&self, file_id: i64, relative_path: &str) -> Result<()> {
        sqlx::query("UPDATE episode_files SET relative_path = ? WHERE id = ?")
            .bind(relative_path)
            .bind(file_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// The grabs of a movie, newest first.
    pub async fn movie_grabs(&self, movie_id: i64) -> Result<Vec<Grab>> {
        let rows = sqlx::query("SELECT * FROM grabs WHERE movie_id = ? ORDER BY id DESC")
            .bind(movie_id)
            .fetch_all(self.pool())
            .await?;
        Ok(rows.iter().map(crate::grab_from_row).collect())
    }

    /// The grabs of a series, newest first.
    pub async fn grabs_of_series(&self, series_id: i64) -> Result<Vec<SeriesGrab>> {
        let rows = sqlx::query("SELECT * FROM series_grabs WHERE series_id = ? ORDER BY id DESC")
            .bind(series_id)
            .fetch_all(self.pool())
            .await?;
        Ok(rows.iter().map(crate::series::grab_from_row).collect())
    }

    /// The blocklist entries of a movie or series, newest first.
    pub async fn blocks_of(&self, media: MediaRef) -> Result<Vec<BlockEntry>> {
        Ok(self
            .blocks()
            .await?
            .into_iter()
            .filter(|b| b.media == media)
            .rev()
            .collect())
    }

    /// The newest grabs and imports of movies and series, by their last change.
    pub async fn activity(&self, limit: u32) -> Result<Vec<Activity>> {
        let rows = sqlx::query(&format!(
            "{ACTIVITY} ORDER BY updated_at DESC, grabbed_at DESC LIMIT ?"
        ))
        .bind(limit)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.iter().map(activity_from_row).collect())
    }

    /// Failed grabs and import problems (completed with a message) changed since `since`.
    pub async fn problems(&self, since: Timestamp) -> Result<Vec<Activity>> {
        let rows = sqlx::query(&format!(
            "{ACTIVITY} WHERE updated_at >= ?
                AND (status = 'failed' OR (status = 'completed' AND message IS NOT NULL))
             ORDER BY updated_at DESC"
        ))
        .bind(since.to_string())
        .fetch_all(self.pool())
        .await?;
        Ok(rows.iter().map(activity_from_row).collect())
    }

    /// Grabs that still wait for the download client or the import.
    pub async fn active_activity(&self) -> Result<Vec<Activity>> {
        let rows = sqlx::query(&format!(
            "{ACTIVITY} WHERE status IN ('queued', 'downloading', 'completed')
             ORDER BY grabbed_at"
        ))
        .fetch_all(self.pool())
        .await?;
        Ok(rows.iter().map(activity_from_row).collect())
    }
}

#[cfg(test)]
mod tests;
