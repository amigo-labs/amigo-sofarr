//! Rules for a film that already has a file: is the release an upgrade?
//!
//! Derived from the release oracle (films with a file, profiles with and without
//! upgrades):
//!
//! - The cutoff is met when the file reaches the profile's cutoff quality and its
//!   custom format score reaches the cutoff score. A profile without upgrades counts
//!   its lowest allowed quality as cutoff, so any file meets it. A higher revision of
//!   the file's quality is still taken.
//! - Without upgrades, a release of higher quality is rejected for that reason too.
//! - With upgrades, the release is compared by quality rank, then revision, then score.
//!   Once the file reaches the cutoff quality, a higher quality alone no longer counts,
//!   but a higher revision still does.
//! - A repack only replaces a file of the same release group.

use std::cmp::Ordering;

use sofarr_parser::Revision;

use crate::{
    decision::{ExistingFile, Rejection},
    profile::QualityProfile,
    quality::QualityId,
    settings::PropersPolicy,
};

/// What the upgrade rules look at in the release.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Candidate<'a> {
    pub quality: QualityId,
    pub revision: Revision,
    pub release_group: Option<&'a str>,
    pub score: i32,
}

pub(crate) fn rules(
    profile: &QualityProfile,
    propers: PropersPolicy,
    release: &Candidate<'_>,
    file: &ExistingFile,
) -> Vec<Rejection> {
    let release_rank = profile.rank(release.quality);
    let file_rank = profile.rank(file.quality);
    let quality = release_rank.cmp(&file_rank);
    let revision = match propers {
        PropersPolicy::DoNotPrefer => Ordering::Equal,
        _ => (release.revision.version, release.revision.real)
            .cmp(&(file.revision.version, file.revision.real)),
    };
    let better_revision = quality == Ordering::Equal && revision == Ordering::Greater;

    let cutoff_rank = if profile.upgrade_allowed {
        profile.cutoff_rank()
    } else {
        profile.items.iter().position(|i| i.allowed())
    };
    let quality_cutoff_met = matches!((file_rank, cutoff_rank), (Some(f), Some(c)) if f >= c);
    let cutoff_met = quality_cutoff_met
        && file.custom_format_score >= profile.cutoff_format_score
        && !better_revision;

    let mut rejections = Vec::new();
    if cutoff_met {
        rejections.push(Rejection::CutoffMet);
    }
    if !profile.upgrade_allowed {
        if quality == Ordering::Greater {
            rejections.push(Rejection::UpgradesNotAllowed);
        }
    } else if !cutoff_met
        && let Some(r) = compare(
            profile,
            propers,
            release,
            file,
            quality,
            revision,
            quality_cutoff_met,
        )
    {
        rejections.push(r);
    }
    if better_revision && release.revision.repack {
        match file.release_group.as_deref() {
            None => rejections.push(Rejection::FileGroupUnknown),
            Some(group)
                if !release
                    .release_group
                    .is_some_and(|r| r.eq_ignore_ascii_case(group)) =>
            {
                rejections.push(Rejection::RepackOtherGroup)
            }
            Some(_) => {}
        }
    }
    rejections
}

/// Quality rank, then revision, then custom format score.
fn compare(
    profile: &QualityProfile,
    propers: PropersPolicy,
    release: &Candidate<'_>,
    file: &ExistingFile,
    quality: Ordering,
    revision: Ordering,
    quality_cutoff_met: bool,
) -> Option<Rejection> {
    let revision_upgrades = propers == PropersPolicy::PreferAndUpgrade;
    match (quality, revision) {
        (Ordering::Less, _) => Some(Rejection::LowerQuality),
        (_, Ordering::Less) if quality == Ordering::Equal || quality_cutoff_met => {
            Some(Rejection::LowerRevision)
        }
        (Ordering::Greater, Ordering::Greater) if revision_upgrades => None,
        (Ordering::Greater, _) => quality_cutoff_met.then_some(Rejection::QualityCutoffMet),
        (_, Ordering::Greater) if revision_upgrades => None,
        _ => {
            let gain = profile.min_upgrade_format_score.unwrap_or(1).max(1);
            (release.score < file.custom_format_score + gain).then_some(Rejection::NoBetterScore)
        }
    }
}
