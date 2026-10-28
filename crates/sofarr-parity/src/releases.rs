//! `sofarr-parity collect --source releases`: records Radarr's grab decisions.
//!
//! `/api/v3/release?movieId=…` runs a real indexer search, so only a few monitored
//! films are queried per run, those with the fewest recorded releases first.
//!
//! Every release is stored twice: raw in `fixtures/private/`, and redacted in
//! `fixtures/public/` – rebuilt from a whitelist, without indexer, GUID or URLs, and
//! with the film reduced to what the decision depends on (no path, no ids but the
//! profile).

use std::{collections::HashMap, fs, path::Path};

use anyhow::Context;
use serde_json::{Map, Value};

use crate::{
    radarr::Radarr,
    store::{Oracle, Record, Store},
};

pub const PUBLIC_FILE: &str = "public/radarr-release.jsonl";
pub const PRIVATE_FILE: &str = "private/radarr-release.jsonl";
pub const ORIGIN: &str = "release";

const RELEASE_KEYS: &[&str] = &[
    "title",
    "size",
    "age",
    "ageHours",
    "ageMinutes",
    "publishDate",
    "protocol",
    "quality",
    "qualityWeight",
    "customFormatScore",
    "languages",
    "releaseGroup",
    "edition",
    "movieTitles",
    "approved",
    "temporarilyRejected",
    "rejected",
    "downloadAllowed",
];
const MOVIE_KEYS: &[&str] = &[
    "title",
    "originalTitle",
    "year",
    "runtime",
    "qualityProfileId",
    "monitored",
    "hasFile",
    "isAvailable",
    "minimumAvailability",
    "status",
];
const MOVIE_FILE_KEYS: &[&str] = &["quality", "customFormatScore", "languages", "edition"];
const NAMED_KEYS: &[&str] = &["id", "name"];

/// Collects releases for up to `movies` monitored films; returns the new public records.
pub fn collect(
    radarr: &Radarr<'_>,
    fixtures: &Path,
    version: &str,
    movies: usize,
) -> anyhow::Result<usize> {
    let private_path = fixtures.join(PRIVATE_FILE);
    let counts = records_per_movie(&private_path)?;
    let mut private = Store::open(&private_path)?;
    let mut public = Store::open(&fixtures.join(PUBLIC_FILE))?;

    let library = radarr.movies().context("Radarr-Bibliothek")?;
    let mut candidates: Vec<&Value> = library
        .iter()
        .filter(|m| m["monitored"].as_bool() == Some(true))
        .filter(|m| m["id"].as_u64().is_some())
        .collect();
    candidates.sort_by_key(|m| {
        let id = m["id"].as_u64().unwrap_or_default();
        (counts.get(&id).copied().unwrap_or(0), id)
    });
    eprintln!(
        "Bibliothek: {} überwachte Filme, frage {} davon ab",
        candidates.len(),
        movies.min(candidates.len())
    );

    let mut added = 0;
    for (n, movie) in candidates.into_iter().take(movies).enumerate() {
        let id = movie["id"].as_u64().unwrap_or_default();
        eprintln!("Film {}/{movies}: Suche läuft …", n + 1);
        let releases = radarr
            .releases(id)
            .with_context(|| format!("Radarr-Releases für Film {id}"))?;
        let releases = releases.as_array().map_or(&[][..], Vec::as_slice);
        let mut new = 0;
        for release in releases {
            let Some(title) = release["title"].as_str() else {
                continue;
            };
            let collected_at = jiff::Timestamp::now()
                .round(jiff::Unit::Second)?
                .to_string();
            let oracle = || Oracle {
                app: "radarr",
                version,
            };
            private.append(&Record {
                title,
                origin: ORIGIN,
                collected_at: collected_at.clone(),
                oracle: oracle(),
                movie: Some(movie.clone()),
                response: release.clone(),
            })?;
            if public.append(&Record {
                title,
                origin: ORIGIN,
                collected_at,
                oracle: oracle(),
                movie: Some(public_movie(movie)),
                response: public_release(release),
            })? {
                new += 1;
            }
        }
        eprintln!("  {} Releases, {new} neu", releases.len());
        added += new;
    }
    Ok(added)
}

/// Number of private records per film id, to query the least covered films first.
fn records_per_movie(path: &Path) -> anyhow::Result<HashMap<u64, usize>> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(e).with_context(|| format!("{} nicht lesbar", path.display())),
    };
    let mut counts = HashMap::new();
    for line in raw.lines() {
        // Incomplete lines are dealt with by `Store::open`.
        if let Ok(value) = serde_json::from_str::<Value>(line)
            && let Some(id) = value["movie"]["id"].as_u64()
        {
            *counts.entry(id).or_default() += 1;
        }
    }
    Ok(counts)
}

fn pick(value: &Value, keys: &[&str]) -> Map<String, Value> {
    keys.iter()
        .filter_map(|k| value.get(*k).map(|v| ((*k).to_owned(), v.clone())))
        .collect()
}

/// A release reduced to what the decision depends on and what the oracle decided.
pub fn public_release(release: &Value) -> Value {
    let mut out = pick(release, RELEASE_KEYS);
    if let Some(formats) = release["customFormats"].as_array() {
        let formats = formats
            .iter()
            .map(|f| Value::Object(pick(f, NAMED_KEYS)))
            .collect();
        out.insert("customFormats".into(), Value::Array(formats));
    }
    if let Some(rejections) = release["rejections"].as_array() {
        // Rejection texts may name the indexer; it is replaced by a placeholder.
        let indexer = release["indexer"].as_str().filter(|i| !i.trim().is_empty());
        let rejections = rejections
            .iter()
            .filter_map(Value::as_str)
            .map(|r| match indexer {
                Some(indexer) => Value::String(r.replace(indexer, "<indexer>")),
                None => Value::String(r.to_owned()),
            })
            .collect();
        out.insert("rejections".into(), Value::Array(rejections));
    }
    Value::Object(out)
}

/// The film reduced to what the decision depends on: no path, no ids but the profile.
pub fn public_movie(movie: &Value) -> Value {
    let mut out = pick(movie, MOVIE_KEYS);
    if let Some(language) = movie.get("originalLanguage") {
        out.insert(
            "originalLanguage".into(),
            Value::Object(pick(language, NAMED_KEYS)),
        );
    }
    // Alternative titles decide whether a release belongs to the film.
    if let Some(alternates) = movie["alternateTitles"].as_array() {
        let titles = alternates
            .iter()
            .filter_map(|a| a["title"].as_str())
            .map(|t| Value::String(t.to_owned()))
            .collect();
        out.insert("alternateTitles".into(), Value::Array(titles));
    }
    if let Some(file) = movie.get("movieFile").filter(|f| f.is_object()) {
        out.insert(
            "movieFile".into(),
            Value::Object(pick(file, MOVIE_FILE_KEYS)),
        );
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SECRETS: &[&str] = &[
        "Mein Privater Indexer",
        "indexer.private.example",
        "SECRET-API-KEY",
        "/data/media/movies",
        "abcdef-guid",
    ];

    #[test]
    fn public_release_is_redacted() {
        let release = json!({
            "guid": "https://indexer.private.example/details/abcdef-guid",
            "title": "Some.Movie.2020.1080p.WEB-DL.H.264-GRP",
            "size": 8_000_000_000_u64,
            "ageMinutes": 600.5,
            "indexer": "Mein Privater Indexer",
            "indexerId": 3,
            "downloadUrl": "https://indexer.private.example/get?apikey=SECRET-API-KEY",
            "infoUrl": "https://indexer.private.example/details/abcdef-guid",
            "quality": {"quality": {"id": 3, "name": "WEBDL-1080p"}, "revision": {"version": 1}},
            "customFormats": [{"id": 2, "name": "German", "specifications": [{"x": "SECRET-API-KEY"}]}],
            "customFormatScore": 100,
            "approved": false,
            "temporarilyRejected": false,
            "rejected": true,
            "rejections": ["Not wanted in Mein Privater Indexer", "Size too big"],
        });
        let public = public_release(&release);
        let text = public.to_string();
        for (i, secret) in SECRETS.iter().enumerate() {
            assert!(!text.contains(secret), "SECRETS[{i}] leaked");
        }
        assert_eq!(
            public["customFormats"],
            json!([{"id": 2, "name": "German"}])
        );
        assert_eq!(
            public["rejections"],
            json!(["Not wanted in <indexer>", "Size too big"])
        );
        assert_eq!(public["quality"]["quality"]["id"], 3);
        assert_eq!(public["ageMinutes"], 600.5);
    }

    #[test]
    fn public_movie_keeps_decision_context_only() {
        let movie = json!({
            "id": 77, "tmdbId": 1234, "title": "Some Movie", "year": 2020, "runtime": 101,
            "qualityProfileId": 4, "monitored": true, "hasFile": true,
            "path": "/data/media/movies/Some Movie (2020)",
            "rootFolderPath": "/data/media/movies",
            "originalLanguage": {"id": 1, "name": "English"},
            "alternateTitles": [{"id": 9, "movieMetadataId": 77, "sourceType": "tmdb", "title": "Ein Film"}],
            "movieFile": {"relativePath": "Some Movie (2020).mkv",
                          "path": "/data/media/movies/Some Movie (2020)/x.mkv",
                          "quality": {"quality": {"id": 7}}, "customFormatScore": 50}
        });
        let public = public_movie(&movie);
        assert!(!public.to_string().contains("/data/media/movies"));
        assert!(public.get("id").is_none());
        assert!(public.get("tmdbId").is_none());
        assert_eq!(public["runtime"], 101);
        assert_eq!(public["movieFile"]["customFormatScore"], 50);
        assert!(public["movieFile"].get("relativePath").is_none());
        assert_eq!(public["alternateTitles"], json!(["Ein Film"]));
    }
}
