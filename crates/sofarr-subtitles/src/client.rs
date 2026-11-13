//! The OpenSubtitles.com REST API (public documentation): `POST /login` for a token,
//! `GET /subtitles` to search, `POST /download` for a link and the remaining quota. The
//! `Api-Key` and a `User-Agent` naming sofarr go with every request; the token only with
//! downloads. Search parameters are sent sorted and in lower case, as the documentation
//! asks to avoid redirects.

use std::time::Duration;

use jiff::Timestamp;
use reqwest::StatusCode;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::rank::Candidate;

/// Connection data of an account.
#[derive(Clone)]
pub struct ProviderConfig {
    /// API base, e.g. `https://api.opensubtitles.com/api/v1`.
    pub url: String,
    pub api_key: String,
    pub username: String,
    pub password: String,
    /// Sent as `User-Agent`, e.g. `amigo-sofarr v0.1.0`.
    pub user_agent: String,
    /// The least time between two requests (the API limits requests per second).
    pub pause: Duration,
}

/// Secrets never reach a log through a debug print.
impl std::fmt::Debug for ProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderConfig")
            .field("url", &self.url)
            .field("user_agent", &self.user_agent)
            .field("pause", &self.pause)
            .finish_non_exhaustive()
    }
}

/// A failure that ends a subtitle run (AC-08); German messages without credentials.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    #[error("OpenSubtitles nicht erreichbar: {0}")]
    Unreachable(String),
    #[error("OpenSubtitles meldet einen Serverfehler (HTTP {0})")]
    Server(u16),
    #[error("OpenSubtitles lehnt die Anmeldung ab (HTTP {0}); Benutzername und Passwort prüfen")]
    LoginRefused(u16),
    #[error("OpenSubtitles lehnt den API-Key ab (HTTP {0})")]
    KeyRefused(u16),
    #[error("OpenSubtitles begrenzt die Anfragen (HTTP 429); nächster Versuch beim nächsten Lauf")]
    RateLimited,
    #[error("OpenSubtitles antwortet unerwartet: {0}")]
    Answer(String),
}

/// Why one download did not deliver a subtitle.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DownloadError {
    /// The run has to stop.
    #[error(transparent)]
    Provider(#[from] ProviderError),
    /// The daily download quota is used up until `reset` (AC-07).
    #[error("Download-Kontingent von OpenSubtitles aufgebraucht")]
    Quota { reset: Option<Timestamp> },
    /// Only this candidate failed (AC-14).
    #[error("Untertitel nicht ladbar: {0}")]
    Candidate(String),
}

/// What a search is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Feature {
    Movie {
        imdb_id: Option<String>,
        tmdb_id: Option<i64>,
    },
    Episode {
        parent_imdb_id: Option<String>,
        parent_tmdb_id: Option<i64>,
        /// The series title, used when neither id is known.
        title: String,
        season: u16,
        episode: u16,
    },
}

/// One search: a feature, a language (ISO 639-1), normal or forced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub feature: Feature,
    pub moviehash: Option<String>,
    pub language: String,
    pub forced: bool,
}

impl SearchQuery {
    /// The query parameters, sorted by name.
    pub fn parameters(&self) -> Vec<(&'static str, String)> {
        let mut params = vec![
            ("languages", self.language.to_lowercase()),
            (
                "foreign_parts_only",
                if self.forced { "only" } else { "exclude" }.to_owned(),
            ),
            ("hearing_impaired", "include".to_owned()),
        ];
        if let Some(hash) = &self.moviehash {
            params.push(("moviehash", hash.to_lowercase()));
        }
        match &self.feature {
            Feature::Movie { imdb_id, tmdb_id } => {
                params.push(("type", "movie".to_owned()));
                match (imdb_id.as_deref().and_then(imdb_number), tmdb_id) {
                    (Some(imdb), _) => params.push(("imdb_id", imdb)),
                    (None, Some(tmdb)) => params.push(("tmdb_id", tmdb.to_string())),
                    (None, None) => {}
                }
            }
            Feature::Episode {
                parent_imdb_id,
                parent_tmdb_id,
                title,
                season,
                episode,
            } => {
                params.push(("type", "episode".to_owned()));
                match (
                    parent_imdb_id.as_deref().and_then(imdb_number),
                    parent_tmdb_id,
                ) {
                    (Some(imdb), _) => params.push(("parent_imdb_id", imdb)),
                    (None, Some(tmdb)) => params.push(("parent_tmdb_id", tmdb.to_string())),
                    (None, None) => params.push(("query", title.to_lowercase())),
                }
                params.push(("season_number", season.to_string()));
                params.push(("episode_number", episode.to_string()));
            }
        }
        params.sort_by_key(|(name, _)| *name);
        params
    }
}

/// An IMDb id as the API wants it: the number without `tt` and leading zeros.
fn imdb_number(id: &str) -> Option<String> {
    let digits = id.trim().trim_start_matches("tt");
    digits.parse::<u64>().ok().map(|n| n.to_string())
}

/// A downloaded subtitle with the quota after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Downloaded {
    pub content: Vec<u8>,
    /// Downloads left until `reset`.
    pub remaining: Option<i64>,
    pub reset: Option<Timestamp>,
}

pub struct OpenSubtitles {
    http: reqwest::Client,
    config: ProviderConfig,
    token: Mutex<Option<String>>,
    last_request: Mutex<Option<tokio::time::Instant>>,
}

fn unreachable(e: reqwest::Error) -> ProviderError {
    ProviderError::Unreachable(e.without_url().to_string())
}

/// The error of a failed answer that ends a run, if it is one.
fn run_error(status: StatusCode) -> Option<ProviderError> {
    match status.as_u16() {
        401 | 403 => Some(ProviderError::KeyRefused(status.as_u16())),
        429 => Some(ProviderError::RateLimited),
        code if status.is_server_error() => Some(ProviderError::Server(code)),
        _ => None,
    }
}

fn reset_time(body: &Value) -> Option<Timestamp> {
    body["reset_time_utc"].as_str()?.parse().ok()
}

fn candidate(item: &Value) -> Option<Candidate> {
    let attributes = &item["attributes"];
    let file_id = attributes["files"].as_array()?.first()?["file_id"].as_i64()?;
    let subtitle_id = attributes["subtitle_id"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| item["id"].as_str().map(str::to_owned))
        .or_else(|| item["id"].as_i64().map(|i| i.to_string()))?;
    Some(Candidate {
        subtitle_id,
        file_id,
        language: attributes["language"].as_str()?.to_owned(),
        release: attributes["release"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        hearing_impaired: attributes["hearing_impaired"].as_bool().unwrap_or(false),
        foreign_parts_only: attributes["foreign_parts_only"].as_bool().unwrap_or(false),
        moviehash_match: attributes["moviehash_match"].as_bool().unwrap_or(false),
        download_count: attributes["download_count"].as_u64().unwrap_or(0),
    })
}

impl OpenSubtitles {
    pub fn new(http: reqwest::Client, config: ProviderConfig) -> Self {
        Self {
            http,
            config: ProviderConfig {
                url: config.url.trim_end_matches('/').to_owned(),
                ..config
            },
            token: Mutex::new(None),
            last_request: Mutex::new(None),
        }
    }

    /// Waits until the pause after the previous request has passed.
    async fn pace(&self) {
        let mut last = self.last_request.lock().await;
        if let Some(previous) = *last {
            tokio::time::sleep_until(previous + self.config.pause).await;
        }
        *last = Some(tokio::time::Instant::now());
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}/{path}", self.config.url))
            .header("Api-Key", &self.config.api_key)
            .header(reqwest::header::USER_AGENT, &self.config.user_agent)
            .header(reqwest::header::ACCEPT, "application/json")
    }

    /// Candidates of the first result page.
    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<Candidate>, ProviderError> {
        self.pace().await;
        let response = self
            .request(reqwest::Method::GET, "subtitles")
            .query(&query.parameters())
            .send()
            .await
            .map_err(unreachable)?;
        let status = response.status();
        if let Some(error) = run_error(status) {
            return Err(error);
        }
        if !status.is_success() {
            return Err(ProviderError::Answer(format!("Suche mit HTTP {status}")));
        }
        let body: Value = response
            .json()
            .await
            .map_err(|e| ProviderError::Answer(e.without_url().to_string()))?;
        let items = body["data"]
            .as_array()
            .ok_or_else(|| ProviderError::Answer("Suche ohne Ergebnisliste".to_owned()))?;
        Ok(items.iter().filter_map(candidate).collect())
    }

    async fn login(&self) -> Result<String, ProviderError> {
        self.pace().await;
        let response = self
            .request(reqwest::Method::POST, "login")
            .json(&json!({
                "username": self.config.username,
                "password": self.config.password,
            }))
            .send()
            .await
            .map_err(unreachable)?;
        let status = response.status();
        match status.as_u16() {
            400 | 401 => return Err(ProviderError::LoginRefused(status.as_u16())),
            _ => {
                if let Some(error) = run_error(status) {
                    return Err(error);
                }
            }
        }
        if !status.is_success() {
            return Err(ProviderError::Answer(format!(
                "Anmeldung mit HTTP {status}"
            )));
        }
        let body: Value = response
            .json()
            .await
            .map_err(|e| ProviderError::Answer(e.without_url().to_string()))?;
        let token = body["token"]
            .as_str()
            .filter(|t| !t.is_empty())
            .ok_or_else(|| ProviderError::Answer("Anmeldung ohne Token".to_owned()))?
            .to_owned();
        *self.token.lock().await = Some(token.clone());
        Ok(token)
    }

    async fn token(&self) -> Result<String, ProviderError> {
        let cached = self.token.lock().await.clone();
        match cached {
            Some(token) => Ok(token),
            None => self.login().await,
        }
    }

    /// Asks for the download link of a file (logging in first if needed).
    async fn link(&self, file_id: i64) -> Result<(String, Value), DownloadError> {
        let mut token = self.token().await?;
        for attempt in 0..2 {
            self.pace().await;
            let response = self
                .request(reqwest::Method::POST, "download")
                .bearer_auth(&token)
                .json(&json!({ "file_id": file_id, "sub_format": "srt" }))
                .send()
                .await
                .map_err(unreachable)?;
            let status = response.status();
            if status == StatusCode::UNAUTHORIZED && attempt == 0 {
                // The token expired: log in again once.
                *self.token.lock().await = None;
                token = self.login().await?;
                continue;
            }
            let body: Value = response.json().await.unwrap_or(Value::Null);
            let link = body["link"].as_str().filter(|l| !l.is_empty());
            let exhausted = body["remaining"].as_i64().is_some_and(|r| r < 0)
                || (link.is_none() && body["remaining"].as_i64().is_some_and(|r| r <= 0));
            if status == StatusCode::NOT_ACCEPTABLE || (!status.is_success() && exhausted) {
                return Err(DownloadError::Quota {
                    reset: reset_time(&body),
                });
            }
            if let Some(error) = run_error(status) {
                return Err(error.into());
            }
            if !status.is_success() {
                return Err(DownloadError::Candidate(format!("HTTP {status}")));
            }
            let Some(link) = link else {
                return Err(DownloadError::Candidate("Antwort ohne Link".to_owned()));
            };
            return Ok((link.to_owned(), body));
        }
        Err(ProviderError::LoginRefused(401).into())
    }

    /// Downloads one subtitle file; uses one download of the quota.
    pub async fn download(&self, file_id: i64) -> Result<Downloaded, DownloadError> {
        let (link, body) = self.link(file_id).await?;
        let response = self
            .http
            .get(&link)
            .header(reqwest::header::USER_AGENT, &self.config.user_agent)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| DownloadError::Candidate(e.without_url().to_string()))?;
        let content = response
            .bytes()
            .await
            .map_err(|e| DownloadError::Candidate(e.without_url().to_string()))?;
        Ok(Downloaded {
            content: content.to_vec(),
            remaining: body["remaining"].as_i64(),
            reset: reset_time(&body),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameters_are_sorted_and_lower_case() {
        let movie = SearchQuery {
            feature: Feature::Movie {
                imdb_id: Some("tt0012345".to_owned()),
                tmdb_id: Some(7),
            },
            moviehash: Some("ABCDEF0123456789".to_owned()),
            language: "DE".to_owned(),
            forced: false,
        };
        let text: Vec<String> = movie
            .parameters()
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        assert_eq!(
            text,
            [
                "foreign_parts_only=exclude",
                "hearing_impaired=include",
                "imdb_id=12345",
                "languages=de",
                "moviehash=abcdef0123456789",
                "type=movie",
            ]
        );
        let episode = SearchQuery {
            feature: Feature::Episode {
                parent_imdb_id: None,
                parent_tmdb_id: None,
                title: "Kleine Insel".to_owned(),
                season: 1,
                episode: 2,
            },
            moviehash: None,
            language: "de".to_owned(),
            forced: true,
        };
        let text: Vec<String> = episode
            .parameters()
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        assert_eq!(
            text,
            [
                "episode_number=2",
                "foreign_parts_only=only",
                "hearing_impaired=include",
                "languages=de",
                "query=kleine insel",
                "season_number=1",
                "type=episode",
            ]
        );
    }

    #[test]
    fn debug_output_hides_secrets() {
        use std::hash::{BuildHasher, Hasher};
        // Every `RandomState` has its own random keys: hashing nothing gives a fresh value.
        let random = || {
            format!(
                "{:016x}",
                std::collections::hash_map::RandomState::new()
                    .build_hasher()
                    .finish()
            )
        };
        let (key, password) = (random(), random());
        let config = ProviderConfig {
            url: "u".to_owned(),
            api_key: key.clone(),
            username: "nutzer".to_owned(),
            password: password.clone(),
            user_agent: "ua".to_owned(),
            pause: Duration::ZERO,
        };
        let text = format!("{config:?}");
        assert!(
            !text.contains(&key) && !text.contains(&password) && !text.contains("nutzer"),
            "the debug output shows an account detail"
        );
    }
}
