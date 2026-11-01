//! Episode parsing against the oracle.
//!
//! * Movie fixtures (`radarr-parse.jsonl`) must almost never look like episodes.
//! * Sonarr fixtures (`fixtures/public/sonarr-parse.jsonl`, collected with
//!   `sofarr-parity collect --app sonarr`) are compared per field and ratcheted in
//!   `fixtures/parser-episode-baseline.toml`, like the movie parser.
//!   `SOFARR_UPDATE_BASELINE=1` rewrites the baseline (only upwards for the same set).

use std::{collections::BTreeMap, fs, path::PathBuf};

use serde_json::Value;
use sofarr_parser::parse_episode;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn lines(path: &PathBuf) -> Vec<Value> {
    fs::read_to_string(path)
        .map(|raw| {
            raw.lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// The movie feed also carries real series (`S01E02`, season packs); only the weaker
/// markers – air dates and absolute numbers – can misfire on movie titles.
#[test]
fn movies_are_rarely_episodes() {
    let fixtures = lines(&root().join("fixtures/public/radarr-parse.jsonl"));
    assert!(!fixtures.is_empty());
    let hits: Vec<&str> = fixtures
        .iter()
        .filter_map(|f| f["title"].as_str())
        .filter(|t| {
            parse_episode(t)
                .is_some_and(|e| e.air_date.is_some() || !e.absolute_episodes.is_empty())
        })
        .collect();
    #[allow(clippy::cast_precision_loss)]
    let rate = hits.len() as f64 / fixtures.len() as f64;
    println!(
        "movie titles parsed as episodes: {}/{}",
        hits.len(),
        fixtures.len()
    );
    for h in hits.iter().take(20) {
        println!("  {h}");
    }
    assert!(
        rate < 0.002,
        "too many movies look like episodes: {rate:.4}"
    );
}

const FIELDS: [&str; 6] = [
    "series_title",
    "season",
    "episodes",
    "absolute",
    "air_date",
    "full_season",
];

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn numbers(v: &Value) -> String {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_u64)
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default()
}

/// Expected value of a field from Sonarr's `parsedEpisodeInfo`.
fn expected(field: &str, info: &Value) -> String {
    match field {
        "series_title" => norm(info["seriesTitle"].as_str().unwrap_or_default()),
        "season" => {
            if info["airDate"].as_str().is_some_and(|d| !d.is_empty())
                || info["absoluteEpisodeNumbers"]
                    .as_array()
                    .is_some_and(|a| !a.is_empty())
                    && info["episodeNumbers"].as_array().is_none_or(Vec::is_empty)
            {
                String::new()
            } else {
                info["seasonNumber"]
                    .as_u64()
                    .map(|n| n.to_string())
                    .unwrap_or_default()
            }
        }
        "episodes" => numbers(&info["episodeNumbers"]),
        "absolute" => numbers(&info["absoluteEpisodeNumbers"]),
        "air_date" => info["airDate"]
            .as_str()
            .unwrap_or_default()
            .get(..10)
            .unwrap_or_default()
            .to_owned(),
        "full_season" => info["fullSeason"].as_bool().unwrap_or(false).to_string(),
        _ => String::new(),
    }
}

fn actual(field: &str, title: &str) -> String {
    let Some(e) = parse_episode(title) else {
        return match field {
            "full_season" => "false".to_owned(),
            _ => String::new(),
        };
    };
    match field {
        "series_title" => norm(&e.series_title),
        "season" => e.season.map(|s| s.to_string()).unwrap_or_default(),
        "episodes" => e
            .episodes
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(","),
        "absolute" => e
            .absolute_episodes
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(","),
        "air_date" => e
            .air_date
            .map(|(y, m, d)| format!("{y:04}-{m:02}-{d:02}"))
            .unwrap_or_default(),
        "full_season" => e.full_season.to_string(),
        _ => String::new(),
    }
}

#[test]
fn sonarr_oracle_ratchet() {
    let path = root().join("fixtures/public/sonarr-parse.jsonl");
    let fixtures: Vec<(String, Value)> = lines(&path)
        .into_iter()
        .filter_map(|f| {
            let info = f["response"]["parsedEpisodeInfo"].clone();
            Some((f["title"].as_str()?.to_owned(), info)).filter(|(_, i)| i.is_object())
        })
        .collect();
    if fixtures.is_empty() {
        println!("no Sonarr fixtures yet – skipping");
        return;
    }
    let mut rates = BTreeMap::new();
    for field in FIELDS {
        let mut failures = Vec::new();
        for (title, info) in &fixtures {
            let (want, got) = (expected(field, info), actual(field, title));
            if want != got {
                failures.push(format!("{title}: erwartet {want:?}, erhalten {got:?}"));
            }
        }
        let pass = fixtures.len() - failures.len();
        let permille = pass * 1000 / fixtures.len();
        println!(
            "{field:<13} {pass:>5}/{:<5} {:5.1} %",
            fixtures.len(),
            permille as f64 / 10.0
        );
        for f in failures.iter().take(20) {
            println!("    {f}");
        }
        rates.insert(field, permille);
    }

    let baseline_path = root().join("fixtures/parser-episode-baseline.toml");
    let count = fixtures.len();
    let baseline: Option<toml::Value> = fs::read_to_string(&baseline_path)
        .ok()
        .and_then(|raw| raw.parse().ok());
    let same_set = baseline
        .as_ref()
        .and_then(|b| b["fixtures"]["count"].as_integer())
        .is_some_and(|c| usize::try_from(c).ok() == Some(count));
    let old = |field: &str| {
        baseline
            .as_ref()
            .and_then(|b| b.get("permille")?.get(field)?.as_integer())
            .and_then(|v| usize::try_from(v).ok())
    };
    if std::env::var_os("SOFARR_UPDATE_BASELINE").is_some() {
        let mut out = String::from(
            "# Episoden-Parser-Ratchet: Pass-Rate pro Feld in Promille über fixtures/public/sonarr-parse.jsonl.\n\
             # Neu schreiben: SOFARR_UPDATE_BASELINE=1 cargo test -p sofarr-parser --test episode_oracle\n\n",
        );
        out.push_str(&format!("[fixtures]\ncount = {count}\n\n[permille]\n"));
        for (field, rate) in &rates {
            let keep = if same_set {
                old(field).unwrap_or(0).max(*rate)
            } else {
                *rate
            };
            out.push_str(&format!("{field} = {keep}\n"));
        }
        fs::write(&baseline_path, out).expect("baseline writable");
        return;
    }
    if same_set {
        for (field, rate) in &rates {
            if let Some(min) = old(field) {
                assert!(*rate >= min, "{field}: {rate}‰ unter der Baseline {min}‰");
            }
        }
    }
}
