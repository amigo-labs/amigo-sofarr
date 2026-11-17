//! The re-encode job (spec reencode-job): rounds around the clock through the candidate
//! list, the largest due file first.

use std::{collections::HashMap, sync::Arc, time::Duration};

use jiff::{SignedDuration, Timestamp};
use serde::Serialize;
use sofarr_db::{FileRef, ReencodeAttempt, ReencodeOutcome};

use super::{
    Candidate,
    run::{self, Report, Runner, Target, Tools},
};
use crate::{
    app::{App, SharedApp},
    config::Mode,
    status::Job,
};

/// How long the job waits when nothing is due, or ffmpeg is missing.
pub const LOOK: Duration = Duration::from_secs(60 * 60);
/// How often the job looks whether the encode slot is free again.
pub const SLOT_POLL: Duration = Duration::from_secs(60);

/// How long a file rests after the first or second failure of its streak.
pub const REST_AFTER_FAILURE: SignedDuration = SignedDuration::from_hours(7 * 24);
/// How long a file rests after a refusal.
pub const REST_AFTER_REFUSAL: SignedDuration = SignedDuration::from_hours(24);
/// The reason of a refusal because another encode holds the slot (spec reencode-file).
pub const SLOT_TAKEN: &str = "Es läuft bereits ein Encode";
/// Failures with a reason starting so (interrupted, stopped with sofarr) say nothing
/// about the file.
const INTERRUPTED: &str = "abgebrochen";

/// Whether a candidate of this size with these attempts (newest first) is due now.
pub fn is_due(size: u64, attempts: &[ReencodeAttempt], now: Timestamp) -> bool {
    let ended = |a: &ReencodeAttempt| a.finished_at.unwrap_or(a.started_at);
    // Settled: an attempt at the size the file has now ended replaced or without gain.
    let settled = attempts.iter().any(|a| match a.outcome {
        Some(ReencodeOutcome::Replaced) => a.size_after == Some(size),
        Some(ReencodeOutcome::NoGain) => a.size_before == size,
        _ => false,
    });
    if settled {
        return false;
    }
    // A refusal rests a day, unless another encode only held the slot.
    if let Some(latest) = attempts.iter().find(|a| a.outcome.is_some())
        && latest.outcome == Some(ReencodeOutcome::Refused)
        && !latest
            .reason
            .as_deref()
            .is_some_and(|r| r.starts_with(SLOT_TAKEN))
        && ended(latest) > now - REST_AFTER_REFUSAL
    {
        return false;
    }
    // The failure streak at this size, newest first; interruptions and refusals do not count.
    let mut streak = 0;
    let mut last_failure = None;
    for attempt in attempts.iter().filter(|a| a.size_before == size) {
        match attempt.outcome {
            Some(ReencodeOutcome::Replaced | ReencodeOutcome::NoGain) => break,
            Some(ReencodeOutcome::Failed)
                if !attempt
                    .reason
                    .as_deref()
                    .is_some_and(|r| r.starts_with(INTERRUPTED)) =>
            {
                streak += 1;
                last_failure.get_or_insert(ended(attempt));
            }
            _ => {}
        }
    }
    match (streak, last_failure) {
        (0, _) => true,
        (1 | 2, Some(at)) => at <= now - REST_AFTER_FAILURE,
        _ => false,
    }
}

/// The due candidates in the order of the candidate list.
pub fn due<'a>(
    candidates: &'a [Candidate],
    attempts: &[ReencodeAttempt],
    now: Timestamp,
) -> Vec<&'a Candidate> {
    let mut by_file: HashMap<FileRef, Vec<ReencodeAttempt>> = HashMap::new();
    for attempt in attempts {
        if let Some(file) = attempt.file {
            by_file.entry(file).or_default().push(attempt.clone());
        }
    }
    candidates
        .iter()
        .filter(|c| is_due(c.size, by_file.get(&c.file).map_or(&[], Vec::as_slice), now))
        .collect()
}

/// How a round ended.
#[derive(Debug)]
pub enum RoundEnd {
    /// One attempt ran to its end.
    Encoded(Report),
    /// Another encode holds the slot.
    SlotHeld,
    /// No due file (left).
    NothingDue,
}

/// The due files now, from the database alone (AC-10).
pub async fn due_files(app: &App, now: Timestamp) -> anyhow::Result<Vec<Candidate>> {
    let thresholds = app.config.reencode.thresholds();
    let list = super::candidates(&app.db, &thresholds).await?;
    let attempts = app.db.all_reencodes().await?;
    Ok(due(&list.candidates, &attempts, now)
        .into_iter()
        .cloned()
        .collect())
}

fn target_of(candidate: &Candidate) -> Option<Target> {
    match (candidate.movie_id, candidate.episode_ids.first()) {
        (Some(id), _) => Some(Target::Movie(id)),
        (None, Some(id)) => Some(Target::Episode(*id)),
        (None, None) => None,
    }
}

/// One round (AC-01 … AC-03): the due files in the order of the candidate list, until an
/// attempt is not refused.
pub async fn round(app: &App, tools: &Tools, now: Timestamp) -> anyhow::Result<RoundEnd> {
    for candidate in &due_files(app, now).await? {
        if run::running(&app.reencode.lock).is_some() {
            return Ok(RoundEnd::SlotHeld);
        }
        let Some(target) = target_of(candidate) else {
            continue;
        };
        match run::prepare(app, tools, target, Runner::Server).await? {
            Ok(prepared) => return Ok(RoundEnd::Encoded(run::run(app, tools, prepared).await?)),
            Err(refusal) if refusal.reason.starts_with(SLOT_TAKEN) => {
                return Ok(RoundEnd::SlotHeld);
            }
            Err(refusal) => {
                tracing::info!(title = %candidate.title, reason = %refusal.reason, "re-encode job: refused, next file");
            }
        }
    }
    Ok(RoundEnd::NothingDue)
}

/// Checks ffmpeg for the job and keeps the result for the system page.
async fn check_encoder(app: &App, tools: &Tools) -> Result<(), String> {
    let encoder = Arc::clone(&tools.encoder);
    let result = tokio::task::spawn_blocking(move || encoder.check())
        .await
        .unwrap_or_else(|e| Err(format!("Prüfung von ffmpeg fehlgeschlagen: {e}")));
    if let Ok(mut stored) = app.reencode.ffmpeg.lock() {
        *stored = Some(result.clone());
    }
    result
}

/// Starts the job (AC-01, AC-04, AC-05, AC-07): in live mode it first repairs interrupted
/// attempts; then, when switched on, it runs rounds one after the other.
pub fn spawn_schedule(app: &SharedApp) -> Option<tokio::task::JoinHandle<()>> {
    if app.config.automation.mode != Mode::Live {
        return None;
    }
    let app = Arc::clone(app);
    Some(tokio::spawn(async move {
        match run::repair_all(&app).await {
            Ok(0) => {}
            Ok(n) => tracing::info!(attempts = n, "interrupted re-encodes repaired"),
            Err(e) => tracing::warn!(
                error = format!("{e:#}"),
                "interrupted re-encodes not repaired"
            ),
        }
        if !app.config.reencode.enabled {
            return;
        }
        loop {
            if app
                .reencode
                .stopping
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                break;
            }
            let wait = next_wait(&app).await;
            if !wait.is_zero() {
                tokio::time::sleep(wait).await;
            }
        }
    }))
}

/// Runs what is to do now and answers how long to wait before the next look; the system
/// page shows that look as the job's next run (AC-08).
pub async fn next_wait(app: &App) -> Duration {
    let wait = look(app).await;
    if !wait.is_zero() {
        app.status.job_waits(Job::Reencode, Timestamp::now(), wait);
    }
    wait
}

async fn look(app: &App) -> Duration {
    let tools = run::tools(app);
    if let Err(reason) = check_encoder(app, &tools).await {
        tracing::warn!(%reason, "re-encode job: ffmpeg missing");
        return LOOK;
    }
    if run::running(&app.reencode.lock).is_some() {
        return SLOT_POLL;
    }
    let at = Timestamp::now();
    // While the round encodes, the job has no next look.
    app.status.job_busy(Job::Reencode);
    let result = round(app, &tools, at).await;
    match result {
        Ok(RoundEnd::Encoded(report)) => {
            tracing::info!(
                title = %report.title,
                outcome = report.outcome.as_str(),
                "re-encode job: round done"
            );
            app.status.job_ran(Job::Reencode, at, &Ok::<(), String>(()));
            Duration::ZERO
        }
        Ok(RoundEnd::SlotHeld) => SLOT_POLL,
        Ok(RoundEnd::NothingDue) => {
            app.status.job_ran(Job::Reencode, at, &Ok::<(), String>(()));
            LOOK
        }
        Err(e) => {
            tracing::warn!(error = format!("{e:#}"), "re-encode round failed");
            app.status
                .job_ran(Job::Reencode, at, &Err(format!("{e:#}")));
            LOOK
        }
    }
}

/// Whether the job encodes, or why not, for the system page (AC-08).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ReencodeSupport {
    pub active: bool,
    /// Why not (German).
    pub message: Option<String>,
    /// The title being encoded (by the job, an API start or the CLI).
    pub running: Option<String>,
}

pub fn support_answer(app: &App) -> ReencodeSupport {
    let reason = if !app.config.reencode.enabled {
        Some("in der Konfiguration ausgeschaltet")
    } else if app.config.automation.mode == Mode::Shadow {
        Some("im Shadow-Mode")
    } else if app
        .reencode
        .ffmpeg
        .lock()
        .ok()
        .is_some_and(|checked| matches!(*checked, Some(Err(_))))
    {
        Some("ffmpeg mit libx265 nicht gefunden")
    } else {
        None
    };
    ReencodeSupport {
        active: reason.is_none(),
        message: reason.map(str::to_owned),
        running: run::running(&app.reencode.lock).map(|r| r.title),
    }
}

#[cfg(test)]
mod tests;
