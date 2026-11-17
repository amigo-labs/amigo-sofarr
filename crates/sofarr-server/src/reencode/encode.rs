//! Starting a re-encode by CLI and API (spec reencode-file AC-01, AC-02).

use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use sofarr_db::{Db, ReencodeAttempt, ReencodeOutcome};

use super::{
    gigabytes,
    run::{self, Prepared, Refusal, Report, Runner, Running, Target},
};
use crate::{
    api::ApiError,
    app::{App, SharedApp},
};

#[cfg(test)]
mod tests;

/// The target of `amigo-sofarr reencode`: a movie id, or a series with season and episode.
pub async fn cli_target(
    db: &Db,
    movie_id: Option<i64>,
    series: Option<i64>,
    season: Option<u16>,
    episode: Option<u16>,
) -> anyhow::Result<Target> {
    match (movie_id, series, season, episode) {
        (Some(id), None, None, None) => Ok(Target::Movie(id)),
        (None, Some(series_id), Some(season), Some(number)) => db
            .episodes(Some(series_id))
            .await?
            .into_iter()
            .find(|e| e.data.season == season && e.data.number == number)
            .map(|e| Target::Episode(e.id))
            .ok_or_else(|| {
                anyhow::anyhow!("Serie {series_id} hat keine Episode S{season:02}E{number:02}")
            }),
        _ => anyhow::bail!(
            "Ziel fehlt: eine Film-ID oder --series <ID> --season <n> --episode <n> angeben"
        ),
    }
}

/// The line the CLI prints when an encode starts.
pub fn start_line(prepared: &Prepared) -> String {
    format!(
        "Encodiere {} ({} GB): {}",
        prepared.title(),
        gigabytes(prepared.size_before()),
        prepared.path().display()
    )
}

/// The line the CLI prints with the outcome.
pub fn outcome_line(report: &Report) -> String {
    let reason = report.reason.as_deref().unwrap_or("");
    match report.outcome {
        ReencodeOutcome::Replaced => {
            let after = report.size_after.unwrap_or(0);
            let gain = report.size_before.saturating_sub(after) * 100 / report.size_before.max(1);
            let mut line = format!(
                "Ersetzt: {} GB → {} GB ({gain} % kleiner)",
                gigabytes(report.size_before),
                gigabytes(after)
            );
            if !reason.is_empty() {
                line.push_str(&format!(" – {reason}"));
            }
            line
        }
        ReencodeOutcome::NoGain => format!("Kein Gewinn, das Original bleibt: {reason}"),
        ReencodeOutcome::Failed => format!("Fehlgeschlagen: {reason}"),
        ReencodeOutcome::Refused => format!("Abgelehnt: {reason}"),
    }
}

/// `amigo-sofarr reencode`: runs one encode in the foreground and prints its start and
/// outcome; `replaced` and `no_gain` succeed, everything else is an error.
pub async fn cli(app: &App, target: Target, print: &mut dyn FnMut(&str)) -> anyhow::Result<()> {
    let tools = run::tools(app);
    let prepared = match run::prepare(app, &tools, target, Runner::Cli).await? {
        Ok(prepared) => prepared,
        Err(refusal) => anyhow::bail!("Abgelehnt: {}", refusal.reason),
    };
    print(&start_line(&prepared));
    let report = run::run(app, &tools, prepared).await?;
    let line = outcome_line(&report);
    match report.outcome {
        ReencodeOutcome::Replaced | ReencodeOutcome::NoGain => {
            print(&line);
            Ok(())
        }
        ReencodeOutcome::Failed | ReencodeOutcome::Refused => anyhow::bail!(line),
    }
}

/// `POST /api/reencode/encode`: exactly one of the two ids.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncodeRequest {
    pub movie_id: Option<i64>,
    pub episode_id: Option<i64>,
}

/// The answer of a started encode.
#[derive(Debug, Serialize)]
pub struct Started {
    pub title: String,
    pub path: String,
}

/// A recorded attempt as the API shows it.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AttemptView {
    pub title: String,
    pub path: String,
    /// `server` or `cli`.
    pub runner: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    /// `replaced`, `no_gain`, `failed`, `refused`; `null` while it runs.
    pub outcome: Option<&'static str>,
    pub reason: Option<String>,
    pub size_before: u64,
    pub size_after: Option<u64>,
}

impl From<ReencodeAttempt> for AttemptView {
    fn from(a: ReencodeAttempt) -> Self {
        Self {
            title: a.title,
            path: a.path,
            runner: a.runner,
            started_at: a.started_at.to_string(),
            finished_at: a.finished_at.map(|t| t.to_string()),
            outcome: a.outcome.map(ReencodeOutcome::as_str),
            reason: a.reason,
            size_before: a.size_before,
            size_after: a.size_after,
        }
    }
}

/// `GET /api/reencode/encode`.
#[derive(Debug, Serialize)]
pub struct EncodeState {
    pub running: Option<Running>,
    pub last: Option<AttemptView>,
}

/// `POST /api/reencode/encode`: checks at once, then encodes in the background.
pub async fn start(
    State(app): State<SharedApp>,
    Json(request): Json<EncodeRequest>,
) -> Result<(StatusCode, Json<Started>), ApiError> {
    let target = match (request.movie_id, request.episode_id) {
        (Some(id), None) => Target::Movie(id),
        (None, Some(id)) => Target::Episode(id),
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Genau eine von movie_id und episode_id angeben",
            ));
        }
    };
    let tools = run::tools(&app);
    let prepared = match run::prepare(&app, &tools, target, Runner::Server).await? {
        Ok(prepared) => prepared,
        Err(Refusal { not_found, reason }) => {
            let status = if not_found {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::CONFLICT
            };
            return Err(ApiError::new(status, reason));
        }
    };
    let started = Started {
        title: prepared.title().to_owned(),
        path: prepared.path().to_string_lossy().into_owned(),
    };
    let background = Arc::clone(&app);
    tokio::spawn(async move {
        if let Err(e) = run::run(&background, &tools, prepared).await {
            tracing::error!(error = format!("{e:#}"), "re-encode failed");
        }
    });
    Ok((StatusCode::ACCEPTED, Json(started)))
}

/// `GET /api/reencode/encode`: the running encode (server or CLI) and the latest attempt.
pub async fn state(State(app): State<SharedApp>) -> Result<Json<EncodeState>, ApiError> {
    Ok(Json(EncodeState {
        running: run::running(&app.reencode.lock),
        last: app.db.last_reencode().await?.map(AttemptView::from),
    }))
}
