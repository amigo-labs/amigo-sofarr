//! `sofarr-parity collect --source releases`: records Radarr's grab decisions.
//!
//! `/api/v3/release?movieId=…` runs a real indexer search, so only a few films are
//! queried per run, those with the fewest recorded releases first. By default these are
//! monitored films; with `with_file` films that already have a file (monitored or not),
//! spread over profile, file quality and revision so upgrade rules get covered; or
//! exactly the films asked for by id. A film's file comes from `/api/v3/moviefile`,
//! which also carries its custom format score.
//!
//! Every release is stored twice: raw in `fixtures/private/`, and redacted in
//! `fixtures/public/` – rebuilt from a whitelist, without indexer, GUID or URLs, and
//! with the film reduced to what the decision depends on (no path, no ids but the
//! profile).

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
};

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
    "secondaryYear",
    "runtime",
    "qualityProfileId",
    "monitored",
    "hasFile",
    "isAvailable",
    "minimumAvailability",
    "status",
];
const MOVIE_FILE_KEYS: &[&str] = &[
    "quality",
    "customFormatScore",
    "languages",
    "edition",
    "releaseGroup",
];
const NAMED_KEYS: &[&str] = &["id", "name"];

/// Collects releases for up to `movies` films; returns the new public records.
pub fn collect(
    radarr: &Radarr<'_>,
    fixtures: &Path,
    version: &str,
    movies: usize,
    with_file: bool,
    ids: &[u64],
) -> anyhow::Result<usize> {
    let private_path = fixtures.join(PRIVATE_FILE);
    let counts = records_per_movie(&private_path)?;
    let mut private = Store::open(&private_path)?;
    let mut public = Store::open(&fixtures.join(PUBLIC_FILE))?;

    let library = radarr.movies().context("Radarr-Bibliothek")?;
    let selected = if ids.is_empty() {
        select(&library, &counts, movies, with_file)
    } else {
        ids.iter()
            .filter_map(|id| library.iter().find(|m| m["id"].as_u64() == Some(*id)))
            .collect()
    };
    eprintln!(
        "Bibliothek: {} Filme, frage {} davon ab ({})",
        library.len(),
        selected.len(),
        if !ids.is_empty() {
            "nach Id"
        } else if with_file {
            "mit Datei"
        } else {
            "überwacht"
        }
    );

    let total = selected.len();
    let mut added = 0;
    for (n, movie) in selected.into_iter().enumerate() {
        let id = movie["id"].as_u64().unwrap_or_default();
        let mut movie = movie.clone();
        if movie["hasFile"].as_bool() == Some(true)
            && let Some(file) = radarr
                .movie_file(id)
                .with_context(|| format!("Radarr-Datei für Film {id}"))?
        {
            movie["movieFile"] = file;
        }
        let movie = &movie;
        eprintln!("Film {}/{total}: Suche läuft …", n + 1);
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

/// Films to query, least covered first.
///
/// Without `with_file`: monitored films. With it: films with a file, taken round-robin
/// from groups of (profile, file quality, file revision), least covered group first.
pub fn select<'a>(
    library: &'a [Value],
    counts: &HashMap<u64, usize>,
    movies: usize,
    with_file: bool,
) -> Vec<&'a Value> {
    let count = |m: &Value| {
        let id = m["id"].as_u64().unwrap_or_default();
        (counts.get(&id).copied().unwrap_or(0), id)
    };
    let mut candidates: Vec<&Value> = library
        .iter()
        .filter(|m| m["id"].as_u64().is_some())
        .filter(|m| {
            if with_file {
                m["hasFile"].as_bool() == Some(true)
            } else {
                m["monitored"].as_bool() == Some(true)
            }
        })
        .collect();
    candidates.sort_by_key(|m| count(m));
    if !with_file {
        candidates.truncate(movies);
        return candidates;
    }

    let mut groups: BTreeMap<(u64, String, u64), Vec<&Value>> = BTreeMap::new();
    for movie in candidates {
        let file = &movie["movieFile"]["quality"];
        let key = (
            movie["qualityProfileId"].as_u64().unwrap_or_default(),
            file["quality"]["name"].as_str().unwrap_or("").to_owned(),
            file["revision"]["version"].as_u64().unwrap_or(1),
        );
        groups.entry(key).or_default().push(movie);
    }
    let mut groups: Vec<Vec<&Value>> = groups.into_values().collect();
    // Stable: equally covered groups keep their key order.
    groups.sort_by_key(|g| g.iter().map(|m| count(m).0).sum::<usize>());
    let mut selected = Vec::new();
    for round in 0.. {
        let before = selected.len();
        for group in &groups {
            if selected.len() == movies {
                return selected;
            }
            if let Some(movie) = group.get(round) {
                selected.push(*movie);
            }
        }
        if selected.len() == before {
            break;
        }
    }
    selected
}

/// Rewrites the public release file from the private one, e.g. after the whitelist
/// changed. Returns the number of public records.
pub fn rebuild_public(fixtures: &Path) -> anyhow::Result<usize> {
    let private_path = fixtures.join(PRIVATE_FILE);
    let raw = fs::read_to_string(&private_path)
        .with_context(|| format!("{} nicht lesbar", private_path.display()))?;
    let public_path = fixtures.join(PUBLIC_FILE);
    let temp_path = public_path.with_extension("jsonl.tmp");
    let _ = fs::remove_file(&temp_path);
    let mut public = Store::open(&temp_path)?;
    for (n, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let at = || format!("{}:{}", private_path.display(), n + 1);
        let record: Value =
            serde_json::from_str(line).with_context(|| format!("{}: ungültiges JSON", at()))?;
        let field = |key: &str| {
            record[key]
                .as_str()
                .with_context(|| format!("{}: `{key}` fehlt", at()))
        };
        public.append(&Record {
            title: field("title")?,
            origin: ORIGIN,
            collected_at: field("collected_at")?.to_owned(),
            oracle: Oracle {
                app: record["oracle"]["app"].as_str().unwrap_or("radarr"),
                version: record["oracle"]["version"].as_str().unwrap_or(""),
            },
            movie: Some(public_movie(&record["movie"])),
            response: public_release(&record["response"]),
        })?;
    }
    let count = public.len();
    drop(public);
    fs::rename(&temp_path, &public_path)
        .with_context(|| format!("{} nicht ersetzbar", public_path.display()))?;
    Ok(count)
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
    fn select_spreads_films_with_file_over_groups() {
        let film = |id: u64, profile: u64, quality: &str, version: u64| {
            json!({"id": id, "monitored": false, "hasFile": true, "qualityProfileId": profile,
                   "movieFile": {"quality": {"quality": {"name": quality},
                                             "revision": {"version": version}}}})
        };
        let library = vec![
            film(1, 4, "Bluray-1080p", 1),
            film(2, 4, "Bluray-1080p", 1),
            film(3, 4, "Bluray-1080p", 1),
            film(4, 4, "Bluray-1080p", 2),
            film(5, 5, "WEBDL-2160p", 1),
            json!({"id": 6, "monitored": true, "hasFile": false}),
        ];
        let ids = |selected: Vec<&Value>| -> Vec<u64> {
            selected.iter().filter_map(|m| m["id"].as_u64()).collect()
        };
        let counts = HashMap::new();
        assert_eq!(ids(select(&library, &counts, 4, true)), [1, 4, 5, 2]);
        assert_eq!(ids(select(&library, &counts, 9, true)), [1, 4, 5, 2, 3]);
        assert_eq!(ids(select(&library, &counts, 5, false)), [6]);
        // A covered group comes last, a covered film last within its group.
        let counts = HashMap::from([(1, 10), (5, 3)]);
        assert_eq!(ids(select(&library, &counts, 3, true)), [4, 5, 2]);
    }

    #[test]
    fn public_movie_keeps_decision_context_only() {
        let movie = json!({
            "id": 77, "tmdbId": 1234, "title": "Some Movie", "year": 2020, "runtime": 101,
            "qualityProfileId": 4, "monitored": true, "hasFile": true,
            "path": "/data/media/movies/Some Movie (2020)",
            "rootFolderPath": "/data/media/movies",
            "originalLanguage": {"id": 1, "name": "English"},
            "secondaryYear": 2019,
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
        assert_eq!(public["secondaryYear"], 2019);
    }
}
