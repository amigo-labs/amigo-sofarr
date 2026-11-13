//! Background detail loads (spec discover, technical notes): titles shown without a
//! cached detail get it loaded for their IMDb id (and a series for its TVDB id), at most
//! 40 TMDB requests per second, a few at a time.

use std::{
    collections::{HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

use jiff::Timestamp;
use sofarr_metadata::browse::Kind;
use tokio::{sync::Semaphore, time::Instant};

use crate::app::SharedApp;

/// TMDB allows about 50 requests per second; background loads stay below.
pub const PER_SECOND: u32 = 40;
/// Requests in flight at once.
const PARALLEL: usize = 8;

#[derive(Debug, Default)]
struct Waiting {
    queue: VecDeque<(Kind, i64)>,
    /// Queued or in flight.
    known: HashSet<(Kind, i64)>,
    running: bool,
}

/// The titles waiting for their detail.
#[derive(Debug, Default)]
pub struct Queue {
    waiting: Mutex<Waiting>,
    /// When the next request may start.
    next_slot: tokio::sync::Mutex<Option<Instant>>,
}

impl Queue {
    /// Nothing is waiting or loading.
    #[cfg(test)]
    pub fn idle(&self) -> bool {
        self.waiting
            .lock()
            .map(|w| w.known.is_empty() && !w.running)
            .unwrap_or(false)
    }

    /// Waits for the next free request slot.
    async fn pace(&self) {
        let mut slot = self.next_slot.lock().await;
        let now = Instant::now();
        let at = slot.map_or(now, |s| s.max(now));
        tokio::time::sleep_until(at).await;
        *slot = Some(at + Duration::from_secs(1) / PER_SECOND);
    }

    fn done(&self, item: (Kind, i64)) {
        if let Ok(mut w) = self.waiting.lock() {
            w.known.remove(&item);
        }
    }

    fn next(&self) -> Option<(Kind, i64)> {
        let mut w = self.waiting.lock().ok()?;
        let next = w.queue.pop_front();
        if next.is_none() {
            w.running = false;
        }
        next
    }
}

/// Queues titles for a detail load; starts the loader when it is not running.
pub fn enqueue(app: &SharedApp, items: Vec<(Kind, i64)>) {
    if items.is_empty() || app.config.tmdb.is_none() {
        return;
    }
    let start = match app.discover.queue.waiting.lock() {
        Ok(mut w) => {
            for item in items {
                if w.known.insert(item) {
                    w.queue.push_back(item);
                }
            }
            let start = !w.running && !w.queue.is_empty();
            w.running |= start;
            start
        }
        Err(_) => false,
    };
    if start {
        tokio::spawn(run(Arc::clone(app)));
    }
}

async fn run(app: SharedApp) {
    let permits = Arc::new(Semaphore::new(PARALLEL));
    while let Some(item) = app.discover.queue.next() {
        app.discover.queue.pace().await;
        let Ok(permit) = Arc::clone(&permits).acquire_owned().await else {
            break;
        };
        let app = Arc::clone(&app);
        tokio::spawn(async move {
            let (kind, id) = item;
            if super::titles::detail(&app, kind, id, Timestamp::now())
                .await
                .is_err()
            {
                tracing::debug!(tmdb = id, "background detail not loaded");
            }
            app.discover.queue.done(item);
            drop(permit);
        });
    }
}
