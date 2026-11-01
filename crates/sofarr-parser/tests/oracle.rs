//! Oracle harness: compares `sofarr_parser::parse` field by field against recorded
//! answers of a running Radarr instance (`GET /api/v3/parse`), see `fixtures/`.
//!
//! Field paths in the oracle response follow the names in the public Radarr API docs and
//! are read tolerantly: a field missing from a response is skipped for that fixture.
//!
//! Ratchet: pass rates over the public fixtures must not drop below
//! `fixtures/parser-baseline.toml`. `SOFARR_UPDATE_BASELINE=1` rewrites the baseline —
//! upwards only while the fixture set is unchanged; after the set changed it records the
//! current rates, even if lower.

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sofarr_parser::{HardcodedSubs, ParsedRelease, parse};

const FIELDS: &[&str] = &[
    "title",
    "year",
    "source",
    "resolution",
    "modifier",
    "revision",
    "edition",
    "release_group",
    "languages",
    "hardcoded_subs",
];
const MAX_FAILURES_SHOWN: usize = 20;
/// Separates title candidates in an expected value; never produced by `normalize`.
const CANDIDATE_SEP: &str = " | ";

struct Fixture {
    title: String,
    response: Value,
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn load_dir(dir: &Path) -> Vec<Fixture> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "jsonl"))
        // Only parse fixtures; release fixtures (`*-release.jsonl`) have another shape.
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains("-parse"))
        })
        .collect();
    files.sort();

    let mut fixtures = Vec::new();
    for file in files {
        let raw = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("{} nicht lesbar: {e}", file.display()));
        for (n, line) in raw.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("{}:{}: ungültiges JSON: {e}", file.display(), n + 1));
            let title = value["title"]
                .as_str()
                .unwrap_or_else(|| panic!("{}:{}: `title` fehlt", file.display(), n + 1))
                .to_owned();
            fixtures.push(Fixture {
                title,
                response: value["response"].clone(),
            });
        }
    }
    fixtures
}

/// Lower-case, punctuation and separators collapsed to single spaces.
fn normalize(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn non_empty(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(normalize)
}

fn show(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("–")
}

/// Expected value of `field` from the oracle response. Outer `None`: not comparable.
fn expected(field: &str, response: &Value) -> Option<Option<String>> {
    let info = response.get("parsedMovieInfo").filter(|v| v.is_object())?;
    let quality = info.pointer("/quality/quality");
    let revision = info.pointer("/quality/revision");
    Some(match field {
        // All title candidates; for `A AKA B` the oracle lists `A AKA B`, `A` and `B`.
        "title" => {
            let titles: Vec<String> = info
                .get("movieTitles")?
                .as_array()?
                .iter()
                .filter_map(non_empty)
                .collect();
            (!titles.is_empty()).then(|| titles.join(CANDIDATE_SEP))
        }
        "year" => {
            let year = info.get("year")?.as_u64()?;
            (year != 0).then(|| year.to_string())
        }
        "source" => non_empty(quality?.get("source")?),
        "resolution" => {
            let resolution = quality?.get("resolution")?.as_u64()?;
            (resolution != 0).then(|| resolution.to_string())
        }
        "modifier" => non_empty(quality?.get("modifier")?),
        "revision" => {
            let revision = revision?;
            Some(format!(
                "v{} real{} repack{}",
                revision.get("version")?.as_u64()?,
                revision.get("real").and_then(Value::as_u64).unwrap_or(0),
                revision
                    .get("isRepack")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            ))
        }
        "edition" => non_empty(info.get("edition")?),
        "release_group" => non_empty(info.get("releaseGroup")?),
        "languages" => {
            let mut names: Vec<String> = info
                .get("languages")?
                .as_array()?
                .iter()
                .filter_map(|l| l.get("name").and_then(non_empty))
                .collect();
            names.sort();
            (!names.is_empty()).then(|| names.join(", "))
        }
        // The oracle leaves the field out when there are none.
        "hardcoded_subs" => info.get("hardcodedSubs").and_then(non_empty),
        _ => unreachable!("unknown field {field}"),
    })
}

fn actual(field: &str, parsed: &ParsedRelease) -> Option<String> {
    match field {
        "title" => parsed.title().map(normalize),
        "year" => parsed.year.map(|y| y.to_string()),
        "source" => Some(parsed.quality.source.as_str().to_owned()),
        "resolution" => parsed.quality.resolution.height().map(|h| h.to_string()),
        "modifier" => Some(parsed.quality.modifier.as_str().to_owned()),
        "revision" => Some(format!(
            "v{} real{} repack{}",
            parsed.revision.version, parsed.revision.real, parsed.revision.repack
        )),
        "edition" => parsed.edition.as_deref().map(normalize),
        "release_group" => parsed.release_group.as_deref().map(normalize),
        "languages" => {
            let mut names: Vec<String> = parsed
                .languages
                .iter()
                .map(|l| normalize(l.name()))
                .collect();
            names.sort();
            // The oracle reports `Unknown` when it finds no language.
            Some(if names.is_empty() {
                "unknown".to_owned()
            } else {
                names.join(", ")
            })
        }
        // The oracle names untagged burnt-in subtitles `Generic Hardcoded Subs`.
        "hardcoded_subs" => parsed.hardcoded_subs.as_ref().map(|subs| match subs {
            HardcodedSubs::Generic => normalize("Generic Hardcoded Subs"),
            HardcodedSubs::Tagged(tag) => normalize(tag),
        }),
        _ => unreachable!("unknown field {field}"),
    }
}

/// Whether the parser's value matches the oracle's. For titles, any candidate counts.
fn matches(field: &str, want: &Option<String>, got: &Option<String>) -> bool {
    match (field, want, got) {
        ("title", Some(want), Some(got)) => want.split(CANDIDATE_SEP).any(|c| c == got),
        _ => want == got,
    }
}

#[derive(Default)]
struct FieldResult {
    pass: usize,
    total: usize,
    failures: Vec<(String, Option<String>, Option<String>)>,
}

impl FieldResult {
    fn permille(&self) -> Option<u32> {
        (self.total > 0).then(|| (self.pass * 1000 / self.total) as u32)
    }
}

fn evaluate(fixtures: &[Fixture]) -> BTreeMap<&'static str, FieldResult> {
    let mut results: BTreeMap<&'static str, FieldResult> = FIELDS
        .iter()
        .map(|f| (*f, FieldResult::default()))
        .collect();
    for fixture in fixtures {
        let parsed = parse(&fixture.title);
        for field in FIELDS {
            let Some(want) = expected(field, &fixture.response) else {
                continue;
            };
            let got = actual(field, &parsed);
            let result = results.get_mut(field).expect("field is registered");
            result.total += 1;
            if matches(field, &want, &got) {
                result.pass += 1;
            } else {
                result.failures.push((fixture.title.clone(), want, got));
            }
        }
    }
    results
}

fn report(label: &str, count: usize, results: &BTreeMap<&'static str, FieldResult>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "\n=== Orakel-Report: {label} ({count} Fixtures) ===");
    if count == 0 {
        let _ = writeln!(out, "keine Fixtures vorhanden");
        return out;
    }
    for field in FIELDS {
        let r = &results[field];
        let pct = if r.total == 0 {
            0.0
        } else {
            r.pass as f64 * 100.0 / r.total as f64
        };
        let _ = writeln!(out, "{field:<14} {:>6}/{:<6} {pct:>6.1} %", r.pass, r.total);
    }
    for field in FIELDS {
        let r = &results[field];
        if r.failures.is_empty() {
            continue;
        }
        let _ = writeln!(
            out,
            "\n--- {field}: erste {} von {} Fehlschlägen ---",
            r.failures.len().min(MAX_FAILURES_SHOWN),
            r.failures.len()
        );
        for (title, want, got) in r.failures.iter().take(MAX_FAILURES_SHOWN) {
            let _ = writeln!(
                out,
                "{title}\n    erwartet: {}\n    erhalten: {}",
                show(want),
                show(got)
            );
        }
    }
    out
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Baseline {
    fixtures: FixtureSet,
    /// Pass rate per field in permille.
    #[serde(default)]
    permille: BTreeMap<String, u32>,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct FixtureSet {
    count: usize,
    fingerprint: String,
}

/// FNV-1a (64 bit) over the sorted titles: stable across Rust versions, unlike `DefaultHasher`.
fn fingerprint(fixtures: &[Fixture]) -> String {
    let mut titles: Vec<&str> = fixtures.iter().map(|f| f.title.as_str()).collect();
    titles.sort_unstable();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for title in titles {
        for byte in title.bytes().chain(std::iter::once(b'\n')) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("fnv1a64:{hash:016x}")
}

const BASELINE_HEADER: &str = "\
# Parser-Ratchet: Pass-Rate pro Feld in Promille, nur über fixtures/public/.
# Nicht von Hand pflegen – neu schreiben mit:
#   SOFARR_UPDATE_BASELINE=1 cargo test -p sofarr-parser --test oracle
";

/// Outcome of comparing current rates with the baseline.
struct Ratchet {
    regressions: Vec<String>,
    set_changed: bool,
    next: Baseline,
}

fn ratchet(baseline: &Baseline, current: &Baseline) -> Ratchet {
    let set_changed = baseline.fixtures != current.fixtures;
    let regressions = baseline
        .permille
        .iter()
        .filter_map(|(field, &old)| {
            let new = *current.permille.get(field)?;
            (new < old).then(|| format!("{field}: {old} ‰ → {new} ‰"))
        })
        .collect();
    let next = if set_changed {
        Baseline {
            fixtures: FixtureSet {
                count: current.fixtures.count,
                fingerprint: current.fixtures.fingerprint.clone(),
            },
            permille: current.permille.clone(),
        }
    } else {
        let mut permille = baseline.permille.clone();
        for (field, &new) in &current.permille {
            let entry = permille.entry(field.clone()).or_insert(new);
            *entry = (*entry).max(new);
        }
        Baseline {
            fixtures: FixtureSet {
                count: baseline.fixtures.count,
                fingerprint: baseline.fixtures.fingerprint.clone(),
            },
            permille,
        }
    };
    Ratchet {
        regressions,
        set_changed,
        next,
    }
}

#[test]
fn oracle() {
    let dir = fixtures_dir();
    let public = load_dir(&dir.join("public"));
    let private = load_dir(&dir.join("private"));

    let public_results = evaluate(&public);
    print!("{}", report("public", public.len(), &public_results));
    if !private.is_empty() {
        print!("{}", report("private", private.len(), &evaluate(&private)));
    }

    let baseline_path = dir.join("parser-baseline.toml");
    let baseline: Baseline = match fs::read_to_string(&baseline_path) {
        Ok(raw) => toml::from_str(&raw)
            .unwrap_or_else(|e| panic!("{} ungültig: {e}", baseline_path.display())),
        Err(_) => Baseline::default(),
    };
    let current = Baseline {
        fixtures: FixtureSet {
            count: public.len(),
            fingerprint: fingerprint(&public),
        },
        permille: public_results
            .iter()
            .filter_map(|(field, r)| Some(((*field).to_owned(), r.permille()?)))
            .collect(),
    };
    let outcome = ratchet(&baseline, &current);

    let update = std::env::var_os("SOFARR_UPDATE_BASELINE").is_some_and(|v| v == "1");
    if update && outcome.next != baseline {
        let body = toml::to_string(&outcome.next).expect("baseline serializes");
        fs::write(&baseline_path, format!("{BASELINE_HEADER}\n{body}"))
            .unwrap_or_else(|e| panic!("{} nicht schreibbar: {e}", baseline_path.display()));
        println!("\nBaseline aktualisiert: {}", baseline_path.display());
    } else if outcome.next != baseline {
        println!(
            "\nHinweis: Baseline kann angehoben werden: \
             SOFARR_UPDATE_BASELINE=1 cargo test -p sofarr-parser --test oracle"
        );
    }

    if outcome.regressions.is_empty() || (update && outcome.set_changed) {
        return;
    }
    let hint = if outcome.set_changed {
        "Das Fixture-Set hat sich geändert. Wenn die neuen Fixtures den Rückgang erklären, \
         Baseline mit SOFARR_UPDATE_BASELINE=1 neu schreiben (dann auch nach unten)."
    } else {
        "Der Parser ist schlechter geworden – das Fixture-Set ist unverändert."
    };
    panic!(
        "Pass-Rate unter der Baseline:\n  {}\n{hint}",
        outcome.regressions.join("\n  ")
    );
}

#[cfg(test)]
mod harness_tests {
    use super::*;

    fn baseline(count: usize, fp: &str, rates: &[(&str, u32)]) -> Baseline {
        Baseline {
            fixtures: FixtureSet {
                count,
                fingerprint: fp.to_owned(),
            },
            permille: rates.iter().map(|(f, r)| ((*f).to_owned(), *r)).collect(),
        }
    }

    #[test]
    fn same_set_only_ratchets_up() {
        let old = baseline(10, "a", &[("title", 900), ("year", 800)]);
        let new = baseline(10, "a", &[("title", 950), ("year", 700)]);
        let outcome = ratchet(&old, &new);
        assert_eq!(outcome.regressions, ["year: 800 ‰ → 700 ‰"]);
        assert!(!outcome.set_changed);
        assert_eq!(outcome.next.permille["title"], 950);
        assert_eq!(outcome.next.permille["year"], 800);
    }

    #[test]
    fn changed_set_may_go_down() {
        let old = baseline(10, "a", &[("title", 900)]);
        let new = baseline(12, "b", &[("title", 850)]);
        let outcome = ratchet(&old, &new);
        assert!(outcome.set_changed);
        assert_eq!(outcome.regressions.len(), 1);
        assert_eq!(outcome.next, new);
    }

    #[test]
    fn expected_values_from_documented_fields() {
        let response = serde_json::json!({
            "parsedMovieInfo": {
                "movieTitles": ["Some Film"],
                "year": 2004,
                "quality": {
                    "quality": {"source": "bluray", "resolution": 1080, "modifier": "none"},
                    "revision": {"version": 2, "real": 0, "isRepack": true}
                },
                "edition": "",
                "releaseGroup": "GRP",
                "languages": [{"id": 4, "name": "German"}]
            }
        });
        let get = |f| expected(f, &response);
        assert_eq!(get("title"), Some(Some("some film".to_owned())));
        assert_eq!(get("year"), Some(Some("2004".to_owned())));
        assert_eq!(get("resolution"), Some(Some("1080".to_owned())));
        assert_eq!(
            get("revision"),
            Some(Some("v2 real0 repacktrue".to_owned()))
        );
        assert_eq!(get("edition"), Some(None));
        assert_eq!(get("languages"), Some(Some("german".to_owned())));
        assert_eq!(expected("title", &serde_json::json!({})), None);
        let aka = serde_json::json!({"parsedMovieInfo": {"movieTitles": ["A AKA B", "A", "B"]}});
        let want = expected("title", &aka).expect("comparable");
        assert!(matches("title", &want, &Some("b".to_owned())));
        assert!(!matches("title", &want, &Some("c".to_owned())));
        let sparse = serde_json::json!({"parsedMovieInfo": {"movieTitles": ["X"]}});
        assert_eq!(expected("release_group", &sparse), None);
        assert_eq!(expected("source", &sparse), None);
    }

    #[test]
    fn normalization_ignores_case_and_punctuation() {
        assert_eq!(normalize("Mr. Smith's  Day-Off!"), "mr smith s day off");
        assert_eq!(normalize("MR SMITH S DAY OFF"), "mr smith s day off");
    }
}
