//! Decision ratchet: compares `sofarr_core::decide` with the release oracle
//! (`fixtures/*/radarr-release.jsonl`) via `sofarr_parity::diff`.
//!
//! Pass rates over the public fixtures must not drop below
//! `fixtures/decision-baseline.toml`. `SOFARR_UPDATE_BASELINE=1` rewrites the baseline –
//! upwards only while the fixture set is unchanged; after the set changed it records the
//! current rates, even if lower. Same rules as the parser ratchet
//! (`crates/sofarr-parser/tests/oracle.rs`); the logic is duplicated on purpose, since
//! nothing may depend on `sofarr-parity`.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sofarr_parity::diff;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
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

const BASELINE_HEADER: &str = "\
# Decision-Ratchet: Pass-Rate pro Feld in Promille, nur über fixtures/public/.
# Nicht von Hand pflegen – neu schreiben mit:
#   SOFARR_UPDATE_BASELINE=1 cargo test -p sofarr-parity --test release_oracle
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
fn release_oracle() {
    let dir = fixtures_dir();
    let config =
        diff::load_config(&dir.join("public/config/radarr")).unwrap_or_else(|e| panic!("{e:#}"));
    let Some(config) = config else {
        eprintln!("übersprungen: keine öffentliche Config unter fixtures/public/config/radarr");
        return;
    };
    let public = diff::load_dir(&dir.join("public")).unwrap_or_else(|e| panic!("{e:#}"));
    if public.is_empty() {
        eprintln!("übersprungen: keine öffentlichen Release-Fixtures");
        return;
    }
    let private = diff::load_dir(&dir.join("private")).unwrap_or_else(|e| panic!("{e:#}"));

    let report = diff::evaluate(&config, &public);
    print!("{}", report.render("public"));
    if !private.is_empty() {
        print!("{}", diff::evaluate(&config, &private).render("private"));
    }

    let baseline_path = dir.join("decision-baseline.toml");
    let baseline: Baseline = match fs::read_to_string(&baseline_path) {
        Ok(raw) => toml::from_str(&raw)
            .unwrap_or_else(|e| panic!("{} ungültig: {e}", baseline_path.display())),
        Err(_) => Baseline::default(),
    };
    let current = Baseline {
        fixtures: FixtureSet {
            count: public.len(),
            fingerprint: diff::fingerprint(&public),
        },
        permille: report.permille(),
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
             SOFARR_UPDATE_BASELINE=1 cargo test -p sofarr-parity --test release_oracle"
        );
    }

    if outcome.regressions.is_empty() || (update && outcome.set_changed) {
        return;
    }
    let hint = if outcome.set_changed {
        "Das Fixture-Set hat sich geändert. Wenn die neuen Fixtures den Rückgang erklären, \
         Baseline mit SOFARR_UPDATE_BASELINE=1 neu schreiben (dann auch nach unten)."
    } else {
        "Die Decision Engine ist schlechter geworden – das Fixture-Set ist unverändert."
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
        let old = baseline(10, "a", &[("score", 900), ("approved", 800)]);
        let new = baseline(10, "a", &[("score", 950), ("approved", 700)]);
        let outcome = ratchet(&old, &new);
        assert_eq!(outcome.regressions, ["approved: 800 ‰ → 700 ‰"]);
        assert!(!outcome.set_changed);
        assert_eq!(outcome.next.permille["score"], 950);
        assert_eq!(outcome.next.permille["approved"], 800);
    }

    #[test]
    fn changed_set_may_go_down() {
        let old = baseline(10, "a", &[("score", 900)]);
        let new = baseline(12, "b", &[("score", 850)]);
        let outcome = ratchet(&old, &new);
        assert!(outcome.set_changed);
        assert_eq!(outcome.regressions.len(), 1);
        assert_eq!(outcome.next, new);
    }
}
