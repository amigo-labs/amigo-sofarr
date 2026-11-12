//! Subtitles (spec subtitles): own subtitle files and the needed subtitles of each file
//! with their search schedule.

use jiff::Timestamp;
use sqlx::{Row, sqlite::SqliteRow};

use crate::{Db, FileRef, Result, ts};

/// A normal subtitle or a forced one (only the foreign-language parts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SubtitleKind {
    Normal,
    Forced,
}

impl SubtitleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Forced => "forced",
        }
    }

    pub fn parse(raw: &str) -> Self {
        if raw == "forced" {
            Self::Forced
        } else {
            Self::Normal
        }
    }
}

/// A subtitle file sofarr wrote next to a video.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnSubtitle {
    pub id: i64,
    pub file: FileRef,
    /// ISO 639-1.
    pub language: String,
    pub kind: SubtitleKind,
    pub hearing_impaired: bool,
    /// File name in the video's folder.
    pub file_name: String,
    pub provider_id: String,
    pub release: String,
    pub match_quality: String,
    pub created_at: Timestamp,
}

/// Input for [`Db::add_own_subtitle`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOwnSubtitle {
    pub file: FileRef,
    pub language: String,
    pub kind: SubtitleKind,
    pub hearing_impaired: bool,
    pub file_name: String,
    pub provider_id: String,
    pub release: String,
    pub match_quality: String,
}

/// A needed subtitle with its search schedule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleNeed {
    pub file: FileRef,
    pub language: String,
    pub kind: SubtitleKind,
    pub last_search: Option<Timestamp>,
    /// `None`: due now.
    pub next_search: Option<Timestamp>,
    /// Whether the last search found a candidate; `None` before the first search.
    pub found: Option<bool>,
    /// Shadow mode: the release that would have been downloaded.
    pub would_download: Option<String>,
}

/// The result of one search for a needed subtitle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchOutcome {
    pub at: Timestamp,
    pub next_search: Option<Timestamp>,
    pub found: bool,
    pub would_download: Option<String>,
}

fn ids(file: FileRef) -> (Option<i64>, Option<i64>) {
    match file {
        FileRef::Movie(id) => (Some(id), None),
        FileRef::Episode(id) => (None, Some(id)),
    }
}

fn file_from_row(row: &SqliteRow) -> FileRef {
    match row.get::<Option<i64>, _>("movie_id") {
        Some(id) => FileRef::Movie(id),
        None => FileRef::Episode(row.get("episode_file_id")),
    }
}

fn own_from_row(row: &SqliteRow) -> OwnSubtitle {
    OwnSubtitle {
        id: row.get("id"),
        file: file_from_row(row),
        language: row.get("language"),
        kind: SubtitleKind::parse(row.get("kind")),
        hearing_impaired: row.get("hearing_impaired"),
        file_name: row.get("file_name"),
        provider_id: row.get("provider_id"),
        release: row.get("release"),
        match_quality: row.get("match_quality"),
        created_at: ts(row.get("created_at")),
    }
}

fn need_from_row(row: &SqliteRow) -> SubtitleNeed {
    let time = |name: &str| row.get::<Option<String>, _>(name).map(|raw| ts(&raw));
    SubtitleNeed {
        file: file_from_row(row),
        language: row.get("language"),
        kind: SubtitleKind::parse(row.get("kind")),
        last_search: time("last_search"),
        next_search: time("next_search"),
        found: row.get("found"),
        would_download: row.get("would_download"),
    }
}

const OWN_COLUMNS: &str = "id, movie_id, episode_file_id, language, kind, hearing_impaired,
    file_name, provider_id, release, match_quality, created_at";

const NEED_COLUMNS: &str = "movie_id, episode_file_id, language, kind, last_search, next_search,
    found, would_download";

impl Db {
    /// Records an own subtitle; one recorded before for the same file, language and kind
    /// (whose file is gone) is replaced.
    pub async fn add_own_subtitle(&self, new: &NewOwnSubtitle, now: Timestamp) -> Result<i64> {
        let (movie_id, episode_file_id) = ids(new.file);
        let mut tx = self.pool().begin().await?;
        sqlx::query(
            "DELETE FROM subtitles WHERE movie_id IS ? AND episode_file_id IS ?
                AND language = ? AND kind = ?",
        )
        .bind(movie_id)
        .bind(episode_file_id)
        .bind(&new.language)
        .bind(new.kind.as_str())
        .execute(&mut *tx)
        .await?;
        let id = sqlx::query_scalar(
            "INSERT INTO subtitles (movie_id, episode_file_id, language, kind, hearing_impaired,
                file_name, provider_id, release, match_quality, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(movie_id)
        .bind(episode_file_id)
        .bind(&new.language)
        .bind(new.kind.as_str())
        .bind(new.hearing_impaired)
        .bind(&new.file_name)
        .bind(&new.provider_id)
        .bind(&new.release)
        .bind(&new.match_quality)
        .bind(now.to_string())
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(id)
    }

    /// Own subtitles of a file, or of every file.
    pub async fn own_subtitles(&self, file: Option<FileRef>) -> Result<Vec<OwnSubtitle>> {
        let (movie_id, episode_file_id) = file.map_or((None, None), ids);
        let rows = sqlx::query(&format!(
            "SELECT {OWN_COLUMNS} FROM subtitles
             WHERE ? IS NULL OR movie_id IS ? AND episode_file_id IS ?
             ORDER BY id"
        ))
        .bind(file.is_some().then_some(1))
        .bind(movie_id)
        .bind(episode_file_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.iter().map(own_from_row).collect())
    }

    /// Own subtitles recorded at or after `since`, newest first.
    pub async fn own_subtitles_since(&self, since: Timestamp) -> Result<Vec<OwnSubtitle>> {
        let rows = sqlx::query(&format!(
            "SELECT {OWN_COLUMNS} FROM subtitles WHERE created_at >= ?
             ORDER BY created_at DESC, id DESC"
        ))
        .bind(since.to_string())
        .fetch_all(self.pool())
        .await?;
        Ok(rows.iter().map(own_from_row).collect())
    }

    pub async fn remove_own_subtitle(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM subtitles WHERE id = ?")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn rename_own_subtitle(&self, id: i64, file_name: &str) -> Result<()> {
        sqlx::query("UPDATE subtitles SET file_name = ? WHERE id = ?")
            .bind(file_name)
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Replaces the needed subtitles of a file; needs that stay keep their schedule.
    pub async fn set_subtitle_needs(
        &self,
        file: FileRef,
        needs: &[(String, SubtitleKind)],
    ) -> Result<()> {
        let (movie_id, episode_file_id) = ids(file);
        let mut tx = self.pool().begin().await?;
        let existing: Vec<(String, String)> = sqlx::query_as(
            "SELECT language, kind FROM subtitle_needs WHERE movie_id IS ? AND episode_file_id IS ?",
        )
        .bind(movie_id)
        .bind(episode_file_id)
        .fetch_all(&mut *tx)
        .await?;
        for (language, kind) in &existing {
            if !needs
                .iter()
                .any(|(l, k)| l == language && k.as_str() == kind)
            {
                sqlx::query(
                    "DELETE FROM subtitle_needs WHERE movie_id IS ? AND episode_file_id IS ?
                        AND language = ? AND kind = ?",
                )
                .bind(movie_id)
                .bind(episode_file_id)
                .bind(language)
                .bind(kind)
                .execute(&mut *tx)
                .await?;
            }
        }
        for (language, kind) in needs {
            if existing
                .iter()
                .any(|(l, k)| l == language && k == kind.as_str())
            {
                continue;
            }
            sqlx::query(
                "INSERT INTO subtitle_needs (movie_id, episode_file_id, language, kind)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(movie_id)
            .bind(episode_file_id)
            .bind(language)
            .bind(kind.as_str())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Forgets the needed subtitles of a file and their schedule (a new video).
    pub async fn reset_subtitle_needs(&self, file: FileRef) -> Result<()> {
        let (movie_id, episode_file_id) = ids(file);
        sqlx::query("DELETE FROM subtitle_needs WHERE movie_id IS ? AND episode_file_id IS ?")
            .bind(movie_id)
            .bind(episode_file_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Needed subtitles of a file, or of every file.
    pub async fn subtitle_needs(&self, file: Option<FileRef>) -> Result<Vec<SubtitleNeed>> {
        let (movie_id, episode_file_id) = file.map_or((None, None), ids);
        let rows = sqlx::query(&format!(
            "SELECT {NEED_COLUMNS} FROM subtitle_needs
             WHERE ? IS NULL OR movie_id IS ? AND episode_file_id IS ?
             ORDER BY movie_id, episode_file_id, language, kind = 'forced'"
        ))
        .bind(file.is_some().then_some(1))
        .bind(movie_id)
        .bind(episode_file_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.iter().map(need_from_row).collect())
    }

    pub async fn record_subtitle_search(
        &self,
        file: FileRef,
        language: &str,
        kind: SubtitleKind,
        outcome: &SearchOutcome,
    ) -> Result<()> {
        let (movie_id, episode_file_id) = ids(file);
        sqlx::query(
            "UPDATE subtitle_needs SET last_search = ?, next_search = ?, found = ?,
                would_download = ?
             WHERE movie_id IS ? AND episode_file_id IS ? AND language = ? AND kind = ?",
        )
        .bind(outcome.at.to_string())
        .bind(outcome.next_search.map(|t| t.to_string()))
        .bind(outcome.found)
        .bind(&outcome.would_download)
        .bind(movie_id)
        .bind(episode_file_id)
        .bind(language)
        .bind(kind.as_str())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// When each library file was added (imported, found or taken over).
    pub async fn file_added_times(&self) -> Result<Vec<(FileRef, Timestamp)>> {
        let rows: Vec<(Option<i64>, Option<i64>, String)> = sqlx::query_as(
            "SELECT movie_id, NULL, added_at FROM movie_files
             UNION ALL SELECT NULL, id, added_at FROM episode_files",
        )
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|(movie, episode, added)| {
                let file = match movie {
                    Some(id) => FileRef::Movie(id),
                    None => FileRef::Episode(episode.unwrap_or_default()),
                };
                (file, ts(&added))
            })
            .collect())
    }
}

#[cfg(test)]
mod tests;
