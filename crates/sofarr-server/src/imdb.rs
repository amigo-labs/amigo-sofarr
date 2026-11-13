//! IMDb ratings (spec imdb-ratings AC-01..AC-03, AC-09): the official ratings dataset,
//! loaded once a day as a whole, retried every hour after a failure.

use std::io::{BufRead, BufReader};

use anyhow::Context;
use flate2::bufread::GzDecoder;
use jiff::{SignedDuration, Timestamp};
use sofarr_db::RatingRow;

use crate::{
    app::{App, SharedApp},
    status::Job,
};

/// IMDb's attribution, shown wherever ratings are (AC-08).
pub const ATTRIBUTION: &str =
    "Information courtesy of IMDb (https://www.imdb.com). Used with permission.";

/// A first load needs at least this many titles (AC-02).
pub const FIRST_LOAD_MIN: u64 = 100_000;
/// Rows handed to the database at once.
const BATCH: usize = 20_000;

const DAY: SignedDuration = SignedDuration::from_hours(24);
const HOUR: SignedDuration = SignedDuration::from_hours(1);

/// Splits one dataset line into a row.
fn row(line: &str, columns: (usize, usize, usize)) -> Option<RatingRow> {
    let fields: Vec<&str> = line.split('\t').collect();
    let id = fields.get(columns.0)?.trim();
    if !id.starts_with("tt") {
        return None;
    }
    let rating: f32 = fields.get(columns.1)?.trim().parse().ok()?;
    let votes: u32 = fields.get(columns.2)?.trim().parse().ok()?;
    (0.0..=10.0)
        .contains(&rating)
        .then(|| (id.to_owned(), rating, votes))
}

/// Decompresses and parses the dataset, handing batches to `send`; stops when `send`
/// says the receiver is gone. Any damage is an error.
fn parse(gz: &[u8], mut send: impl FnMut(Vec<RatingRow>) -> bool) -> anyhow::Result<u64> {
    let mut lines = BufReader::new(GzDecoder::new(gz)).lines();
    let header = lines
        .next()
        .context("the dataset is empty")?
        .context("the dataset is no readable gzip file")?;
    let names: Vec<&str> = header.split('\t').map(str::trim).collect();
    let column = |name: &str| {
        names
            .iter()
            .position(|c| *c == name)
            .with_context(|| format!("the dataset has no column {name}"))
    };
    let columns = (
        column("tconst")?,
        column("averageRating")?,
        column("numVotes")?,
    );
    let mut batch = Vec::with_capacity(BATCH);
    let mut count = 0u64;
    for (n, line) in lines.enumerate() {
        let line = line.context("the dataset is damaged or incomplete")?;
        if line.is_empty() {
            continue;
        }
        let parsed = row(&line, columns)
            .with_context(|| format!("line {} of the dataset is damaged", n + 2))?;
        batch.push(parsed);
        count += 1;
        if batch.len() == BATCH && !send(std::mem::replace(&mut batch, Vec::with_capacity(BATCH))) {
            anyhow::bail!("the load was stopped");
        }
    }
    if !batch.is_empty() && !send(batch) {
        anyhow::bail!("the load was stopped");
    }
    Ok(count)
}

/// Downloads the dataset and replaces the ratings with it as a whole; returns the number
/// of titles. On any failure the previous ratings stay (AC-02, AC-03).
pub async fn load(app: &App, now: Timestamp) -> anyhow::Result<u64> {
    let url = &app.config.imdb.url;
    let gz = app
        .http
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(reqwest::Error::without_url)
        .context("IMDb dataset not downloadable")?
        .bytes()
        .await
        .map_err(reqwest::Error::without_url)
        .context("IMDb dataset not downloadable")?;
    let previous = app.db.imdb_rating_count().await?;
    let mut load = app.db.begin_imdb_load().await?;
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<Vec<RatingRow>>(2);
    let parser = tokio::task::spawn_blocking(move || {
        parse(&gz, |batch| sender.blocking_send(batch).is_ok())
    });
    let mut stored = Ok(());
    while let Some(batch) = receiver.recv().await {
        stored = load.add(&batch).await;
        if stored.is_err() {
            break;
        }
    }
    // Ends the parser if storing stopped early.
    drop(receiver);
    let parsed = parser.await.context("the dataset parser stopped")?;
    let checked = stored
        .map_err(anyhow::Error::from)
        .and(parsed)
        .and_then(|count| {
            let minimum = if previous == 0 {
                FIRST_LOAD_MIN
            } else {
                previous.div_ceil(2)
            };
            anyhow::ensure!(
                count >= minimum,
                "the dataset has only {count} titles (at least {minimum} expected)"
            );
            Ok(count)
        });
    match checked {
        Ok(_) => Ok(load.finish(now).await?),
        Err(e) => {
            load.abandon().await?;
            Err(e)
        }
    }
}

/// When to try the next load: a day after the last success, or an hour after a later
/// failure; never in the past.
pub fn next_attempt(
    loaded_at: Option<Timestamp>,
    failed_at: Option<Timestamp>,
    now: Timestamp,
) -> Timestamp {
    let after_success = loaded_at.and_then(|t| t.checked_add(DAY).ok());
    let after_failure = failed_at
        .filter(|f| loaded_at.is_none_or(|l| *f > l))
        .and_then(|t| t.checked_add(HOUR).ok());
    after_failure.or(after_success).map_or(now, |t| t.max(now))
}

/// What the UI shows next to ratings: whether they are on, the date of the data and
/// IMDb's attribution (AC-08, AC-09).
#[derive(Debug, serde::Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ImdbInfo {
    pub enabled: bool,
    /// When the current ratings data was loaded; `null` before the first load.
    pub loaded_at: Option<String>,
    pub titles: u32,
    pub attribution: String,
    pub link: String,
}

/// `GET /api/imdb`.
pub async fn info(
    axum::extract::State(app): axum::extract::State<SharedApp>,
) -> Result<axum::Json<ImdbInfo>, crate::api::ApiError> {
    let enabled = app.config.imdb.enabled;
    let (loaded_at, titles) = if enabled {
        (
            app.db.imdb_loaded_at().await?,
            app.db.imdb_rating_count().await?,
        )
    } else {
        (None, 0)
    };
    Ok(axum::Json(ImdbInfo {
        enabled,
        loaded_at: loaded_at.map(|t| t.to_string()),
        titles: u32::try_from(titles).unwrap_or(u32::MAX),
        attribution: ATTRIBUTION.to_owned(),
        link: "https://www.imdb.com".to_owned(),
    }))
}

/// Starts the daily load; `None` when `[imdb] enabled = false` (AC-09).
pub fn spawn_schedule(app: &SharedApp) -> Option<tokio::task::JoinHandle<()>> {
    if !app.config.imdb.enabled {
        return None;
    }
    let app = std::sync::Arc::clone(app);
    Some(tokio::spawn(async move {
        let mut failed_at = None;
        loop {
            let now = Timestamp::now();
            let loaded_at = app.db.imdb_loaded_at().await.ok().flatten();
            let next = next_attempt(loaded_at, failed_at, now);
            let wait = std::time::Duration::try_from(next.duration_since(now)).unwrap_or_default();
            app.status.job_waits(Job::Imdb, now, wait);
            tokio::time::sleep(wait).await;
            let at = Timestamp::now();
            let result = load(&app, at).await;
            match &result {
                Ok(titles) => {
                    tracing::info!(titles, "IMDb ratings loaded");
                    failed_at = None;
                }
                Err(e) => {
                    tracing::warn!(error = format!("{e:#}"), "IMDb ratings not loaded");
                    failed_at = Some(at);
                }
            }
            app.status.job_ran(
                Job::Imdb,
                at,
                &result.map(|_| ()).map_err(|e| format!("{e:#}")),
            );
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_checked() {
        let cols = (0, 1, 2);
        assert_eq!(
            row("tt0000001\t5.7\t2100", cols),
            Some(("tt0000001".to_owned(), 5.7, 2100))
        );
        assert_eq!(row("nm0000001\t5.7\t2100", cols), None);
        assert_eq!(row("tt0000001\tfünf\t2100", cols), None);
        assert_eq!(row("tt0000001\t11.0\t2100", cols), None);
        assert_eq!(row("tt0000001\t5.7", cols), None);
        assert_eq!(row("tt0000001\t5.7\t-1", cols), None);
    }
}
