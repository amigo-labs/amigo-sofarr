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
    upgrade::{self, Candidate},
};

/// A release offered by an indexer.
#[derive(Debug, Clone, Copy)]
pub struct ReleaseInput<'a> {
    pub title: &'a str,
    pub size_bytes: u64,
    /// Time since the release was posted.
    pub age_minutes: f64,
    /// Languages reported for the release (title plus indexer metadata); `None` takes
    /// the languages parsed from the title.
    pub languages: Option<&'a [Language]>,
}

/// What is known about the film the release is evaluated for.
#[derive(Debug, Clone, Default)]
pub struct MovieContext {
    /// Release year and secondary year; empty skips the year check.
    pub years: Vec<u16>,
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
    pub release_group: Option<String>,
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
    /// Without a year, only titles marked German are understood; no other rule is
    /// evaluated then.
    Unparsable,
    /// The release names a year that is not the film's; no other rule is evaluated then.
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
    /// The release has a higher quality than the file, but the profile does not upgrade.
    UpgradesNotAllowed,
    /// The existing file already meets the quality and score cutoff.
    CutoffMet,
    /// The file already reaches the cutoff quality; a higher quality is no upgrade.
    QualityCutoffMet,
    /// The quality of the file ranks higher in the profile.
    LowerQuality,
    /// Same quality, but the file has a higher revision.
    LowerRevision,
    /// Same quality and revision, and the score does not improve enough.
    NoBetterScore,
    /// A repack of another release group than the one of the file.
    RepackOtherGroup,
    /// A repack, but the release group of the file is unknown.
    FileGroupUnknown,
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
            Self::Unparsable => "unparsable",
            Self::UnknownMovie => "unknown_movie",
            Self::HardcodedSubs { .. } => "hardcoded_subs",
            Self::QualityNotWanted { .. } => "quality_not_wanted",
            Self::LanguageNotWanted { .. } => "language_not_wanted",
            Self::TooSmall { .. } => "too_small",
            Self::TooLarge { .. } => "too_large",
            Self::BelowMinimumScore { .. } => "below_minimum_score",
            Self::UpgradesNotAllowed => "upgrades_not_allowed",
            Self::CutoffMet => "cutoff_met",
            Self::QualityCutoffMet => "quality_cutoff_met",
            Self::LowerQuality => "lower_quality",
            Self::LowerRevision => "lower_revision",
            Self::NoBetterScore => "no_better_score",
            Self::RepackOtherGroup => "repack_other_group",
            Self::FileGroupUnknown => "file_group_unknown",
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
            Self::Unparsable => f.write_str("Release-Name nicht verständlich"),
            Self::UnknownMovie => f.write_str("Release-Jahr passt nicht zum Film"),
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
            Self::QualityCutoffMet => f.write_str("vorhandene Datei erreicht die Cutoff-Quality"),
            Self::LowerQuality => f.write_str("vorhandene Datei hat eine bessere Quality"),
            Self::LowerRevision => f.write_str("vorhandene Datei hat eine höhere Revision"),
            Self::NoBetterScore => {
                f.write_str("Custom-Format-Score nicht besser als der der vorhandenen Datei")
            }
            Self::RepackOtherGroup => {
                f.write_str("Repack einer anderen Release-Gruppe als die der Datei")
            }
            Self::FileGroupUnknown => {
                f.write_str("Repack, aber die Release-Gruppe der Datei ist unbekannt")
            }
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
    let rejected = |parsed, quality, rejection| Decision {
        parsed,
        quality,
        custom_formats: Vec::new(),
        score: 0,
        rejections: vec![rejection],
    };
    // Observed: titles without a year are only understood when marked German.
    if parsed.year.is_none() && !release.title.to_ascii_lowercase().contains("german") {
        return rejected(parsed, quality, Rejection::Unparsable);
    }
    // In a search for one film, the release year decides, not the title: the oracle takes
    // releases without a year, and with a matching year, whatever their title says.
    if let Some(year) = parsed.year
        && !movie.years.is_empty()
        && !movie.years.contains(&year)
    {
        return rejected(parsed, quality, Rejection::UnknownMovie);
    }
    let languages = release.languages.unwrap_or(&parsed.languages);
    let facts = ReleaseFacts {
        title: release.title,
        parsed: &parsed,
        languages,
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
    if !profile.language.matches(languages, movie.original_language) {
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
    if let Some(file) = &movie.file {
        let candidate = Candidate {
            quality: quality.id,
            revision: parsed.revision,
            release_group: parsed.release_group.as_deref(),
            score,
        };
        rejections.extend(upgrade::rules(
            profile,
            config.settings.propers,
            &candidate,
            file,
        ));
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
        settings::{PropersPolicy, Settings},
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
            languages: None,
        }
    }

    fn movie(file: Option<ExistingFile>) -> MovieContext {
        MovieContext {
            years: vec![2020],
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
            release_group: Some("GRP".into()),
        }
    }

    fn file_v2(quality: u32) -> ExistingFile {
        ExistingFile {
            revision: Revision {
                version: 2,
                ..Revision::default()
            },
            ..file(quality, 0)
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
    fn upgrades_rank_then_revision_then_score() {
        let config = config();
        let web = "Some.Movie.2020.1080p.WEB-DL.DDP5.1.H.264-GRP";
        let bluray = "Some.Movie.2020.1080p.BluRay.x264-GRP";
        let check = |title, file| keys(&config, title, &movie(Some(file)));
        // Lower quality loses whatever the revision.
        assert_eq!(check(web, file(7, 0)), ["lower_quality"]);
        assert_eq!(
            check("Some.Movie.2020.1080p.WEB-DL.H.264.PROPER-GRP", file(7, 0)),
            ["lower_quality"]
        );
        // Higher quality below the cutoff wins, even with a lower revision.
        assert!(check(bluray, file(1, 0)).is_empty());
        assert!(check(bluray, file_v2(3)).is_empty());
        // Same quality group: revision, then score.
        assert_eq!(check(web, file_v2(15)), ["lower_revision"]);
        assert!(check("Some.Movie.2020.1080p.WEB-DL.H.264.PROPER-GRP", file(15, 0)).is_empty());
        assert_eq!(check(web, file(15, 0)), ["no_better_score"]);
        assert!(check("Some.Movie.2020.1080p.WEB-DL.H.264-GOOD", file(15, 0)).is_empty());
    }

    #[test]
    fn file_at_cutoff_quality() {
        let config = config();
        let remux = "Some.Movie.2020.1080p.BluRay.REMUX.AVC.DTS-HD.MA-GRP";
        let check = |title, file| keys(&config, title, &movie(Some(file)));
        // Score cutoff (100) not met: a higher quality is no upgrade, a revision is.
        assert_eq!(
            check(remux, file(7, 0)),
            ["quality_not_wanted", "quality_cutoff_met"]
        );
        assert_eq!(
            check(
                "Some.Movie.2020.1080p.BluRay.REMUX.AVC.DTS-HD.MA.PROPER-GRP",
                file(7, 0)
            ),
            ["quality_not_wanted"]
        );
        assert_eq!(
            check(remux, file_v2(7)),
            ["quality_not_wanted", "lower_revision"]
        );
        // Same quality with a better score is still an upgrade.
        assert!(check("Some.Movie.2020.1080p.BluRay.x264-GOOD", file(7, 0)).is_empty());
        // Quality and score cutoff met: only a better revision of that quality counts.
        assert_eq!(
            check("Some.Movie.2020.1080p.BluRay.x264-GOOD", file(7, 100)),
            ["cutoff_met"]
        );
        assert!(
            check(
                "Some.Movie.2020.1080p.BluRay.x264.PROPER-GOOD",
                file(7, 100)
            )
            .is_empty()
        );
    }

    #[test]
    fn profile_without_upgrades() {
        let mut config = config();
        config.profiles[0].upgrade_allowed = false;
        config.profiles[0].cutoff_format_score = 0;
        let check = |title, file| keys(&config, title, &movie(Some(file)));
        // The lowest allowed quality is the cutoff: every file meets it.
        assert_eq!(
            check("Some.Movie.2020.1080p.WEB-DL.H.264-GRP", file(7, 0)),
            ["cutoff_met"]
        );
        assert_eq!(
            check("Some.Movie.2020.1080p.BluRay.x264-GRP", file(3, 0)),
            ["cutoff_met", "upgrades_not_allowed"]
        );
        // A better revision of the quality of the file passes.
        assert!(check("Some.Movie.2020.1080p.WEB-DL.H.264.PROPER-GRP", file(3, 0)).is_empty());
    }

    #[test]
    fn repacks_need_the_group_of_the_file() {
        let config = config();
        let check = |title, file| keys(&config, title, &movie(Some(file)));
        assert!(check("Some.Movie.2020.1080p.WEB-DL.H.264.REPACK-GRP", file(3, 0)).is_empty());
        assert_eq!(
            check(
                "Some.Movie.2020.1080p.WEB-DL.H.264.REPACK-OTHER",
                file(3, 0)
            ),
            ["repack_other_group"]
        );
        let unknown = ExistingFile {
            release_group: None,
            ..file(3, 0)
        };
        assert_eq!(
            check("Some.Movie.2020.1080p.WEB-DL.H.264.REPACK-GRP", unknown),
            ["file_group_unknown"]
        );
    }

    #[test]
    fn propers_policy() {
        let mut config = config();
        let proper = "Some.Movie.2020.1080p.WEB-DL.H.264.PROPER-GRP";
        config.settings.propers = PropersPolicy::DoNotPrefer;
        assert_eq!(
            keys(&config, proper, &movie(Some(file(3, 0)))),
            ["no_better_score"]
        );
        config.settings.propers = PropersPolicy::DoNotUpgrade;
        assert_eq!(
            keys(&config, proper, &movie(Some(file(3, 0)))),
            ["no_better_score"]
        );
        assert_eq!(
            keys(
                &config,
                "Some.Movie.2020.1080p.WEB-DL.H.264-GRP",
                &movie(Some(file_v2(3)))
            ),
            ["lower_revision"]
        );
    }

    #[test]
    fn titles_without_year_need_german() {
        let config = config();
        let d = decide(
            &config,
            &config.profiles[0],
            &release("Some.Movie.1080p.BluRay.x264-GOOD"),
            &movie(None),
        );
        let got: Vec<_> = d.rejections.iter().map(Rejection::key).collect();
        assert_eq!(got, ["unparsable"]);
        assert_eq!(d.quality.name, "Bluray-1080p");
        assert_eq!(d.score, 0);
        assert!(
            keys(
                &config,
                "Some.Movie.German.1080p.BluRay.x264-GRP",
                &movie(None)
            )
            .is_empty()
        );
    }

    #[test]
    fn reported_languages_replace_parsed_ones() {
        let mut config = config();
        config.profiles[0].language = LanguageRef::Language(Language::German);
        let title = "Some.Movie.2020.1080p.WEB-DL.H.264-GRP";
        assert_eq!(keys(&config, title, &movie(None)), ["language_not_wanted"]);
        let reported = ReleaseInput {
            languages: Some(&[Language::German]),
            ..release(title)
        };
        let d = decide(&config, &config.profiles[0], &reported, &movie(None));
        assert!(d.approved(), "{:?}", d.rejections);
    }

    #[test]
    fn other_year_skips_all_rules() {
        let config = config();
        let d = decide(
            &config,
            &config.profiles[0],
            &release("Some.Movie.2.2022.1080p.BluRay.REMUX.AVC.DTS-HD.MA-GOOD"),
            &movie(None),
        );
        let got: Vec<_> = d.rejections.iter().map(Rejection::key).collect();
        assert_eq!(got, ["unknown_movie"]);
        assert_eq!(d.quality.name, "Remux-1080p");
        assert!(d.custom_formats.is_empty());
        assert_eq!(d.score, 0);
        // The title does not matter; a release without a year passes.
        let m = movie(None);
        assert!(keys(&config, "Other.Movie.2020.1080p.BluRay.x264-GRP", &m).is_empty());
        assert!(keys(&config, "Some.Movie.2.German.1080p.BluRay.x264-GRP", &m).is_empty());
        // The secondary year counts too; without known years the check is skipped.
        let secondary = MovieContext {
            years: vec![2020, 2022],
            ..movie(None)
        };
        assert!(keys(&config, "Some.Movie.2022.1080p.BluRay.x264-GRP", &secondary).is_empty());
        let unknown = MovieContext {
            years: Vec::new(),
            ..movie(None)
        };
        assert!(keys(&config, "Some.Movie.1999.1080p.BluRay.x264-GRP", &unknown).is_empty());
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
