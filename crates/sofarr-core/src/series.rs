//! Decisions for series: which episodes a release covers and whether it is wanted.
//!
//! The quality, language, size, custom format and upgrade rules are the film rules
//! ([`crate::evaluate`]); only identifying the episodes is series-specific. Matching the
//! series title is the caller's job.

use sofarr_parser::{EpisodeInfo, Language};

use crate::{
    Decision, ExistingFile, MovieContext, QualityProfile, RadarrConfig, Rejection, ReleaseInput,
    evaluate,
};

/// One episode of a series as the library knows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeState {
    pub id: i64,
    pub season: u16,
    pub number: u16,
    pub absolute: Option<u16>,
    /// `YYYY-MM-DD`.
    pub air_date: Option<String>,
    pub monitored: bool,
    /// Already broadcast (decided by the caller with the current date).
    pub aired: bool,
    pub file: Option<ExistingFile>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeriesTarget {
    pub original_language: Option<Language>,
    /// Typical runtime of one episode.
    pub runtime_minutes: u32,
    pub episodes: Vec<EpisodeState>,
}

/// The outcome for a series release.
#[derive(Debug, Clone)]
pub struct EpisodeDecision {
    /// Ids of the episodes the release covers.
    pub episodes: Vec<i64>,
    pub decision: Decision,
}

/// Episodes a release covers.
pub fn covered<'a>(info: &EpisodeInfo, series: &'a SeriesTarget) -> Vec<&'a EpisodeState> {
    let eps = series.episodes.iter();
    if let Some((y, m, d)) = info.air_date {
        let date = format!("{y:04}-{m:02}-{d:02}");
        return eps
            .filter(|e| e.air_date.as_deref() == Some(&date))
            .collect();
    }
    if !info.absolute_episodes.is_empty() {
        return eps
            .filter(|e| {
                e.absolute
                    .is_some_and(|a| info.absolute_episodes.contains(&a))
            })
            .collect();
    }
    let Some(season) = info.season else {
        return Vec::new();
    };
    if info.full_season {
        let last = info.last_season.unwrap_or(season);
        return eps
            .filter(|e| (season..=last).contains(&e.season))
            .collect();
    }
    eps.filter(|e| e.season == season && info.episodes.contains(&e.number))
        .collect()
}

/// The file a release has to beat: none if a wanted episode has no file, otherwise the
/// weakest file among the wanted episodes.
fn weakest_file(wanted: &[&EpisodeState], profile: &QualityProfile) -> Option<ExistingFile> {
    let mut weakest: Option<&ExistingFile> = None;
    for e in wanted {
        let file = e.file.as_ref()?;
        let worse = weakest.is_none_or(|w| {
            (profile.rank(file.quality), file.custom_format_score)
                < (profile.rank(w.quality), w.custom_format_score)
        });
        if worse {
            weakest = Some(file);
        }
    }
    weakest.cloned()
}

/// Evaluates a series release.
pub fn evaluate_episodes(
    config: &RadarrConfig,
    profile: &QualityProfile,
    release: &ReleaseInput<'_>,
    info: &EpisodeInfo,
    series: &SeriesTarget,
) -> EpisodeDecision {
    let covered = covered(info, series);
    let wanted: Vec<&EpisodeState> = covered
        .iter()
        .copied()
        .filter(|e| e.monitored && e.aired)
        .collect();
    let count = u32::try_from(covered.len().max(1)).unwrap_or(1);
    let context = MovieContext {
        years: Vec::new(),
        runtime_minutes: series.runtime_minutes.saturating_mul(count),
        original_language: series.original_language,
        file: weakest_file(&wanted, profile),
    };
    let mut decision = evaluate(config, profile, release, info.release.clone(), &context);
    if covered.is_empty() {
        decision.rejections.insert(0, Rejection::UnknownEpisode);
    } else if wanted.is_empty() {
        decision.rejections.insert(0, Rejection::EpisodeNotWanted);
    }
    EpisodeDecision {
        episodes: covered.iter().map(|e| e.id).collect(),
        decision,
    }
}

#[cfg(test)]
mod tests {
    use sofarr_parser::{Revision, parse_episode};

    use super::*;
    use crate::{
        LanguageRef, ProfileId, ProfileItem, QualityCatalog, QualityDefinition, QualityDefinitions,
        QualityId, Settings,
    };

    const WEBDL_1080: QualityId = QualityId(3);
    const BLURAY_1080: QualityId = QualityId(7);

    fn config() -> RadarrConfig {
        let catalog = QualityCatalog::observed();
        RadarrConfig {
            definitions: QualityDefinitions(
                catalog
                    .iter()
                    .map(|q| QualityDefinition {
                        quality: q.clone(),
                        title: q.name.clone(),
                        min_size: 0.0,
                        max_size: None,
                        preferred_size: None,
                    })
                    .collect(),
            ),
            profiles: vec![QualityProfile {
                id: ProfileId(1),
                name: "HD".to_owned(),
                upgrade_allowed: true,
                cutoff: BLURAY_1080.0,
                items: vec![
                    ProfileItem::Quality {
                        quality: WEBDL_1080,
                        allowed: true,
                    },
                    ProfileItem::Quality {
                        quality: BLURAY_1080,
                        allowed: true,
                    },
                ],
                format_scores: Vec::new(),
                min_format_score: 0,
                cutoff_format_score: 0,
                min_upgrade_format_score: None,
                language: LanguageRef::Any,
            }],
            custom_formats: Vec::new(),
            settings: Settings::default(),
        }
    }

    fn file(quality: QualityId) -> ExistingFile {
        ExistingFile {
            quality,
            revision: Revision::default(),
            custom_format_score: 0,
            release_group: Some("OLD".to_owned()),
        }
    }

    fn episode(id: i64, season: u16, number: u16, file: Option<QualityId>) -> EpisodeState {
        EpisodeState {
            id,
            season,
            number,
            absolute: Some(u16::try_from(id).unwrap_or(0)),
            air_date: Some(format!("2026-10-{:02}", id + 10)),
            monitored: true,
            aired: true,
            file: file.map(self::file),
        }
    }

    fn series() -> SeriesTarget {
        SeriesTarget {
            original_language: Some(Language::English),
            runtime_minutes: 45,
            episodes: vec![
                episode(1, 1, 1, Some(BLURAY_1080)),
                episode(2, 1, 2, Some(WEBDL_1080)),
                episode(3, 2, 1, None),
                EpisodeState {
                    aired: false,
                    ..episode(4, 2, 2, None)
                },
                EpisodeState {
                    monitored: false,
                    ..episode(5, 0, 1, None)
                },
            ],
        }
    }

    fn decide(title: &str) -> EpisodeDecision {
        let config = config();
        let info = parse_episode(title).expect("episode");
        let release = ReleaseInput {
            title,
            size_bytes: 2_000_000_000,
            age_minutes: 60.0,
            languages: None,
        };
        evaluate_episodes(&config, &config.profiles[0], &release, &info, &series())
    }

    fn keys(d: &EpisodeDecision) -> Vec<&'static str> {
        d.decision.rejections.iter().map(Rejection::key).collect()
    }

    #[test]
    fn missing_episode_is_wanted() {
        let d = decide("Kleine.Insel.S02E01.German.1080p.WEB-DL.h264-GRP");
        assert_eq!(d.episodes, [3]);
        assert!(d.decision.approved(), "{:?}", keys(&d));
    }

    #[test]
    fn upgrades_and_season_packs() {
        assert!(
            decide("Small.Island.S01E02.1080p.BluRay.x264-GRP")
                .decision
                .approved()
        );
        assert!(
            !decide("Small.Island.S01E01.1080p.BluRay.x264-GRP")
                .decision
                .approved()
        );
        // Season pack: the weakest file (WEBDL) decides.
        let pack = decide("Small.Island.S01.1080p.BluRay.x264-GRP");
        assert_eq!(pack.episodes, [1, 2]);
        assert!(pack.decision.approved(), "{:?}", keys(&pack));
    }

    #[test]
    fn unwanted_and_unknown() {
        assert_eq!(
            keys(&decide("Kleine.Insel.S02E02.1080p.WEB-DL-G"))[0],
            "episode_not_wanted"
        );
        assert_eq!(
            keys(&decide("Kleine.Insel.S00E01.1080p.WEB-DL-G"))[0],
            "episode_not_wanted"
        );
        assert_eq!(
            keys(&decide("Kleine.Insel.S05E01.1080p.WEB-DL-G"))[0],
            "unknown_episode"
        );
    }

    #[test]
    fn daily_and_absolute() {
        let daily = parse_episode("Kleine.Insel.2026.10.13.1080p.WEB-DL-G").expect("daily");
        let ids = |info| {
            covered(&info, &series())
                .iter()
                .map(|e| e.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(daily), [3]);
        let abs = parse_episode("[G] Kleine Insel - 02 [1080p]").expect("absolute");
        assert_eq!(ids(abs), [2]);
    }
}
