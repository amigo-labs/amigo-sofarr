//! Configuration loaded from a TOML file. A missing file means "all defaults".

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde::Deserialize;

pub const DEFAULT_PORT: u16 = 7676;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub server: ServerConfig,
    pub log: LogConfig,
    pub database: DatabaseConfig,
    pub automation: AutomationConfig,
    pub rss: RssConfig,
    pub tmdb: Option<TmdbConfig>,
    pub radarr: Option<RadarrConfig>,
    /// The Sonarr instance for series import and comparisons (Phase 5).
    pub sonarr: Option<RadarrConfig>,
    pub tvdb: Option<TvdbConfig>,
    #[serde(rename = "indexer")]
    pub indexers: Vec<IndexerConfig>,
    pub nzbget: Option<NzbgetConfig>,
    pub download: DownloadConfig,
    pub import_lists: ImportListConfig,
    pub library: LibraryConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LibraryConfig {
    /// Hours between metadata refreshes (TMDB/TVDB, if configured) with the disk scan
    /// that `rescanAfterRefresh` asks for; 0 disables them.
    pub refresh_interval_hours: u64,
}

impl Default for LibraryConfig {
    fn default() -> Self {
        Self {
            refresh_interval_hours: 24,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NzbgetConfig {
    /// Base URL, e.g. `http://nzbget:6789`.
    pub url: String,
    pub username: String,
    pub password: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default = "default_tv_category")]
    pub tv_category: String,
}

fn default_tv_category() -> String {
    "tv".to_owned()
}

fn default_category() -> String {
    "movies".to_owned()
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DownloadConfig {
    /// Minutes between checks of the download client.
    pub check_interval_minutes: u64,
    /// Path translations between the download client's view and sofarr's.
    pub path_map: Vec<PathMap>,
    /// After an import, remove the download from the client's history and delete what
    /// is left of its folder (only if no video remains), like "Remove Completed".
    pub remove_completed: bool,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            check_interval_minutes: 2,
            path_map: Vec::new(),
            remove_completed: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathMap {
    /// Prefix as the download client reports it.
    pub remote: String,
    /// The same folder inside the sofarr container.
    pub local: String,
}

impl DownloadConfig {
    /// Translates a path reported by the download client.
    pub fn map_path(&self, remote: &str) -> PathBuf {
        for map in &self.path_map {
            let prefix = map.remote.trim_end_matches('/');
            if let Some(rest) = remote.strip_prefix(prefix)
                && (rest.is_empty() || rest.starts_with('/'))
            {
                return PathBuf::from(format!("{}{rest}", map.local.trim_end_matches('/')));
            }
        }
        PathBuf::from(remote)
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImportListConfig {
    /// Hours between import list syncs; 0 disables them.
    pub interval_hours: u64,
}

impl Default for ImportListConfig {
    fn default() -> Self {
        Self { interval_hours: 24 }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DatabaseConfig {
    pub path: PathBuf,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("/config/sofarr.db"),
        }
    }
}

/// Whether sofarr acts or only logs what it would do.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Read-only: decisions are logged and stored, nothing is sent or moved.
    #[default]
    Shadow,
    /// Grabs go to the download client, finished downloads are imported.
    Live,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AutomationConfig {
    pub mode: Mode,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RssConfig {
    /// Minutes between RSS syncs; 0 disables the scheduler.
    pub interval_minutes: u64,
}

impl Default for RssConfig {
    fn default() -> Self {
        Self {
            interval_minutes: 30,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TmdbConfig {
    /// TMDB v3 API key or v4 read access token.
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TvdbConfig {
    pub api_key: String,
    /// Subscriber PIN, if the key needs one.
    pub pin: Option<String>,
}

/// The Radarr instance sofarr imports from and compares against.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadarrConfig {
    pub url: String,
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexerConfig {
    pub name: String,
    /// Full Newznab API endpoint, e.g. `https://indexer.example/api`.
    pub url: String,
    pub api_key: String,
    #[serde(default = "default_categories")]
    pub categories: Vec<u32>,
    /// Series categories, fetched together with the movie categories.
    #[serde(default = "default_tv_categories")]
    pub tv_categories: Vec<u32>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// Tag labels (as in Radarr/Sonarr): if set, the indexer only serves movies and
    /// series with one of these tags.
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_tv_categories() -> Vec<u32> {
    vec![5000]
}

fn default_categories() -> Vec<u32> {
    vec![2000]
}

fn default_limit() -> u32 {
    100
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    pub bind: SocketAddr,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), DEFAULT_PORT),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LogConfig {
    /// Default filter if `RUST_LOG` is not set, e.g. `info` or `sofarr_server=debug`.
    pub level: String,
    pub format: LogFormat,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".to_owned(),
            format: LogFormat::Pretty,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Pretty,
    Json,
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("Konfiguration {} nicht lesbar", path.display()))?;
        Self::parse(&raw).with_context(|| format!("Konfiguration {} ungültig", path.display()))
    }

    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        let config: Self = toml::from_str(raw)?;
        let mut names: Vec<&str> = config.indexers.iter().map(|i| i.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        anyhow::ensure!(
            names.len() == config.indexers.len(),
            "Indexer-Namen müssen eindeutig sein"
        );
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_yields_defaults() {
        let config = Config::parse("").expect("empty config must parse");
        assert_eq!(config.server.bind.port(), DEFAULT_PORT);
        assert_eq!(config.log.format, LogFormat::Pretty);
    }

    #[test]
    fn example_config_is_valid() {
        let raw = include_str!("../../../config/sofarr.example.toml");
        Config::parse(raw).expect("config/sofarr.example.toml must stay valid");
    }

    #[test]
    fn full_config_parses() {
        let config = Config::parse(
            "[automation]\nmode = \"live\"\n[rss]\ninterval_minutes = 15\n\
             [tmdb]\napi_key = \"k\"\n[[indexer]]\nname = \"a\"\nurl = \"http://i/api\"\napi_key = \"k\"\n",
        )
        .expect("valid");
        assert_eq!(config.automation.mode, Mode::Live);
        assert_eq!(config.rss.interval_minutes, 15);
        assert_eq!(config.indexers[0].categories, [2000]);
        assert!(
            Config::parse("[[indexer]]\nname = \"a\"\nurl = \"u\"\napi_key = \"k\"\n[[indexer]]\nname = \"a\"\nurl = \"u\"\napi_key = \"k\"\n")
                .is_err()
        );
    }

    #[test]
    fn maps_download_paths() {
        let config = Config::parse(
            "[[download.path_map]]\nremote = \"/downloads/\"\nlocal = \"/data/usenet\"\n",
        )
        .expect("valid");
        assert_eq!(
            config.download.map_path("/downloads/completed/movies/A"),
            PathBuf::from("/data/usenet/completed/movies/A")
        );
        assert_eq!(
            config.download.map_path("/downloadsx/a"),
            PathBuf::from("/downloadsx/a")
        );
        assert_eq!(config.download.check_interval_minutes, 2);
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(Config::parse("[server]\nbnid = \"0.0.0.0:1\"").is_err());
    }
}
