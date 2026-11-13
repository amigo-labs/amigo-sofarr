//! The calendar (spec web-ui-library AC-07): release dates of monitored movies and air
//! dates of the episodes of monitored series in a range, each marked as present, not
//! yet released, missing or neutral.

use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
    http::StatusCode,
};
use jiff::civil::Date;
use serde::{Deserialize, Serialize};

use crate::{
    api::ApiError,
    app::{App, SharedApp},
    library_list::{movie_judge, series_judge},
};

/// The longest range one request may cover (a month view with its weeks).
const MAX_DAYS: i32 = 62;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum EntryState {
    /// The item has a file.
    Present,
    /// The date lies ahead.
    Upcoming,
    /// The item counts as missing.
    Missing,
    /// Anything else, e.g. an unmonitored episode or a movie not yet available.
    Neutral,
}

/// Which release of a movie a date is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Release {
    Cinema,
    Digital,
    Physical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Movie,
    Episode,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct CalendarEntry {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub kind: EntryKind,
    /// Movies: which release date this is.
    pub release: Option<Release>,
    pub movie_id: Option<i64>,
    pub series_id: Option<i64>,
    pub episode_id: Option<i64>,
    pub season: Option<u16>,
    pub episode: Option<u16>,
    /// Movie or series title.
    pub title: String,
    pub episode_title: Option<String>,
    pub year: Option<u16>,
    pub state: EntryState,
    /// The movie, or the episode and its series, are monitored.
    pub monitored: bool,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct CalendarAnswer {
    pub from: String,
    pub to: String,
    pub entries: Vec<CalendarEntry>,
    /// A German note when the range holds no release.
    pub note: Option<String>,
}

/// Query of `GET /api/calendar`: both dates included.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct CalendarQuery {
    /// `YYYY-MM-DD`.
    pub from: String,
    /// `YYYY-MM-DD`, at most 62 days after `from`.
    pub to: String,
}

const BAD_RANGE: &str =
    "Ungültiger Zeitraum: from und to als JJJJ-MM-TT angeben, to nicht vor from, höchstens 62 Tage";

const EMPTY: &str = "Keine Veröffentlichungen in diesem Zeitraum";

fn state(has_file: bool, date: Date, today: Date, missing: bool) -> EntryState {
    if has_file {
        EntryState::Present
    } else if date > today {
        EntryState::Upcoming
    } else if missing {
        EntryState::Missing
    } else {
        EntryState::Neutral
    }
}

fn parse(raw: Option<&str>) -> Option<Date> {
    raw.and_then(sofarr_core::availability::parse_date)
}

/// The entries between `from` and `to` (both included), by date and title.
pub async fn entries(app: &App, from: Date, to: Date) -> anyhow::Result<Vec<CalendarEntry>> {
    let mut out = Vec::new();
    let inside = |d: Date| d >= from && d <= to;
    let judge = movie_judge(app).await;
    for m in app.db.movies().await?.iter().filter(|m| m.monitored) {
        let a = &m.availability;
        let missing = judge.movie(m).missing;
        for (release, raw) in [
            (Release::Cinema, &a.in_cinemas),
            (Release::Digital, &a.digital_release),
            (Release::Physical, &a.physical_release),
        ] {
            let Some(date) = parse(raw.as_deref()).filter(|d| inside(*d)) else {
                continue;
            };
            out.push(CalendarEntry {
                date: date.to_string(),
                kind: EntryKind::Movie,
                release: Some(release),
                movie_id: Some(m.id),
                series_id: None,
                episode_id: None,
                season: None,
                episode: None,
                title: m.title.clone(),
                episode_title: None,
                year: m.year,
                state: state(m.file.is_some(), date, judge.today, missing),
                monitored: true,
            });
        }
    }
    let judge = series_judge(app).await;
    let today = judge.today_text();
    let shows = app.db.all_series().await?;
    let episodes = app.db.episodes(None).await?;
    for show in shows.iter().filter(|s| s.data.monitored) {
        for e in episodes.iter().filter(|e| e.series_id == show.id) {
            let Some(date) = parse(e.data.air_date.as_deref()).filter(|d| inside(*d)) else {
                continue;
            };
            let s = judge.episode(show, e, &today);
            out.push(CalendarEntry {
                date: date.to_string(),
                kind: EntryKind::Episode,
                release: None,
                movie_id: None,
                series_id: Some(show.id),
                episode_id: Some(e.id),
                season: Some(e.data.season),
                episode: Some(e.data.number),
                title: show.data.title.clone(),
                episode_title: e.data.title.clone(),
                year: show.data.year,
                state: state(e.file.is_some(), date, judge.today, s.missing),
                monitored: s.monitored,
            });
        }
    }
    out.sort_by(|a, b| {
        (&a.date, a.title.to_lowercase(), a.season, a.episode).cmp(&(
            &b.date,
            b.title.to_lowercase(),
            b.season,
            b.episode,
        ))
    });
    Ok(out)
}

pub async fn calendar(
    State(app): State<SharedApp>,
    query: Result<Query<CalendarQuery>, QueryRejection>,
) -> Result<Json<CalendarAnswer>, ApiError> {
    let bad = || ApiError::new(StatusCode::BAD_REQUEST, BAD_RANGE);
    let Query(q) = query.map_err(|_| bad())?;
    let (Ok(from), Ok(to)) = (q.from.parse::<Date>(), q.to.parse::<Date>()) else {
        return Err(bad());
    };
    let days = from.until(to).map_err(|_| bad())?.get_days();
    if !(0..=MAX_DAYS).contains(&days) {
        return Err(bad());
    }
    let entries = entries(&app, from, to).await?;
    Ok(Json(CalendarAnswer {
        from: from.to_string(),
        to: to.to_string(),
        note: entries.is_empty().then(|| EMPTY.to_owned()),
        entries,
    }))
}
