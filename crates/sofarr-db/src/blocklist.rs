//! Blocklist: releases sofarr never takes again for a movie or series.

use jiff::Timestamp;
use sqlx::{Row, sqlite::SqliteRow};

use crate::{Db, Result};

/// The movie or series a blocklist entry belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MediaRef {
    Movie(i64),
    Series(i64),
}

/// Input for [`Db::add_block`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewBlock {
    pub media: MediaRef,
    pub title: String,
    /// The title as `sofarr_core::normalize_release_title` writes it.
    pub normalized_title: String,
    pub guid: String,
    pub indexer: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockEntry {
    pub id: i64,
    pub media: MediaRef,
    /// Title of the movie or series.
    pub media_title: String,
    pub title: String,
    pub normalized_title: String,
    pub guid: String,
    pub indexer: String,
    pub reason: String,
    pub blocklisted_at: Timestamp,
}

const SELECT: &str = "SELECT b.id, b.movie_id, b.series_id, b.title, b.normalized_title, b.guid,
        b.indexer, b.reason, b.blocklisted_at, COALESCE(m.title, s.title, '') AS media_title
    FROM blocklist b
    LEFT JOIN movies m ON m.id = b.movie_id
    LEFT JOIN series s ON s.id = b.series_id";

fn entry_from_row(row: &SqliteRow) -> BlockEntry {
    let media = match row.get::<Option<i64>, _>("movie_id") {
        Some(id) => MediaRef::Movie(id),
        None => MediaRef::Series(row.get("series_id")),
    };
    BlockEntry {
        id: row.get("id"),
        media,
        media_title: row.get("media_title"),
        title: row.get("title"),
        normalized_title: row.get("normalized_title"),
        guid: row.get("guid"),
        indexer: row.get("indexer"),
        reason: row.get("reason"),
        blocklisted_at: row
            .get::<String, _>("blocklisted_at")
            .parse()
            .unwrap_or(Timestamp::UNIX_EPOCH),
    }
}

fn ids(media: MediaRef) -> (Option<i64>, Option<i64>) {
    match media {
        MediaRef::Movie(id) => (Some(id), None),
        MediaRef::Series(id) => (None, Some(id)),
    }
}

impl Db {
    /// Puts a release on the blocklist of its movie or series; returns false if the
    /// release (same GUID or same normalized title) is already on it.
    pub async fn add_block(&self, block: &NewBlock, now: Timestamp) -> Result<bool> {
        let (movie_id, series_id) = ids(block.media);
        let mut tx = self.pool().begin().await?;
        let known: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM blocklist
             WHERE movie_id IS ? AND series_id IS ? AND (guid = ? OR normalized_title = ?)",
        )
        .bind(movie_id)
        .bind(series_id)
        .bind(&block.guid)
        .bind(&block.normalized_title)
        .fetch_one(&mut *tx)
        .await?;
        if known > 0 {
            return Ok(false);
        }
        sqlx::query(
            "INSERT INTO blocklist (movie_id, series_id, title, normalized_title, guid, indexer,
                reason, blocklisted_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(movie_id)
        .bind(series_id)
        .bind(&block.title)
        .bind(&block.normalized_title)
        .bind(&block.guid)
        .bind(&block.indexer)
        .bind(&block.reason)
        .bind(now.to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// All blocklist entries (for the decisions).
    pub async fn blocks(&self) -> Result<Vec<BlockEntry>> {
        let rows = sqlx::query(&format!("{SELECT} ORDER BY b.id"))
            .fetch_all(self.pool())
            .await?;
        Ok(rows.iter().map(entry_from_row).collect())
    }

    /// One page of the blocklist, newest first (pages counted from 1), and the total.
    pub async fn block_page(&self, page: u32, page_size: u32) -> Result<(Vec<BlockEntry>, u64)> {
        let offset = i64::from(page.saturating_sub(1)) * i64::from(page_size);
        let rows = sqlx::query(&format!(
            "{SELECT} ORDER BY b.blocklisted_at DESC, b.id DESC LIMIT ? OFFSET ?"
        ))
        .bind(page_size)
        .bind(offset)
        .fetch_all(self.pool())
        .await?;
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blocklist")
            .fetch_one(self.pool())
            .await?;
        Ok((
            rows.iter().map(entry_from_row).collect(),
            u64::try_from(total).unwrap_or(0),
        ))
    }

    /// Removes a blocklist entry; returns false if there is none with this id.
    pub async fn remove_block(&self, id: i64) -> Result<bool> {
        let done = sqlx::query("DELETE FROM blocklist WHERE id = ?")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(done.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NewMovie, NewSeries};

    fn at(minute: u8) -> Timestamp {
        format!("2026-11-01T12:{minute:02}:00Z")
            .parse()
            .expect("ts")
    }

    fn block(media: MediaRef, title: &str, guid: &str) -> NewBlock {
        NewBlock {
            media,
            title: title.to_owned(),
            normalized_title: title.to_lowercase().replace(['.', '_', '-'], " "),
            guid: guid.to_owned(),
            indexer: "idx".to_owned(),
            reason: "FAILURE/PAR".to_owned(),
        }
    }

    async fn library(db: &Db) -> (i64, i64) {
        let movie = db
            .upsert_movie(
                &NewMovie {
                    tmdb_id: 1,
                    imdb_id: None,
                    title: "Film".to_owned(),
                    original_title: None,
                    title_de: None,
                    year: Some(2020),
                    runtime_minutes: 90,
                    original_language: None,
                    monitored: true,
                    quality_profile_id: 1,
                    path: "/m/Film".to_owned(),
                    radarr_id: None,
                    alternative_titles: vec![],
                    availability: crate::Availability::default(),
                    tags: vec![],
                },
                at(0),
            )
            .await
            .expect("movie");
        let series = db
            .upsert_series(
                &NewSeries {
                    tvdb_id: 7,
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
                    path: "/tv/Serie".to_owned(),
                    sonarr_id: None,
                    alternative_titles: vec![],
                },
                at(0),
            )
            .await
            .expect("series");
        (movie, series)
    }

    #[tokio::test]
    async fn blocklist_holds_a_release_once_per_item() {
        let db = Db::in_memory().await.expect("db");
        let (movie, series) = library(&db).await;
        let m = MediaRef::Movie(movie);
        assert!(
            db.add_block(&block(m, "Film.2020.1080p-A", "g1"), at(1))
                .await
                .expect("add")
        );
        // Same GUID, same normalized title: no second entry.
        assert!(
            !db.add_block(&block(m, "Other.Name", "g1"), at(2))
                .await
                .expect("guid")
        );
        assert!(
            !db.add_block(&block(m, "film_2020_1080p_a", "g2"), at(2))
                .await
                .expect("title")
        );
        // The same release is a separate entry for another item.
        assert!(
            db.add_block(
                &block(MediaRef::Series(series), "Film.2020.1080p-A", "g1"),
                at(3)
            )
            .await
            .expect("series")
        );
        let all = db.blocks().await.expect("blocks");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].media_title, "Film");
        assert_eq!(all[1].media, MediaRef::Series(series));
    }

    #[tokio::test]
    async fn blocklist_pages_newest_first() {
        let db = Db::in_memory().await.expect("db");
        let (movie, _) = library(&db).await;
        for i in 0..5u8 {
            db.add_block(
                &block(MediaRef::Movie(movie), &format!("R{i}"), &format!("g{i}")),
                at(i),
            )
            .await
            .expect("add");
        }
        let (page, total) = db.block_page(1, 2).await.expect("page 1");
        assert_eq!(total, 5);
        let titles: Vec<&str> = page.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, ["R4", "R3"]);
        let (page, _) = db.block_page(3, 2).await.expect("page 3");
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].blocklisted_at, at(0));
        assert!(db.block_page(4, 2).await.expect("past end").0.is_empty());
    }

    #[tokio::test]
    async fn removing_entries_and_items() {
        let db = Db::in_memory().await.expect("db");
        let (movie, series) = library(&db).await;
        db.add_block(&block(MediaRef::Movie(movie), "A", "a"), at(1))
            .await
            .expect("movie");
        db.add_block(&block(MediaRef::Series(series), "B", "b"), at(2))
            .await
            .expect("series");
        db.add_block(&block(MediaRef::Movie(movie), "C", "c"), at(3))
            .await
            .expect("movie");
        let id = db.blocks().await.expect("blocks")[0].id;
        assert!(db.remove_block(id).await.expect("remove"));
        assert!(!db.remove_block(id).await.expect("again"));
        assert_eq!(db.blocks().await.expect("blocks").len(), 2);
        // Removing the movie or series from the library removes its entries.
        for sql in [
            "DELETE FROM movies WHERE id = ?",
            "DELETE FROM series WHERE id = ?",
        ] {
            let id = if sql.contains("movies") {
                movie
            } else {
                series
            };
            sqlx::query(sql)
                .bind(id)
                .execute(db.pool())
                .await
                .expect("delete");
        }
        assert!(db.blocks().await.expect("blocks").is_empty());
    }
}
