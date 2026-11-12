//! Configuration loaded from a TOML file. A missing file means "all defaults".

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde::{Deserialize, Serialize};

pub const DEFAULT_PORT: u16 = 7676;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
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
    pub backup: BackupConfig,
    pub auth: AuthConfig,
}

/// The longest session lifetime: ten years, far below where timestamps overflow.
const MAX_SESSION_DAYS: u32 = 3650;

/// `[auth] session_days` as written: anything but an integer that fits becomes 0, which
/// [`AuthConfig::check`] refuses with a German message naming the key.
fn lenient_days<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let value = toml::Value::deserialize(d)?;
    Ok(value
        .as_integer()
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(0))
}

/// `[auth]`: optional login, API key, trusted networks and proxies (spec auth).
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AuthConfig {
    pub enabled: bool,
    pub username: Option<String>,
    /// Argon2id PHC string (`amigo-sofarr hash-password`).
    pub password_hash: Option<String>,
    pub api_key: Option<String>,
    #[serde(deserialize_with = "lenient_days")]
    pub session_days: u32,
    pub trusted_networks: Vec<String>,
    pub trusted_proxies: Vec<String>,
}

/// Secrets never go to the log, not even through a debug print of the config.
impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthConfig")
            .field("enabled", &self.enabled)
            .field("username", &self.username.as_ref().map(|_| "…"))
            .field("password_hash", &self.password_hash.as_ref().map(|_| "…"))
            .field("api_key", &self.api_key.as_ref().map(|_| "…"))
            .field("session_days", &self.session_days)
            .field("trusted_networks", &self.trusted_networks)
            .field("trusted_proxies", &self.trusted_proxies)
            .finish()
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            username: None,
            password_hash: None,
            api_key: None,
            session_days: 30,
            trusted_networks: Vec::new(),
            trusted_proxies: Vec::new(),
        }
    }
}

impl AuthConfig {
    /// While auth is on: everything needed is there and well-formed (German messages
    /// naming the key).
    fn check(&self) -> anyhow::Result<()> {
        if !self.enabled {
            return Ok(());
        }
        anyhow::ensure!(
            self.username.as_deref().is_some_and(|u| !u.is_empty()),
            "[auth] username fehlt"
        );
        let hash = self
            .password_hash
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("[auth] password_hash fehlt"))?;
        anyhow::ensure!(
            crate::auth::is_argon2id(hash),
            "[auth] password_hash ist kein gültiger Argon2id-Hash (amigo-sofarr hash-password)"
        );
        anyhow::ensure!(
            self.api_key.as_deref().is_none_or(|k| !k.is_empty()),
            "[auth] api_key ist leer"
        );
        anyhow::ensure!(
            (1..=MAX_SESSION_DAYS).contains(&self.session_days),
            "[auth] session_days muss eine ganze Zahl zwischen 1 und {MAX_SESSION_DAYS} sein"
        );
        for (key, list) in [
            ("trusted_networks", &self.trusted_networks),
            ("trusted_proxies", &self.trusted_proxies),
        ] {
            for entry in list {
                anyhow::ensure!(
                    entry.parse::<crate::auth_net::Cidr>().is_ok(),
                    "[auth] {key}: „{entry}“ ist keine gültige Adresse oder Adressbereich"
                );
            }
        }
        Ok(())
    }
}

/// `[backup]`: where backups go, how often and how many scheduled ones are kept.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(try_from = "RawBackupConfig")]
pub struct BackupConfig {
    pub dir: PathBuf,
    /// Hours between scheduled backups; 0 disables them.
    pub interval_hours: u64,
    /// Scheduled backups kept (at least 1).
    pub keep: u32,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            dir: PathBuf::from("/config/backups"),
            interval_hours: 24,
            keep: 7,
        }
    }
}

/// `[backup]` as written, checked with German messages that name the key.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBackupConfig {
    dir: Option<PathBuf>,
    interval_hours: Option<toml::Value>,
    keep: Option<toml::Value>,
}

impl TryFrom<RawBackupConfig> for BackupConfig {
    type Error = String;

    fn try_from(raw: RawBackupConfig) -> Result<Self, Self::Error> {
        let defaults = Self::default();
        let whole = |value: Option<toml::Value>, min: i64, key: &str, rule: &str| match value {
            None => Ok(None),
            Some(toml::Value::Integer(n)) if n >= min => Ok(Some(n)),
            Some(_) => Err(format!("[backup] {key} muss {rule} sein")),
        };
        let interval_hours = whole(
            raw.interval_hours,
            0,
            "interval_hours",
            "eine ganze Zahl ab 0",
        )?
        .map_or(Ok(defaults.interval_hours), |n| {
            u64::try_from(n).map_err(|_| "[backup] interval_hours ist zu groß".to_owned())
        })?;
        let keep = whole(raw.keep, 1, "keep", "eine ganze Zahl ab 1")?
            .map_or(Ok(defaults.keep), |n| {
                u32::try_from(n).map_err(|_| "[backup] keep ist zu groß".to_owned())
            })?;
        Ok(Self {
            dir: raw.dir.unwrap_or(defaults.dir),
            interval_hours,
            keep,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DownloadConfig {
    /// Minutes between checks of the download client.
    pub check_interval_minutes: u64,
    /// Path translations between the download client's view and sofarr's.
    pub path_map: Vec<PathMap>,
    /// After an import, remove the download from the client's history and delete what
    /// is left of its folder (only if no video remains), like "Remove Completed".
    pub remove_completed: bool,
    /// Search a movie again after its grab failed; unset: Radarr's imported
    /// `autoRedownloadFailed`, without one off.
    pub redownload_failed_movies: Option<bool>,
    /// The same for series (Sonarr's imported setting).
    pub redownload_failed_series: Option<bool>,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            check_interval_minutes: 2,
            path_map: Vec::new(),
            remove_completed: true,
            redownload_failed_movies: None,
            redownload_failed_series: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Read-only: decisions are logged and stored, nothing is sent or moved.
    #[default]
    Shadow,
    /// Grabs go to the download client, finished downloads are imported.
    Live,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AutomationConfig {
    pub mode: Mode,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TmdbConfig {
    /// TMDB v3 API key or v4 read access token.
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TvdbConfig {
    pub api_key: String,
    /// Subscriber PIN, if the key needs one.
    pub pin: Option<String>,
}

/// The Radarr instance sofarr imports from and compares against.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RadarrConfig {
    pub url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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
        config.auth.check()?;
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
    fn backup_settings_are_checked() {
        let config = Config::parse("").expect("defaults");
        assert_eq!(config.backup.interval_hours, 24);
        assert_eq!(config.backup.keep, 7);
        assert_eq!(config.backup.dir, PathBuf::from("/config/backups"));
        let config =
            Config::parse("[backup]\ninterval_hours = 0\nkeep = 1\ndir = \"/b\"\n").expect("valid");
        assert_eq!((config.backup.interval_hours, config.backup.keep), (0, 1));
        for (raw, key) in [
            ("keep = 0", "keep"),
            ("keep = -3", "keep"),
            ("keep = 2.5", "keep"),
            ("interval_hours = -1", "interval_hours"),
            ("interval_hours = 1.5", "interval_hours"),
            ("interval_hours = \"24\"", "interval_hours"),
        ] {
            let error = Config::parse(&format!("[backup]\n{raw}\n"))
                .err()
                .unwrap_or_else(|| panic!("{raw} must be rejected"));
            let message = format!("{error:#}");
            assert!(
                message.contains(&format!("[backup] {key} muss")),
                "{raw}: {message}"
            );
        }
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(Config::parse("[server]\nbnid = \"0.0.0.0:1\"").is_err());
    }
}
