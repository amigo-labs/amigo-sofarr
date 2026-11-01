//! Quality profiles: which qualities are wanted, in which order, and when to stop
//! upgrading.

use std::fmt;

use crate::{custom_format::CustomFormatId, language::LanguageRef, quality::QualityId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProfileId(pub u32);

impl fmt::Display for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityProfile {
    pub id: ProfileId,
    pub name: String,
    pub upgrade_allowed: bool,
    /// Id of the item (quality or group) at which upgrading stops.
    pub cutoff: u32,
    /// Ordered from lowest to highest preference.
    pub items: Vec<ProfileItem>,
    pub format_scores: Vec<(CustomFormatId, i32)>,
    pub min_format_score: i32,
    pub cutoff_format_score: i32,
    /// Minimum score gain for an upgrade; `None` if the export does not have it.
    pub min_upgrade_format_score: Option<i32>,
    pub language: LanguageRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileItem {
    Quality {
        quality: QualityId,
        allowed: bool,
    },
    /// Qualities of a group rank equally.
    Group {
        id: u32,
        name: String,
        allowed: bool,
        qualities: Vec<QualityId>,
    },
}

impl ProfileItem {
    pub fn allowed(&self) -> bool {
        match self {
            Self::Quality { allowed, .. } | Self::Group { allowed, .. } => *allowed,
        }
    }

    /// The id the cutoff refers to.
    pub fn id(&self) -> u32 {
        match self {
            Self::Quality { quality, .. } => quality.0,
            Self::Group { id, .. } => *id,
        }
    }

    pub fn contains(&self, quality: QualityId) -> bool {
        match self {
            Self::Quality { quality: q, .. } => *q == quality,
            Self::Group { qualities, .. } => qualities.contains(&quality),
        }
    }
}

impl QualityProfile {
    /// Position of the quality in the profile (higher is better), if listed.
    pub fn rank(&self, quality: QualityId) -> Option<usize> {
        self.items.iter().position(|i| i.contains(quality))
    }

    pub fn is_allowed(&self, quality: QualityId) -> bool {
        self.items
            .iter()
            .find(|i| i.contains(quality))
            .is_some_and(ProfileItem::allowed)
    }

    /// Position of the cutoff item; `None` if the cutoff points nowhere.
    pub fn cutoff_rank(&self) -> Option<usize> {
        self.items.iter().position(|i| i.id() == self.cutoff)
    }

    /// Profile score for a set of matched custom formats.
    pub fn score(&self, formats: &[CustomFormatId]) -> i32 {
        self.format_scores
            .iter()
            .filter(|(id, _)| formats.contains(id))
            .map(|(_, score)| score)
            .sum()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// SDTV < [WEBDL-1080p, WEBRip-1080p] < Bluray-1080p (cutoff) < Remux-1080p (not allowed).
    pub(crate) fn profile() -> QualityProfile {
        QualityProfile {
            id: ProfileId(1),
            name: "HD".into(),
            upgrade_allowed: true,
            cutoff: 7,
            items: vec![
                ProfileItem::Quality {
                    quality: QualityId(1),
                    allowed: true,
                },
                ProfileItem::Group {
                    id: 1000,
                    name: "WEB 1080p".into(),
                    allowed: true,
                    qualities: vec![QualityId(3), QualityId(15)],
                },
                ProfileItem::Quality {
                    quality: QualityId(7),
                    allowed: true,
                },
                ProfileItem::Quality {
                    quality: QualityId(30),
                    allowed: false,
                },
            ],
            format_scores: vec![(CustomFormatId(1), 100), (CustomFormatId(2), -50)],
            min_format_score: 0,
            cutoff_format_score: 100,
            min_upgrade_format_score: Some(1),
            language: LanguageRef::Any,
        }
    }

    #[test]
    fn ranks_and_groups() {
        let p = profile();
        assert_eq!(p.rank(QualityId(3)), p.rank(QualityId(15)));
        assert!(p.rank(QualityId(7)) > p.rank(QualityId(3)));
        assert_eq!(p.rank(QualityId(19)), None);
        assert_eq!(p.cutoff_rank(), Some(2));
        assert!(p.is_allowed(QualityId(15)));
        assert!(!p.is_allowed(QualityId(30)));
        assert!(!p.is_allowed(QualityId(19)));
    }

    #[test]
    fn scores_sum_matched_formats() {
        let p = profile();
        assert_eq!(p.score(&[CustomFormatId(1), CustomFormatId(2)]), 50);
        assert_eq!(p.score(&[CustomFormatId(9)]), 0);
    }
}
