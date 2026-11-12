//! The TMDB answer cache (spec discover AC-13, AC-14): an answer counts as fresh for its
//! time to live; an older one is kept and served, marked stale, while TMDB cannot be
//! reached.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use jiff::{SignedDuration, Timestamp};
use serde::Serialize;
use sofarr_metadata::MetadataError;

/// Shelves, filtered lists and searches (AC-13).
pub const LIST_TTL: SignedDuration = SignedDuration::from_hours(6);
/// Titles and people (AC-13).
pub const DETAIL_TTL: SignedDuration = SignedDuration::from_hours(24);
/// Answers older than this are dropped, also as stale fallback.
const KEEP: SignedDuration = SignedDuration::from_hours(7 * 24);

struct Entry<V> {
    value: Arc<V>,
    fetched_at: Timestamp,
}

/// Answers of one kind by request key.
pub struct Cache<V> {
    entries: Mutex<HashMap<String, Entry<V>>>,
    ttl: SignedDuration,
    capacity: usize,
}

impl<V> std::fmt::Debug for Cache<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache")
            .field("entries", &self.entries.lock().map(|e| e.len()).ok())
            .field("ttl", &self.ttl)
            .finish_non_exhaustive()
    }
}

/// A cached answer with the time it was fetched.
#[derive(Debug)]
pub struct Cached<V> {
    pub value: Arc<V>,
    pub fetched_at: Timestamp,
    /// TMDB could not be reached and the answer is older than its time to live.
    pub stale: bool,
}

impl<V> Clone for Cached<V> {
    fn clone(&self) -> Self {
        Self {
            value: Arc::clone(&self.value),
            fetched_at: self.fetched_at,
            stale: self.stale,
        }
    }
}

/// Why a TMDB answer is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// TMDB does not know it.
    NotFound,
    /// TMDB cannot be reached (or answers nonsense) and nothing is cached.
    Unreachable,
}

impl<V> Cache<V> {
    pub fn new(ttl: SignedDuration, capacity: usize) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            ttl,
            capacity,
        }
    }

    /// The cached answer, whatever its age.
    pub fn any(&self, key: &str) -> Option<Cached<V>> {
        let entries = self.entries.lock().ok()?;
        entries.get(key).map(|e| Cached {
            value: Arc::clone(&e.value),
            fetched_at: e.fetched_at,
            stale: false,
        })
    }

    fn fresh(&self, key: &str, now: Timestamp) -> Option<Cached<V>> {
        self.any(key)
            .filter(|c| now.duration_since(c.fetched_at) < self.ttl)
    }

    fn put(&self, key: &str, value: V, now: Timestamp) -> Arc<V> {
        let value = Arc::new(value);
        if let Ok(mut entries) = self.entries.lock() {
            entries.retain(|_, e| now.duration_since(e.fetched_at) < KEEP);
            if entries.len() >= self.capacity
                && let Some(oldest) = entries
                    .iter()
                    .min_by_key(|(_, e)| e.fetched_at)
                    .map(|(k, _)| k.clone())
            {
                entries.remove(&oldest);
            }
            entries.insert(
                key.to_owned(),
                Entry {
                    value: Arc::clone(&value),
                    fetched_at: now,
                },
            );
        }
        value
    }

    /// Moves every entry `by` into the past (tests of the time to live).
    #[cfg(test)]
    pub fn age(&self, by: SignedDuration) {
        if let Ok(mut entries) = self.entries.lock() {
            for e in entries.values_mut() {
                e.fetched_at -= by;
            }
        }
    }

    /// The fresh cached answer, else a fetched one (stored), else, while TMDB cannot
    /// be reached, the stale cached one.
    pub async fn load<F, Fut>(
        &self,
        key: &str,
        now: Timestamp,
        fetch: F,
    ) -> Result<Cached<V>, Failure>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<V, MetadataError>>,
    {
        if let Some(cached) = self.fresh(key, now) {
            return Ok(cached);
        }
        match fetch().await {
            Ok(value) => Ok(Cached {
                value: self.put(key, value, now),
                fetched_at: now,
                stale: false,
            }),
            Err(MetadataError::NotFound { .. }) => Err(Failure::NotFound),
            Err(e) => {
                tracing::warn!(key, error = %e, "TMDB request failed");
                self.any(key)
                    .map(|c| Cached { stale: true, ..c })
                    .ok_or(Failure::Unreachable)
            }
        }
    }
}

/// When an answer was fetched, and the German notice while it is stale (AC-13, AC-14).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Freshness {
    /// When TMDB gave the (oldest) answer shown.
    pub fetched_at: String,
    /// TMDB could not be reached; the answer is older than its time to live.
    pub stale: bool,
    /// German notice with the age of the data while stale.
    pub notice: Option<String>,
}

/// German age text: „5 Minuten“, „3 Stunden“, „2 Tagen“.
fn age_text(age: SignedDuration) -> String {
    let minutes = age.as_secs().max(0) / 60;
    let (n, one, many) = if minutes < 60 {
        (minutes, "Minute", "Minuten")
    } else if minutes < 48 * 60 {
        (minutes / 60, "Stunde", "Stunden")
    } else {
        (minutes / (24 * 60), "Tag", "Tagen")
    };
    format!("{n} {}", if n == 1 { one } else { many })
}

impl Freshness {
    /// The freshness of answers combined: the oldest time, stale if any is.
    pub fn of(times: impl IntoIterator<Item = (Timestamp, bool)>, now: Timestamp) -> Self {
        let mut oldest: Option<Timestamp> = None;
        let mut stale = false;
        for (at, s) in times {
            oldest = Some(oldest.map_or(at, |o| o.min(at)));
            stale |= s;
        }
        let at = oldest.unwrap_or(now);
        Self {
            fetched_at: at.to_string(),
            stale,
            notice: stale.then(|| {
                format!(
                    "TMDB ist nicht erreichbar – angezeigt werden zwischengespeicherte Daten von vor {}.",
                    age_text(now.duration_since(at))
                )
            }),
        }
    }

    pub fn one<V>(cached: &Cached<V>, now: Timestamp) -> Self {
        Self::of([(cached.fetched_at, cached.stale)], now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ages_read_in_german() {
        assert_eq!(age_text(SignedDuration::from_mins(1)), "1 Minute");
        assert_eq!(age_text(SignedDuration::from_mins(59)), "59 Minuten");
        assert_eq!(age_text(SignedDuration::from_hours(7)), "7 Stunden");
        assert_eq!(age_text(SignedDuration::from_hours(72)), "3 Tagen");
    }

    #[tokio::test]
    async fn fresh_answers_come_from_the_cache_stale_ones_only_while_tmdb_is_down() {
        let cache: Cache<u32> = Cache::new(LIST_TTL, 2);
        let now = Timestamp::now();
        let ok = |n: u32| async move { Ok::<_, MetadataError>(n) };
        let down = || async { Err::<u32, _>(MetadataError::Decode("down".to_owned())) };
        assert_eq!(*cache.load("a", now, || ok(1)).await.expect("a").value, 1);
        // Within the time to live TMDB is not asked.
        assert_eq!(*cache.load("a", now, || ok(2)).await.expect("a").value, 1);
        cache.age(SignedDuration::from_hours(6));
        let fetched = cache.load("a", now, || ok(3)).await.expect("a");
        assert_eq!((*fetched.value, fetched.stale), (3, false));
        cache.age(SignedDuration::from_hours(7));
        let stale = cache.load("a", now, down).await.expect("stale");
        assert_eq!((*stale.value, stale.stale), (3, true));
        assert_eq!(
            cache.load("b", now, down).await.expect_err("nothing"),
            Failure::Unreachable
        );
        // The capacity drops the oldest answer.
        cache.load("b", now, || ok(4)).await.expect("b");
        cache.load("c", now, || ok(5)).await.expect("c");
        assert!(cache.any("a").is_none());
    }
}
