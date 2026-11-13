//! Trailers (spec trailers): per movie or series the state of its trailer download.

use jiff::Timestamp;
use sqlx::{Row, sqlite::SqliteRow};

use crate::{Db, MediaRef, Result};

/// Where the trailer of an item stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrailerState {
    /// Nothing tried yet.
    Waiting,
    /// sofarr wrote a trailer (see [`TrailerRecord::file`]).
    Downloaded,
    /// TMDB listed no candidate.
    NoCandidate,
    /// Every candidate failed.
    Failed,
    /// The item's file system had too little free space.
    LowSpace,
}

impl TrailerState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Downloaded => "downloaded",
            Self::NoCandidate => "no_candidate",
            Self::Failed => "failed",
            Self::LowSpace => "low_space",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "waiting" => Self::Waiting,
            "downloaded" => Self::Downloaded,
            "no_candidate" => Self::NoCandidate,
            "failed" => Self::Failed,
            "low_space" => Self::LowSpace,
            _ => return None,
        })
    }
}

/// The stored trailer state of one item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrailerRecord {
    pub state: TrailerState,
    /// ISO 639-1 code of the downloaded trailer; `None` without language.
    pub language: Option<String>,
    /// YouTube video id of the downloaded trailer.
    pub video_key: Option<String>,
    /// The trailer sofarr wrote, relative to the item folder (`/`-separated).
    pub file: Option<String>,
    /// Why the last attempt failed.
    pub message: Option<String>,
    pub attempts: u32,
    pub next_attempt: Option<Timestamp>,
    /// When an import made the item eligible.
    pub first_in_line: Option<Timestamp>,
}

impl Default for TrailerRecord {
    fn default() -> Self {
        Self {
            state: TrailerState::Waiting,
            language: None,
            video_key: None,
            file: None,
            message: None,
            attempts: 0,
            next_attempt: None,
            first_in_line: None,
        }
    }
}

/// A movie or series as the trailer job sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrailerItem {
    pub media: MediaRef,
    pub title: String,
    /// Absolute folder of the item.
    pub path: String,
    /// TMDB id (a series only has one once the metadata refresh found it).
    pub tmdb_id: Option<i64>,
    pub added_at: Timestamp,
    /// The movie has a file, or the series at least one episode file.
    pub has_video: bool,
    pub trailer: Option<TrailerRecord>,
}

const ITEMS: &str = "SELECT * FROM (
    SELECT m.id AS movie_id, NULL AS series_id, m.title, m.path, m.tmdb_id, m.added_at,
        EXISTS (SELECT 1 FROM movie_files f WHERE f.movie_id = m.id) AS has_video
    FROM movies m
    UNION ALL
    SELECT NULL, s.id, s.title, s.path, s.tmdb_id, s.added_at,
        EXISTS (SELECT 1 FROM episode_files f WHERE f.series_id = s.id)
    FROM series s
) i LEFT JOIN trailers t
    ON t.movie_id = i.movie_id OR t.series_id = i.series_id";

fn timestamp(row: &SqliteRow, column: &str) -> Option<Timestamp> {
    row.get::<Option<String>, _>(column)
        .and_then(|raw| raw.parse().ok())
}

fn item_from_row(row: &SqliteRow) -> TrailerItem {
    let media = match row.get::<Option<i64>, _>(0) {
        Some(id) => MediaRef::Movie(id),
        None => MediaRef::Series(row.get(1)),
    };
    let trailer = row
        .get::<Option<String>, _>("state")
        .map(|state| TrailerRecord {
            state: TrailerState::parse(&state).unwrap_or(TrailerState::Waiting),
            language: row.get("language"),
            video_key: row.get("video_key"),
            file: row.get("file"),
            message: row.get("message"),
            attempts: u32::try_from(row.get::<i64, _>("attempts")).unwrap_or(0),
            next_attempt: timestamp(row, "next_attempt"),
            first_in_line: timestamp(row, "first_in_line"),
        });
    TrailerItem {
        media,
        title: row.get("title"),
        path: row.get("path"),
        tmdb_id: row.get("tmdb_id"),
        added_at: timestamp(row, "added_at").unwrap_or(Timestamp::UNIX_EPOCH),
        has_video: row.get("has_video"),
        trailer,
    }
}

/// The trailer table's column of a media kind.
fn column(media: MediaRef) -> (&'static str, i64) {
    match media {
        MediaRef::Movie(id) => ("movie_id", id),
        MediaRef::Series(id) => ("series_id", id),
    }
}

impl Db {
    /// Every movie and series with its trailer state.
    pub async fn trailer_items(&self) -> Result<Vec<TrailerItem>> {
        let rows = sqlx::query(ITEMS).fetch_all(self.pool()).await?;
        Ok(rows.iter().map(item_from_row).collect())
    }

    /// One movie or series with its trailer state; `None` if it is not in the library.
    pub async fn trailer_item(&self, media: MediaRef) -> Result<Option<TrailerItem>> {
        let (key, id) = column(media);
        let row = sqlx::query(&format!("{ITEMS} WHERE i.{key} = ?"))
            .bind(id)
            .fetch_optional(self.pool())
            .await?;
        Ok(row.as_ref().map(item_from_row))
    }

    /// Stores the trailer state of an item.
    pub async fn put_trailer(
        &self,
        media: MediaRef,
        record: &TrailerRecord,
        now: Timestamp,
    ) -> Result<()> {
        let (key, id) = column(media);
        sqlx::query(&format!(
            "INSERT INTO trailers ({key}, state, language, video_key, file, message, attempts,
                next_attempt, first_in_line, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT ({key}) DO UPDATE SET
                state = excluded.state, language = excluded.language,
                video_key = excluded.video_key, file = excluded.file,
                message = excluded.message, attempts = excluded.attempts,
                next_attempt = excluded.next_attempt, first_in_line = excluded.first_in_line,
                updated_at = excluded.updated_at"
        ))
        .bind(id)
        .bind(record.state.as_str())
        .bind(&record.language)
        .bind(&record.video_key)
        .bind(&record.file)
        .bind(&record.message)
        .bind(i64::from(record.attempts))
        .bind(record.next_attempt.map(|t| t.to_string()))
        .bind(record.first_in_line.map(|t| t.to_string()))
        .bind(now.to_string())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Puts an item first in line for the next trailer run (an import made it eligible);
    /// the rest of its state stays.
    pub async fn put_trailer_first_in_line(&self, media: MediaRef, at: Timestamp) -> Result<()> {
        let (key, id) = column(media);
        sqlx::query(&format!(
            "INSERT INTO trailers ({key}, state, first_in_line, updated_at)
             VALUES (?, 'waiting', ?, ?)
             ON CONFLICT ({key}) DO UPDATE SET
                first_in_line = excluded.first_in_line, updated_at = excluded.updated_at"
        ))
        .bind(id)
        .bind(at.to_string())
        .bind(at.to_string())
        .execute(self.pool())
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MovieFile, NewMovie, NewSeries};

    fn at(raw: &str) -> Timestamp {
        raw.parse().expect("timestamp")
    }

    async fn library() -> (Db, i64, i64, i64) {
        let db = Db::in_memory().await.expect("db");
        let movie = |tmdb_id: i64, title: &str| NewMovie {
            tmdb_id,
            imdb_id: None,
            title: title.to_owned(),
            original_title: None,
            title_de: None,
            year: Some(1995),
            runtime_minutes: 170,
            original_language: None,
            monitored: true,
            quality_profile_id: 1,
            path: format!("/data/movies/{title}"),
            radarr_id: None,
            alternative_titles: Vec::new(),
            availability: crate::Availability::default(),
            tags: Vec::new(),
        };
        let heat = db
            .upsert_movie(&movie(949, "Heat (1995)"), at("2026-01-01T00:00:00Z"))
            .await
            .expect("heat");
        let wanted = db
            .upsert_movie(&movie(950, "Wanted (2026)"), at("2026-02-01T00:00:00Z"))
            .await
            .expect("wanted");
        db.set_file(
            heat,
            &MovieFile {
                relative_path: "Heat (1995).mkv".to_owned(),
                size: 1,
                quality: "Bluray-1080p".to_owned(),
                revision_version: 1,
                revision_real: 0,
                release_group: None,
                scene_name: None,
                custom_format_score: 0,
            },
            at("2026-01-02T00:00:00Z"),
        )
        .await
        .expect("file");
        let show = db
            .upsert_series(
                &NewSeries {
                    tvdb_id: 81189,
                    imdb_id: None,
                    title: "Kleine Insel".to_owned(),
                    title_de: None,
                    year: Some(2020),
                    runtime_minutes: 45,
                    original_language: None,
                    series_type: "standard".to_owned(),
                    monitored: true,
                    season_folder: true,
                    quality_profile_id: 1,
                    path: "/data/tv/Kleine Insel".to_owned(),
                    sonarr_id: None,
                    alternative_titles: Vec::new(),
                },
                at("2026-03-01T00:00:00Z"),
            )
            .await
            .expect("series");
        (db, heat, wanted, show)
    }

    #[tokio::test]
    async fn items_carry_video_tmdb_id_and_trailer_state() {
        let (db, heat, wanted, show) = library().await;
        let items = db.trailer_items().await.expect("items");
        assert_eq!(items.len(), 3);
        let heat_item = db
            .trailer_item(MediaRef::Movie(heat))
            .await
            .expect("read")
            .expect("heat");
        assert_eq!(heat_item.title, "Heat (1995)");
        assert_eq!(heat_item.path, "/data/movies/Heat (1995)");
        assert_eq!(heat_item.tmdb_id, Some(949));
        assert_eq!(heat_item.added_at, at("2026-01-01T00:00:00Z"));
        assert!(heat_item.has_video);
        assert_eq!(heat_item.trailer, None);
        let wanted_item = db
            .trailer_item(MediaRef::Movie(wanted))
            .await
            .expect("read")
            .expect("wanted");
        assert!(!wanted_item.has_video);
        let show_item = db
            .trailer_item(MediaRef::Series(show))
            .await
            .expect("read")
            .expect("series");
        assert_eq!(show_item.tmdb_id, None);
        assert!(!show_item.has_video);
        assert_eq!(show_item.added_at, at("2026-03-01T00:00:00Z"));
        assert_eq!(
            db.trailer_item(MediaRef::Movie(999)).await.expect("read"),
            None
        );

        let record = TrailerRecord {
            state: TrailerState::Failed,
            message: Some("yt-dlp: Video nicht verfügbar".to_owned()),
            attempts: 2,
            next_attempt: Some(at("2026-11-19T03:00:00Z")),
            ..TrailerRecord::default()
        };
        db.put_trailer(MediaRef::Series(show), &record, at("2026-11-12T03:00:00Z"))
            .await
            .expect("put");
        let downloaded = TrailerRecord {
            state: TrailerState::Downloaded,
            language: Some("de".to_owned()),
            video_key: Some("abcdefghijk".to_owned()),
            file: Some("trailers/Heat (1995) - Trailer (Deutsch).mkv".to_owned()),
            ..TrailerRecord::default()
        };
        db.put_trailer(MediaRef::Movie(heat), &record, at("2026-11-12T03:00:00Z"))
            .await
            .expect("put");
        db.put_trailer(
            MediaRef::Movie(heat),
            &downloaded,
            at("2026-11-12T04:00:00Z"),
        )
        .await
        .expect("replace");
        let items = db.trailer_items().await.expect("items");
        let state = |media: MediaRef| {
            items
                .iter()
                .find(|i| i.media == media)
                .and_then(|i| i.trailer.clone())
        };
        assert_eq!(state(MediaRef::Movie(heat)), Some(downloaded));
        assert_eq!(state(MediaRef::Series(show)), Some(record));
        assert_eq!(state(MediaRef::Movie(wanted)), None);
    }

    #[tokio::test]
    async fn an_import_puts_an_item_first_in_line_and_keeps_its_state() {
        let (db, heat, wanted, _) = library().await;
        let t = at("2026-11-12T10:00:00Z");
        db.put_trailer_first_in_line(MediaRef::Movie(wanted), t)
            .await
            .expect("new row");
        let record = TrailerRecord {
            state: TrailerState::NoCandidate,
            attempts: 1,
            ..TrailerRecord::default()
        };
        db.put_trailer(MediaRef::Movie(heat), &record, t)
            .await
            .expect("put");
        db.put_trailer_first_in_line(MediaRef::Movie(heat), t)
            .await
            .expect("existing row");
        let read = |id| {
            let db = db.clone();
            async move {
                db.trailer_item(MediaRef::Movie(id))
                    .await
                    .expect("read")
                    .and_then(|i| i.trailer)
                    .expect("trailer")
            }
        };
        let wanted_state = read(wanted).await;
        assert_eq!(wanted_state.state, TrailerState::Waiting);
        assert_eq!(wanted_state.first_in_line, Some(t));
        let heat_state = read(heat).await;
        assert_eq!(heat_state.state, TrailerState::NoCandidate);
        assert_eq!(heat_state.attempts, 1);
        assert_eq!(heat_state.first_in_line, Some(t));
    }

    #[tokio::test]
    async fn removing_an_item_removes_its_trailer_state() {
        let (db, heat, _, _) = library().await;
        let t = at("2026-11-12T10:00:00Z");
        db.put_trailer(MediaRef::Movie(heat), &TrailerRecord::default(), t)
            .await
            .expect("put");
        sqlx::query("DELETE FROM movies WHERE id = ?")
            .bind(heat)
            .execute(db.pool())
            .await
            .expect("delete");
        let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trailers")
            .fetch_one(db.pool())
            .await
            .expect("count");
        assert_eq!(left, 0);
    }
}
