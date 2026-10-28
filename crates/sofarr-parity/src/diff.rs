//! Compares sofarr's grab decisions with the release oracle (`*-release.jsonl`, see
//! `releases`), field by field.
//!
//! Shared by the CI ratchet (`tests/release_oracle.rs`) and `sofarr-parity diff
//! releases`. Field paths follow the recorded responses themselves; rejection texts are
//! mapped to `Rejection::key()` by the patterns observed in the fixtures. Texts without
//! a mapping are counted, with numbers normalised, so gaps show up in the report.

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde_json::Value;
use sofarr_core::{
    ExistingFile, LanguageRef, MovieContext, ProfileId, QualityId, RadarrConfig, Rejection,
    ReleaseInput, decide, import,
};
use sofarr_parser::Revision;

pub const FIELDS: &[&str] = &[
    "quality",
    "custom_formats",
    "score",
    "approved",
    "rejections",
];
const MAX_FAILURES_SHOWN: usize = 20;
/// Prefix of an expected rejection whose oracle text has no mapping.
const UNMAPPED: &str = "?";

/// One recorded release with the film it was searched for.
pub struct Fixture {
    pub title: String,
    pub movie: Value,
    pub response: Value,
}

/// Reads all `*-release.jsonl` in `dir`; a missing directory gives no fixtures.
pub fn load_dir(dir: &Path) -> anyhow::Result<Vec<Fixture>> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(Vec::new());
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("-release.jsonl"))
        })
        .collect();
    files.sort();

    let mut fixtures = Vec::new();
    for file in files {
        let raw = fs::read_to_string(&file)
            .with_context(|| format!("{} nicht lesbar", file.display()))?;
        for (n, line) in raw.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let at = || format!("{}:{}", file.display(), n + 1);
            let mut value: Value =
                serde_json::from_str(line).with_context(|| format!("{}: ungültiges JSON", at()))?;
            let title = value["title"]
                .as_str()
                .with_context(|| format!("{}: `title` fehlt", at()))?
                .to_owned();
            fixtures.push(Fixture {
                title,
                movie: value["movie"].take(),
                response: value["response"].take(),
            });
        }
    }
    Ok(fixtures)
}

/// Imports the config from an `export-public` directory; `None` if it is incomplete.
pub fn load_config(dir: &Path) -> anyhow::Result<Option<RadarrConfig>> {
    let read = |name: &str| fs::read_to_string(dir.join(name)).ok();
    let (Some(definitions), Some(profiles), Some(formats)) = (
        read("qualitydefinition.json"),
        read("qualityprofile.json"),
        read("customformat.json"),
    ) else {
        return Ok(None);
    };
    let settings = read("settings.json");
    import::radarr(&definitions, &profiles, &formats, settings.as_deref())
        .map(Some)
        .with_context(|| format!("Config unter {} nicht importierbar", dir.display()))
}

/// FNV-1a (64 bit) over the sorted `title` + film title pairs.
pub fn fingerprint(fixtures: &[Fixture]) -> String {
    let mut keys: Vec<String> = fixtures
        .iter()
        .map(|f| format!("{}\t{}", f.title, f.movie["title"].as_str().unwrap_or("")))
        .collect();
    keys.sort_unstable();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for key in keys {
        for byte in key.bytes().chain(std::iter::once(b'\n')) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("fnv1a64:{hash:016x}")
}

/// Maps an oracle rejection text to `Rejection::key()`.
pub fn rejection_key(text: &str) -> Option<&'static str> {
    let text = text.trim();
    let key = if text.starts_with("Unknown Movie.") {
        "unknown_movie"
    } else if text.starts_with("Hardcode subs found") {
        "hardcoded_subs"
    } else if text.ends_with(" is not wanted in profile") {
        "quality_not_wanted"
    } else if text.contains(" is wanted, but found ") {
        "language_not_wanted"
    } else if text.starts_with("Custom Formats ") && text.contains(" below ") {
        "below_minimum_score"
    } else if text.contains(" is larger than maximum allowed ") {
        "too_large"
    } else {
        return None;
    };
    Some(key)
}

/// Replaces numbers (`17.3`, `-30000`) with `#`, so texts of one kind group together.
pub fn normalize_numbers(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            while chars
                .peek()
                .is_some_and(|n| n.is_ascii_digit() || *n == '.' || *n == ',')
            {
                chars.next();
            }
            // A minus sign, not a hyphen inside a word (`DTS-HD`).
            let mut before = out.chars().rev();
            if before.next() == Some('-') && !before.next().is_some_and(char::is_alphanumeric) {
                out.pop();
            }
            out.push('#');
        } else {
            out.push(c);
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub title: String,
    pub movie: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Default)]
pub struct FieldResult {
    pub pass: usize,
    pub total: usize,
    pub failures: Vec<Failure>,
}

impl FieldResult {
    pub fn permille(&self) -> Option<u32> {
        (self.total > 0).then(|| (self.pass * 1000 / self.total) as u32)
    }
}

#[derive(Debug, Default)]
pub struct Report {
    pub fixtures: usize,
    /// Fixtures whose film has a profile the config does not know.
    pub skipped: usize,
    pub fields: BTreeMap<&'static str, FieldResult>,
    /// Oracle rejection texts without a mapping, numbers normalised, with counts.
    pub unmapped: BTreeMap<String, usize>,
}

impl Report {
    /// Pass rate per field in permille, for the baseline.
    pub fn permille(&self) -> BTreeMap<String, u32> {
        self.fields
            .iter()
            .filter_map(|(field, r)| Some(((*field).to_owned(), r.permille()?)))
            .collect()
    }

    pub fn render(&self, label: &str) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "\n=== Release-Orakel: {label} ({} Fixtures) ===",
            self.fixtures
        );
        if self.fixtures == 0 {
            let _ = writeln!(out, "keine Fixtures vorhanden");
            return out;
        }
        if self.skipped > 0 {
            let _ = writeln!(
                out,
                "{} übersprungen: Profil nicht in der Config",
                self.skipped
            );
        }
        for field in FIELDS {
            let r = &self.fields[field];
            let pct = if r.total == 0 {
                0.0
            } else {
                r.pass as f64 * 100.0 / r.total as f64
            };
            let _ = writeln!(out, "{field:<15} {:>5}/{:<5} {pct:>6.1} %", r.pass, r.total);
        }
        if !self.unmapped.is_empty() {
            let _ = writeln!(out, "\n--- Rejection-Texte ohne Mapping ---");
            for (text, count) in &self.unmapped {
                let _ = writeln!(out, "{count:>5}× {text}");
            }
        }
        for field in FIELDS {
            let r = &self.fields[field];
            if r.failures.is_empty() {
                continue;
            }
            let _ = writeln!(
                out,
                "\n--- {field}: erste {} von {} Fehlschlägen ---",
                r.failures.len().min(MAX_FAILURES_SHOWN),
                r.failures.len()
            );
            for f in r.failures.iter().take(MAX_FAILURES_SHOWN) {
                let _ = writeln!(
                    out,
                    "{} [{}]\n    erwartet: {}\n    erhalten: {}",
                    f.title, f.movie, f.expected, f.actual
                );
            }
        }
        out
    }
}

/// Decides every fixture with `config` and compares the result with the oracle.
pub fn evaluate(config: &RadarrConfig, fixtures: &[Fixture]) -> Report {
    let mut report = Report {
        fixtures: fixtures.len(),
        fields: FIELDS
            .iter()
            .map(|f| (*f, FieldResult::default()))
            .collect(),
        ..Report::default()
    };
    for fixture in fixtures {
        let profile = fixture.movie["qualityProfileId"]
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .and_then(|id| config.profile(ProfileId(id)));
        let Some(profile) = profile else {
            report.skipped += 1;
            continue;
        };
        let release = ReleaseInput {
            title: &fixture.title,
            size_bytes: fixture.response["size"].as_u64().unwrap_or(0),
            age_minutes: fixture.response["ageMinutes"].as_f64().unwrap_or(0.0),
        };
        let decision = decide(config, profile, &release, &movie_context(&fixture.movie));

        let response = &fixture.response;
        let expected_rejections = expected_rejections(response, &mut report.unmapped);
        let mut rejections: Vec<&str> = decision.rejections.iter().map(Rejection::key).collect();
        rejections.sort_unstable();
        rejections.dedup();
        let mut formats: Vec<String> = decision
            .custom_formats
            .iter()
            .filter_map(|id| config.custom_formats.iter().find(|cf| cf.id == *id))
            .map(|cf| cf.name.clone())
            .collect();
        formats.sort_unstable();

        let comparisons = [
            (
                "quality",
                response
                    .pointer("/quality/quality/name")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                decision.quality.name.clone(),
            ),
            (
                "custom_formats",
                response["customFormats"].as_array().map(|formats| {
                    let mut names: Vec<&str> =
                        formats.iter().filter_map(|f| f["name"].as_str()).collect();
                    names.sort_unstable();
                    names.join(", ")
                }),
                formats.join(", "),
            ),
            (
                "score",
                response["customFormatScore"]
                    .as_i64()
                    .map(|s| s.to_string()),
                decision.score.to_string(),
            ),
            (
                "approved",
                response["approved"].as_bool().map(|a| a.to_string()),
                decision.approved().to_string(),
            ),
            ("rejections", expected_rejections, rejections.join(", ")),
        ];
        for (field, expected, actual) in comparisons {
            let Some(expected) = expected else {
                continue;
            };
            let result = report.fields.get_mut(field).expect("field is registered");
            result.total += 1;
            if expected == actual {
                result.pass += 1;
            } else {
                result.failures.push(Failure {
                    title: fixture.title.clone(),
                    movie: fixture.movie["title"].as_str().unwrap_or("?").to_owned(),
                    expected: show(&expected),
                    actual: show(&actual),
                });
            }
        }
    }
    report
}

fn show(value: &str) -> String {
    if value.is_empty() {
        "–".to_owned()
    } else {
        value.to_owned()
    }
}

/// Sorted, deduplicated keys; unmapped texts stay visible as `? <text>`.
fn expected_rejections(response: &Value, unmapped: &mut BTreeMap<String, usize>) -> Option<String> {
    let mut keys: Vec<String> = response["rejections"]
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .map(|text| match rejection_key(text) {
            Some(key) => key.to_owned(),
            None => {
                let text = normalize_numbers(text.trim());
                *unmapped.entry(text.clone()).or_default() += 1;
                format!("{UNMAPPED} {text}")
            }
        })
        .collect();
    keys.sort_unstable();
    keys.dedup();
    Some(keys.join(", "))
}

/// The film as the decision sees it.
pub fn movie_context(movie: &Value) -> MovieContext {
    let mut titles: Vec<String> = ["title", "originalTitle"]
        .iter()
        .filter_map(|k| movie[*k].as_str())
        .map(str::to_owned)
        .collect();
    if let Some(alternates) = movie["alternateTitles"].as_array() {
        titles.extend(
            alternates
                .iter()
                .filter_map(|a| a.as_str().or_else(|| a["title"].as_str()))
                .map(str::to_owned),
        );
    }
    let original_language = movie
        .pointer("/originalLanguage/name")
        .and_then(Value::as_str)
        .and_then(LanguageRef::from_name)
        .and_then(|l| match l {
            LanguageRef::Language(l) => Some(l),
            _ => None,
        });
    let file = movie["movieFile"]
        .is_object()
        .then(|| existing_file(&movie["movieFile"]))
        .flatten()
        .filter(|_| movie["hasFile"].as_bool() != Some(false));
    MovieContext {
        titles,
        runtime_minutes: movie["runtime"]
            .as_u64()
            .and_then(|r| u32::try_from(r).ok())
            .unwrap_or(0),
        original_language,
        file,
    }
}

fn existing_file(file: &Value) -> Option<ExistingFile> {
    let quality = file.pointer("/quality/quality/id")?.as_u64()?;
    let revision = file.pointer("/quality/revision");
    let number = |key: &str| {
        revision
            .and_then(|r| r[key].as_u64())
            .and_then(|v| u8::try_from(v).ok())
    };
    Some(ExistingFile {
        quality: QualityId(u32::try_from(quality).ok()?),
        revision: Revision {
            version: number("version").unwrap_or(1),
            real: number("real").unwrap_or(0),
            repack: revision
                .and_then(|r| r["isRepack"].as_bool())
                .unwrap_or(false),
            ..Revision::default()
        },
        custom_format_score: file["customFormatScore"]
            .as_i64()
            .and_then(|s| i32::try_from(s).ok())
            .unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_observed_rejection_texts() {
        let cases = [
            (
                "Unknown Movie. Unable to match to correct movie using release title.",
                "unknown_movie",
            ),
            ("Hardcode subs found: XYsub", "hardcoded_subs"),
            ("WEBDL-2160p is not wanted in profile", "quality_not_wanted"),
            (
                "German is wanted, but found English, French",
                "language_not_wanted",
            ),
            (
                "Custom Formats A, B have score -300 below Movie's profile minimum 0",
                "below_minimum_score",
            ),
            (
                "17.3 GB is larger than maximum allowed 11.7 GB (for Some Film)",
                "too_large",
            ),
        ];
        for (text, key) in cases {
            assert_eq!(rejection_key(text), Some(key), "{text}");
        }
        assert_eq!(rejection_key("Something new"), None);
    }

    #[test]
    fn numbers_are_normalised() {
        assert_eq!(
            normalize_numbers("17.3 GB is larger than 11.7 GB, score -30000"),
            "# GB is larger than # GB, score #"
        );
        assert_eq!(normalize_numbers("x264-GRP"), "x#-GRP");
        assert_eq!(normalize_numbers("DTS-HD-5"), "DTS-HD-#");
    }

    #[test]
    fn movie_context_from_fixture() {
        let movie = json!({
            "title": "Some Film", "originalTitle": "Ein Film", "runtime": 101,
            "alternateTitles": ["Film, Some", {"title": "Some Film II"}],
            "originalLanguage": {"id": 4, "name": "German"},
            "hasFile": true,
            "movieFile": {"quality": {"quality": {"id": 7}, "revision": {"version": 2, "real": 1, "isRepack": true}},
                          "customFormatScore": 50}
        });
        let context = movie_context(&movie);
        assert_eq!(
            context.titles,
            ["Some Film", "Ein Film", "Film, Some", "Some Film II"]
        );
        assert_eq!(context.runtime_minutes, 101);
        assert_eq!(
            context.original_language,
            Some(sofarr_parser::Language::German)
        );
        let file = context.file.expect("file");
        assert_eq!(file.quality, QualityId(7));
        assert_eq!(
            (
                file.revision.version,
                file.revision.real,
                file.revision.repack
            ),
            (2, 1, true)
        );
        assert_eq!(file.custom_format_score, 50);

        let without = movie_context(&json!({"title": "X", "hasFile": false}));
        assert!(without.file.is_none());
        assert_eq!(without.runtime_minutes, 0);
    }

    #[test]
    fn unmapped_texts_are_counted() {
        let mut unmapped = BTreeMap::new();
        let response = json!({"rejections": [
            "German is wanted, but found English",
            "Release is 3 days old",
            "Release is 5 days old",
        ]});
        let keys = expected_rejections(&response, &mut unmapped).expect("rejections");
        assert_eq!(keys, "? Release is # days old, language_not_wanted");
        assert_eq!(unmapped["Release is # days old"], 2);
    }
}
