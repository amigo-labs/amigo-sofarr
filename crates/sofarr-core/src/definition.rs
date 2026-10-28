//! Quality definitions: size limits per quality, in megabytes per minute of runtime.

use crate::quality::{NamedQuality, QualityCatalog, QualityId};

#[derive(Debug, Clone, PartialEq)]
pub struct QualityDefinition {
    pub quality: NamedQuality,
    pub title: String,
    pub min_size: f64,
    /// `None` means unlimited.
    pub max_size: Option<f64>,
    pub preferred_size: Option<f64>,
}

impl QualityDefinition {
    /// Allowed size range in bytes for a film of `runtime_minutes`.
    pub fn size_limits(&self, runtime_minutes: u32) -> (u64, Option<u64>) {
        let bytes = |mb_per_minute: f64| (mb_per_minute * f64::from(runtime_minutes) * MB) as u64;
        (bytes(self.min_size), self.max_size.map(bytes))
    }
}

const MB: f64 = 1024.0 * 1024.0;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct QualityDefinitions(pub Vec<QualityDefinition>);

impl QualityDefinitions {
    pub fn get(&self, quality: QualityId) -> Option<&QualityDefinition> {
        self.0.iter().find(|d| d.quality.id == quality)
    }

    /// Catalog of all defined qualities; empty definitions give the observed catalog.
    pub fn catalog(&self) -> QualityCatalog {
        if self.0.is_empty() {
            QualityCatalog::observed()
        } else {
            QualityCatalog::new(self.0.iter().map(|d| d.quality.clone()).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_scale_with_runtime() {
        let quality = QualityCatalog::observed()
            .by_id(QualityId(7))
            .unwrap()
            .clone();
        let def = QualityDefinition {
            quality,
            title: "Bluray-1080p".into(),
            min_size: 10.0,
            max_size: None,
            preferred_size: None,
        };
        let (min, max) = def.size_limits(100);
        assert_eq!(min, 1000 * 1024 * 1024);
        assert_eq!(max, None);
    }
}
