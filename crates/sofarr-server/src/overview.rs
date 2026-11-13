//! The overview page (spec web-ui-library AC-01): the queue with the download client's
//! progress, the activity of grabs and imports, failures and import problems of the
//! last days, and what is released in the next days.

use std::sync::Mutex;

use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
    http::StatusCode,
};
use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use sofarr_db::{Activity, MediaRef};
use sofarr_download::{DownloadClient, DownloadItem, DownloadState};

use crate::{
    api::ApiError,
    app::{App, SharedApp},
    calendar::{self, CalendarEntry},
    status::Peer,
};

/// How many activities the overview shows.
const ACTIVITY: u32 = 20;
/// Failures and import problems of these last days.
const PROBLEM_DAYS: i64 = 7;
/// Releases of these next days.
const UPCOMING_DAYS: i64 = 7;

/// Movie or series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Movie,
    Series,
}

/// A grab or import.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ActivityEntry {
    pub kind: MediaKind,
    /// The grab's id (for marking it as failed: `/api/grabs/{kind}/{id}/fail`).
    pub grab_id: i64,
    /// Movie or series id.
    pub media_id: i64,
    pub media_title: String,
    pub release_title: String,
    pub quality: String,
    pub size: u64,
    /// `shadow`, `queued`, `downloading`, `completed`, `imported` or `failed`.
    pub status: String,
    pub message: Option<String>,
    pub episode_ids: Vec<i64>,
    pub grabbed_at: String,
    pub updated_at: String,
}

/// An active grab with what the download client says about it.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct QueueItem {
    #[serde(flatten)]
    pub grab: ActivityEntry,
    /// The client's state: `queued`, `downloading`, `paused`, `completed`, `failed`;
    /// `null` when the client does not (or no longer) list it.
    pub download_state: Option<String>,
    /// 0 to 1.
    pub progress: Option<f64>,
    pub remaining_bytes: Option<u64>,
    /// `null` where the client reports none (paused, waiting, unknown speed).
    pub remaining_seconds: Option<u64>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct QueueAnswer {
    pub items: Vec<QueueItem>,
    /// Why the client's state may be missing or old, in German.
    pub notice: Option<String>,
    /// When the client's state shown was read.
    pub as_of: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct OverviewAnswer {
    pub queue: QueueAnswer,
    /// The last 20 grabs and imports.
    pub activity: Vec<ActivityEntry>,
    /// Failed grabs and import problems of the last 7 days.
    pub problems: Vec<ActivityEntry>,
    /// Release dates of monitored items in the next 7 days.
    pub upcoming: Vec<CalendarEntry>,
}

/// The download client's last answer (spec edge case: unreachable client).
#[derive(Debug, Clone)]
struct Seen {
    at: Timestamp,
    items: Vec<DownloadItem>,
    rate: u64,
}

/// The last state the download client reported.
#[derive(Debug, Default)]
pub struct QueueCache(Mutex<Option<Seen>>);

fn entry(a: Activity) -> ActivityEntry {
    let (kind, media_id) = match a.media {
        MediaRef::Movie(id) => (MediaKind::Movie, id),
        MediaRef::Series(id) => (MediaKind::Series, id),
    };
    ActivityEntry {
        kind,
        grab_id: a.grab_id,
        media_id,
        media_title: a.media_title,
        release_title: a.title,
        quality: a.quality,
        size: a.size,
        status: a.status.as_str().to_owned(),
        message: a.message,
        episode_ids: a.episode_ids,
        grabbed_at: a.grabbed_at.to_string(),
        updated_at: a.updated_at.to_string(),
    }
}

fn state_name(state: DownloadState) -> &'static str {
    match state {
        DownloadState::Queued => "queued",
        DownloadState::Downloading => "downloading",
        DownloadState::Paused => "paused",
        DownloadState::Completed => "completed",
        DownloadState::Failed(_) => "failed",
    }
}

/// The queue entry of an active grab and the client's item for it.
fn queue_item(grab: Activity, item: Option<&DownloadItem>, rate: u64) -> QueueItem {
    let size = item.map_or(grab.size, |i| i.size_bytes);
    let progress = item.map(|i| match i.state {
        DownloadState::Completed => 1.0,
        _ if i.size_bytes == 0 => 0.0,
        _ => {
            let done = i.size_bytes.saturating_sub(i.remaining_bytes);
            // Sizes stay far below 2^52 bytes, where f64 loses precision.
            done as f64 / i.size_bytes as f64
        }
    });
    let remaining_seconds = item
        .filter(|i| i.state == DownloadState::Downloading && rate > 0)
        .map(|i| i.remaining_bytes.div_ceil(rate));
    let mut grab = entry(grab);
    grab.size = size;
    QueueItem {
        grab,
        download_state: item.map(|i| state_name(i.state).to_owned()),
        progress,
        remaining_bytes: item.map(|i| i.remaining_bytes),
        remaining_seconds,
    }
}

impl App {
    /// The client's items and speed, or the last known ones with a notice.
    async fn client_state(&self) -> (Option<Seen>, Option<String>) {
        let Some(client) = &self.nzbget else {
            return (None, Some("Kein Download-Client konfiguriert".to_owned()));
        };
        let listed = client.items().await;
        self.status
            .contact(Peer::DownloadClient, Timestamp::now(), &listed);
        match listed {
            Ok(items) => {
                let rate = client.download_rate().await.unwrap_or(0);
                let seen = Seen {
                    at: Timestamp::now(),
                    items,
                    rate,
                };
                if let Ok(mut last) = self.queue_cache.0.lock() {
                    *last = Some(seen.clone());
                }
                (Some(seen), None)
            }
            Err(e) => {
                // The upstream error stays in the log (auth AC-14).
                tracing::warn!(error = %e, "download client unreachable");
                let last = self.queue_cache.0.lock().ok().and_then(|l| l.clone());
                let notice = match &last {
                    Some(seen) => format!(
                        "Download-Client nicht erreichbar – Stand von {}",
                        seen.at
                            .to_zoned(jiff::tz::TimeZone::system())
                            .strftime("%d.%m.%Y %H:%M")
                    ),
                    None => "Download-Client nicht erreichbar".to_owned(),
                };
                (last, Some(notice))
            }
        }
    }

    /// The active grabs with the client's progress.
    pub async fn queue(&self) -> anyhow::Result<QueueAnswer> {
        let grabs = self.db.active_activity().await?;
        let (seen, notice) = self.client_state().await;
        let items = grabs
            .into_iter()
            .map(|g| {
                let item = seen.as_ref().and_then(|s| {
                    let id = g.download_id.as_deref()?;
                    s.items.iter().find(|i| i.id == id)
                });
                queue_item(g, item, seen.as_ref().map_or(0, |s| s.rate))
            })
            .collect();
        Ok(QueueAnswer {
            items,
            notice,
            as_of: seen.map(|s| s.at.to_string()),
        })
    }
}

pub async fn queue(State(app): State<SharedApp>) -> Result<Json<QueueAnswer>, ApiError> {
    Ok(Json(app.queue().await?))
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct ActivityQuery {
    /// 1 to 500, default 20.
    pub limit: Option<u32>,
}

pub async fn activity(
    State(app): State<SharedApp>,
    query: Result<Query<ActivityQuery>, QueryRejection>,
) -> Result<Json<Vec<ActivityEntry>>, ApiError> {
    let bad = || {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "Ungültige Anfrage: limit von 1 bis 500",
        )
    };
    let Query(q) = query.map_err(|_| bad())?;
    let limit = q.limit.unwrap_or(ACTIVITY);
    if !(1..=500).contains(&limit) {
        return Err(bad());
    }
    Ok(Json(
        app.db
            .activity(limit)
            .await?
            .into_iter()
            .map(entry)
            .collect(),
    ))
}

pub async fn overview(State(app): State<SharedApp>) -> Result<Json<OverviewAnswer>, ApiError> {
    let now = Timestamp::now();
    let today = crate::library_view::today(now);
    let upcoming = calendar::entries(
        &app,
        today,
        today.saturating_add(jiff::Span::new().days(UPCOMING_DAYS)),
    )
    .await?
    .into_iter()
    .filter(|e| e.monitored)
    .collect();
    Ok(Json(OverviewAnswer {
        queue: app.queue().await?,
        activity: app
            .db
            .activity(ACTIVITY)
            .await?
            .into_iter()
            .map(entry)
            .collect(),
        problems: app
            .db
            .problems(now - SignedDuration::from_hours(PROBLEM_DAYS * 24))
            .await?
            .into_iter()
            .map(entry)
            .collect(),
        upcoming,
    }))
}
