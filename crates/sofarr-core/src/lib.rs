//! Domain model and decision engine: qualities, quality profiles,
//! custom formats, and grab/upgrade decisions.
//!
//! Depends on `sofarr-parser`; must stay free of IO (design: docs/specs/sofarr.md §3).

pub mod availability;
pub mod custom_format;
pub mod decision;
pub mod definition;
pub mod import;
pub mod language;
pub mod naming;
pub mod pattern;
pub mod profile;
pub mod quality;
pub mod series;
pub mod settings;
mod upgrade;

pub use custom_format::{CustomFormat, CustomFormatId, ReleaseFacts};
pub use decision::{
    Decision, ExistingFile, MovieContext, Rejection, ReleaseInput, compare, decide, evaluate,
};
pub use definition::{QualityDefinition, QualityDefinitions};
pub use import::RadarrConfig;
pub use language::LanguageRef;
pub use profile::{ProfileId, ProfileItem, QualityProfile};
pub use quality::{NamedQuality, QualityCatalog, QualityId};
pub use settings::{PropersPolicy, Settings};

/// Normalises a title for comparisons: lower-case letters and digits only;
/// `&` counts as `and`.
pub fn normalize_title(title: &str) -> String {
    title
        .replace('&', " and ")
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Normalises a release name for the blocklist: lower-case, every run of characters
/// that are not letters or digits becomes one space, no leading or trailing space.
pub fn normalize_release_title(title: &str) -> String {
    title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_titles_differing_in_case_and_separators_are_equal() {
        let a = normalize_release_title("Film.2020.1080p");
        assert_eq!(a, "film 2020 1080p");
        assert_eq!(normalize_release_title("film 2020 1080p"), a);
        assert_eq!(normalize_release_title("Film_2020-1080p."), a);
        assert_eq!(normalize_release_title("..Film -- 2020__1080p"), a);
        // Separators still separate: joined words are a different release.
        assert_ne!(normalize_release_title("Film2020.1080p"), a);
    }
}
