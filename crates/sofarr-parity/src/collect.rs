//! `sofarr-parity collect`: records oracle answers for new release titles.

use std::path::Path;

use anyhow::Context;
use serde_json::{Map, Value};

use crate::{
    config::ParityConfig,
    http::{HttpClient, host_of},
    newznab,
    radarr::Radarr,
    store::{Oracle, Record, Store},
};

pub const PUBLIC_FILE: &str = "public/radarr-parse.jsonl";
/// Print a progress line after this many oracle requests.
const PROGRESS_EVERY: usize = 50;
pub const HISTORY_FILE: &str = "private/radarr-parse-history.jsonl";

/// Top-level keys of a parse answer that may go into public fixtures. Everything else –
/// in particular a matched `movie` with paths and library data – stays out.
const PUBLIC_RESPONSE_KEYS: &[&str] = &["title", "parsedMovieInfo", "languages"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sources {
    Rss,
    History,
    All,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub rss_new: usize,
    pub history_new: usize,
}

pub fn run(
    config: &ParityConfig,
    http: &HttpClient,
    fixtures: &Path,
    sources: Sources,
) -> anyhow::Result<Summary> {
    let radarr = Radarr::new(http, &config.radarr);
    let version = radarr
        .version()
        .with_context(|| format!("Orakel {} nicht erreichbar", host_of(&config.radarr.url)))?;
    eprintln!("Orakel: Radarr {version}");
    let mut summary = Summary::default();

    if matches!(sources, Sources::Rss | Sources::All) {
        let mut store = Store::open(&fixtures.join(PUBLIC_FILE))?;
        for (n, indexer) in config.indexers.iter().enumerate() {
            // Page by page, so an error later on keeps what earlier pages collected.
            // The offset advances by what the indexer actually returned: it may cap `limit`.
            let mut offset = 0;
            for page in 1..=indexer.pages {
                let titles = newznab::fetch_page(http, indexer, offset)
                    .with_context(|| format!("Indexer {} ({})", n + 1, host_of(&indexer.url)))?;
                eprintln!(
                    "Indexer {}, Seite {page}/{}: {} Releases",
                    n + 1,
                    indexer.pages,
                    titles.len()
                );
                if titles.is_empty() {
                    break;
                }
                offset += titles.len();
                summary.rss_new += record_new(&radarr, &mut store, &titles, "rss", &version, true)?;
            }
        }
    }

    if matches!(sources, Sources::History | Sources::All) {
        let mut store = Store::open(&fixtures.join(HISTORY_FILE))?;
        let titles = radarr.history_titles().context("Radarr-History")?;
        eprintln!("History: {} Titel", titles.len());
        summary.history_new +=
            record_new(&radarr, &mut store, &titles, "history", &version, false)?;
    }

    Ok(summary)
}

fn record_new(
    radarr: &Radarr<'_>,
    store: &mut Store,
    titles: &[String],
    origin: &str,
    version: &str,
    public: bool,
) -> anyhow::Result<usize> {
    let new: Vec<&String> = titles.iter().filter(|t| !store.contains(t)).collect();
    if !new.is_empty() {
        eprintln!("  {} neue Titel, frage Radarr …", new.len());
    }
    let mut added = 0;
    for (n, title) in new.into_iter().enumerate() {
        if n > 0 && n % PROGRESS_EVERY == 0 {
            eprintln!("  … {n} erledigt");
        }
        let response = radarr
            .parse(title)
            .with_context(|| format!("Radarr-Parse für „{title}“"))?;
        let record = Record {
            title,
            origin,
            collected_at: jiff::Timestamp::now()
                .round(jiff::Unit::Second)?
                .to_string(),
            oracle: Oracle {
                app: "radarr",
                version,
            },
            response: if public {
                public_response(response)
            } else {
                response
            },
        };
        if store.append(&record)? {
            added += 1;
        }
    }
    Ok(added)
}

/// Strips everything but the parse result from a response destined for `fixtures/public/`.
pub fn public_response(response: Value) -> Value {
    match response {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(key, _)| PUBLIC_RESPONSE_KEYS.contains(&key.as_str()))
                .collect::<Map<_, _>>(),
        ),
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_response_drops_library_data() {
        let response = serde_json::json!({
            "title": "X.2001.1080p-G",
            "parsedMovieInfo": {"movieTitles": ["X"]},
            "languages": [],
            "movie": {"path": "/data/media/movies/X (2001)"},
            "customFormats": [{"name": "Mein Format"}]
        });
        let public = public_response(response);
        let keys: Vec<&String> = public.as_object().expect("object").keys().collect();
        assert_eq!(keys, ["languages", "parsedMovieInfo", "title"]);
        assert!(!public.to_string().contains("/data/media"));
    }
}
