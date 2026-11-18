//! Re-encoding (spec reencode-file): the attempts to re-encode a library file, and the
//! update of a file record after its video was replaced on disk.

use jiff::Timestamp;
use sqlx::{Row, sqlite::SqliteRow};

use crate::{Db, FileRef, Result, ts};

/// How an attempt ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReencodeOutcome {
    Replaced,
    NoGain,
    Failed,
    Refused,
}

impl ReencodeOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Replaced => "replaced",
            Self::NoGain => "no_gain",
            Self::Failed => "failed",
            Self::Refused => "refused",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "replaced" => Self::Replaced,
            "no_gain" => Self::NoGain,
            "failed" => Self::Failed,
            "refused" => Self::Refused,
            _ => return None,
        })
    }
}

/// An attempt as it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewReencode {
    pub file: FileRef,
    pub title: String,
    /// Absolute path of the original.
    pub path: String,
    /// `server` or `cli`.
    pub runner: String,
    pub size_before: u64,
}

/// A recorded attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReencodeAttempt {
    pub id: i64,
    /// `None` once the movie or episode file is gone.
    pub file: Option<FileRef>,
    pub title: String,
    pub path: String,
    pub runner: String,
    pub started_at: Timestamp,
    pub finished_at: Option<Timestamp>,
    /// `None` while it runs.
    pub outcome: Option<ReencodeOutcome>,
    pub reason: Option<String>,
    pub size_before: u64,
    pub size_after: Option<u64>,
}

fn ids(file: FileRef) -> (Option<i64>, Option<i64>) {
    match file {
        FileRef::Movie(id) => (Some(id), None),
        FileRef::Episode(id) => (None, Some(id)),
    }
}

fn size(raw: i64) -> u64 {
    u64::try_from(raw).unwrap_or(0)
}

fn attempt_from_row(row: &SqliteRow) -> ReencodeAttempt {
    let movie: Option<i64> = row.get("movie_id");
    let episode: Option<i64> = row.get("episode_file_id");
    let file = movie
        .map(FileRef::Movie)
        .or_else(|| episode.map(FileRef::Episode));
    let finished: Option<String> = row.get("finished_at");
    let outcome: Option<String> = row.get("outcome");
    let after: Option<i64> = row.get("size_after");
    ReencodeAttempt {
        id: row.get("id"),
        file,
        title: row.get("title"),
        path: row.get("path"),
        runner: row.get("runner"),
        started_at: ts(row.get::<&str, _>("started_at")),
        finished_at: finished.as_deref().map(ts),
        outcome: outcome.as_deref().and_then(ReencodeOutcome::parse),
        reason: row.get("reason"),
        size_before: size(row.get("size_before")),
        size_after: after.map(size),
    }
}

impl Db {
    /// Records the start of an attempt; returns its id.
    pub async fn start_reencode(&self, new: &NewReencode, now: Timestamp) -> Result<i64> {
        let (movie_id, episode_file_id) = ids(new.file);
        let id = sqlx::query_scalar(
            "INSERT INTO reencode_attempts (movie_id, episode_file_id, title, path, runner,
                started_at, size_before)
             VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(movie_id)
        .bind(episode_file_id)
        .bind(&new.title)
        .bind(&new.path)
        .bind(&new.runner)
        .bind(now.to_string())
        .bind(i64::try_from(new.size_before).unwrap_or(i64::MAX))
        .fetch_one(self.pool())
        .await?;
        Ok(id)
    }

    /// Records how an attempt ended.
    pub async fn finish_reencode(
        &self,
        id: i64,
        outcome: ReencodeOutcome,
        reason: Option<&str>,
        size_after: Option<u64>,
        now: Timestamp,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE reencode_attempts SET outcome = ?, reason = ?, size_after = ?, finished_at = ?
             WHERE id = ?",
        )
        .bind(outcome.as_str())
        .bind(reason)
        .bind(size_after.map(|s| i64::try_from(s).unwrap_or(i64::MAX)))
        .bind(now.to_string())
        .bind(id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// The latest attempt, by start.
    pub async fn last_reencode(&self) -> Result<Option<ReencodeAttempt>> {
        let row = sqlx::query(
            "SELECT * FROM reencode_attempts ORDER BY started_at DESC, id DESC LIMIT 1",
        )
        .fetch_optional(self.pool())
        .await?;
        Ok(row.as_ref().map(attempt_from_row))
    }

    /// The attempts that never ended (no outcome yet), oldest first.
    pub async fn open_reencodes(&self) -> Result<Vec<ReencodeAttempt>> {
        let rows = sqlx::query(
            "SELECT * FROM reencode_attempts WHERE outcome IS NULL ORDER BY started_at, id",
        )
        .fetch_all(self.pool())
        .await?;
        Ok(rows.iter().map(attempt_from_row).collect())
    }

    /// Every attempt that still belongs to a library file, newest first.
    pub async fn all_reencodes(&self) -> Result<Vec<ReencodeAttempt>> {
        let rows = sqlx::query(
            "SELECT * FROM reencode_attempts
             WHERE movie_id IS NOT NULL OR episode_file_id IS NOT NULL
             ORDER BY started_at DESC, id DESC",
        )
        .fetch_all(self.pool())
        .await?;
        Ok(rows.iter().map(attempt_from_row).collect())
    }

    /// The latest `limit` attempts, newest first, also of files that are gone.
    pub async fn recent_reencodes(&self, limit: u32) -> Result<Vec<ReencodeAttempt>> {
        let rows = sqlx::query(
            "SELECT * FROM reencode_attempts ORDER BY started_at DESC, id DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.iter().map(attempt_from_row).collect())
    }

    /// How many attempts ended `replaced`, and the bytes they saved in total.
    pub async fn reencode_savings(&self) -> Result<(u64, u64)> {
        let (count, saved): (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), COALESCE(SUM(size_before - COALESCE(size_after, size_before)), 0)
             FROM reencode_attempts WHERE outcome = 'replaced'",
        )
        .fetch_one(self.pool())
        .await?;
        Ok((size(count), size(saved)))
    }

    /// Keeps a library file out of the background job (spec ui-reencode).
    pub async fn exclude_from_reencode(&self, file: FileRef, now: Timestamp) -> Result<()> {
        let (movie_id, episode_file_id) = ids(file);
        sqlx::query(
            "INSERT INTO reencode_exclusions (movie_id, episode_file_id, excluded_at)
             VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
        )
        .bind(movie_id)
        .bind(episode_file_id)
        .bind(now.to_string())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Takes a library file back into the background job.
    pub async fn include_in_reencode(&self, file: FileRef) -> Result<()> {
        let (movie_id, episode_file_id) = ids(file);
        sqlx::query("DELETE FROM reencode_exclusions WHERE movie_id IS ? AND episode_file_id IS ?")
            .bind(movie_id)
            .bind(episode_file_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// The library files kept out of the background job.
    pub async fn reencode_exclusions(&self) -> Result<Vec<FileRef>> {
        let rows: Vec<(Option<i64>, Option<i64>)> =
            sqlx::query_as("SELECT movie_id, episode_file_id FROM reencode_exclusions")
                .fetch_all(self.pool())
                .await?;
        Ok(rows
            .into_iter()
            .filter_map(|(movie, episode)| {
                movie.map(FileRef::Movie).or(episode.map(FileRef::Episode))
            })
            .collect())
    }

    /// The attempts on one library file, newest first.
    pub async fn reencodes_of(&self, file: FileRef) -> Result<Vec<ReencodeAttempt>> {
        let (movie_id, episode_file_id) = ids(file);
        let rows = sqlx::query(
            "SELECT * FROM reencode_attempts WHERE movie_id IS ? AND episode_file_id IS ?
             ORDER BY started_at DESC, id DESC",
        )
        .bind(movie_id)
        .bind(episode_file_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.iter().map(attempt_from_row).collect())
    }

    /// The video of a file was replaced on disk: its new path (relative to the movie or
    /// series folder, `/`-separated) and size. Quality, custom format score, scene name
    /// and episode links stay.
    pub async fn replace_file_on_disk(
        &self,
        file: FileRef,
        relative_path: &str,
        size: u64,
    ) -> Result<()> {
        let size = i64::try_from(size).unwrap_or(i64::MAX);
        let query = match file {
            FileRef::Movie(id) => {
                sqlx::query("UPDATE movie_files SET relative_path = ?, size = ? WHERE movie_id = ?")
                    .bind(relative_path)
                    .bind(size)
                    .bind(id)
            }
            FileRef::Episode(id) => {
                sqlx::query("UPDATE episode_files SET relative_path = ?, size = ? WHERE id = ?")
                    .bind(relative_path)
                    .bind(size)
                    .bind(id)
            }
        };
        query.execute(self.pool()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MovieFile, NewEpisode, NewMovie, NewSeries};

    fn now() -> Timestamp {
        "2026-11-17T08:00:00Z".parse().expect("ts")
    }

    fn file(name: &str) -> MovieFile {
        MovieFile {
            relative_path: name.to_owned(),
            size: 1000,
            quality: "Remux-2160p".to_owned(),
            revision_version: 2,
            revision_real: 0,
            release_group: Some("GRP".to_owned()),
            scene_name: Some("Film.2019.2160p.Remux-GRP".to_owned()),
            custom_format_score: 150,
        }
    }

    async fn movie(db: &Db) -> i64 {
        let id = db
            .upsert_movie(
                &NewMovie {
                    tmdb_id: 1,
                    imdb_id: None,
                    title: "Film".to_owned(),
                    original_title: None,
                    title_de: None,
                    year: Some(2019),
                    runtime_minutes: 100,
                    original_language: None,
                    monitored: true,
                    quality_profile_id: 1,
                    path: "/filme/Film".to_owned(),
                    radarr_id: None,
                    alternative_titles: vec![],
                    availability: crate::Availability::default(),
                    tags: vec![],
                },
                now(),
            )
            .await
            .expect("movie");
        db.set_file(id, &file("Film.mp4"), now())
            .await
            .expect("file");
        id
    }

    async fn double_episode(db: &Db) -> (i64, Vec<i64>) {
        let series = db
            .upsert_series(
                &NewSeries {
                    tvdb_id: 1,
                    imdb_id: None,
                    title: "Insel".to_owned(),
                    title_de: None,
                    year: None,
                    runtime_minutes: 45,
                    original_language: None,
                    series_type: "standard".to_owned(),
                    monitored: true,
                    season_folder: true,
                    quality_profile_id: 1,
                    path: "/serien/Insel".to_owned(),
                    sonarr_id: None,
                    alternative_titles: vec![],
                },
                now(),
            )
            .await
            .expect("series");
        let mut episodes = Vec::new();
        for number in [2, 3] {
            let episode = NewEpisode {
                tvdb_id: None,
                season: 1,
                number,
                absolute: None,
                title: None,
                title_de: None,
                air_date: None,
                monitored: true,
            };
            episodes.push(db.upsert_episode(series, &episode).await.expect("episode"));
        }
        let file_id = db
            .set_episode_file(series, &episodes, &file("Season 01/S01E02.mp4"), now())
            .await
            .expect("episode file");
        (file_id, episodes)
    }

    #[tokio::test]
    async fn a_replaced_video_changes_only_path_and_size() {
        // Spec reencode-file AC-06, AC-14.
        let db = Db::in_memory().await.expect("db");
        let id = movie(&db).await;
        db.replace_file_on_disk(FileRef::Movie(id), "Film.mkv", 400)
            .await
            .expect("replace");
        let stored = db.movie(id).await.expect("read").expect("movie");
        let expected = MovieFile {
            relative_path: "Film.mkv".to_owned(),
            size: 400,
            ..file("Film.mp4")
        };
        assert_eq!(stored.file, Some(expected.clone()));

        let (file_id, episodes) = double_episode(&db).await;
        db.replace_file_on_disk(FileRef::Episode(file_id), "Season 01/S01E02.mkv", 300)
            .await
            .expect("replace");
        let all = db.episodes(None).await.expect("episodes");
        assert_eq!(all.len(), 2);
        for episode in &all {
            assert!(episodes.contains(&episode.id));
            assert_eq!(episode.file_id, Some(file_id), "the episodes stay linked");
            let stored = episode.file.as_ref().expect("file");
            assert_eq!(stored.relative_path, "Season 01/S01E02.mkv");
            assert_eq!(stored.size, 300);
            assert_eq!(stored.quality, "Remux-2160p");
            assert_eq!(stored.custom_format_score, 150);
        }
    }

    #[tokio::test]
    async fn exclusions_follow_their_file() {
        // Spec ui-reencode AC-05.
        let db = Db::in_memory().await.expect("db");
        let id = movie(&db).await;
        let (file_id, _) = double_episode(&db).await;
        assert!(db.reencode_exclusions().await.expect("read").is_empty());
        db.exclude_from_reencode(FileRef::Movie(id), now())
            .await
            .expect("exclude");
        db.exclude_from_reencode(FileRef::Movie(id), now())
            .await
            .expect("twice is fine");
        db.exclude_from_reencode(FileRef::Episode(file_id), now())
            .await
            .expect("exclude");
        let mut excluded = db.reencode_exclusions().await.expect("read");
        excluded.sort_by_key(|f| format!("{f:?}"));
        assert_eq!(excluded, [FileRef::Episode(file_id), FileRef::Movie(id)]);
        // A movie's upgrade keeps the file record and the exclusion.
        db.set_file(id, &file("Film.Upgrade.mkv"), now())
            .await
            .expect("upgrade");
        assert!(
            db.reencode_exclusions()
                .await
                .expect("read")
                .contains(&FileRef::Movie(id))
        );
        db.include_in_reencode(FileRef::Episode(file_id))
            .await
            .expect("include");
        assert_eq!(
            db.reencode_exclusions().await.expect("read"),
            [FileRef::Movie(id)]
        );
        // A removed file takes its exclusion with it.
        db.remove_file(id).await.expect("remove");
        assert!(db.reencode_exclusions().await.expect("read").is_empty());
        // A new episode file is a new record: the old one and its exclusion go.
        let (file_id, episodes) = double_episode(&db).await;
        db.exclude_from_reencode(FileRef::Episode(file_id), now())
            .await
            .expect("exclude");
        let series = db.episodes(None).await.expect("episodes")[0].series_id;
        let new_id = db
            .set_episode_file(
                series,
                &episodes,
                &file("Season 01/S01E02.Upgrade.mkv"),
                now(),
            )
            .await
            .expect("upgrade");
        assert_ne!(new_id, file_id);
        assert!(db.reencode_exclusions().await.expect("read").is_empty());
    }

    #[tokio::test]
    async fn recent_attempts_are_limited_and_keep_files_that_are_gone() {
        let db = Db::in_memory().await.expect("db");
        let id = movie(&db).await;
        for n in 0..5 {
            let new = NewReencode {
                file: FileRef::Movie(id),
                title: format!("Film {n}"),
                path: "/filme/Film/Film.mp4".to_owned(),
                runner: "server".to_owned(),
                size_before: 1000,
            };
            db.start_reencode(&new, now() + jiff::SignedDuration::from_secs(n))
                .await
                .expect("start");
        }
        let ids: Vec<i64> = db
            .recent_reencodes(5)
            .await
            .expect("recent")
            .iter()
            .map(|a| a.id)
            .collect();
        db.finish_reencode(ids[0], ReencodeOutcome::Replaced, None, Some(400), now())
            .await
            .expect("finish");
        db.finish_reencode(ids[1], ReencodeOutcome::Replaced, None, Some(100), now())
            .await
            .expect("finish");
        db.finish_reencode(ids[2], ReencodeOutcome::NoGain, None, Some(900), now())
            .await
            .expect("finish");
        assert_eq!(db.reencode_savings().await.expect("saved"), (2, 1500));
        db.delete_movie(id).await.expect("remove");
        assert_eq!(
            db.reencode_savings().await.expect("saved"),
            (2, 1500),
            "stays with the file gone"
        );
        let recent = db.recent_reencodes(3).await.expect("recent");
        assert_eq!(
            recent.iter().map(|a| a.title.as_str()).collect::<Vec<_>>(),
            ["Film 4", "Film 3", "Film 2"]
        );
        assert!(recent.iter().all(|a| a.file.is_none()));
    }

    #[tokio::test]
    async fn attempts_are_recorded_and_outlive_their_file() {
        // Spec reencode-file AC-15.
        let db = Db::in_memory().await.expect("db");
        let id = movie(&db).await;
        assert_eq!(db.last_reencode().await.expect("last"), None);
        let new = NewReencode {
            file: FileRef::Movie(id),
            title: "Film (2019)".to_owned(),
            path: "/filme/Film/Film.mp4".to_owned(),
            runner: "cli".to_owned(),
            size_before: 1000,
        };
        let first = db.start_reencode(&new, now()).await.expect("start");
        let open = db.open_reencodes().await.expect("open");
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, first);
        assert_eq!(open[0].outcome, None);
        assert_eq!(open[0].finished_at, None);
        assert_eq!(open[0].file, Some(FileRef::Movie(id)));
        assert_eq!(open[0].started_at, now());
        let later = now() + jiff::SignedDuration::from_hours(3);
        db.finish_reencode(
            first,
            ReencodeOutcome::NoGain,
            Some("zu groß"),
            Some(900),
            later,
        )
        .await
        .expect("finish");
        assert!(db.open_reencodes().await.expect("open").is_empty());

        let second_start = later + jiff::SignedDuration::from_secs(1);
        let second = db
            .start_reencode(
                &NewReencode {
                    runner: "server".to_owned(),
                    ..new.clone()
                },
                second_start,
            )
            .await
            .expect("start");
        db.finish_reencode(
            second,
            ReencodeOutcome::Replaced,
            None,
            Some(400),
            second_start,
        )
        .await
        .expect("finish");
        let last = db.last_reencode().await.expect("last").expect("an attempt");
        assert_eq!(last.id, second);
        assert_eq!(last.runner, "server");
        assert_eq!(last.outcome, Some(ReencodeOutcome::Replaced));
        assert_eq!((last.size_before, last.size_after), (1000, Some(400)));
        assert_eq!(last.finished_at, Some(second_start));

        let both = db.reencodes_of(FileRef::Movie(id)).await.expect("of file");
        assert_eq!(
            db.all_reencodes()
                .await
                .expect("all")
                .iter()
                .map(|a| a.id)
                .collect::<Vec<_>>(),
            [second, first]
        );
        assert_eq!(
            both.iter().map(|a| a.id).collect::<Vec<_>>(),
            [second, first]
        );
        assert_eq!(both[1].reason.as_deref(), Some("zu groß"));
        assert_eq!(both[1].outcome, Some(ReencodeOutcome::NoGain));

        // The records stay when the movie goes; only the link is cleared.
        db.delete_movie(id).await.expect("remove");
        let last = db
            .last_reencode()
            .await
            .expect("last")
            .expect("still there");
        assert_eq!(last.file, None);
        assert_eq!(last.title, "Film (2019)");
        assert_eq!(last.path, "/filme/Film/Film.mp4");

        // Episode files too.
        let (file_id, _) = double_episode(&db).await;
        let episode = NewReencode {
            file: FileRef::Episode(file_id),
            title: "Insel – S01E02-E03".to_owned(),
            path: "/serien/Insel/Season 01/S01E02.mp4".to_owned(),
            ..new
        };
        db.start_reencode(&episode, later).await.expect("start");
        let open = db.open_reencodes().await.expect("open");
        assert_eq!(open[0].file, Some(FileRef::Episode(file_id)));
        assert_eq!(
            ReencodeOutcome::parse("no_gain"),
            Some(ReencodeOutcome::NoGain)
        );
        assert_eq!(ReencodeOutcome::Refused.as_str(), "refused");
    }
}
