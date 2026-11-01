//! Named qualities against the oracle: every quality the oracle reported for the public
//! parse fixtures must be in the built-in catalog with the same id, name and parts.
//! The end-to-end rate (parser → named quality) is reported; the parser ratchet guards
//! the parts it is built from.

use std::{fs, path::Path};

use serde_json::Value;
use sofarr_core::{QualityCatalog, QualityId, quality::parts_from_api};

/// `(title, quality object)` of all public parse fixtures that carry a quality.
fn oracle_qualities() -> Vec<(String, Value)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/public");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut files: Vec<_> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("-parse.jsonl"))
        })
        .collect();
    files.sort();

    let mut out = Vec::new();
    for file in files {
        let raw = fs::read_to_string(&file).expect("fixture lesbar");
        for line in raw.lines().filter(|l| !l.trim().is_empty()) {
            let value: Value = serde_json::from_str(line).expect("gültiges JSON");
            let quality = &value["response"]["parsedMovieInfo"]["quality"]["quality"];
            if quality.is_object() {
                out.push((
                    value["title"].as_str().unwrap_or("").to_owned(),
                    quality.clone(),
                ));
            }
        }
    }
    out
}

#[test]
fn catalog_matches_oracle() {
    let catalog = QualityCatalog::observed();
    let fixtures = oracle_qualities();
    let mut mismatches = Vec::new();
    for (title, q) in &fixtures {
        let id = QualityId(q["id"].as_u64().expect("id") as u32);
        let name = q["name"].as_str().expect("name");
        let parts = parts_from_api(
            q["source"].as_str().expect("source"),
            q["resolution"].as_u64().expect("resolution") as u32,
            q["modifier"].as_str().expect("modifier"),
        );
        let found = parts.and_then(|p| catalog.find(&p));
        if found.map(|f| (f.id, f.name.as_str())) != Some((id, name)) {
            mismatches.push(format!("{title}: Orakel {id} {name}, Katalog {found:?}"));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} von {} Qualities passen nicht:\n{}",
        mismatches.len(),
        fixtures.len(),
        mismatches.join("\n")
    );
}

#[test]
fn parsed_quality_report() {
    let catalog = QualityCatalog::observed();
    let fixtures = oracle_qualities();
    let mut pass = 0;
    let mut failures = Vec::new();
    for (title, q) in &fixtures {
        let want = q["name"].as_str().unwrap_or("");
        let got = &catalog.resolve(&sofarr_parser::parse(title).quality).name;
        if want == got {
            pass += 1;
        } else {
            failures.push(format!(
                "{title}\n    erwartet: {want}\n    erhalten: {got}"
            ));
        }
    }
    println!(
        "=== Named Quality: {pass}/{} ({:.1} %) ===",
        fixtures.len(),
        100.0 * pass as f64 / fixtures.len().max(1) as f64
    );
    for failure in failures.iter().take(20) {
        println!("{failure}");
    }
}
