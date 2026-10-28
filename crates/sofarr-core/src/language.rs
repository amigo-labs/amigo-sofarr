//! Languages as profiles and custom formats refer to them.
//!
//! The v3 API identifies languages by id. The ids below are the ones the oracle
//! returned in the public parse fixtures; unobserved ids are rejected on import so
//! a gap shows up instead of silently matching nothing.

use sofarr_parser::Language;

/// A language reference from the config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageRef {
    /// No language could be recognised.
    Unknown,
    /// The film's original language.
    Original,
    /// Any language (profile setting only).
    Any,
    Language(Language),
}

/// `(id, language)` as observed in the oracle's parse responses.
const OBSERVED_IDS: &[(i64, LanguageRef)] = &[
    (-2, LanguageRef::Original),
    (0, LanguageRef::Unknown),
    (1, LanguageRef::Language(Language::English)),
    (2, LanguageRef::Language(Language::French)),
    (3, LanguageRef::Language(Language::Spanish)),
    (4, LanguageRef::Language(Language::German)),
    (5, LanguageRef::Language(Language::Italian)),
    (6, LanguageRef::Language(Language::Danish)),
    (7, LanguageRef::Language(Language::Dutch)),
    (8, LanguageRef::Language(Language::Japanese)),
    (10, LanguageRef::Language(Language::Chinese)),
    (13, LanguageRef::Language(Language::Vietnamese)),
    (14, LanguageRef::Language(Language::Swedish)),
    (15, LanguageRef::Language(Language::Norwegian)),
    (21, LanguageRef::Language(Language::Korean)),
    (23, LanguageRef::Language(Language::Hebrew)),
    (28, LanguageRef::Language(Language::Thai)),
];

impl LanguageRef {
    pub fn from_id(id: i64) -> Option<Self> {
        OBSERVED_IDS.iter().find(|(i, _)| *i == id).map(|(_, l)| *l)
    }

    /// Resolves the English name the API uses (`"German"`, `"Any"`, …).
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Unknown" => Some(Self::Unknown),
            "Original" => Some(Self::Original),
            "Any" => Some(Self::Any),
            _ => LANGUAGES
                .iter()
                .find(|l| l.name().eq_ignore_ascii_case(name))
                .map(|l| Self::Language(*l)),
        }
    }

    /// Whether a release with `languages` is in this language.
    ///
    /// `original` is the film's original language, if known.
    pub fn matches(self, languages: &[Language], original: Option<Language>) -> bool {
        match self {
            Self::Any => true,
            Self::Unknown => languages.is_empty(),
            Self::Original => {
                languages.contains(&Language::Original)
                    || original.is_some_and(|o| languages.contains(&o))
            }
            Self::Language(l) => languages.contains(&l),
        }
    }
}

const LANGUAGES: &[Language] = &[
    Language::Arabic,
    Language::Bulgarian,
    Language::Chinese,
    Language::Czech,
    Language::Danish,
    Language::Dutch,
    Language::English,
    Language::Finnish,
    Language::French,
    Language::German,
    Language::Greek,
    Language::Hebrew,
    Language::Hindi,
    Language::Hungarian,
    Language::Icelandic,
    Language::Italian,
    Language::Japanese,
    Language::Korean,
    Language::Lithuanian,
    Language::Norwegian,
    Language::Polish,
    Language::Portuguese,
    Language::Romanian,
    Language::Russian,
    Language::Spanish,
    Language::Swedish,
    Language::Thai,
    Language::Turkish,
    Language::Ukrainian,
    Language::Vietnamese,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_ids_and_names() {
        assert_eq!(
            LanguageRef::from_id(4),
            Some(LanguageRef::Language(Language::German))
        );
        assert_eq!(LanguageRef::from_id(-2), Some(LanguageRef::Original));
        assert_eq!(LanguageRef::from_id(9999), None);
        assert_eq!(
            LanguageRef::from_name("english"),
            Some(LanguageRef::Language(Language::English))
        );
        assert_eq!(LanguageRef::from_name("Any"), Some(LanguageRef::Any));
        assert_eq!(LanguageRef::from_name("Klingon"), None);
    }

    #[test]
    fn matching() {
        let german = LanguageRef::Language(Language::German);
        assert!(german.matches(&[Language::German, Language::English], None));
        assert!(!german.matches(&[Language::English], None));
        assert!(LanguageRef::Unknown.matches(&[], None));
        assert!(LanguageRef::Original.matches(&[Language::Original], None));
        assert!(LanguageRef::Original.matches(&[Language::French], Some(Language::French)));
        assert!(!LanguageRef::Original.matches(&[Language::German], Some(Language::French)));
        assert!(LanguageRef::Any.matches(&[], None));
    }
}
