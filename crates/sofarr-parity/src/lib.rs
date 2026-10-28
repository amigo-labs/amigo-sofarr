//! Parity tooling: collects oracle fixtures from running Radarr/Sonarr instances
//! (black-box, via their HTTP API only) and summarises their configuration.
//!
//! The binary lives in `main.rs`; the modules are a library so integration tests can
//! drive them against a mock server.

pub mod collect;
pub mod config;
pub mod export;
pub mod http;
pub mod matrix;
pub mod newznab;
pub mod radarr;
pub mod store;
