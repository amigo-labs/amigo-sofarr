//! `parity.toml`: where the oracle and the indexers live.

use std::path::Path;

use anyhow::Context;
use serde::Deserialize;

pub const DEFAULT_RATE_LIMIT: f64 = 2.0;
/// Newznab movie category; most indexers include its sub-categories.
pub const DEFAULT_CATEGORIES: &[u32] = &[2000];
/// Releases requested per run; indexers cap this at their own maximum.
pub const DEFAULT_LIMIT: u32 = 200;
/// Pages fetched per run; each page continues where the previous one ended.
pub const DEFAULT_PAGES: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParityConfig {
    pub radarr: RadarrConfig,
    #[serde(default, rename = "indexer")]
    pub indexers: Vec<IndexerConfig>,
    /// Maximum requests per second, over all hosts together.
    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_sec: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadarrConfig {
    pub url: String,
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexerConfig {
    /// Full Newznab API endpoint, e.g. `https://indexer.example/api`.
    pub url: String,
    pub api_key: String,
    #[serde(default = "default_categories")]
    pub categories: Vec<u32>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default = "default_pages")]
    pub pages: u32,
}

fn default_rate_limit() -> f64 {
    DEFAULT_RATE_LIMIT
}

fn default_categories() -> Vec<u32> {
    DEFAULT_CATEGORIES.to_vec()
}

fn default_limit() -> u32 {
    DEFAULT_LIMIT
}

fn default_pages() -> u32 {
    DEFAULT_PAGES
}

impl ParityConfig {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path).with_context(|| {
            format!(
                "Konfiguration {} nicht lesbar (Vorlage: config/parity.example.toml)",
                path.display()
            )
        })?;
        Self::parse(&raw).with_context(|| format!("Konfiguration {} ungültig", path.display()))
    }

    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        let config: Self = toml::from_str(raw)?;
        anyhow::ensure!(
            config.rate_limit_per_sec.is_finite() && config.rate_limit_per_sec > 0.0,
            "rate_limit_per_sec muss größer als 0 sein"
        );
        anyhow::ensure!(
            config.indexers.iter().all(|i| i.limit > 0 && i.pages > 0),
            "limit und pages müssen größer als 0 sein"
        );
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_config_is_valid() {
        let raw = include_str!("../../../config/parity.example.toml");
        let config = ParityConfig::parse(raw).expect("config/parity.example.toml must stay valid");
        assert!(!config.indexers.is_empty());
    }

    #[test]
    fn defaults_apply() {
        let config = ParityConfig::parse(
            "[radarr]\nurl = \"http://r\"\napi_key = \"k\"\n[[indexer]]\nurl = \"http://i/api\"\napi_key = \"k\"\n",
        )
        .expect("valid");
        assert_eq!(config.rate_limit_per_sec, DEFAULT_RATE_LIMIT);
        assert_eq!(config.indexers[0].categories, DEFAULT_CATEGORIES);
        assert_eq!(config.indexers[0].limit, DEFAULT_LIMIT);
        assert_eq!(config.indexers[0].pages, DEFAULT_PAGES);
    }

    #[test]
    fn rejects_unknown_keys_and_bad_rate() {
        assert!(
            ParityConfig::parse("[radarr]\nurl = \"u\"\napi_key = \"k\"\napikey = \"x\"").is_err()
        );
        assert!(
            ParityConfig::parse("rate_limit_per_sec = 0\n[radarr]\nurl = \"u\"\napi_key = \"k\"")
                .is_err()
        );
    }
}
