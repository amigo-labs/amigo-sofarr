//! `compare-mediainfo` (spec mediainfo AC-08, AC-09): sofarr's labels from the library
//! files against Radarr's or Sonarr's `mediaInfo`.

use std::{collections::BTreeMap, fmt::Write as _, path::PathBuf};

use serde_json::Value;
use sofarr_mediainfo::{MediaInfo, ReadStatus};

use crate::{app::App, media, radarr::Radarr};

/// Which app is the oracle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OracleApp {
    Radarr,
    Sonarr,
}

/// A library file the oracle reports, with its local path and the oracle's `mediaInfo`.
struct OracleFile {
    name: String,
    path: Option<PathBuf>,
    media: Value,
}

const FIELDS: [&str; 6] = [
    "Video",
    "Audio",
    "Kanäle",
    "Dynamikumfang",
    "Audiosprachen",
    "Untertitelsprachen",
];

/// How many mismatches are printed per field.
const SHOWN: usize = 20;

#[derive(Default)]
struct Field {
    passed: usize,
    total: usize,
    mismatches: Vec<(String, String, String)>,
}

/// The oracle's languages (`German/English`) as ISO 639-2/B; a name that cannot be
/// mapped stays as it is, so it shows up as a mismatch.
fn oracle_languages(raw: &str) -> Vec<String> {
    raw.split(['/', ','])
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|n| sofarr_mediainfo::language_from_name(n).unwrap_or_else(|| n.to_owned()))
        .collect()
}

fn channels(value: &Value) -> String {
    value
        .as_f64()
        .map(|c| format!("{c:.1}"))
        .or_else(|| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// The six compared values of a file: (sofarr, oracle) per field.
fn values(info: &MediaInfo, oracle: &Value) -> [(String, String); 6] {
    let labels = info.labels();
    let text = |v: &Value| v.as_str().unwrap_or_default().to_owned();
    let audio_languages: Vec<String> = info.audio.iter().map(|a| a.language.clone()).collect();
    let subtitle_languages: Vec<String> =
        info.subtitles.iter().map(|s| s.language.clone()).collect();
    [
        (
            labels.video.unwrap_or_default(),
            text(&oracle["videoCodec"]),
        ),
        (
            labels.audio.unwrap_or_default(),
            text(&oracle["audioCodec"]),
        ),
        (
            labels.channels.unwrap_or_default(),
            channels(&oracle["audioChannels"]),
        ),
        (
            labels.dynamic_range.unwrap_or_default(),
            text(&oracle["videoDynamicRangeType"]),
        ),
        (
            audio_languages.join("/"),
            oracle_languages(&text(&oracle["audioLanguages"])).join("/"),
        ),
        (
            subtitle_languages.join("/"),
            oracle_languages(&text(&oracle["subtitles"])).join("/"),
        ),
    ]
}

async fn radarr_files(app: &App, radarr: &Radarr) -> anyhow::Result<Vec<OracleFile>> {
    let movies = app.db.movies().await?;
    let theirs = radarr.get("movie", &[]).await?;
    Ok(theirs
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|m| {
            let file = &m["movieFile"];
            let relative = file["relativePath"].as_str()?;
            let ours = movies
                .iter()
                .find(|x| x.radarr_id.is_some() && x.radarr_id == m["id"].as_i64());
            Some(OracleFile {
                name: relative.to_owned(),
                path: ours.map(|x| PathBuf::from(&x.path).join(relative)),
                media: file["mediaInfo"].clone(),
            })
        })
        .collect())
}

async fn sonarr_files(app: &App, sonarr: &Radarr) -> anyhow::Result<Vec<OracleFile>> {
    let shows = app.db.all_series().await?;
    let theirs = sonarr.get("series", &[]).await?;
    let mut out = Vec::new();
    for s in theirs.as_array().map_or(&[][..], Vec::as_slice) {
        let Some(id) = s["id"].as_i64() else {
            continue;
        };
        let ours = shows.iter().find(|x| x.data.sonarr_id == Some(id));
        let id = id.to_string();
        let files = sonarr
            .get("episodefile", &[("seriesId", id.as_str())])
            .await?;
        for file in files.as_array().map_or(&[][..], Vec::as_slice) {
            let Some(relative) = file["relativePath"].as_str() else {
                continue;
            };
            out.push(OracleFile {
                name: relative.to_owned(),
                path: ours.map(|x| PathBuf::from(&x.data.path).join(relative)),
                media: file["mediaInfo"].clone(),
            });
        }
    }
    Ok(out)
}

/// Compares sofarr's media info labels with the oracle's; returns the report and
/// whether every compared value matched.
pub async fn run(app: &App, oracle: &Radarr, which: OracleApp) -> anyhow::Result<(String, bool)> {
    let files = match which {
        OracleApp::Radarr => radarr_files(app, oracle).await?,
        OracleApp::Sonarr => sonarr_files(app, oracle).await?,
    };
    let mut fields: Vec<Field> = FIELDS.iter().map(|_| Field::default()).collect();
    let mut skipped: BTreeMap<&'static str, usize> = BTreeMap::new();
    for file in &files {
        if !file.media.is_object() {
            *skipped.entry("Orakel ohne mediaInfo").or_default() += 1;
            continue;
        }
        let Some(path) = &file.path else {
            *skipped.entry("nicht in sofarrs Bibliothek").or_default() += 1;
            continue;
        };
        let Some(read) = media::read(path.clone()).await else {
            *skipped.entry("Datei fehlt").or_default() += 1;
            continue;
        };
        match read.info.status {
            ReadStatus::Unreadable => {
                *skipped.entry("Datei nicht lesbar").or_default() += 1;
                continue;
            }
            ReadStatus::Unsupported => {
                *skipped.entry("Container nicht unterstützt").or_default() += 1;
                continue;
            }
            _ => {}
        }
        for (field, (ours, theirs)) in fields.iter_mut().zip(values(&read.info, &file.media)) {
            field.total += 1;
            if ours == theirs {
                field.passed += 1;
            } else {
                field.mismatches.push((file.name.clone(), theirs, ours));
            }
        }
    }
    let mut report = String::new();
    let mut clean = true;
    for (name, field) in FIELDS.iter().zip(&fields) {
        let _ = writeln!(report, "{name}: {}/{} gleich", field.passed, field.total);
        for (file, expected, found) in field.mismatches.iter().take(SHOWN) {
            let _ = writeln!(
                report,
                "  {file}: erwartet „{expected}“, gefunden „{found}“"
            );
        }
        clean &= field.mismatches.is_empty();
    }
    for (reason, n) in &skipped {
        let _ = writeln!(report, "Übersprungen ({reason}): {n}");
    }
    Ok((report, clean))
}
