//! Configuration loaded from a TOML file. A missing file means "all defaults".

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
};

use anyhow::Context;
use serde::Deserialize;

pub const DEFAULT_PORT: u16 = 7676;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub server: ServerConfig,
    pub log: LogConfig,
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
        Ok(toml::from_str(raw)?)
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
    fn unknown_keys_are_rejected() {
        assert!(Config::parse("[server]\nbnid = \"0.0.0.0:1\"").is_err());
    }
}
