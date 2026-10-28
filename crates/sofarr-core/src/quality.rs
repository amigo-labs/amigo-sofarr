//! Named qualities: the fixed steps (`WEBDL-1080p`, `Remux-2160p`, …) that profiles,
//! definitions and decisions refer to.
//!
//! A named quality is identified by its parts (source, resolution, modifier); the
//! parser already reports parts in the oracle's vocabulary. Ids and names are only
//! known from black-box observation: the built-in catalog holds what the oracle
//! returned for the public parse fixtures, a full one comes from the quality
//! definitions of the config export.

use std::fmt;

use sofarr_parser::{Modifier, Quality as Parts, Resolution, Source};

/// Id of a named quality as used by the v3 API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QualityId(pub u32);

impl fmt::Display for QualityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedQuality {
    pub id: QualityId,
    pub name: String,
    pub parts: Parts,
}

/// The id the oracle reports when nothing is known about the quality.
pub const UNKNOWN: QualityId = QualityId(0);

/// Qualities returned by the oracle for the public parse fixtures:
/// id, name, source, resolution (0 = none), modifier.
const OBSERVED: &[(u32, &str, Source, u16, Modifier)] = &[
    (0, "Unknown", Source::Unknown, 0, Modifier::None),
    (1, "SDTV", Source::Tv, 480, Modifier::None),
    (2, "DVD", Source::Dvd, 0, Modifier::None),
    (3, "WEBDL-1080p", Source::WebDl, 1080, Modifier::None),
    (4, "HDTV-720p", Source::Tv, 720, Modifier::None),
    (5, "WEBDL-720p", Source::WebDl, 720, Modifier::None),
    (6, "Bluray-720p", Source::BluRay, 720, Modifier::None),
    (7, "Bluray-1080p", Source::BluRay, 1080, Modifier::None),
    (8, "WEBDL-480p", Source::WebDl, 480, Modifier::None),
    (9, "HDTV-1080p", Source::Tv, 1080, Modifier::None),
    (12, "WEBRip-480p", Source::WebRip, 480, Modifier::None),
    (14, "WEBRip-720p", Source::WebRip, 720, Modifier::None),
    (15, "WEBRip-1080p", Source::WebRip, 1080, Modifier::None),
    (16, "HDTV-2160p", Source::Tv, 2160, Modifier::None),
    (17, "WEBRip-2160p", Source::WebRip, 2160, Modifier::None),
    (18, "WEBDL-2160p", Source::WebDl, 2160, Modifier::None),
    (19, "Bluray-2160p", Source::BluRay, 2160, Modifier::None),
    (20, "Bluray-480p", Source::BluRay, 480, Modifier::None),
    (21, "Bluray-576p", Source::BluRay, 576, Modifier::None),
    (22, "BR-DISK", Source::BluRay, 1080, Modifier::BrDisk),
    (23, "DVD-R", Source::Dvd, 480, Modifier::Remux),
    (24, "WORKPRINT", Source::Workprint, 0, Modifier::None),
    (26, "TELESYNC", Source::Telesync, 1080, Modifier::None),
    (29, "REGIONAL", Source::Dvd, 480, Modifier::Regional),
    (30, "Remux-1080p", Source::BluRay, 1080, Modifier::Remux),
    (31, "Remux-2160p", Source::BluRay, 2160, Modifier::Remux),
];

/// Builds parts from the API vocabulary (`"webdl"`, `1080`, `"none"`).
pub fn parts_from_api(source: &str, resolution: u32, modifier: &str) -> Option<Parts> {
    Some(Parts {
        source: *Source::ALL.iter().find(|s| s.as_str() == source)?,
        resolution: match resolution {
            0 => Resolution::Unknown,
            h => Resolution::from_height(h)?,
        },
        modifier: *Modifier::ALL.iter().find(|m| m.as_str() == modifier)?,
    })
}

/// A set of named qualities, always including [`UNKNOWN`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityCatalog {
    qualities: Vec<NamedQuality>,
}

impl QualityCatalog {
    /// Catalog from the given qualities; adds `Unknown` if missing.
    pub fn new(mut qualities: Vec<NamedQuality>) -> Self {
        if !qualities.iter().any(|q| q.id == UNKNOWN) {
            qualities.insert(0, observed_entry(OBSERVED[0]));
        }
        Self { qualities }
    }

    /// The qualities observed in the public parse fixtures.
    pub fn observed() -> Self {
        Self::new(OBSERVED.iter().copied().map(observed_entry).collect())
    }

    pub fn iter(&self) -> impl Iterator<Item = &NamedQuality> {
        self.qualities.iter()
    }

    pub fn by_id(&self, id: QualityId) -> Option<&NamedQuality> {
        self.qualities.iter().find(|q| q.id == id)
    }

    /// The named quality with exactly these parts, if the catalog has one.
    pub fn find(&self, parts: &Parts) -> Option<&NamedQuality> {
        self.qualities.iter().find(|q| q.parts == *parts)
    }

    /// The named quality for parsed parts; `Unknown` if no quality has these parts.
    ///
    /// Without an exact match, the resolution is ignored when only one quality has this
    /// source and modifier: the oracle reports `TELESYNC` at 0, 720 and 1080 lines.
    pub fn resolve(&self, parts: &Parts) -> &NamedQuality {
        self.find(parts)
            .or_else(|| {
                let mut same = self.qualities.iter().filter(|q| {
                    q.id != UNKNOWN
                        && q.parts.source == parts.source
                        && q.parts.modifier == parts.modifier
                });
                let only = same.next()?;
                same.next().is_none().then_some(only)
            })
            .unwrap_or_else(|| self.unknown())
    }

    pub fn unknown(&self) -> &NamedQuality {
        self.by_id(UNKNOWN)
            .expect("catalog always contains Unknown")
    }
}

fn observed_entry(
    (id, name, source, height, modifier): (u32, &str, Source, u16, Modifier),
) -> NamedQuality {
    NamedQuality {
        id: QualityId(id),
        name: name.to_owned(),
        parts: Parts {
            source,
            resolution: Resolution::from_height(height.into()).unwrap_or_default(),
            modifier,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observed_parts_are_unique() {
        let catalog = QualityCatalog::observed();
        for q in catalog.iter() {
            assert_eq!(
                catalog.find(&q.parts).map(|f| f.id),
                Some(q.id),
                "{}",
                q.name
            );
        }
    }

    #[test]
    fn resolves_parsed_release() {
        let catalog = QualityCatalog::observed();
        let parsed = sofarr_parser::parse("Some.Movie.2020.1080p.WEB-DL.DDP5.1.H.264-GRP");
        assert_eq!(catalog.resolve(&parsed.quality).name, "WEBDL-1080p");
    }

    #[test]
    fn unmatched_parts_are_unknown() {
        let catalog = QualityCatalog::observed();
        let parts = Parts {
            source: Source::Cam,
            resolution: Resolution::R720,
            modifier: Modifier::None,
        };
        assert_eq!(catalog.resolve(&parts).id, UNKNOWN);
    }

    #[test]
    fn unique_source_ignores_resolution() {
        let catalog = QualityCatalog::new(vec![
            observed_entry((26, "TELESYNC", Source::Telesync, 0, Modifier::None)),
            observed_entry((3, "WEBDL-1080p", Source::WebDl, 1080, Modifier::None)),
            observed_entry((5, "WEBDL-720p", Source::WebDl, 720, Modifier::None)),
        ]);
        let parts = |source, resolution| Parts {
            source,
            resolution,
            modifier: Modifier::None,
        };
        assert_eq!(
            catalog
                .resolve(&parts(Source::Telesync, Resolution::R1080))
                .name,
            "TELESYNC"
        );
        // Several WEB-DL qualities: no guess.
        assert_eq!(
            catalog.resolve(&parts(Source::WebDl, Resolution::R480)).id,
            UNKNOWN
        );
        assert_eq!(
            catalog.resolve(&parts(Source::Cam, Resolution::R720)).id,
            UNKNOWN
        );
    }

    #[test]
    fn new_catalog_keeps_unknown() {
        let catalog = QualityCatalog::new(Vec::new());
        assert_eq!(catalog.unknown().name, "Unknown");
    }

    #[test]
    fn api_vocabulary() {
        let parts = parts_from_api("bluray", 2160, "remux").unwrap();
        assert_eq!(
            QualityCatalog::observed().resolve(&parts).name,
            "Remux-2160p"
        );
        assert_eq!(
            parts_from_api("dvd", 0, "none").unwrap().resolution,
            Resolution::Unknown
        );
        assert!(parts_from_api("laserdisc", 0, "none").is_none());
        assert!(parts_from_api("dvd", 123, "none").is_none());
    }
}
