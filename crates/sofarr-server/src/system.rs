//! `GET /api/system` (spec web-ui-shell AC-12): version, mode, scheduled jobs, the last
//! contact with the download client and each indexer, and the configuration in effect
//! with every secret masked.

use axum::{Json, extract::State};
use jiff::Timestamp;
use serde::Serialize;
use serde_json::Value;

use crate::{
    app::{App, SharedApp},
    config::Mode,
    status::{Job, Peer},
};

/// What replaces a secret in the configuration.
pub const MASK: &str = "***";

/// Keys whose values are secrets.
const SECRET_KEYS: [&str; 7] = [
    "password",
    "password_hash",
    "api_key",
    "key",
    "token",
    "pin",
    "secret",
];

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SystemAnswer {
    pub version: String,
    pub mode: Mode,
    pub jobs: Vec<JobAnswer>,
    pub connections: Vec<ConnectionAnswer>,
    /// The configuration in effect (defaults filled in), secrets masked.
    #[cfg_attr(test, ts(type = "Record<string, unknown>"))]
    pub config: Value,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct JobAnswer {
    pub name: String,
    pub label: String,
    /// `null` when the job is switched off.
    pub interval_minutes: Option<u32>,
    pub last_run: Option<String>,
    pub result: Option<JobResult>,
    pub next_run: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum JobResult {
    Ok,
    Error { message: String },
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PeerKind {
    DownloadClient,
    Indexer,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ContactState {
    NotConfigured,
    NoContact,
    Ok,
    Error,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ConnectionAnswer {
    pub kind: PeerKind,
    /// The indexer's name, or the download client's (`nzbget`); `null` for "no indexer".
    pub name: Option<String>,
    pub state: ContactState,
    pub at: Option<String>,
    pub message: Option<String>,
}

fn time(t: Option<Timestamp>) -> Option<String> {
    t.map(|t| t.to_string())
}

fn jobs(app: &App, now: Timestamp) -> Vec<JobAnswer> {
    Job::ALL
        .iter()
        .map(|&job| {
            let interval = job.interval(app);
            let state = app.status.job(job);
            let next_run = match (interval, job) {
                (None, _) => None,
                (Some(_), Job::Backup) => app.next_backup(now).ok().flatten(),
                (Some(_), _) => state.next_run,
            };
            JobAnswer {
                name: job.name().to_owned(),
                label: job.label().to_owned(),
                interval_minutes: interval
                    .map(|d| u32::try_from(d.as_secs() / 60).unwrap_or(u32::MAX)),
                last_run: time(state.last_run),
                result: state.result.map(|r| match r {
                    Ok(()) => JobResult::Ok,
                    Err(message) => JobResult::Error { message },
                }),
                next_run: time(next_run),
            }
        })
        .collect()
}

fn connection(
    app: &App,
    kind: PeerKind,
    name: Option<String>,
    peer: Option<Peer>,
) -> ConnectionAnswer {
    let contact = peer.as_ref().and_then(|p| app.status.last_contact(p));
    let state = match (&peer, &contact) {
        (None, _) => ContactState::NotConfigured,
        (Some(_), None) => ContactState::NoContact,
        (Some(_), Some(c)) if c.result.is_ok() => ContactState::Ok,
        (Some(_), Some(_)) => ContactState::Error,
    };
    ConnectionAnswer {
        kind,
        name,
        state,
        at: time(contact.as_ref().map(|c| c.at)),
        message: contact.and_then(|c| c.result.err()),
    }
}

fn connections(app: &App) -> Vec<ConnectionAnswer> {
    let mut out = vec![connection(
        app,
        PeerKind::DownloadClient,
        Some("nzbget".to_owned()),
        app.nzbget.as_ref().map(|_| Peer::DownloadClient),
    )];
    if app.indexers.is_empty() {
        out.push(connection(app, PeerKind::Indexer, None, None));
    }
    for indexer in &app.indexers {
        let name = indexer.name().to_owned();
        out.push(connection(
            app,
            PeerKind::Indexer,
            Some(name.clone()),
            Some(Peer::Indexer(name)),
        ));
    }
    out
}

/// A URL with its user info and query replaced by [`MASK`]; `None` for other text.
fn mask_url(text: &str) -> Option<String> {
    let (scheme, rest) = text.split_once("://")?;
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    {
        return None;
    }
    let rest = rest.split('#').next().unwrap_or_default();
    let (before_query, query) = match rest.split_once('?') {
        Some((b, _)) => (b, true),
        None => (rest, false),
    };
    let (authority, path) = match before_query.find('/') {
        Some(i) => before_query.split_at(i),
        None => (before_query, ""),
    };
    let authority = match authority.rsplit_once('@') {
        Some((_, host)) => format!("{MASK}@{host}"),
        None => authority.to_owned(),
    };
    let query = if query {
        format!("?{MASK}")
    } else {
        String::new()
    };
    Some(format!("{scheme}://{authority}{path}{query}"))
}

/// Masks every secret in a configuration value: values of [`SECRET_KEYS`] (unless
/// empty) and the user info and query of URLs.
pub fn redact(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let secret = SECRET_KEYS.contains(&key.as_str()) && !value.is_null();
                    let value = if secret {
                        Value::String(MASK.to_owned())
                    } else {
                        redact(value)
                    };
                    (key, value)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(redact).collect()),
        Value::String(text) => Value::String(mask_url(&text).unwrap_or(text)),
        other => other,
    }
}

pub fn answer(app: &App, now: Timestamp) -> SystemAnswer {
    SystemAnswer {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        mode: app.config.automation.mode,
        jobs: jobs(app, now),
        connections: connections(app),
        config: redact(serde_json::to_value(&app.config).unwrap_or(Value::Null)),
    }
}

/// `GET /api/system`.
pub async fn system(State(app): State<SharedApp>) -> Json<SystemAnswer> {
    Json(answer(&app, Timestamp::now()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn urls_lose_user_info_and_query() {
        assert_eq!(
            mask_url("http://user:pw@nzbget:6789/jsonrpc?x=1#f").as_deref(),
            Some("http://***@nzbget:6789/jsonrpc?***")
        );
        assert_eq!(
            mask_url("https://indexer.example/api").as_deref(),
            Some("https://indexer.example/api")
        );
        assert_eq!(mask_url("/data/movies"), None);
        assert_eq!(mask_url("Kein Link: ://"), None);
    }

    #[test]
    fn secrets_are_masked_at_any_depth() {
        let config = json!({
            "auth": {"password_hash": "$argon2id$x", "api_key": null, "username": "daniel"},
            "indexer": [{"name": "idx", "api_key": "k1", "url": "https://i.example/api?apikey=k1"}],
            "tvdb": {"api_key": "k2", "pin": "1234"},
            "nzbget": {"password": "pw", "url": "http://u:p@host:6789"},
            "other": {"token": "t", "secret": "s", "key": "k"}
        });
        let text = redact(config).to_string();
        for secret in [
            "$argon2id$x",
            "k1",
            "k2",
            "1234",
            "pw",
            "u:p",
            "\"t\"",
            "\"s\"",
            "\"k\"",
        ] {
            assert!(!text.contains(secret), "{secret} in {text}");
        }
        assert!(text.contains("daniel") && text.contains("idx"), "{text}");
        assert!(text.contains("\"api_key\":null"), "{text}");
    }
}
