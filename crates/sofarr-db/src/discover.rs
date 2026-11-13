//! What the discover pages need from the library (spec discover): the ids by which
//! TMDB titles are found in it, and whether a series monitors seasons that appear later.

use crate::{Db, Result};

/// A library movie by its ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovieKey {
    pub id: i64,
    pub tmdb_id: i64,
    pub imdb_id: Option<String>,
}

/// A library series by its ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesKey {
    pub id: i64,
    pub tvdb_id: i64,
    pub tmdb_id: Option<i64>,
    pub imdb_id: Option<String>,
}

impl Db {
    pub async fn movie_keys(&self) -> Result<Vec<MovieKey>> {
        let rows: Vec<(i64, i64, Option<String>)> =
            sqlx::query_as("SELECT id, tmdb_id, imdb_id FROM movies ORDER BY id")
                .fetch_all(self.pool())
                .await?;
        Ok(rows
            .into_iter()
            .map(|(id, tmdb_id, imdb_id)| MovieKey {
                id,
                tmdb_id,
                imdb_id,
            })
            .collect())
    }

    pub async fn series_keys(&self) -> Result<Vec<SeriesKey>> {
        let rows: Vec<(i64, i64, Option<i64>, Option<String>)> =
            sqlx::query_as("SELECT id, tvdb_id, tmdb_id, imdb_id FROM series ORDER BY id")
                .fetch_all(self.pool())
                .await?;
        Ok(rows
            .into_iter()
            .map(|(id, tvdb_id, tmdb_id, imdb_id)| SeriesKey {
                id,
                tvdb_id,
                tmdb_id,
                imdb_id,
            })
            .collect())
    }

    /// Whether seasons that appear later are monitored (default for every series).
    pub async fn monitors_new_seasons(&self, series_id: i64) -> Result<bool> {
        let value: Option<bool> =
            sqlx::query_scalar("SELECT monitor_new_seasons FROM series WHERE id = ?")
                .bind(series_id)
                .fetch_optional(self.pool())
                .await?;
        Ok(value.unwrap_or(true))
    }

    pub async fn set_monitor_new_seasons(&self, series_id: i64, monitor: bool) -> Result<()> {
        sqlx::query("UPDATE series SET monitor_new_seasons = ? WHERE id = ?")
            .bind(monitor)
            .bind(series_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;

    use crate::{Db, NewSeries};

    #[tokio::test]
    async fn keys_and_the_new_season_switch() {
        let db = Db::in_memory().await.expect("db");
        let id = db
            .upsert_series(
                &NewSeries {
                    tvdb_id: 77,
                    imdb_id: Some("tt1".to_owned()),
                    title: "Kleine Insel".to_owned(),
                    title_de: None,
                    year: None,
                    runtime_minutes: 0,
                    original_language: None,
                    series_type: "standard".to_owned(),
                    monitored: true,
                    season_folder: true,
                    quality_profile_id: 1,
                    path: "/tv/Kleine Insel".to_owned(),
                    sonarr_id: None,
                    alternative_titles: Vec::new(),
                },
                Timestamp::now(),
            )
            .await
            .expect("series");
        db.set_series_tmdb_id(id, 1399).await.expect("tmdb");
        let keys = db.series_keys().await.expect("keys");
        assert_eq!((keys[0].tvdb_id, keys[0].tmdb_id), (77, Some(1399)));
        assert!(db.monitors_new_seasons(id).await.expect("default"));
        db.set_monitor_new_seasons(id, false).await.expect("set");
        assert!(!db.monitors_new_seasons(id).await.expect("off"));
        assert!(db.movie_keys().await.expect("movies").is_empty());
    }
}
