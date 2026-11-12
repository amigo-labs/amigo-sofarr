//! What the system page shows about the running server (spec web-ui-shell AC-12): the
//! last run, result and next run of each scheduled job, and the last contact with the
//! download client and each indexer. Kept in memory only: after a restart nothing has
//! run yet.

use std::{collections::HashMap, fmt::Display, sync::Mutex, time::Duration};

use jiff::{SignedDuration, Timestamp};

use crate::{app::App, config::Mode};

/// A job the server schedules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Job {
    Rss,
    Downloads,
    ImportLists,
    Refresh,
    Backup,
}

impl Job {
    pub const ALL: [Self; 5] = [
        Self::Rss,
        Self::Downloads,
        Self::ImportLists,
        Self::Refresh,
        Self::Backup,
    ];

    /// The name in the log and the API.
    pub fn name(self) -> &'static str {
        match self {
            Self::Rss => "rss",
            Self::Downloads => "downloads",
            Self::ImportLists => "import-lists",
            Self::Refresh => "refresh",
            Self::Backup => "backup",
        }
    }

    /// The German name on the system page.
    pub fn label(self) -> &'static str {
        match self {
            Self::Rss => "RSS-Sync",
            Self::Downloads => "Download-Prüfung",
            Self::ImportLists => "Import-Listen",
            Self::Refresh => "Metadaten aktualisieren",
            Self::Backup => "Sicherung",
        }
    }

    /// How often the job runs with this app's configuration; `None` when it is off.
    pub fn interval(self, app: &App) -> Option<Duration> {
        let config = &app.config;
        let live = config.automation.mode == Mode::Live;
        let minutes = |m: u64| Some(Duration::from_secs(m * 60));
        let hours = |h: u64| Some(Duration::from_secs(h * 3600));
        match self {
            Self::Rss if config.rss.interval_minutes > 0 && !app.indexers.is_empty() => {
                minutes(config.rss.interval_minutes)
            }
            Self::Downloads
                if live && app.nzbget.is_some() && config.download.check_interval_minutes > 0 =>
            {
                minutes(config.download.check_interval_minutes)
            }
            Self::ImportLists
                if live && config.tmdb.is_some() && config.import_lists.interval_hours > 0 =>
            {
                hours(config.import_lists.interval_hours)
            }
            Self::Refresh if config.library.refresh_interval_hours > 0 => {
                hours(config.library.refresh_interval_hours)
            }
            Self::Backup if config.backup.interval_hours > 0 => hours(config.backup.interval_hours),
            _ => None,
        }
    }
}

/// What is known about one job.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JobState {
    pub last_run: Option<Timestamp>,
    /// `Err` holds the message of the failure.
    pub result: Option<Result<(), String>>,
    pub next_run: Option<Timestamp>,
}

/// A remote service sofarr talks to.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Peer {
    DownloadClient,
    Indexer(String),
}

/// The last contact with a peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    pub at: Timestamp,
    /// `Err` holds the message of the failure.
    pub result: Result<(), String>,
}

#[derive(Debug, Default)]
pub struct Status {
    jobs: Mutex<HashMap<Job, JobState>>,
    contacts: Mutex<HashMap<Peer, Contact>>,
}

fn add(at: Timestamp, after: Duration) -> Option<Timestamp> {
    SignedDuration::try_from(after)
        .ok()
        .and_then(|d| at.checked_add(d).ok())
}

impl Status {
    /// The job waits for its next run at `now + wait`.
    pub fn job_waits(&self, job: Job, now: Timestamp, wait: Duration) {
        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.entry(job).or_default().next_run = add(now, wait);
        }
    }

    /// The job ran at `at` with `result`.
    pub fn job_ran<E: Display>(&self, job: Job, at: Timestamp, result: &Result<(), E>) {
        if let Ok(mut jobs) = self.jobs.lock() {
            let state = jobs.entry(job).or_default();
            state.last_run = Some(at);
            state.result = Some(result.as_ref().map(|_| ()).map_err(|e| e.to_string()));
        }
    }

    pub fn job(&self, job: Job) -> JobState {
        self.jobs
            .lock()
            .ok()
            .and_then(|jobs| jobs.get(&job).cloned())
            .unwrap_or_default()
    }

    /// Records a contact with `peer` at `at`; any answer the peer gave counts as
    /// answered unless `result` is an error.
    pub fn contact<T, E: Display>(&self, peer: Peer, at: Timestamp, result: &Result<T, E>) {
        if let Ok(mut contacts) = self.contacts.lock() {
            contacts.insert(
                peer,
                Contact {
                    at,
                    result: result.as_ref().map(|_| ()).map_err(|e| e.to_string()),
                },
            );
        }
    }

    pub fn last_contact(&self, peer: &Peer) -> Option<Contact> {
        self.contacts
            .lock()
            .ok()
            .and_then(|contacts| contacts.get(peer).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jobs_and_contacts_keep_the_last_state() {
        let status = Status::default();
        let t0: Timestamp = "2026-11-12T10:00:00Z".parse().expect("t0");
        assert_eq!(status.job(Job::Rss), JobState::default());
        status.job_waits(Job::Rss, t0, Duration::from_secs(900));
        status.job_ran(Job::Rss, t0, &Err::<(), _>("Indexer down"));
        let state = status.job(Job::Rss);
        assert_eq!(state.last_run, Some(t0));
        assert_eq!(state.result, Some(Err("Indexer down".to_owned())));
        assert_eq!(
            state.next_run,
            Some("2026-11-12T10:15:00Z".parse().expect("next"))
        );
        status.job_ran(Job::Rss, t0, &Ok::<(), String>(()));
        assert_eq!(status.job(Job::Rss).result, Some(Ok(())));

        let idx = Peer::Indexer("idx".to_owned());
        assert_eq!(status.last_contact(&idx), None);
        status.contact(idx.clone(), t0, &Err::<(), _>("502"));
        assert_eq!(
            status.last_contact(&idx).map(|c| c.result),
            Some(Err("502".to_owned()))
        );
        status.contact(idx.clone(), t0, &Ok::<u8, String>(1));
        assert_eq!(status.last_contact(&idx).map(|c| c.result), Some(Ok(())));
        assert_eq!(status.last_contact(&Peer::DownloadClient), None);
    }
}
