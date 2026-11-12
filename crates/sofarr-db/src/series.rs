//! Series repositories.

use jiff::Timestamp;
use sqlx::{Row, sqlite::SqliteRow};

use crate::{Db, GrabStatus, MovieFile, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSeries {
    pub tvdb_id: i64,
    pub imdb_id: Option<String>,
    pub title: String,
    pub title_de: Option<String>,
    pub year: Option<u16>,
    pub runtime_minutes: u32,
    pub original_language: Option<String>,
    /// `standard`, `daily` or `anime`.
    pub series_type: String,
    pub monitored: bool,
    pub season_folder: bool,
    pub quality_profile_id: i64,
    pub path: String,
    pub sonarr_id: Option<i64>,
    pub alternative_titles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Series {
    pub id: i64,
    pub data: NewSeries,
    /// Tag ids (of the `sonarr_tag` setting), see [`Db::set_series_tags`].
    pub tags: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEpisode {
    pub tvdb_id: Option<i64>,
    pub season: u16,
    pub number: u16,
    pub absolute: Option<u16>,
    pub title: Option<String>,
    pub title_de: Option<String>,
    pub air_date: Option<String>,
    pub monitored: bool,
}

/// An episode with its file (the file record is shared by multi-episode files).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Episode {
    pub id: i64,
    pub series_id: i64,
    pub data: NewEpisode,
    pub file_id: Option<i64>,
    pub file: Option<MovieFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesGrab {
    pub id: i64,
    pub series_id: i64,
    pub episode_ids: Vec<i64>,
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
}

fn to_i64(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

use crate::{ids_from_text, ids_text};

fn series_from_row(row: &SqliteRow) -> Series {
    Series {
        id: row.get("id"),
        data: NewSeries {
            tvdb_id: row.get("tvdb_id"),
            imdb_id: row.get("imdb_id"),
            title: row.get("title"),
            title_de: row.get("title_de"),
            year: row.get("year"),
            runtime_minutes: row.get("runtime"),
            original_language: row.get("original_language"),
            series_type: row.get("series_type"),
            monitored: row.get("monitored"),
            season_folder: row.get("season_folder"),
            quality_profile_id: row.get("quality_profile_id"),
            path: row.get("path"),
            sonarr_id: row.get("sonarr_id"),
            alternative_titles: Vec::new(),
        },
        tags: ids_from_text(row.get("tags")),
    }
}

fn grab_from_row(row: &SqliteRow) -> SeriesGrab {
    SeriesGrab {
        id: row.get("id"),
        series_id: row.get("series_id"),
        episode_ids: row
            .get::<String, _>("episode_ids")
            .split(',')
            .filter_map(|i| i.parse().ok())
            .collect(),
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
        grabbed_at: row
            .get::<String, _>("grabbed_at")
            .parse()
            .unwrap_or(Timestamp::UNIX_EPOCH),
    }
}

impl Db {
    pub async fn upsert_series(&self, series: &NewSeries, now: Timestamp) -> Result<i64> {
        let mut tx = self.pool().begin().await?;
        let (id,): (i64,) = sqlx::query_as(
            "INSERT INTO series (tvdb_id, imdb_id, title, title_de, year, runtime, original_language,
                series_type, monitored, season_folder, quality_profile_id, path, sonarr_id, added_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (tvdb_id) DO UPDATE SET
                imdb_id = excluded.imdb_id, title = excluded.title,
                title_de = COALESCE(excluded.title_de, series.title_de), year = excluded.year,
                runtime = excluded.runtime, original_language = excluded.original_language,
                series_type = excluded.series_type, monitored = excluded.monitored,
                season_folder = excluded.season_folder,
                quality_profile_id = excluded.quality_profile_id, path = excluded.path,
                sonarr_id = COALESCE(excluded.sonarr_id, series.sonarr_id)
             RETURNING id",
        )
        .bind(series.tvdb_id)
        .bind(&series.imdb_id)
        .bind(&series.title)
        .bind(&series.title_de)
        .bind(series.year)
        .bind(series.runtime_minutes)
        .bind(&series.original_language)
        .bind(&series.series_type)
        .bind(series.monitored)
        .bind(series.season_folder)
        .bind(series.quality_profile_id)
        .bind(&series.path)
        .bind(series.sonarr_id)
        .bind(now.to_string())
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM series_titles WHERE series_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for title in &series.alternative_titles {
            sqlx::query("INSERT OR IGNORE INTO series_titles (series_id, title) VALUES (?, ?)")
                .bind(id)
                .bind(title)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    pub async fn all_series(&self) -> Result<Vec<Series>> {
        let rows = sqlx::query("SELECT * FROM series ORDER BY id")
            .fetch_all(self.pool())
            .await?;
        let mut out: Vec<Series> = rows.iter().map(series_from_row).collect();
        let titles: Vec<(i64, String)> =
            sqlx::query_as("SELECT series_id, title FROM series_titles ORDER BY series_id, rowid")
                .fetch_all(self.pool())
                .await?;
        for (series_id, title) in titles {
            if let Some(s) = out.iter_mut().find(|s| s.id == series_id) {
                s.data.alternative_titles.push(title);
            }
        }
        Ok(out)
    }

    pub async fn series_by_tvdb(&self, tvdb_id: i64) -> Result<Option<Series>> {
        Ok(self
            .all_series()
            .await?
            .into_iter()
            .find(|s| s.data.tvdb_id == tvdb_id))
    }

    /// Inserts or updates an episode (key: series, season, number); returns its id.
    pub async fn upsert_episode(&self, series_id: i64, episode: &NewEpisode) -> Result<i64> {
        let id = sqlx::query_scalar(
            "INSERT INTO episodes (series_id, tvdb_id, season, number, absolute, title, title_de,
                air_date, monitored)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (series_id, season, number) DO UPDATE SET
                tvdb_id = COALESCE(excluded.tvdb_id, episodes.tvdb_id),
                absolute = COALESCE(excluded.absolute, episodes.absolute),
                title = COALESCE(excluded.title, episodes.title),
                title_de = COALESCE(excluded.title_de, episodes.title_de),
                air_date = COALESCE(excluded.air_date, episodes.air_date),
                monitored = excluded.monitored
             RETURNING id",
        )
        .bind(series_id)
        .bind(episode.tvdb_id)
        .bind(episode.season)
        .bind(episode.number)
        .bind(episode.absolute)
        .bind(&episode.title)
        .bind(&episode.title_de)
        .bind(&episode.air_date)
        .bind(episode.monitored)
        .fetch_one(self.pool())
        .await?;
        Ok(id)
    }

    /// Episodes of a series (or of all series), with files.
    pub async fn episodes(&self, series_id: Option<i64>) -> Result<Vec<Episode>> {
        let rows = sqlx::query(
            "SELECT e.id, e.series_id, e.tvdb_id, e.season, e.number, e.absolute, e.title,
                e.title_de, e.air_date, e.monitored, e.file_id,
                f.relative_path, f.size, f.quality, f.revision_version, f.revision_real,
                f.release_group, f.scene_name, f.custom_format_score
             FROM episodes e LEFT JOIN episode_files f ON f.id = e.file_id
             WHERE (? IS NULL OR e.series_id = ?)
             ORDER BY e.series_id, e.season, e.number",
        )
        .bind(series_id)
        .bind(series_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .iter()
            .map(|row| Episode {
                id: row.get("id"),
                series_id: row.get("series_id"),
                data: NewEpisode {
                    tvdb_id: row.get("tvdb_id"),
                    season: row.get("season"),
                    number: row.get("number"),
                    absolute: row.get("absolute"),
                    title: row.get("title"),
                    title_de: row.get("title_de"),
                    air_date: row.get("air_date"),
                    monitored: row.get("monitored"),
                },
                file_id: row.get("file_id"),
                file: row
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
                    }),
            })
            .collect())
    }

    /// Stores a file for episodes (replacing their previous file records).
    pub async fn set_episode_file(
        &self,
        series_id: i64,
        episode_ids: &[i64],
        file: &MovieFile,
        now: Timestamp,
    ) -> Result<i64> {
        let mut tx = self.pool().begin().await?;
        let file_id: i64 = sqlx::query_scalar(
            "INSERT INTO episode_files (series_id, relative_path, size, quality, revision_version,
                revision_real, release_group, scene_name, custom_format_score, added_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(series_id)
        .bind(&file.relative_path)
        .bind(to_i64(file.size))
        .bind(&file.quality)
        .bind(file.revision_version)
        .bind(file.revision_real)
        .bind(&file.release_group)
        .bind(&file.scene_name)
        .bind(file.custom_format_score)
        .bind(now.to_string())
        .fetch_one(&mut *tx)
        .await?;
        for id in episode_ids {
            sqlx::query("UPDATE episodes SET file_id = ? WHERE id = ?")
                .bind(file_id)
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        // File records no episode points to any more are gone.
        sqlx::query(
            "DELETE FROM episode_files WHERE series_id = ?
                AND id NOT IN (SELECT file_id FROM episodes WHERE file_id IS NOT NULL)",
        )
        .bind(series_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(file_id)
    }

    pub async fn add_series_grab(
        &self,
        series_id: i64,
        episode_ids: &[i64],
        grab: &crate::NewGrab,
        now: Timestamp,
    ) -> Result<i64> {
        let id = sqlx::query_scalar(
            "INSERT INTO series_grabs (series_id, episode_ids, title, guid, indexer, download_url,
                quality, custom_format_score, size, status, download_id, grabbed_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(series_id)
        .bind(ids_text(episode_ids))
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
        .fetch_one(self.pool())
        .await?;
        Ok(id)
    }

    pub async fn update_series_grab(
        &self,
        id: i64,
        status: GrabStatus,
        download_id: Option<&str>,
        message: Option<&str>,
        now: Timestamp,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE series_grabs SET status = ?, download_id = COALESCE(?, download_id),
                message = ?, updated_at = ? WHERE id = ?",
        )
        .bind(status.as_str())
        .bind(download_id)
        .bind(message)
        .bind(now.to_string())
        .bind(id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_series_tags(&self, series_id: i64, tags: &[i64]) -> Result<()> {
        sqlx::query("UPDATE series SET tags = ? WHERE id = ?")
            .bind(ids_text(tags))
            .bind(series_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn set_episodes_monitored(&self, episode_ids: &[i64], monitored: bool) -> Result<()> {
        for id in episode_ids {
            sqlx::query("UPDATE episodes SET monitored = ? WHERE id = ?")
                .bind(monitored)
                .bind(id)
                .execute(self.pool())
                .await?;
        }
        Ok(())
    }

    /// Forgets an episode file (the file on disk is gone); its episodes lose their file.
    pub async fn remove_episode_file(&self, file_id: i64) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("UPDATE episodes SET file_id = NULL WHERE file_id = ?")
            .bind(file_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM episode_files WHERE id = ?")
            .bind(file_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn series_grabs(&self, status: Option<GrabStatus>) -> Result<Vec<SeriesGrab>> {
        let rows = sqlx::query(
            "SELECT * FROM series_grabs WHERE (? IS NULL OR status = ?) ORDER BY id DESC",
        )
        .bind(status.map(GrabStatus::as_str))
        .bind(status.map(GrabStatus::as_str))
        .fetch_all(self.pool())
        .await?;
        Ok(rows.iter().map(grab_from_row).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NewGrab;

    fn now() -> Timestamp {
        "2026-11-01T12:00:00Z".parse().expect("ts")
    }

    fn series() -> NewSeries {
        NewSeries {
            tvdb_id: 77,
            imdb_id: None,
            title: "Small Island".to_owned(),
            title_de: Some("Kleine Insel".to_owned()),
            year: Some(2019),
            runtime_minutes: 45,
            original_language: Some("English".to_owned()),
            series_type: "standard".to_owned(),
            monitored: true,
            season_folder: true,
            quality_profile_id: 1,
            path: "/data/media/tv/Small Island".to_owned(),
            sonarr_id: Some(3),
            alternative_titles: vec!["Kleine Insel".to_owned()],
        }
    }

    fn episode(season: u16, number: u16) -> NewEpisode {
        NewEpisode {
            tvdb_id: None,
            season,
            number,
            absolute: None,
            title: Some(format!("Folge {number}")),
            title_de: None,
            air_date: Some("2026-10-01".to_owned()),
            monitored: true,
        }
    }

    fn file(name: &str) -> MovieFile {
        MovieFile {
            relative_path: name.to_owned(),
            size: 1000,
            quality: "WEBDL-1080p".to_owned(),
            revision_version: 1,
            revision_real: 0,
            release_group: None,
            scene_name: None,
            custom_format_score: 0,
        }
    }

    #[tokio::test]
    async fn series_episodes_and_files() {
        let db = Db::in_memory().await.expect("db");
        let id = db.upsert_series(&series(), now()).await.expect("series");
        assert_eq!(db.upsert_series(&series(), now()).await.expect("again"), id);
        let e1 = db.upsert_episode(id, &episode(1, 1)).await.expect("e1");
        let e2 = db.upsert_episode(id, &episode(1, 2)).await.expect("e2");
        assert_eq!(
            db.upsert_episode(id, &episode(1, 1)).await.expect("same"),
            e1
        );

        db.set_episode_file(id, &[e1, e2], &file("S01E01-E02.mkv"), now())
            .await
            .expect("double");
        let eps = db.episodes(Some(id)).await.expect("episodes");
        assert_eq!(eps.len(), 2);
        assert_eq!(eps[0].file_id, eps[1].file_id);
        // Replacing one episode keeps the shared record for the other.
        db.set_episode_file(id, &[e1], &file("S01E01.mkv"), now())
            .await
            .expect("single");
        let eps = db.episodes(None).await.expect("episodes");
        assert_eq!(
            eps[0].file.as_ref().map(|f| f.relative_path.as_str()),
            Some("S01E01.mkv")
        );
        assert_eq!(
            eps[1].file.as_ref().map(|f| f.relative_path.as_str()),
            Some("S01E01-E02.mkv")
        );
        let all = db.all_series().await.expect("series");
        assert_eq!(all[0].data.alternative_titles, ["Kleine Insel"]);
        assert!(db.series_by_tvdb(77).await.expect("tvdb").is_some());

        let grab = NewGrab {
            movie_id: 0,
            title: "Small.Island.S01E03.1080p.WEB-DL-G".to_owned(),
            guid: "g".to_owned(),
            indexer: "idx".to_owned(),
            download_url: None,
            quality: "WEBDL-1080p".to_owned(),
            custom_format_score: 0,
            size: 1,
            status: GrabStatus::Shadow,
            download_id: None,
        };
        let gid = db
            .add_series_grab(id, &[e1, e2], &grab, now())
            .await
            .expect("grab");
        db.update_series_grab(gid, GrabStatus::Queued, Some("9"), None, now())
            .await
            .expect("update");
        let grabs = db
            .series_grabs(Some(GrabStatus::Queued))
            .await
            .expect("grabs");
        assert_eq!(grabs[0].episode_ids, [e1, e2]);
        assert_eq!(grabs[0].download_id.as_deref(), Some("9"));

        db.set_series_tags(id, &[2]).await.expect("tags");
        assert_eq!(db.all_series().await.expect("series")[0].tags, [2]);
        let shared = db.episodes(Some(id)).await.expect("eps")[1]
            .file_id
            .expect("file");
        db.remove_episode_file(shared).await.expect("remove");
        db.set_episodes_monitored(&[e2], false)
            .await
            .expect("unmonitor");
        let eps = db.episodes(Some(id)).await.expect("eps");
        assert_eq!(eps[1].file, None);
        assert!(!eps[1].data.monitored);
        assert!(eps[0].file.is_some());
    }
}
