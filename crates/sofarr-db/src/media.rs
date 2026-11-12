//! Media info of movie and episode files (spec mediainfo), stored as JSON.

use jiff::Timestamp;

use crate::{Db, Result};

/// The file media info belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileRef {
    /// The file of a movie, by movie id.
    Movie(i64),
    /// An episode file, by its id.
    Episode(i64),
}

/// Stored media info with the file's size and modification time when it was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMediaInfo {
    pub info: String,
    pub size: u64,
    pub modified: String,
}

fn ids(file: FileRef) -> (Option<i64>, Option<i64>) {
    match file {
        FileRef::Movie(id) => (Some(id), None),
        FileRef::Episode(id) => (None, Some(id)),
    }
}

impl Db {
    pub async fn put_media_info(
        &self,
        file: FileRef,
        stored: &StoredMediaInfo,
        now: Timestamp,
    ) -> Result<()> {
        let (movie_id, episode_file_id) = ids(file);
        let mut tx = self.pool().begin().await?;
        sqlx::query("DELETE FROM media_info WHERE movie_id IS ? AND episode_file_id IS ?")
            .bind(movie_id)
            .bind(episode_file_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO media_info (movie_id, episode_file_id, info, size, modified, read_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(movie_id)
        .bind(episode_file_id)
        .bind(&stored.info)
        .bind(i64::try_from(stored.size).unwrap_or(i64::MAX))
        .bind(&stored.modified)
        .bind(now.to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn media_info(&self, file: FileRef) -> Result<Option<StoredMediaInfo>> {
        let (movie_id, episode_file_id) = ids(file);
        let row: Option<(String, i64, String)> = sqlx::query_as(
            "SELECT info, size, modified FROM media_info
             WHERE movie_id IS ? AND episode_file_id IS ?",
        )
        .bind(movie_id)
        .bind(episode_file_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|(info, size, modified)| StoredMediaInfo {
            info,
            size: u64::try_from(size).unwrap_or(0),
            modified,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MovieFile, NewEpisode, NewMovie, NewSeries};

    fn file(name: &str) -> MovieFile {
        MovieFile {
            relative_path: name.to_owned(),
            size: 10,
            quality: "WEBDL-1080p".to_owned(),
            revision_version: 1,
            revision_real: 0,
            release_group: None,
            scene_name: None,
            custom_format_score: 0,
        }
    }

    #[tokio::test]
    async fn media_info_follows_its_file() {
        let db = Db::in_memory().await.expect("db");
        let now: Timestamp = "2026-11-12T08:00:00Z".parse().expect("ts");
        let movie = db
            .upsert_movie(
                &NewMovie {
                    tmdb_id: 1,
                    imdb_id: None,
                    title: "Film".to_owned(),
                    original_title: None,
                    title_de: None,
                    year: None,
                    runtime_minutes: 90,
                    original_language: None,
                    monitored: true,
                    quality_profile_id: 1,
                    path: "/m".to_owned(),
                    radarr_id: None,
                    alternative_titles: vec![],
                    availability: crate::Availability::default(),
                    tags: vec![],
                },
                now,
            )
            .await
            .expect("movie");
        db.set_file(movie, &file("a.mkv"), now).await.expect("file");
        let stored = StoredMediaInfo {
            info: "{\"status\":\"complete\"}".to_owned(),
            size: 10,
            modified: "2026-11-12T07:00:00Z".to_owned(),
        };
        db.put_media_info(FileRef::Movie(movie), &stored, now)
            .await
            .expect("put");
        db.put_media_info(FileRef::Movie(movie), &stored, now)
            .await
            .expect("again");
        assert_eq!(
            db.media_info(FileRef::Movie(movie)).await.expect("read"),
            Some(stored.clone())
        );
        // Gone with the file record.
        db.remove_file(movie).await.expect("remove");
        assert_eq!(
            db.media_info(FileRef::Movie(movie)).await.expect("read"),
            None
        );

        let series = db
            .upsert_series(
                &NewSeries {
                    tvdb_id: 1,
                    imdb_id: None,
                    title: "Serie".to_owned(),
                    title_de: None,
                    year: None,
                    runtime_minutes: 45,
                    original_language: None,
                    series_type: "standard".to_owned(),
                    monitored: true,
                    season_folder: true,
                    quality_profile_id: 1,
                    path: "/tv".to_owned(),
                    sonarr_id: None,
                    alternative_titles: vec![],
                },
                now,
            )
            .await
            .expect("series");
        let episode = db
            .upsert_episode(
                series,
                &NewEpisode {
                    tvdb_id: None,
                    season: 1,
                    number: 1,
                    absolute: None,
                    title: None,
                    title_de: None,
                    air_date: None,
                    monitored: true,
                },
            )
            .await
            .expect("episode");
        let file_id = db
            .set_episode_file(series, &[episode], &file("e.mkv"), now)
            .await
            .expect("episode file");
        db.put_media_info(FileRef::Episode(file_id), &stored, now)
            .await
            .expect("put");
        assert!(
            db.media_info(FileRef::Episode(file_id))
                .await
                .expect("read")
                .is_some()
        );
        db.remove_episode_file(file_id).await.expect("remove");
        assert_eq!(
            db.media_info(FileRef::Episode(file_id))
                .await
                .expect("read"),
            None
        );
    }
}
