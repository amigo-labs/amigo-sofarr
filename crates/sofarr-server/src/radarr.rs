//! Read-only access to a running Radarr: one-time import and parity comparisons.

use anyhow::Context;
use serde_json::Value;

use crate::config::RadarrConfig;

pub struct Radarr {
    client: reqwest::Client,
    url: String,
    api_key: String,
}

const PAGE_SIZE: u64 = 250;

impl Radarr {
    pub fn new(client: reqwest::Client, config: &RadarrConfig) -> Self {
        Self {
            client,
            url: config.url.trim_end_matches('/').to_owned(),
            api_key: config.api_key.clone(),
        }
    }

    /// GET `api/v3/{path}`.
    pub async fn get(&self, path: &str, query: &[(&str, &str)]) -> anyhow::Result<Value> {
        self.client
            .get(format!(
                "{}/api/v3/{}",
                self.url,
                path.trim_start_matches('/')
            ))
            .query(query)
            .header("X-Api-Key", &self.api_key)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(reqwest::Error::without_url)
            .with_context(|| format!("Radarr: {path} nicht abrufbar"))?
            .json()
            .await
            .with_context(|| format!("Radarr: {path} liefert kein JSON"))
    }

    /// All records of a paged endpoint (`wanted/missing`, `history`, …).
    pub async fn paged(&self, path: &str, query: &[(&str, &str)]) -> anyhow::Result<Vec<Value>> {
        let mut records = Vec::new();
        let size = PAGE_SIZE.to_string();
        for page in 1u64.. {
            let page_str = page.to_string();
            let mut q = query.to_vec();
            q.extend([("page", page_str.as_str()), ("pageSize", size.as_str())]);
            let body = self.get(path, &q).await?;
            let batch = body["records"]
                .as_array()
                .with_context(|| format!("Radarr: {path} ohne records"))?;
            records.extend(batch.iter().cloned());
            let total = body["totalRecords"].as_u64().unwrap_or(0);
            if batch.is_empty() || page * PAGE_SIZE >= total {
                break;
            }
        }
        Ok(records)
    }
}
