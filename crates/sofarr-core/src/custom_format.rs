//! Custom formats: named sets of conditions on a release, scored by the profile.
//!
//! Semantics as described in the Servarr user docs and checked against the release
//! oracle: conditions of the same kind are alternatives (one of them must match),
//! different kinds must all match, a `required` condition must match in any case,
//! and `negate` inverts a single condition.

use std::fmt;

use sofarr_parser::{Language, ParsedRelease};

use crate::language::LanguageRef;
pub use crate::pattern::Pattern;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CustomFormatId(pub u32);

impl fmt::Display for CustomFormatId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomFormat {
    pub id: CustomFormatId,
    pub name: String,
    pub specifications: Vec<Specification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Specification {
    pub name: String,
    pub condition: Condition,
    pub negate: bool,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    /// The release is in this language; with `except`, in any language but this one.
    Language { language: LanguageRef, except: bool },
    /// The release group matches the pattern.
    ReleaseGroup(Pattern),
    /// The release title matches the pattern.
    ReleaseTitle(Pattern),
}

impl Condition {
    /// Conditions of the same kind are alternatives.
    fn kind(&self) -> u8 {
        match self {
            Self::Language { .. } => 0,
            Self::ReleaseGroup(_) => 1,
            Self::ReleaseTitle(_) => 2,
        }
    }
}

/// What custom formats are evaluated against.
#[derive(Debug, Clone, Copy)]
pub struct ReleaseFacts<'a> {
    pub title: &'a str,
    pub parsed: &'a ParsedRelease,
    /// Languages of the release (parsed or reported).
    pub languages: &'a [Language],
    /// Original language of the film, if known.
    pub original_language: Option<Language>,
}

impl Specification {
    pub fn matches(&self, release: &ReleaseFacts<'_>) -> bool {
        let hit = match &self.condition {
            Condition::Language { language, except } => {
                language.matches(release.languages, release.original_language) != *except
            }
            Condition::ReleaseGroup(pattern) => release
                .parsed
                .release_group
                .as_deref()
                .is_some_and(|g| pattern.is_match(g)),
            Condition::ReleaseTitle(pattern) => pattern.is_match(release.title),
        };
        hit != self.negate
    }
}

impl CustomFormat {
    pub fn matches(&self, release: &ReleaseFacts<'_>) -> bool {
        if self.specifications.is_empty() {
            return false;
        }
        let mut kinds: Vec<u8> = self
            .specifications
            .iter()
            .map(|s| s.condition.kind())
            .collect();
        kinds.sort_unstable();
        kinds.dedup();
        kinds.into_iter().all(|kind| {
            let of_kind = || {
                self.specifications
                    .iter()
                    .filter(move |s| s.condition.kind() == kind)
            };
            let required_ok = of_kind().filter(|s| s.required).all(|s| s.matches(release));
            let mut optional = of_kind().filter(|s| !s.required).peekable();
            let optional_ok = optional.peek().is_none() || optional.any(|s| s.matches(release));
            required_ok && optional_ok
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(condition: Condition, negate: bool, required: bool) -> Specification {
        Specification {
            name: String::new(),
            condition,
            negate,
            required,
        }
    }

    fn title(p: &str) -> Condition {
        Condition::ReleaseTitle(Pattern::new(p).unwrap())
    }

    fn format(specs: Vec<Specification>) -> CustomFormat {
        CustomFormat {
            id: CustomFormatId(1),
            name: "test".into(),
            specifications: specs,
        }
    }

    fn check(cf: &CustomFormat, release_title: &str) -> bool {
        let parsed = sofarr_parser::parse(release_title);
        cf.matches(&ReleaseFacts {
            title: release_title,
            parsed: &parsed,
            languages: &parsed.languages,
            original_language: None,
        })
    }

    #[test]
    fn same_kind_is_any_of() {
        let cf = format(vec![
            spec(title(r"\bx265\b"), false, false),
            spec(title(r"\bhevc\b"), false, false),
        ]);
        assert!(check(&cf, "Some.Movie.2020.1080p.BluRay.HEVC-GRP"));
        assert!(check(&cf, "Some.Movie.2020.1080p.BluRay.x265-GRP"));
        assert!(!check(&cf, "Some.Movie.2020.1080p.BluRay.x264-GRP"));
    }

    #[test]
    fn required_and_negate() {
        let cf = format(vec![
            spec(title(r"\b2160p\b"), false, true),
            spec(title(r"\bHDR\b"), true, true),
        ]);
        assert!(check(&cf, "Some.Movie.2020.2160p.WEB-DL.x265-GRP"));
        assert!(!check(&cf, "Some.Movie.2020.2160p.HDR.WEB-DL.x265-GRP"));
        assert!(!check(&cf, "Some.Movie.2020.1080p.WEB-DL.x265-GRP"));
    }

    #[test]
    fn different_kinds_must_all_match() {
        let cf = format(vec![
            spec(
                Condition::Language {
                    language: LanguageRef::Language(Language::German),
                    except: false,
                },
                false,
                false,
            ),
            spec(
                Condition::ReleaseGroup(Pattern::new("^grp$").unwrap()),
                false,
                false,
            ),
        ]);
        assert!(check(&cf, "Some.Movie.2020.German.1080p.WEB-DL.x264-GRP"));
        assert!(!check(&cf, "Some.Movie.2020.1080p.WEB-DL.x264-GRP"));
        assert!(!check(
            &cf,
            "Some.Movie.2020.German.1080p.WEB-DL.x264-OTHER"
        ));
    }

    #[test]
    fn lookaround_patterns_compile() {
        let cf = format(vec![spec(title(r"(?<!DL\.)German"), false, false)]);
        assert!(check(&cf, "Some.Movie.2020.German.1080p.WEB-DL.x264-GRP"));
        assert!(!check(
            &cf,
            "Some.Movie.2020.DL.German.1080p.WEB-DL.x264-GRP"
        ));
    }

    #[test]
    fn empty_format_never_matches() {
        assert!(!check(&format(Vec::new()), "Some.Movie.2020.1080p-GRP"));
    }
}
