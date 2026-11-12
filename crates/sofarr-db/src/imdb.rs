//! IMDb ratings (spec imdb-ratings): loaded into a side table in batches and swapped in
//! as a whole, so readers never see a mixture and a crash keeps the previous data.

use jiff::Timestamp;

use crate::{Db, Result};

/// The rating of one title.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImdbRating {
    pub rating: f32,
    pub votes: u32,
}

/// One row of the dataset: IMDb id, rating, votes.
pub type RatingRow = (String, f32, u32);

const LOADED_AT: &str = "imdb_loaded_at";
/// Rows per INSERT (3 parameters each, below SQLite's 999).
const ROWS_PER_INSERT: usize = 300;

/// A load in progress: rows go to `imdb_ratings_next` until [`RatingsLoad::finish`]
/// swaps it in. Dropped without finishing, the previous ratings stay.
pub struct RatingsLoad<'a> {
    db: &'a Db,
    count: u64,
}

impl Db {
    /// Starts a load into a fresh side table (the leftovers of a crashed load are dropped).
    pub async fn begin_imdb_load(&self) -> Result<RatingsLoad<'_>> {
        sqlx::query("DROP TABLE IF EXISTS imdb_ratings_next")
            .execute(self.pool())
            .await?;
        sqlx::query(
            "CREATE TABLE imdb_ratings_next (
                imdb_id TEXT PRIMARY KEY,
                rating  REAL NOT NULL,
                votes   INTEGER NOT NULL
            ) WITHOUT ROWID",
        )
        .execute(self.pool())
        .await?;
        Ok(RatingsLoad { db: self, count: 0 })
    }

    pub async fn imdb_rating(&self, imdb_id: &str) -> Result<Option<ImdbRating>> {
        let row: Option<(f64, i64)> =
            sqlx::query_as("SELECT rating, votes FROM imdb_ratings WHERE imdb_id = ?")
                .bind(imdb_id)
                .fetch_optional(self.pool())
                .await?;
        Ok(row.map(|(rating, votes)| ImdbRating {
            rating: rating as f32,
            votes: u32::try_from(votes).unwrap_or(0),
        }))
    }

    /// The ratings of every movie and series of the library, by IMDb id.
    pub async fn library_imdb_ratings(
        &self,
    ) -> Result<std::collections::HashMap<String, ImdbRating>> {
        let rows: Vec<(String, f64, i64)> = sqlx::query_as(
            "SELECT r.imdb_id, r.rating, r.votes FROM imdb_ratings r
             WHERE r.imdb_id IN (SELECT imdb_id FROM movies WHERE imdb_id IS NOT NULL
                                 UNION SELECT imdb_id FROM series WHERE imdb_id IS NOT NULL)",
        )
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, rating, votes)| {
                (
                    id,
                    ImdbRating {
                        rating: rating as f32,
                        votes: u32::try_from(votes).unwrap_or(0),
                    },
                )
            })
            .collect())
    }

    /// How many titles the current ratings data has.
    pub async fn imdb_rating_count(&self) -> Result<u64> {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM imdb_ratings")
            .fetch_one(self.pool())
            .await?;
        Ok(u64::try_from(n).unwrap_or(0))
    }

    /// When the current ratings data was loaded; `None` before the first load.
    pub async fn imdb_loaded_at(&self) -> Result<Option<Timestamp>> {
        Ok(self
            .setting(LOADED_AT)
            .await?
            .and_then(|v| v.as_str().and_then(|s| s.parse().ok())))
    }
}

impl RatingsLoad<'_> {
    /// Adds rows in one transaction; a later row with the same id replaces an earlier one.
    pub async fn add(&mut self, rows: &[RatingRow]) -> Result<()> {
        let mut tx = self.db.pool().begin().await?;
        for chunk in rows.chunks(ROWS_PER_INSERT) {
            let mut insert = sqlx::QueryBuilder::new(
                "INSERT OR REPLACE INTO imdb_ratings_next (imdb_id, rating, votes) ",
            );
            insert.push_values(chunk, |mut b, (id, rating, votes)| {
                b.push_bind(id)
                    .push_bind(f64::from(*rating))
                    .push_bind(i64::from(*votes));
            });
            insert.build().execute(&mut *tx).await?;
        }
        tx.commit().await?;
        self.count += rows.len() as u64;
        Ok(())
    }

    /// Rows added so far.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Swaps the loaded rows in as the ratings data, in one transaction; returns the
    /// number of titles.
    pub async fn finish(self, loaded_at: Timestamp) -> Result<u64> {
        let mut tx = self.db.pool().begin().await?;
        sqlx::query("DROP TABLE imdb_ratings")
            .execute(&mut *tx)
            .await?;
        sqlx::query("ALTER TABLE imdb_ratings_next RENAME TO imdb_ratings")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        )
        .bind(LOADED_AT)
        .bind(serde_json::Value::String(loaded_at.to_string()).to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM imdb_ratings")
            .fetch_one(self.db.pool())
            .await?;
        Ok(u64::try_from(n).unwrap_or(0))
    }

    /// Gives the load up: the side table goes, the previous ratings stay.
    pub async fn abandon(self) -> Result<()> {
        sqlx::query("DROP TABLE IF EXISTS imdb_ratings_next")
            .execute(self.db.pool())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(prefix: &str, n: u32) -> Vec<RatingRow> {
        (0..n)
            .map(|i| (format!("tt{prefix}{i:07}"), 7.5, 100 + i))
            .collect()
    }

    #[tokio::test]
    async fn a_load_is_swapped_in_as_a_whole() {
        let db = Db::in_memory().await.expect("db");
        assert_eq!(db.imdb_rating_count().await.expect("count"), 0);
        assert_eq!(db.imdb_loaded_at().await.expect("at"), None);
        let t0: Timestamp = "2026-11-12T03:00:00Z".parse().expect("t0");

        let mut first = db.begin_imdb_load().await.expect("begin");
        first.add(&rows("1", 700)).await.expect("add");
        first
            .add(&[("tt0000042".to_owned(), 8.25, 12_345)])
            .await
            .expect("add");
        assert_eq!(first.count(), 701);
        // Until the swap nothing of the load is visible.
        assert_eq!(db.imdb_rating("tt0000042").await.expect("read"), None);
        assert_eq!(first.finish(t0).await.expect("finish"), 701);
        assert_eq!(
            db.imdb_rating("tt0000042").await.expect("read"),
            Some(ImdbRating {
                rating: 8.25,
                votes: 12_345
            })
        );
        assert_eq!(db.imdb_loaded_at().await.expect("at"), Some(t0));

        // An abandoned (or crashed) load keeps the previous data.
        let mut broken = db.begin_imdb_load().await.expect("begin");
        broken.add(&rows("2", 10)).await.expect("add");
        broken.abandon().await.expect("abandon");
        assert_eq!(db.imdb_rating_count().await.expect("count"), 701);
        {
            // A load that never finishes, as after a crash.
            let _crashed = db.begin_imdb_load().await.expect("begin");
        }
        assert_eq!(db.imdb_rating_count().await.expect("count"), 701);

        // The next load replaces everything: old titles it lacks are gone.
        let t1: Timestamp = "2026-11-13T03:00:00Z".parse().expect("t1");
        let mut second = db.begin_imdb_load().await.expect("begin");
        second.add(&rows("3", 5)).await.expect("add");
        assert_eq!(second.finish(t1).await.expect("finish"), 5);
        assert_eq!(db.imdb_rating("tt0000042").await.expect("read"), None);
        assert_eq!(db.imdb_loaded_at().await.expect("at"), Some(t1));
    }
}
