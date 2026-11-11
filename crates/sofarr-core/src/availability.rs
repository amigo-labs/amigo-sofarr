//! When a movie counts as available: its `minimumAvailability`, release dates and the
//! global `availabilityDelay`. Only automatic grabs (RSS, search on add) wait for it.

use jiff::{ToSpan, civil::Date};

/// The release stage a movie must have reached (`minimumAvailability`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    Announced,
    InCinemas,
    #[default]
    Released,
}

impl Stage {
    /// Reads Radarr's `minimumAvailability` or `status`, or TMDB's `status`. TMDB's
    /// `Released` only says the movie was shown somewhere (a theatrical release), not
    /// that it is out for home viewing, so it counts as in cinemas.
    pub fn from_api(value: &str) -> Option<Self> {
        Some(match value {
            "tba" | "announced" | "Rumored" | "Planned" | "In Production" | "Post Production" => {
                Self::Announced
            }
            "inCinemas" | "Released" => Self::InCinemas,
            "released" | "preDB" => Self::Released,
            _ => return None,
        })
    }

    pub fn as_api(self) -> &'static str {
        match self {
            Self::Announced => "announced",
            Self::InCinemas => "inCinemas",
            Self::Released => "released",
        }
    }
}

/// What is known about a movie's release.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReleaseInfo {
    /// The stage the source reports, used when the dates are unknown.
    pub status: Option<Stage>,
    pub in_cinemas: Option<Date>,
    pub digital: Option<Date>,
    pub physical: Option<Date>,
}

impl ReleaseInfo {
    /// The home release: the earlier of digital and physical release.
    pub fn home_release(&self) -> Option<Date> {
        match (self.digital, self.physical) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

/// Parses `YYYY-MM-DD`, also the date part of an RFC 3339 timestamp.
pub fn parse_date(raw: &str) -> Option<Date> {
    raw.get(..10)?.parse().ok()
}

/// Whether a movie with `minimum` availability counts as available on `today`.
/// A known date wins over the reported status; the delay shifts every date. Without
/// any information (neither status nor dates) a movie counts as available.
pub fn is_available(minimum: Stage, info: &ReleaseInfo, delay_days: i32, today: Date) -> bool {
    if *info == ReleaseInfo::default() {
        return true;
    }
    let reached = |date: Option<Date>| {
        date.and_then(|d| d.checked_add(i64::from(delay_days).days()).ok())
            .is_some_and(|d| d <= today)
    };
    let released = match info.home_release() {
        Some(_) => reached(info.home_release()),
        None => info.status == Some(Stage::Released),
    };
    match minimum {
        Stage::Announced => true,
        Stage::Released => released,
        Stage::InCinemas => {
            released
                || match info.in_cinemas {
                    Some(_) => reached(info.in_cinemas),
                    None => info.status >= Some(Stage::InCinemas),
                }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().expect("date")
    }

    #[test]
    fn stages() {
        assert_eq!(Stage::from_api("inCinemas"), Some(Stage::InCinemas));
        assert_eq!(Stage::from_api("Post Production"), Some(Stage::Announced));
        assert_eq!(Stage::from_api("Released"), Some(Stage::InCinemas));
        assert_eq!(Stage::from_api("released"), Some(Stage::Released));
        assert_eq!(Stage::from_api("whatever"), None);
        assert_eq!(parse_date("2026-11-01T00:00:00Z"), Some(d("2026-11-01")));
    }

    #[test]
    fn released_needs_home_release() {
        let today = d("2026-11-11");
        let info = ReleaseInfo {
            status: Some(Stage::InCinemas),
            in_cinemas: Some(d("2026-09-01")),
            digital: Some(d("2026-11-20")),
            physical: Some(d("2026-11-10")),
        };
        assert!(is_available(Stage::Released, &info, 0, today));
        assert!(!is_available(Stage::Released, &info, 2, today));
        assert!(is_available(Stage::Released, &info, -10, d("2026-11-01")));
        assert!(is_available(Stage::InCinemas, &info, 2, today));
        assert!(is_available(
            Stage::Announced,
            &ReleaseInfo::default(),
            0,
            today
        ));
    }

    #[test]
    fn status_without_dates() {
        let today = d("2026-11-11");
        let status = |s| ReleaseInfo {
            status: Some(s),
            ..ReleaseInfo::default()
        };
        assert!(is_available(
            Stage::Released,
            &status(Stage::Released),
            0,
            today
        ));
        assert!(!is_available(
            Stage::Released,
            &status(Stage::InCinemas),
            0,
            today
        ));
        assert!(is_available(
            Stage::InCinemas,
            &status(Stage::InCinemas),
            0,
            today
        ));
        assert!(!is_available(
            Stage::InCinemas,
            &status(Stage::Announced),
            0,
            today
        ));
        assert!(is_available(
            Stage::Released,
            &ReleaseInfo::default(),
            0,
            today
        ));
    }
}
