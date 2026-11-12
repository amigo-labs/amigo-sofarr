//! How the library pages judge an item (spec web-ui-library, terms): missing, cutoff
//! unmet, available, the release stage of a movie and the counts of a series.

use jiff::civil::Date;
use serde::Serialize;
use sofarr_core::{
    QualityProfile,
    availability::{self, ReleaseInfo, Stage},
    profile::ProfileItem,
};
use sofarr_db::{Episode, Movie, MovieFile, Series};

use crate::library::Rules;

/// Today as the pages see it (UTC, like the rest of sofarr).
pub fn today(now: jiff::Timestamp) -> Date {
    now.to_zoned(jiff::tz::TimeZone::UTC).date()
}

/// The decision rules of one media kind, if imported; without them nothing is below a
/// cutoff and movies count as available without delay.
pub struct Judge {
    pub rules: Option<Rules>,
    pub today: Date,
}

/// Release stage of a movie as the pages show it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ReleaseStage {
    Announced,
    InCinemas,
    Released,
}

/// What the pages say about a movie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MovieState {
    pub available: bool,
    pub missing: bool,
    pub cutoff_unmet: bool,
}

/// What the pages say about an episode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpisodeState {
    /// The episode and its series are monitored.
    pub monitored: bool,
    pub aired: bool,
    pub missing: bool,
    pub cutoff_unmet: bool,
}

/// Counts of a series.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SeriesCounts {
    pub episodes: u32,
    pub monitored: u32,
    pub files: u32,
    pub missing: u32,
    pub cutoff_unmet: u32,
    /// Size of its files (a multi-episode file counts once).
    pub size: u64,
}

impl SeriesCounts {
    /// None of its monitored, aired episodes is missing.
    pub fn complete(&self) -> bool {
        self.missing == 0
    }
}

fn date(raw: Option<&str>) -> Option<Date> {
    raw.and_then(availability::parse_date)
}

fn release_info(movie: &Movie) -> ReleaseInfo {
    let a = &movie.availability;
    ReleaseInfo {
        status: a.status.as_deref().and_then(Stage::from_api),
        in_cinemas: date(a.in_cinemas.as_deref()),
        digital: date(a.digital_release.as_deref()),
        physical: date(a.physical_release.as_deref()),
    }
}

/// The name of the cutoff a profile asks for.
pub fn cutoff_name(rules: &Rules, profile: &QualityProfile) -> Option<String> {
    let item = profile.items.iter().find(|i| i.id() == profile.cutoff)?;
    Some(match item {
        ProfileItem::Quality { quality, .. } => rules.catalog.by_id(*quality)?.name.clone(),
        ProfileItem::Group { name, .. } => name.clone(),
    })
}

impl Judge {
    pub fn new(rules: Option<Rules>, today: Date) -> Self {
        Self { rules, today }
    }

    /// Today as `YYYY-MM-DD`, for comparing air dates.
    pub fn today_text(&self) -> String {
        self.today.to_string()
    }

    pub fn profile(&self, id: i64) -> Option<&QualityProfile> {
        self.rules.as_ref()?.profile(id)
    }

    pub fn profile_name(&self, id: i64) -> String {
        self.profile(id)
            .map_or_else(|| format!("Profil {id}"), |p| p.name.clone())
    }

    /// Whether a file is below the cutoff of its profile; unknown profiles count as met.
    pub fn below_cutoff(&self, profile_id: i64, file: &MovieFile) -> bool {
        let Some(rules) = &self.rules else {
            return false;
        };
        rules.profile(profile_id).is_some_and(|p| {
            !p.cutoff_met(rules.quality_id(&file.quality), file.custom_format_score)
        })
    }

    /// Why a file misses its cutoff, e.g. `WEBDL-1080p statt Bluray-1080p`.
    pub fn cutoff_reason(&self, profile_id: i64, file: &MovieFile) -> String {
        let cutoff = self.rules.as_ref().and_then(|rules| {
            let profile = rules.profile(profile_id)?;
            let quality_met = profile
                .rank(rules.quality_id(&file.quality))
                .zip(profile.effective_cutoff_rank())
                .is_some_and(|(f, c)| f >= c);
            if quality_met {
                return Some(format!(
                    "Custom-Format-Score {} statt {}",
                    file.custom_format_score, profile.cutoff_format_score
                ));
            }
            cutoff_name(rules, profile).map(|c| format!("{} statt {c}", file.quality))
        });
        cutoff.unwrap_or_else(|| file.quality.clone())
    }

    /// Whether the movie counts as available today by its minimum availability.
    pub fn available(&self, movie: &Movie) -> bool {
        let delay = self
            .rules
            .as_ref()
            .map_or(0, |r| r.config.settings.availability_delay_days);
        availability::is_available(
            Stage::from_api(&movie.availability.minimum).unwrap_or_default(),
            &release_info(movie),
            delay,
            self.today,
        )
    }

    pub fn movie(&self, movie: &Movie) -> MovieState {
        let available = self.available(movie);
        MovieState {
            available,
            missing: movie.monitored && movie.file.is_none() && available,
            cutoff_unmet: movie.monitored
                && movie
                    .file
                    .as_ref()
                    .is_some_and(|f| self.below_cutoff(movie.quality_profile_id, f)),
        }
    }

    /// The release stage: the reported status where known, otherwise from the dates.
    pub fn stage(&self, movie: &Movie) -> ReleaseStage {
        let info = release_info(movie);
        let reached = |d: Option<Date>| d.is_some_and(|d| d <= self.today);
        let stage = if reached(info.home_release()) {
            Stage::Released
        } else if reached(info.in_cinemas) {
            Stage::InCinemas
        } else if info.home_release().is_none() && info.in_cinemas.is_none() {
            info.status.unwrap_or(Stage::Announced)
        } else {
            Stage::Announced
        };
        match stage {
            Stage::Announced => ReleaseStage::Announced,
            Stage::InCinemas => ReleaseStage::InCinemas,
            Stage::Released => ReleaseStage::Released,
        }
    }

    pub fn episode(&self, series: &Series, episode: &Episode, today: &str) -> EpisodeState {
        let monitored = series.data.monitored && episode.data.monitored;
        let aired = episode.data.air_date.as_deref().is_some_and(|d| d <= today);
        EpisodeState {
            monitored,
            aired,
            missing: monitored && aired && episode.file.is_none(),
            cutoff_unmet: monitored
                && episode
                    .file
                    .as_ref()
                    .is_some_and(|f| self.below_cutoff(series.data.quality_profile_id, f)),
        }
    }

    /// Counts over the episodes of one series (`episodes` may hold other series too).
    pub fn series(&self, series: &Series, episodes: &[&Episode]) -> SeriesCounts {
        let today = self.today_text();
        let mut counts = SeriesCounts::default();
        let mut files = std::collections::HashSet::new();
        for e in episodes.iter().filter(|e| e.series_id == series.id) {
            let state = self.episode(series, e, &today);
            counts.episodes += 1;
            counts.monitored += u32::from(e.data.monitored);
            counts.missing += u32::from(state.missing);
            counts.cutoff_unmet += u32::from(state.cutoff_unmet);
            if let (Some(id), Some(f)) = (e.file_id, &e.file) {
                counts.files += 1;
                if files.insert(id) {
                    counts.size += f.size;
                }
            }
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sofarr_db::{Availability, NewEpisode};

    use super::*;

    fn rules() -> Rules {
        let docs = crate::library::tests::documents();
        Rules::from_documents(
            &docs["qualityprofile"],
            &docs["qualitydefinition"],
            &docs["customformat"],
            None,
            None,
        )
        .expect("rules")
    }

    fn day(d: &str) -> Date {
        d.parse().expect("date")
    }

    fn file(quality: &str, score: i32) -> MovieFile {
        MovieFile {
            relative_path: "a.mkv".to_owned(),
            size: 10,
            quality: quality.to_owned(),
            revision_version: 1,
            revision_real: 0,
            release_group: None,
            scene_name: None,
            custom_format_score: score,
        }
    }

    fn movie(availability: Availability, file: Option<MovieFile>) -> Movie {
        let (m, _) = crate::library::movie_from_radarr(
            &json!({"tmdbId": 1, "title": "Film", "monitored": true, "qualityProfileId": 1}),
            None,
        )
        .expect("movie");
        let mut m = crate::metadata::as_library_movie(&m);
        m.availability = availability;
        m.file = file;
        m
    }

    #[test]
    fn movies_are_missing_only_when_available() {
        let judge = Judge::new(Some(rules()), day("2026-11-12"));
        let soon = Availability {
            minimum: "released".to_owned(),
            in_cinemas: Some("2026-11-01".to_owned()),
            digital_release: Some("2027-01-01".to_owned()),
            ..Availability::default()
        };
        let state = judge.movie(&movie(soon.clone(), None));
        assert!(!state.available && !state.missing);
        assert_eq!(
            judge.stage(&movie(soon.clone(), None)),
            ReleaseStage::InCinemas
        );
        let cinema = Availability {
            minimum: "inCinemas".to_owned(),
            ..soon
        };
        assert!(judge.movie(&movie(cinema, None)).missing);
        let out = Availability {
            digital_release: Some("2026-01-01".to_owned()),
            ..Availability::default()
        };
        assert_eq!(
            judge.stage(&movie(out.clone(), None)),
            ReleaseStage::Released
        );
        let below = judge.movie(&movie(out.clone(), Some(file("WEBDL-1080p", 10))));
        assert!(below.cutoff_unmet && !below.missing);
        assert!(
            !judge
                .movie(&movie(out, Some(file("Bluray-1080p", 0))))
                .cutoff_unmet
        );
        assert_eq!(
            judge.cutoff_reason(1, &file("WEBDL-1080p", 10)),
            "WEBDL-1080p statt Bluray-1080p"
        );
        let unknown = Availability {
            status: Some("announced".to_owned()),
            ..Availability::default()
        };
        assert_eq!(judge.stage(&movie(unknown, None)), ReleaseStage::Announced);
    }

    #[test]
    fn series_counts_follow_the_series_rule() {
        let judge = Judge::new(Some(rules()), day("2026-11-12"));
        let (docs, _) = crate::series::tests::sonarr_documents();
        let series = Series {
            id: 1,
            data: crate::series::series_from_sonarr(&docs["series"][0]).expect("series"),
            tags: vec![],
        };
        let ep = |n: u16, aired: &str, monitored: bool, f: Option<MovieFile>| Episode {
            id: i64::from(n),
            series_id: 1,
            data: NewEpisode {
                tvdb_id: None,
                season: 1,
                number: n,
                absolute: None,
                title: None,
                title_de: None,
                air_date: Some(aired.to_owned()),
                monitored,
            },
            file_id: f.as_ref().map(|_| i64::from(n) * 10),
            file: f,
        };
        let eps = [
            ep(1, "2026-01-01", true, Some(file("WEBDL-1080p", 0))),
            ep(2, "2026-01-08", true, None),
            ep(3, "2026-01-15", false, None),
            ep(4, "2099-01-01", true, None),
        ];
        let refs: Vec<&Episode> = eps.iter().collect();
        let counts = judge.series(&series, &refs);
        assert_eq!(
            counts,
            SeriesCounts {
                episodes: 4,
                monitored: 3,
                files: 1,
                missing: 1,
                cutoff_unmet: 1,
                size: 10,
            }
        );
        assert!(!counts.complete());
        let refs: Vec<&Episode> = eps.iter().filter(|e| e.id != 2).collect();
        assert!(judge.series(&series, &refs).complete());
    }
}
