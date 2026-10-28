//! The oracle: a running Radarr instance, queried only through its public HTTP API.

use std::time::Duration;

use anyhow::Context;
use serde_json::Value;

use crate::{
    config::RadarrConfig,
    http::{HttpClient, join},
};

const HISTORY_PAGE_SIZE: u64 = 250;
/// A release search queries every indexer and can take a while.
const SEARCH_TIMEOUT: Duration = Duration::from_secs(300);

pub struct Radarr<'a> {
    http: &'a HttpClient,
    config: &'a RadarrConfig,
}

impl<'a> Radarr<'a> {
    pub fn new(http: &'a HttpClient, config: &'a RadarrConfig) -> Self {
        Self { http, config }
    }

    fn get_json(&self, path: &str, query: &[(&str, &str)]) -> anyhow::Result<Value> {
        self.get_json_with_timeout(path, query, None)
    }

    fn get_json_with_timeout(
        &self,
        path: &str,
        query: &[(&str, &str)],
        timeout: Option<Duration>,
    ) -> anyhow::Result<Value> {
        let url = join(&self.config.url, path);
        let headers = [("X-Api-Key", self.config.api_key.as_str())];
        let body = match timeout {
            Some(timeout) => self.http.get_with_timeout(&url, query, &headers, timeout)?,
            None => self.http.get(&url, query, &headers)?,
        };
        serde_json::from_str(&body).with_context(|| format!("Radarr: {path} liefert kein JSON"))
    }

    /// Version reported by `/api/v3/system/status`.
    pub fn version(&self) -> anyhow::Result<String> {
        let status = self.get_json("api/v3/system/status", &[])?;
        status["version"]
            .as_str()
            .map(str::to_owned)
            .context("Radarr: system/status enthält keine Version")
    }

    /// Raw answer of `/api/v3/parse` for one release title.
    pub fn parse(&self, title: &str) -> anyhow::Result<Value> {
        self.get_json("api/v3/parse", &[("title", title)])
    }

    /// All films of the library (`/api/v3/movie`).
    pub fn movies(&self) -> anyhow::Result<Vec<Value>> {
        match self.get_json("api/v3/movie", &[])? {
            Value::Array(movies) => Ok(movies),
            _ => anyhow::bail!("Radarr: movie liefert keine Liste"),
        }
    }

    /// Releases found by a search for one film, with Radarr's decision
    /// (`/api/v3/release?movieId=…`). Triggers a real indexer search.
    pub fn releases(&self, movie_id: u64) -> anyhow::Result<Value> {
        let id = movie_id.to_string();
        self.get_json_with_timeout("api/v3/release", &[("movieId", &id)], Some(SEARCH_TIMEOUT))
    }

    /// All `sourceTitle`s of the grab/import history, newest first, without duplicates.
    pub fn history_titles(&self) -> anyhow::Result<Vec<String>> {
        let page_size = HISTORY_PAGE_SIZE.to_string();
        let mut titles = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for page in 1.. {
            let page_str = page.to_string();
            let body = self.get_json(
                "api/v3/history",
                &[
                    ("page", &page_str),
                    ("pageSize", &page_size),
                    ("sortKey", "date"),
                    ("sortDirection", "descending"),
                ],
            )?;
            let records = body["records"]
                .as_array()
                .context("Radarr: history enthält keine records")?;
            for title in records
                .iter()
                .filter_map(|r| r["sourceTitle"].as_str())
                .map(str::trim)
                .filter(|t| !t.is_empty())
            {
                if seen.insert(title.to_owned()) {
                    titles.push(title.to_owned());
                }
            }
            let total = body["totalRecords"].as_u64().unwrap_or(0);
            if records.is_empty() || page * HISTORY_PAGE_SIZE >= total {
                break;
            }
        }
        Ok(titles)
    }
}
