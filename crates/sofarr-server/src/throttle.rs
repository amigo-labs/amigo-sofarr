//! Login throttling (spec auth AC-09): after 5 failed logins in a row an address waits
//! 1 minute; every failure after a wait doubles the next one, up to 15 minutes.

use std::{collections::HashMap, net::IpAddr};

use jiff::{SignedDuration, Timestamp};

const MAX_FAILURES: u32 = 5;
const FIRST_WAIT_SECONDS: i64 = 60;
const MAX_WAIT_SECONDS: i64 = 15 * 60;
/// An entry is forgotten this long after its last failure or the end of its wait.
const FORGET_SECONDS: i64 = 15 * 60;
const MAX_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, Copy)]
struct Entry {
    failures: u32,
    /// Waits so far; the next one lasts `FIRST_WAIT * 2^waits`.
    waits: u32,
    blocked_until: Option<Timestamp>,
    last_failure: Timestamp,
}

impl Entry {
    /// When the entry may be forgotten.
    fn expires(&self) -> Timestamp {
        let base = self
            .blocked_until
            .map_or(self.last_failure, |b| b.max(self.last_failure));
        base + SignedDuration::from_secs(FORGET_SECONDS)
    }
}

/// Whether a login may be checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    Open,
    /// Wait this many seconds (rounded up).
    Wait(u64),
}

#[derive(Debug, Default)]
pub struct Throttle {
    entries: HashMap<IpAddr, Entry>,
}

impl Throttle {
    pub fn check(&mut self, key: IpAddr, now: Timestamp) -> Gate {
        self.forget(now);
        let Some(until) = self.entries.get(&key).and_then(|e| e.blocked_until) else {
            return Gate::Open;
        };
        if until <= now {
            return Gate::Open;
        }
        let millis = until.duration_since(now).as_millis();
        Gate::Wait(
            u64::try_from(millis.div_euclid(1000) + i128::from(millis % 1000 != 0))
                .unwrap_or(u64::MAX),
        )
    }

    pub fn failed(&mut self, key: IpAddr, now: Timestamp) {
        self.forget(now);
        if !self.entries.contains_key(&key) && self.entries.len() >= MAX_ENTRIES {
            // The oldest entry goes first.
            if let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.last_failure)
                .map(|(k, _)| *k)
            {
                self.entries.remove(&oldest);
            }
        }
        let entry = self.entries.entry(key).or_insert(Entry {
            failures: 0,
            waits: 0,
            blocked_until: None,
            last_failure: now,
        });
        entry.failures += 1;
        entry.last_failure = now;
        if entry.waits > 0 || entry.failures >= MAX_FAILURES {
            let seconds = FIRST_WAIT_SECONDS
                .saturating_mul(1i64 << entry.waits.min(10))
                .min(MAX_WAIT_SECONDS);
            entry.blocked_until = Some(now + SignedDuration::from_secs(seconds));
            entry.waits += 1;
        }
    }

    pub fn succeeded(&mut self, key: IpAddr) {
        self.entries.remove(&key);
    }

    fn forget(&mut self, now: Timestamp) {
        self.entries.retain(|_, e| e.expires() > now);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().expect("ip")
    }

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_second(1_800_000_000 + seconds).expect("ts")
    }

    #[test]
    fn five_failures_then_doubling_waits() {
        let mut t = Throttle::default();
        let a = ip("203.0.113.1");
        for i in 0..4 {
            t.failed(a, at(i));
            assert_eq!(t.check(a, at(i)), Gate::Open);
        }
        t.failed(a, at(4));
        assert_eq!(t.check(a, at(4)), Gate::Wait(60));
        assert_eq!(t.check(a, at(30)), Gate::Wait(34));
        // Another address is not throttled.
        assert_eq!(t.check(ip("203.0.113.2"), at(30)), Gate::Open);
        assert_eq!(t.check(a, at(64)), Gate::Open);
        // Each failure after a wait doubles the next one: 2, 4, 8, then 15 minutes.
        for (start, wait) in [(64, 120), (184, 240), (424, 480), (904, 900), (1804, 900)] {
            t.failed(a, at(start));
            assert_eq!(t.check(a, at(start)), Gate::Wait(wait), "after {start}");
        }
        t.succeeded(a);
        assert_eq!(t.check(a, at(2704)), Gate::Open);
        for i in 0..4 {
            t.failed(a, at(2704 + i));
        }
        assert_eq!(t.check(a, at(2710)), Gate::Open, "the count starts over");
    }

    #[test]
    fn entries_are_forgotten_and_capped() {
        let mut t = Throttle::default();
        let a = ip("203.0.113.1");
        t.failed(a, at(0));
        assert_eq!(t.len(), 1);
        t.check(a, at(15 * 60 + 1));
        assert_eq!(t.len(), 0, "forgotten 15 minutes after the last failure");
        for i in 0..10_001u32 {
            let addr = IpAddr::from(std::net::Ipv4Addr::from(0x0a00_0000 + i));
            t.failed(addr, at(i64::from(i) / 100));
        }
        assert_eq!(t.len(), 10_000);
    }
}
