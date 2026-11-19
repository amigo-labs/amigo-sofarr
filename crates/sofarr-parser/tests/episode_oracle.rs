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

/// Whether a title names a season itself: `S01`/`S01E02`, `1x02` or a season word with
/// a number (`Season 2`).
fn has_season_marker(title: &str) -> bool {
    let lower = title.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    words.iter().enumerate().any(|(i, w)| {
        let season = w
            .strip_prefix('s')
            .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()));
        let cross = w
            .split_once('x')
            .is_some_and(|(a, b)| digits(a) && b.starts_with(|c: char| c.is_ascii_digit()));
        let word = matches!(
            *w,
            "season" | "staffel" | "saison" | "temporada" | "stagione"
        ) && words.get(i + 1).is_some_and(|n| digits(n));
        season || cross || word
    })
}

#[test]
fn season_marker_detection() {
    assert!(has_season_marker("Show.S01E02.1080p"));
    assert!(has_season_marker("Show.S03.German.720p"));
    assert!(has_season_marker("Old Show 3x07 Title"));
    assert!(has_season_marker("Show Staffel 2 German"));
    assert!(!has_season_marker("Show.E06.1080p.WEB"));
    assert!(!has_season_marker("Unforgettable (2011-2015) S 1-4"));
}

/// The movie feed also carries real series (`S01E02`, season packs); only the weaker
/// markers – air dates, absolute numbers and episodes without a season marker – can
/// misfire on movie titles (spec parser-episode-gaps, AC-06).
#[test]
fn movies_are_rarely_episodes() {
    let fixtures = lines(&root().join("fixtures/public/radarr-parse.jsonl"));
    assert!(!fixtures.is_empty());
    let hits: Vec<&str> = fixtures
        .iter()
        .filter_map(|f| f["title"].as_str())
        .filter(|t| {
            parse_episode(t).is_some_and(|e| {
                e.air_date.is_some()
                    || !e.absolute_episodes.is_empty()
                    || (e.season.is_some() && !has_season_marker(t))
            })
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

/// The oracle titles of spec parser-episode-gaps (AC-07): each must match Sonarr on every
/// compared field.
const GAP_TITLES: [&str; 25] = [
    "[AsukaRaws].Magic.Knight.Rayearth.(2026).-.06.(WEB-DL.1280x720.x264.AAC)",
    "[SubsPlease].Magic.Knight.Rayearth.(2026).-.06.(1080p).[96B7C6E9]",
    "[SubsPlease].Magic.Knight.Rayearth.(2026).-.06.(720p).[2DA08BB3]",
    "[SubsPlease].Magic.Knight.Rayearth.(2026).-.06.(480p).[ED116078]",
    "[Erai-raws].Magic.Knight.Rayearth.(2026).-.06.[480p.CR.WEB-DL.AVC.AAC][MultiSub][0C8F4306]",
    "[Erai-raws].Magic.Knight.Rayearth.(2026).-.06.[720p.CR.WEB-DL.AVC.AAC][MultiSub][B923BEA0]",
    "[Erai-raws].Magic.Knight.Rayearth.(2026).-.06.[1080p.CR.WEB-DL.AVC.AAC][MultiSub][3329235B]",
    "[Erai-raws].Magic.Knight.Rayearth.(2026).-.06.[1080p.CR.WEBRip.HEVC.AAC][MultiSub][0AE67E21]",
    "[ASW].Magic.Knight.Rayearth.(2026).-.06.[1080p.HEVC][688DE02C]",
    "Jyuou.Mujin.Dandivine.E06.SUBFRENCH.1080p.WEB.H264-SLEEPINGFOREST",
    "Magic.Knight.Rayearth.2026.E06.SUBFRENCH.1080p.WEB.H264-SLEEPINGFOREST",
    "Sekai.Saikyou.no.Majo.Hajimemashita.E06.SUBFRENCH.1080p.WEB.H264-SLEEPINGFOREST",
    "The.Human.Spark.E02.So.Human.So.Chimp.HDTV.XviD-FQM",
    "[SubsPlease].Yamato.yo.Towa.ni.-.Rebel.3199.(23-26).(1080p).[Batch]",
    "[SubsPlease].Yamato.yo.Towa.ni.-.Rebel.3199.(23-26).(720p).[Batch]",
    "[SubsPlease].Yamato.yo.Towa.ni.-.Rebel.3199.(23-26).(480p).[Batch]",
    "[KitaujiSub].FX.Senshi.Kurumi-chan.[03][WebRip][HEVC AAC][CHS JP&CHT JP]",
    "[Studio.GreenTea].FX.Senshi.Kurumi-chan.[06][WebRip][HEVC-10bit.1080p.AAC][JPTC]",
    "[Studio.GreenTea].Toaru.Anbu.no.Item.[03][WebRip][HEVC-10bit.1080p.AAC.ASSx2]",
    "[Studio.GreenTea].Kusuriya.no.Hitorigoto.[52][WebRip][HEVC-10bit.1080p.AAC.ASSx2]",
    "[Studio.GreenTea].Mahou.Shoujo.Ikusei.Keikaku.Restart.[06][WebRip][HEVC-10bit.1080p.AAC][JPSC]",
    "[Studio.GreenTea].Mahou.Shoujo.Ikusei.Keikaku.Restart.[06][WebRip][HEVC-10bit.1080p.AAC][JPTC]",
    "[Sakurato].Koori.no.Jouheki.[20][HEVC-10bit.1080P.AAC][CHS&CHT]",
    "[Sakurato].Koori.no.Jouheki.[20][AVC-8bit.1080P.AAC][CHS]",
    "[Sakurato].Koori.no.Jouheki.[20][AVC-8bit.1080P.AAC][CHT]",
];

#[test]
fn episode_gap_titles_match_oracle() {
    let fixtures: BTreeMap<String, Value> =
        lines(&root().join("fixtures/public/sonarr-parse.jsonl"))
            .into_iter()
            .filter_map(|f| {
                Some((
                    f["title"].as_str()?.to_owned(),
                    f["response"]["parsedEpisodeInfo"].clone(),
                ))
            })
            .collect();
    if fixtures.is_empty() {
        println!("no Sonarr fixtures yet – skipping");
        return;
    }
    let mut mismatches = Vec::new();
    for title in GAP_TITLES {
        let info = fixtures
            .get(title)
            .unwrap_or_else(|| panic!("{title} fehlt in den Fixtures"));
        for field in FIELDS {
            let (want, got) = (expected(field, info), actual(field, title));
            if want != got {
                mismatches.push(format!(
                    "{title} {field}: erwartet {want:?}, erhalten {got:?}"
                ));
            }
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

/// parser-season-in-title AC-05: form 1 matches Sonarr on every field; form 2 on series
/// title and season (episodes and full season are a deliberate deviation).
const SEASON_IN_TITLE: [(&str, &[&str]); 6] = [
    (
        "[Erai-raws].Kikansha.no.Mahou.wa.Tokubetsu.desu.2nd.Season.-.06.[1080p.CR.WEB-DL.AVC.AAC][MultiSub][B49B1654]",
        &FIELDS,
    ),
    (
        "[Erai-raws].Kikansha.no.Mahou.wa.Tokubetsu.desu.2nd.Season.-.06.[1080p.CR.WEBRip.HEVC.AAC][MultiSub][BFA1EA3B]",
        &FIELDS,
    ),
    (
        "[Erai-raws].Kikansha.no.Mahou.wa.Tokubetsu.desu.2nd.Season.-.06.[480p.CR.WEB-DL.AVC.AAC][MultiSub][504C1099]",
        &FIELDS,
    ),
    (
        "[Erai-raws].Kikansha.no.Mahou.wa.Tokubetsu.desu.2nd.Season.-.06.[720p.CR.WEB-DL.AVC.AAC][MultiSub][9D4839E6]",
        &FIELDS,
    ),
    (
        "[Nekomoe.kissaten][Sasaki.to.Pii-chan.S2][05][1080p][JPSC]",
        &["series_title", "season"],
    ),
    (
        "[Nekomoe.kissaten][Sasaki.to.Pii-chan.S2][05][1080p][JPTC]",
        &["series_title", "season"],
    ),
];

#[test]
fn season_in_title_matches_oracle() {
    let fixtures: BTreeMap<String, Value> =
        lines(&root().join("fixtures/public/sonarr-parse.jsonl"))
            .into_iter()
            .filter_map(|f| {
                Some((
                    f["title"].as_str()?.to_owned(),
                    f["response"]["parsedEpisodeInfo"].clone(),
                ))
            })
            .collect();
    if fixtures.is_empty() {
        println!("no Sonarr fixtures yet – skipping");
        return;
    }
    let mut mismatches = Vec::new();
    for (title, fields) in SEASON_IN_TITLE {
        let info = fixtures
            .get(title)
            .unwrap_or_else(|| panic!("{title} fehlt in den Fixtures"));
        for field in fields {
            let (want, got) = (expected(field, info), actual(field, title));
            if want != got {
                mismatches.push(format!(
                    "{title} {field}: erwartet {want:?}, erhalten {got:?}"
                ));
            }
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
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
