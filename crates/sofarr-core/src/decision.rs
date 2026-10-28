//! The grab decision: is a release wanted for a film, and if not, why not.
//!
//! Pure: everything time-dependent (the release age) is passed in. The rules follow
//! the behaviour described in the Servarr user docs; the release oracle checks them.

use std::fmt;

use sofarr_parser::{HardcodedSubs, Language, ParsedRelease, Revision};

use crate::{
    custom_format::{CustomFormatId, ReleaseFacts},
    import::RadarrConfig,
    language::LanguageRef,
    profile::QualityProfile,
    quality::{NamedQuality, QualityId},
    settings::PropersPolicy,
    title_match,
};

/// A release offered by an indexer.
#[derive(Debug, Clone, Copy)]
pub struct ReleaseInput<'a> {
    pub title: &'a str,
    pub size_bytes: u64,
    /// Time since the release was posted.
    pub age_minutes: f64,
}

/// What is known about the film the release is evaluated for.
#[derive(Debug, Clone, Default)]
pub struct MovieContext {
    /// Title, original title and alternative titles; empty skips the title check.
    pub titles: Vec<String>,
    /// Runtime in minutes; `0` if unknown (size limits are skipped then).
    pub runtime_minutes: u32,
    pub original_language: Option<Language>,
    pub file: Option<ExistingFile>,
}

/// The film's current file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingFile {
    pub quality: QualityId,
    pub revision: Revision,
    pub custom_format_score: i32,
}

#[derive(Debug, Clone)]
pub struct Decision {
    pub parsed: ParsedRelease,
    pub quality: NamedQuality,
    pub custom_formats: Vec<CustomFormatId>,
    pub score: i32,
    pub rejections: Vec<Rejection>,
}

impl Decision {
    pub fn approved(&self) -> bool {
        self.rejections.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Rejection {
    /// The release title names another film; no other rule is evaluated then.
    UnknownMovie,
    /// The release has burnt-in subtitles and the settings do not allow them.
    HardcodedSubs {
        subs: HardcodedSubs,
    },
    /// The quality is not allowed in the profile.
    QualityNotWanted {
        quality: String,
    },
    /// The release language does not fit the profile language.
    LanguageNotWanted {
        wanted: LanguageRef,
    },
    TooSmall {
        size: u64,
        minimum: u64,
    },
    TooLarge {
        size: u64,
        maximum: u64,
    },
    /// The custom format score is below the profile's minimum.
    BelowMinimumScore {
        score: i32,
        minimum: i32,
    },
    /// The film already has a file and the profile does not upgrade.
    UpgradesNotAllowed,
    /// The existing file already meets the quality and score cutoff.
    CutoffMet,
    /// The release is not better than the existing file.
    NotAnUpgrade,
    /// Larger than the indexer setting `maximumSize`.
    MaximumSizeExceeded {
        size: u64,
        maximum: u64,
    },
    /// Older than the indexer setting `retention`.
    OutsideRetention {
        age_days: f64,
        retention: u32,
    },
    /// Younger than the indexer setting `minimumAge`; may be grabbed later.
    TooYoung {
        age_minutes: f64,
        minimum: u32,
    },
}

impl Rejection {
    /// Permanent rejections never change; temporary ones may pass later.
    pub fn permanent(&self) -> bool {
        !matches!(self, Self::TooYoung { .. })
    }

    /// Stable identifier for reports and the oracle mapping.
    pub fn key(&self) -> &'static str {
        match self {
            Self::UnknownMovie => "unknown_movie",
            Self::HardcodedSubs { .. } => "hardcoded_subs",
            Self::QualityNotWanted { .. } => "quality_not_wanted",
            Self::LanguageNotWanted { .. } => "language_not_wanted",
            Self::TooSmall { .. } => "too_small",
            Self::TooLarge { .. } => "too_large",
            Self::BelowMinimumScore { .. } => "below_minimum_score",
            Self::UpgradesNotAllowed => "upgrades_not_allowed",
            Self::CutoffMet => "cutoff_met",
            Self::NotAnUpgrade => "not_an_upgrade",
            Self::MaximumSizeExceeded { .. } => "maximum_size_exceeded",
            Self::OutsideRetention { .. } => "outside_retention",
            Self::TooYoung { .. } => "too_young",
        }
    }
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const MB: u64 = 1024 * 1024;
        match self {
            Self::UnknownMovie => f.write_str("Release passt zu keinem Titel des Films"),
            Self::HardcodedSubs {
                subs: HardcodedSubs::Generic,
            } => f.write_str("eingebrannte Untertitel"),
            Self::HardcodedSubs {
                subs: HardcodedSubs::Tagged(tag),
            } => write!(f, "eingebrannte Untertitel ({tag})"),
            Self::QualityNotWanted { quality } => {
                write!(f, "Quality {quality} ist im Profil nicht erlaubt")
            }
            Self::LanguageNotWanted { wanted } => {
                write!(f, "Sprache passt nicht zum Profil ({wanted:?})")
            }
            Self::TooSmall { size, minimum } => write!(
                f,
                "zu klein: {} MB, mindestens {} MB",
                size / MB,
                minimum / MB
            ),
            Self::TooLarge { size, maximum } => write!(
                f,
                "zu groß: {} MB, höchstens {} MB",
                size / MB,
                maximum / MB
            ),
            Self::BelowMinimumScore { score, minimum } => {
                write!(f, "Custom-Format-Score {score} unter dem Minimum {minimum}")
            }
            Self::UpgradesNotAllowed => {
                f.write_str("Datei vorhanden, Profil erlaubt keine Upgrades")
            }
            Self::CutoffMet => f.write_str("vorhandene Datei erreicht den Cutoff"),
            Self::NotAnUpgrade => f.write_str("kein Upgrade gegenüber der vorhandenen Datei"),
            Self::MaximumSizeExceeded { size, maximum } => write!(
                f,
                "größer als die Indexer-Grenze: {} MB, höchstens {maximum} MB",
                size / MB
            ),
            Self::OutsideRetention {
                age_days,
                retention,
            } => {
                write!(f, "{age_days:.0} Tage alt, Retention {retention} Tage")
            }
            Self::TooYoung {
                age_minutes,
                minimum,
            } => write!(
                f,
                "{age_minutes:.0} Minuten alt, Mindestalter {minimum} Minuten"
            ),
        }
    }
}

/// Evaluates a release for a film with the given profile.
pub fn decide(
    config: &RadarrConfig,
    profile: &QualityProfile,
    release: &ReleaseInput<'_>,
    movie: &MovieContext,
) -> Decision {
    let parsed = sofarr_parser::parse(release.title);
    let quality = config
        .definitions
        .catalog()
        .resolve(&parsed.quality)
        .clone();
    if !movie.titles.is_empty() && !title_match::matches(&parsed.titles, &movie.titles) {
        return Decision {
            parsed,
            quality,
            custom_formats: Vec::new(),
            score: 0,
            rejections: vec![Rejection::UnknownMovie],
        };
    }
    let facts = ReleaseFacts {
        title: release.title,
        parsed: &parsed,
        original_language: movie.original_language,
    };
    let custom_formats: Vec<CustomFormatId> = config
        .custom_formats
        .iter()
        .filter(|cf| cf.matches(&facts))
        .map(|cf| cf.id)
        .collect();
    let score = profile.score(&custom_formats);

    let mut rejections = Vec::new();
    if !profile.is_allowed(quality.id) {
        rejections.push(Rejection::QualityNotWanted {
            quality: quality.name.clone(),
        });
    }
    if let Some(subs) = parsed
        .hardcoded_subs
        .clone()
        .filter(|_| !config.settings.allow_hardcoded_subs)
    {
        rejections.push(Rejection::HardcodedSubs { subs });
    }
    if !profile
        .language
        .matches(&parsed.languages, movie.original_language)
    {
        rejections.push(Rejection::LanguageNotWanted {
            wanted: profile.language,
        });
    }
    size_rules(config, &quality, release, movie, &mut rejections);
    if score < profile.min_format_score {
        rejections.push(Rejection::BelowMinimumScore {
            score,
            minimum: profile.min_format_score,
        });
    }
    if let Some(file) = &movie.file
        && let Some(r) = upgrade_rule(config, profile, &quality, parsed.revision, score, file)
    {
        rejections.push(r);
    }
    indexer_rules(config, release, &mut rejections);

    Decision {
        parsed,
        quality,
        custom_formats,
        score,
        rejections,
    }
}

fn size_rules(
    config: &RadarrConfig,
    quality: &NamedQuality,
    release: &ReleaseInput<'_>,
    movie: &MovieContext,
    rejections: &mut Vec<Rejection>,
) {
    if movie.runtime_minutes == 0 {
        return;
    }
    let Some(definition) = config.definitions.get(quality.id) else {
        return;
    };
    let (minimum, maximum) = definition.size_limits(movie.runtime_minutes);
    if release.size_bytes < minimum {
        rejections.push(Rejection::TooSmall {
            size: release.size_bytes,
            minimum,
        });
    }
    if let Some(maximum) = maximum.filter(|m| release.size_bytes > *m) {
        rejections.push(Rejection::TooLarge {
            size: release.size_bytes,
            maximum,
        });
    }
}

fn upgrade_rule(
    config: &RadarrConfig,
    profile: &QualityProfile,
    quality: &NamedQuality,
    revision: Revision,
    score: i32,
    file: &ExistingFile,
) -> Option<Rejection> {
    if !profile.upgrade_allowed {
        return Some(Rejection::UpgradesNotAllowed);
    }
    let file_rank = profile.rank(file.quality);
    let quality_cutoff_met = match (file_rank, profile.cutoff_rank()) {
        (Some(file), Some(cutoff)) => file >= cutoff,
        _ => false,
    };
    if quality_cutoff_met && file.custom_format_score >= profile.cutoff_format_score {
        return Some(Rejection::CutoffMet);
    }

    let release_rank = profile.rank(quality.id);
    if release_rank != file_rank {
        return (release_rank < file_rank).then_some(Rejection::NotAnUpgrade);
    }
    if config.settings.propers != PropersPolicy::DoNotPrefer {
        let ordering =
            (revision.version, revision.real).cmp(&(file.revision.version, file.revision.real));
        match ordering {
            std::cmp::Ordering::Less => return Some(Rejection::NotAnUpgrade),
            std::cmp::Ordering::Greater
                if config.settings.propers == PropersPolicy::PreferAndUpgrade =>
            {
                return None;
            }
            _ => {}
        }
    }
    let gain = profile.min_upgrade_format_score.unwrap_or(1).max(1);
    (score < file.custom_format_score + gain).then_some(Rejection::NotAnUpgrade)
}

fn indexer_rules(
    config: &RadarrConfig,
    release: &ReleaseInput<'_>,
    rejections: &mut Vec<Rejection>,
) {
    let settings = &config.settings;
    const MB: u64 = 1024 * 1024;
    if settings.maximum_size_mb > 0 && release.size_bytes > u64::from(settings.maximum_size_mb) * MB
    {
        rejections.push(Rejection::MaximumSizeExceeded {
            size: release.size_bytes,
            maximum: settings.maximum_size_mb.into(),
        });
    }
    let age_days = release.age_minutes / (24.0 * 60.0);
    if settings.retention_days > 0 && age_days > f64::from(settings.retention_days) {
        rejections.push(Rejection::OutsideRetention {
            age_days,
            retention: settings.retention_days,
        });
    }
    if settings.minimum_age_minutes > 0
        && release.age_minutes < f64::from(settings.minimum_age_minutes)
    {
        rejections.push(Rejection::TooYoung {
            age_minutes: release.age_minutes,
            minimum: settings.minimum_age_minutes,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        custom_format::{Condition, CustomFormat, Pattern, Specification},
        definition::{QualityDefinition, QualityDefinitions},
        profile::tests::profile,
        quality::QualityCatalog,
        settings::Settings,
    };

    const GB: u64 = 1024 * 1024 * 1024;

    fn config() -> RadarrConfig {
        let catalog = QualityCatalog::observed();
        let definitions = catalog
            .iter()
            .map(|q| QualityDefinition {
                quality: q.clone(),
                title: q.name.clone(),
                min_size: 5.0,
                max_size: Some(200.0),
                preferred_size: None,
            })
            .collect();
        RadarrConfig {
            definitions: QualityDefinitions(definitions),
            profiles: vec![profile()],
            custom_formats: vec![
                CustomFormat {
                    id: CustomFormatId(1),
                    name: "Gute Gruppe".into(),
                    specifications: vec![Specification {
                        name: "Gruppe".into(),
                        condition: Condition::ReleaseGroup(Pattern::new("^GOOD$").unwrap()),
                        negate: false,
                        required: false,
                    }],
                },
                CustomFormat {
                    id: CustomFormatId(2),
                    name: "3D".into(),
                    specifications: vec![Specification {
                        name: "3D".into(),
                        condition: Condition::ReleaseTitle(Pattern::new(r"\b3D\b").unwrap()),
                        negate: false,
                        required: false,
                    }],
                },
            ],
            settings: Settings::default(),
        }
    }

    fn release(title: &str) -> ReleaseInput<'_> {
        ReleaseInput {
            title,
            size_bytes: 8 * GB,
            age_minutes: 600.0,
        }
    }

    fn movie(file: Option<ExistingFile>) -> MovieContext {
        MovieContext {
            titles: vec!["Some Movie".into()],
            runtime_minutes: 100,
            original_language: Some(Language::English),
            file,
        }
    }

    fn file(quality: u32, score: i32) -> ExistingFile {
        ExistingFile {
            quality: QualityId(quality),
            revision: Revision::default(),
            custom_format_score: score,
        }
    }

    fn keys(config: &RadarrConfig, title: &str, movie: &MovieContext) -> Vec<&'static str> {
        decide(config, &config.profiles[0], &release(title), movie)
            .rejections
            .iter()
            .map(Rejection::key)
            .collect()
    }

    #[test]
    fn wanted_release_without_file() {
        let config = config();
        let d = decide(
            &config,
            &config.profiles[0],
            &release("Some.Movie.2020.1080p.WEB-DL.DDP5.1.H.264-GOOD"),
            &movie(None),
        );
        assert!(d.approved(), "{:?}", d.rejections);
        assert_eq!(d.quality.name, "WEBDL-1080p");
        assert_eq!(d.custom_formats, [CustomFormatId(1)]);
        assert_eq!(d.score, 100);
    }

    #[test]
    fn quality_score_and_size() {
        let config = config();
        let m = movie(None);
        assert_eq!(
            keys(
                &config,
                "Some.Movie.2020.1080p.BluRay.REMUX.AVC.DTS-HD.MA-GRP",
                &m
            ),
            ["quality_not_wanted"]
        );
        assert_eq!(
            keys(&config, "Some.Movie.2020.3D.1080p.BluRay.x264-GRP", &m),
            ["below_minimum_score"]
        );
        let big = ReleaseInput {
            size_bytes: 30 * GB,
            ..release("Some.Movie.2020.1080p.BluRay.x264-GRP")
        };
        let d = decide(&config, &config.profiles[0], &big, &m);
        assert_eq!(
            d.rejections.iter().map(Rejection::key).collect::<Vec<_>>(),
            ["too_large"]
        );
        let small = ReleaseInput {
            size_bytes: GB / 4,
            ..release("Some.Movie.2020.1080p.BluRay.x264-GRP")
        };
        let d = decide(&config, &config.profiles[0], &small, &m);
        assert_eq!(
            d.rejections.iter().map(Rejection::key).collect::<Vec<_>>(),
            ["too_small"]
        );
    }

    #[test]
    fn upgrades() {
        let config = config();
        let web = "Some.Movie.2020.1080p.WEB-DL.DDP5.1.H.264-GRP";
        let bluray = "Some.Movie.2020.1080p.BluRay.x264-GRP";
        // Lower quality than the file.
        assert_eq!(
            keys(&config, web, &movie(Some(file(7, 0)))),
            ["not_an_upgrade"]
        );
        // Higher quality than the file.
        assert!(keys(&config, bluray, &movie(Some(file(1, 0)))).is_empty());
        // Same group, same score: no upgrade; better score: upgrade.
        assert_eq!(
            keys(&config, web, &movie(Some(file(15, 0)))),
            ["not_an_upgrade"]
        );
        assert!(
            keys(
                &config,
                "Some.Movie.2020.1080p.WEB-DL.H.264-GOOD",
                &movie(Some(file(15, 0)))
            )
            .is_empty()
        );
    }

    #[test]
    fn cutoff_needs_quality_and_score() {
        let config = config();
        let good = "Some.Movie.2020.1080p.BluRay.x264-GOOD";
        // Quality cutoff met, score cutoff (100) met.
        assert_eq!(
            keys(&config, good, &movie(Some(file(7, 100)))),
            ["cutoff_met"]
        );
        // Quality cutoff met, score below: a better score is still an upgrade.
        assert!(keys(&config, good, &movie(Some(file(7, 0)))).is_empty());
    }

    #[test]
    fn propers_and_upgrade_settings() {
        let mut config = config();
        let proper = "Some.Movie.2020.1080p.WEB-DL.H.264.PROPER-GRP";
        assert!(keys(&config, proper, &movie(Some(file(3, 0)))).is_empty());
        config.settings.propers = PropersPolicy::DoNotPrefer;
        assert_eq!(
            keys(&config, proper, &movie(Some(file(3, 0)))),
            ["not_an_upgrade"]
        );
        config.profiles[0].upgrade_allowed = false;
        assert_eq!(
            keys(&config, proper, &movie(Some(file(3, 0)))),
            ["upgrades_not_allowed"]
        );
    }

    #[test]
    fn other_film_skips_all_rules() {
        let config = config();
        let d = decide(
            &config,
            &config.profiles[0],
            &release("Other.Movie.2020.1080p.BluRay.REMUX.AVC.DTS-HD.MA-GOOD"),
            &movie(None),
        );
        let got: Vec<_> = d.rejections.iter().map(Rejection::key).collect();
        assert_eq!(got, ["unknown_movie"]);
        assert_eq!(d.quality.name, "Remux-1080p");
        assert!(d.custom_formats.is_empty());
        assert_eq!(d.score, 0);
        // Without known titles the check is skipped.
        let unknown = MovieContext {
            titles: Vec::new(),
            ..movie(None)
        };
        assert!(keys(&config, "Other.Movie.2020.1080p.BluRay.x264-GRP", &unknown).is_empty());
    }

    #[test]
    fn hardcoded_subs_follow_the_setting() {
        let mut config = config();
        let title = "Some.Movie.2020.SUBBED.1080p.BluRay.x264-GRP";
        assert_eq!(keys(&config, title, &movie(None)), ["hardcoded_subs"]);
        config.settings.allow_hardcoded_subs = true;
        assert!(keys(&config, title, &movie(None)).is_empty());
    }

    #[test]
    fn language_and_indexer_settings() {
        let mut config = config();
        config.profiles[0].language = LanguageRef::Language(Language::German);
        let title = "Some.Movie.2020.1080p.WEB-DL.H.264-GRP";
        assert_eq!(keys(&config, title, &movie(None)), ["language_not_wanted"]);
        assert!(
            keys(
                &config,
                "Some.Movie.2020.German.1080p.WEB-DL.H.264-GRP",
                &movie(None)
            )
            .is_empty()
        );

        config.profiles[0].language = LanguageRef::Any;
        config.settings.maximum_size_mb = 1024;
        config.settings.retention_days = 0;
        config.settings.minimum_age_minutes = 720;
        let d = decide(&config, &config.profiles[0], &release(title), &movie(None));
        let keys: Vec<_> = d.rejections.iter().map(Rejection::key).collect();
        assert_eq!(keys, ["maximum_size_exceeded", "too_young"]);
        assert!(d.rejections[0].permanent());
        assert!(!d.rejections[1].permanent());
    }
}
