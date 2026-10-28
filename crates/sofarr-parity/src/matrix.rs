//! `sofarr-parity matrix`: turns the config export of `scripts/export-arr-config.sh`
//! into the parity matrix `docs/parity.md`.
//!
//! The export contains secrets (API keys, passwords, URLs, paths). Only whitelisted,
//! non-identifying values are read: implementation types, flags, counts, naming tokens.
//! Nested `fields` arrays, URLs, hosts, paths and names are never touched.

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde_json::Value;

const APPS: &[&str] = &["radarr", "sonarr"];
const MANUAL_MARKER: &str = "<!-- Ab hier manuell gepflegt; bleibt beim Neuerzeugen erhalten. -->";
const DEFAULT_STATUS: &str = "offen";
/// Everything Sonarr-only belongs to the series phase.
const SERIES_PHASE: &str = "5+";

const MEDIA_MANAGEMENT_KEYS: &[(&str, &str)] = &[
    ("downloadPropersAndRepacks", "2"),
    ("copyUsingHardlinks", "4"),
    ("importExtraFiles", "4"),
    ("autoUnmonitorPreviouslyDownloadedMovies", "4"),
    ("autoUnmonitorPreviouslyDownloadedEpisodes", "4"),
    ("createEmptyMovieFolders", "4"),
    ("createEmptySeriesFolders", "4"),
    ("deleteEmptyFolders", "4"),
    ("autoRenameFolders", "4"),
    ("episodeTitleRequired", "4"),
    ("fileDate", "4"),
    ("rescanAfterRefresh", "4"),
    ("setPermissionsLinux", "4"),
    ("skipFreeSpaceCheckWhenImporting", "4"),
    ("minimumFreeSpaceWhenImporting", "4"),
    ("enableMediaInfo", "4"),
    ("recycleBinCleanupDays", "4"),
];
const INDEXER_CONFIG_KEYS: &[(&str, &str)] = &[
    ("minimumAge", "2"),
    ("maximumSize", "2"),
    ("retention", "2"),
    ("preferIndexerFlags", "2"),
    ("availabilityDelay", "2"),
    ("allowHardcodedSubs", "2"),
    ("rssSyncInterval", "3"),
];
const DOWNLOAD_CLIENT_CONFIG_KEYS: &[(&str, &str)] = &[
    ("enableCompletedDownloadHandling", "4"),
    ("autoRedownloadFailed", "4"),
    ("autoRedownloadFailedFromInteractiveSearch", "4"),
    ("checkForFinishedDownloadInterval", "4"),
];
const NAMING_SETTING_KEYS: &[&str] = &[
    "renameMovies",
    "renameEpisodes",
    "replaceIllegalCharacters",
    "colonReplacementFormat",
    "multiEpisodeStyle",
];

/// One app's export: file stem (`qualityprofile`, `config_naming`, …) → JSON.
pub struct AppExport {
    pub app: &'static str,
    pub files: BTreeMap<String, Value>,
}

impl AppExport {
    fn get(&self, stem: &str) -> Option<&Value> {
        self.files.get(stem)
    }

    fn list(&self, stem: &str) -> &[Value] {
        self.get(stem)
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub feature: String,
    pub uses: Vec<String>,
    pub phase: String,
}

#[derive(Default)]
struct Matrix {
    rows: Vec<Row>,
}

impl Matrix {
    fn add(&mut self, app: &str, feature: impl Into<String>, detail: impl AsRef<str>, phase: &str) {
        let feature = feature.into();
        let phase = if app == "radarr" { phase } else { SERIES_PHASE };
        let usage = format!("{app}: {}", detail.as_ref());
        match self.rows.iter_mut().find(|r| r.feature == feature) {
            Some(row) => {
                row.uses.push(usage);
                if app == "radarr" {
                    phase.clone_into(&mut row.phase);
                }
            }
            None => self.rows.push(Row {
                feature,
                uses: vec![usage],
                phase: phase.to_owned(),
            }),
        }
    }
}

pub fn load_exports(dir: &Path) -> anyhow::Result<Vec<AppExport>> {
    let mut apps = Vec::new();
    for app in APPS {
        let app_dir = dir.join(app);
        let Ok(entries) = fs::read_dir(&app_dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .collect();
        paths.sort();
        let mut files = BTreeMap::new();
        for path in paths {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("{} nicht lesbar", path.display()))?;
            let value: Value = serde_json::from_str(&raw)
                .with_context(|| format!("{}: ungültiges JSON", path.display()))?;
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_owned();
            files.insert(stem, value);
        }
        if !files.is_empty() {
            apps.push(AppExport { app, files });
        }
    }
    Ok(apps)
}

pub fn rows(apps: &[AppExport]) -> Vec<Row> {
    let mut m = Matrix::default();
    for export in apps {
        quality_profiles(&mut m, export);
        custom_formats(&mut m, export);
        count(
            &mut m,
            export,
            "qualitydefinition",
            "Quality Definitions",
            "2",
        );
        count(&mut m, export, "releaseprofile", "Release Profiles", "2");
        settings(
            &mut m,
            export,
            "config_indexer",
            "Indexer-Einstellung",
            INDEXER_CONFIG_KEYS,
        );
        implementations(&mut m, export, "indexer", "Indexer", "3");
        delay_profiles(&mut m, export);
        count(&mut m, export, "tag", "Tags", "3");
        count(&mut m, export, "rootfolder", "Root Folders", "3");
        library(&mut m, export, "movie", "Bibliothek: Filme");
        library(&mut m, export, "series", "Bibliothek: Serien");
        implementations(&mut m, export, "importlist", "Import List", "4");
        implementations(&mut m, export, "downloadclient", "Download Client", "4");
        settings(
            &mut m,
            export,
            "config_downloadclient",
            "Download-Client-Einstellung",
            DOWNLOAD_CLIENT_CONFIG_KEYS,
        );
        settings(
            &mut m,
            export,
            "config_mediamanagement",
            "Media Management",
            MEDIA_MANAGEMENT_KEYS,
        );
        naming(&mut m, export);
        notifications(&mut m, export);
    }
    m.rows
}

fn count(m: &mut Matrix, export: &AppExport, stem: &str, feature: &str, phase: &str) {
    let n = export.list(stem).len();
    if n > 0 {
        m.add(export.app, feature, n.to_string(), phase);
    }
}

fn quality_profiles(m: &mut Matrix, export: &AppExport) {
    let profiles = export.list("qualityprofile");
    if profiles.is_empty() {
        return;
    }
    let app = export.app;
    let total = profiles.len();
    let of_total = |n: usize| format!("{n} von {total} Profilen");
    m.add(
        app,
        "Quality Profiles (Cutoff)",
        format!("{total} Profile"),
        "2",
    );

    let flagged = |pred: &dyn Fn(&Value) -> bool| profiles.iter().filter(|p| pred(p)).count();
    let checks: [(&str, usize); 4] = [
        (
            "Quality Profile: Upgrades erlaubt",
            flagged(&|p| p["upgradeAllowed"].as_bool() == Some(true)),
        ),
        (
            "Quality Profile: Quality-Gruppen",
            flagged(&|p| {
                p["items"].as_array().is_some_and(|items| {
                    items
                        .iter()
                        .any(|i| i["items"].as_array().is_some_and(|sub| !sub.is_empty()))
                })
            }),
        ),
        (
            "Quality Profile: Custom-Format-Scores",
            flagged(&|p| {
                p["formatItems"].as_array().is_some_and(|items| {
                    items.iter().any(|i| i["score"].as_i64().unwrap_or(0) != 0)
                })
            }),
        ),
        (
            "Quality Profile: Mindest-/Cutoff-Score",
            flagged(&|p| {
                p["minFormatScore"].as_i64().unwrap_or(0) != 0
                    || p["cutoffFormatScore"].as_i64().unwrap_or(0) != 0
            }),
        ),
    ];
    for (feature, n) in checks {
        if n > 0 {
            m.add(app, feature, of_total(n), "2");
        }
    }

    let mut languages: BTreeMap<String, usize> = BTreeMap::new();
    let mut allowed: BTreeMap<String, usize> = BTreeMap::new();
    for profile in profiles {
        if let Some(name) = profile["language"]["name"].as_str() {
            *languages.entry(name.to_owned()).or_default() += 1;
        }
        let mut names = Vec::new();
        for item in profile["items"].as_array().map_or(&[][..], Vec::as_slice) {
            if item["allowed"].as_bool() != Some(true) {
                continue;
            }
            match item["items"].as_array().filter(|sub| !sub.is_empty()) {
                Some(sub) => names.extend(sub.iter().filter_map(|s| s["quality"]["name"].as_str())),
                None => names.extend(item["quality"]["name"].as_str()),
            }
        }
        names.sort_unstable();
        names.dedup();
        for name in names {
            *allowed.entry(name.to_owned()).or_default() += 1;
        }
    }
    for (language, n) in languages {
        m.add(app, format!("Profil-Sprache: {language}"), of_total(n), "2");
    }
    for (quality, n) in allowed {
        m.add(
            app,
            format!("Quality: {quality}"),
            format!("erlaubt in {}", of_total(n)),
            "2",
        );
    }
}

fn custom_formats(m: &mut Matrix, export: &AppExport) {
    let formats = export.list("customformat");
    if formats.is_empty() {
        return;
    }
    m.add(export.app, "Custom Formats", formats.len().to_string(), "2");

    #[derive(Default)]
    struct Usage {
        total: usize,
        negated: usize,
        required: usize,
    }
    let mut by_type: BTreeMap<String, Usage> = BTreeMap::new();
    for spec in formats
        .iter()
        .filter_map(|f| f["specifications"].as_array())
        .flatten()
    {
        let Some(implementation) = spec["implementation"].as_str() else {
            continue;
        };
        let usage = by_type.entry(implementation.to_owned()).or_default();
        usage.total += 1;
        usage.negated += usize::from(spec["negate"].as_bool() == Some(true));
        usage.required += usize::from(spec["required"].as_bool() == Some(true));
    }
    for (implementation, usage) in by_type {
        m.add(
            export.app,
            format!("CF-Bedingung: {implementation}"),
            format!(
                "{}× (negiert {}, Pflicht {})",
                usage.total, usage.negated, usage.required
            ),
            "2",
        );
    }
}

/// Counts the `implementation`s of a provider list (indexers, clients, …).
fn implementations(m: &mut Matrix, export: &AppExport, stem: &str, label: &str, phase: &str) {
    let mut by_type: BTreeMap<String, (usize, usize, Option<String>)> = BTreeMap::new();
    for item in export.list(stem) {
        let Some(implementation) = item["implementation"].as_str() else {
            continue;
        };
        let entry = by_type.entry(implementation.to_owned()).or_default();
        entry.0 += 1;
        let enabled =
            item["enable"].as_bool().unwrap_or(true) && item["enableRss"].as_bool().unwrap_or(true);
        entry.1 += usize::from(enabled);
        if let Some(protocol) = item["protocol"].as_str() {
            entry.2 = Some(protocol.to_owned());
        }
    }
    for (implementation, (n, enabled, protocol)) in by_type {
        let protocol = protocol.map(|p| format!(", {p}")).unwrap_or_default();
        m.add(
            export.app,
            format!("{label}: {implementation}"),
            format!("{n} ({enabled} aktiv{protocol})"),
            phase,
        );
    }
}

fn delay_profiles(m: &mut Matrix, export: &AppExport) {
    let profiles = export.list("delayprofile");
    if profiles.is_empty() {
        return;
    }
    m.add(
        export.app,
        "Delay Profiles",
        profiles.len().to_string(),
        "3",
    );
    let max_delay = profiles
        .iter()
        .filter_map(|p| p["usenetDelay"].as_u64())
        .max()
        .unwrap_or(0);
    if max_delay > 0 {
        m.add(
            export.app,
            "Delay Profile: Usenet-Verzögerung",
            format!("bis {max_delay} min"),
            "3",
        );
    }
    let bypass = profiles
        .iter()
        .filter(|p| p["bypassIfHighestQuality"].as_bool() == Some(true))
        .count();
    if bypass > 0 {
        m.add(
            export.app,
            "Delay Profile: Bypass bei höchster Quality",
            format!("{bypass} von {}", profiles.len()),
            "3",
        );
    }
}

fn library(m: &mut Matrix, export: &AppExport, stem: &str, feature: &str) {
    let items = export.list(stem);
    if items.is_empty() {
        return;
    }
    let monitored = items
        .iter()
        .filter(|i| i["monitored"].as_bool() == Some(true))
        .count();
    m.add(
        export.app,
        feature,
        format!("{} ({monitored} überwacht)", items.len()),
        "3",
    );
}

/// Scalar values of whitelisted keys of a settings object.
fn settings(m: &mut Matrix, export: &AppExport, stem: &str, label: &str, keys: &[(&str, &str)]) {
    let Some(config) = export.get(stem) else {
        return;
    };
    for (key, phase) in keys {
        if let Some(value) = scalar(&config[*key]) {
            m.add(
                export.app,
                format!("{label}: {key}"),
                format!("= {value}"),
                phase,
            );
        }
    }
}

fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

fn naming(m: &mut Matrix, export: &AppExport) {
    let Some(config) = export.get("config_naming").and_then(Value::as_object) else {
        return;
    };
    for key in NAMING_SETTING_KEYS {
        if let Some(value) = config.get(*key).and_then(scalar) {
            m.add(
                export.app,
                format!("Benennung: {key}"),
                format!("= {value}"),
                "4",
            );
        }
    }
    let mut tokens: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for (key, value) in config {
        let Some(format) = value.as_str().filter(|_| key.ends_with("Format")) else {
            continue;
        };
        for token in naming_tokens(format) {
            tokens.entry(token).or_default().push(key);
        }
    }
    for (token, keys) in tokens {
        let mut keys = keys;
        keys.dedup();
        m.add(
            export.app,
            format!("Naming-Token `{token}`"),
            keys.join(", "),
            "4",
        );
    }
}

/// `{Movie Title} ({Release Year})` → `{Movie Title}`, `{Release Year}`.
pub fn naming_tokens(format: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut rest = format;
    while let Some(open) = rest.find('{') {
        let Some(len) = rest[open..].find('}') else {
            break;
        };
        tokens.push(rest[open..open + len + 1].to_owned());
        rest = &rest[open + len + 1..];
    }
    tokens
}

fn notifications(m: &mut Matrix, export: &AppExport) {
    let mut by_type: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    for item in export.list("notification") {
        let Some(implementation) = item["implementation"].as_str() else {
            continue;
        };
        let entry = by_type.entry(implementation.to_owned()).or_default();
        entry.0 += 1;
        if let Some(object) = item.as_object() {
            for (key, value) in object {
                let trigger = key.starts_with("on")
                    && key.chars().nth(2).is_some_and(char::is_uppercase)
                    && value.as_bool() == Some(true);
                if trigger && !entry.1.contains(key) {
                    entry.1.push(key.clone());
                }
            }
        }
    }
    for (implementation, (n, mut triggers)) in by_type {
        triggers.sort();
        let triggers = if triggers.is_empty() {
            String::new()
        } else {
            format!(", Auslöser: {}", triggers.join(", "))
        };
        m.add(
            export.app,
            format!("Notification: {implementation}"),
            format!("{n}{triggers}"),
            "4",
        );
    }
}

fn versions(apps: &[AppExport]) -> String {
    apps.iter()
        .filter_map(|a| {
            let version = a.get("system_status")?["version"].as_str()?;
            Some(format!("{} {version}", a.app))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn escape(cell: &str) -> String {
    cell.replace('|', "\\|").replace('\n', " ")
}

/// `|---|---|` or `| :--- | --- |`, however a Markdown formatter rewrote it.
fn is_separator_row(line: &str) -> bool {
    line.contains('-') && line.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// Status per feature from a previously generated matrix.
fn existing_statuses(doc: &str) -> BTreeMap<String, String> {
    let generated = doc.split(MANUAL_MARKER).next().unwrap_or_default();
    generated
        .lines()
        .filter(|l| l.starts_with('|') && !is_separator_row(l) && !l.starts_with("| Feature |"))
        .filter_map(|line| {
            let cells: Vec<String> = line
                .replace("\\|", "\u{0}")
                .trim_matches('|')
                .split('|')
                .map(|c| c.replace('\u{0}', "|").trim().to_owned())
                .collect();
            match cells.as_slice() {
                [feature, _, _, status] => Some((feature.clone(), status.clone())),
                _ => None,
            }
        })
        .collect()
}

const DEFAULT_MANUAL: &str = "

## Bewusste Abweichungen

Verhalten, in dem sofarr absichtlich von Radarr/Sonarr abweicht (Spec §5.4).
Diese Fälle werden aus dem Parity-Diff ausgenommen.

| Feature | Abweichung | Begründung |
|---|---|---|
";

pub fn render(apps: &[AppExport], rows: &[Row], existing: Option<&str>) -> String {
    let statuses = existing.map(existing_statuses).unwrap_or_default();
    let manual = existing
        .and_then(|doc| doc.split_once(MANUAL_MARKER))
        .map_or(DEFAULT_MANUAL, |(_, manual)| manual);

    let mut out = String::new();
    let _ = writeln!(out, "# Parity-Matrix\n");
    let _ = writeln!(
        out,
        "> Erzeugt mit `cargo run -p sofarr-parity -- matrix` aus dem Config-Export\n\
         > (`scripts/export-arr-config.sh`). Die Tabelle nicht von Hand ändern – beim\n\
         > Neuerzeugen bleibt nur die Spalte **Status** erhalten. Enthält bewusst keine\n\
         > Namen, URLs, Pfade oder Keys.\n"
    );
    let versions = versions(apps);
    if !versions.is_empty() {
        let _ = writeln!(out, "Orakel: {versions}\n");
    }
    let _ = writeln!(out, "| Feature | genutzt in | Phase | Status |");
    let _ = writeln!(out, "|---|---|---|---|");
    for row in rows {
        let status = statuses
            .get(&escape(&row.feature))
            .map_or(DEFAULT_STATUS, String::as_str);
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            escape(&row.feature),
            escape(&row.uses.join("; ")),
            escape(&row.phase),
            escape(status)
        );
    }
    let _ = write!(out, "\n{MANUAL_MARKER}{manual}");
    out
}

/// Reads the export, writes the matrix and returns the number of feature rows.
pub fn write(exports: &Path, out: &Path) -> anyhow::Result<usize> {
    let apps = load_exports(exports)?;
    anyhow::ensure!(
        !apps.is_empty(),
        "keine Exporte unter {} – zuerst scripts/export-arr-config.sh ausführen",
        exports.display()
    );
    let rows = rows(&apps);
    let existing = fs::read_to_string(out).ok();
    let doc = render(&apps, &rows, existing.as_deref());
    fs::write(out, doc).with_context(|| format!("{} nicht schreibbar", out.display()))?;
    Ok(rows.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SECRETS: &[&str] = &[
        "SECRET-API-KEY",
        "geheimes-passwort",
        "indexer.private.example",
        "/data/media/movies",
        "Mein Privater Indexer",
        "nzbget.lan",
        "Mein Lieblingsfilm",
    ];

    /// Invented export with secrets in all the places a real one has them.
    fn fake_radarr() -> AppExport {
        let files = [
            (
                "system_status",
                json!({"appName": "Radarr", "version": "1.2.3"}),
            ),
            (
                "qualityprofile",
                json!([{
                    "name": "HD",
                    "upgradeAllowed": true,
                    "cutoff": 7,
                    "minFormatScore": 10,
                    "cutoffFormatScore": 0,
                    "language": {"id": 1, "name": "German"},
                    "formatItems": [{"format": 1, "name": "X", "score": 50}],
                    "items": [
                        {"quality": {"id": 7, "name": "Bluray-1080p"}, "items": [], "allowed": true},
                        {"name": "WEB 1080p", "allowed": true, "items": [
                            {"quality": {"id": 3, "name": "WEBDL-1080p"}, "items": [], "allowed": true},
                            {"quality": {"id": 15, "name": "WEBRip-1080p"}, "items": [], "allowed": true}
                        ]},
                        {"quality": {"id": 1, "name": "SDTV"}, "items": [], "allowed": false}
                    ]
                }]),
            ),
            (
                "customformat",
                json!([{"name": "Mein Format", "specifications": [
                    {"implementation": "ReleaseTitleSpecification", "negate": false, "required": true,
                     "fields": [{"name": "value", "value": "SECRET-API-KEY"}]},
                    {"implementation": "SourceSpecification", "negate": true, "required": false, "fields": []}
                ]}]),
            ),
            (
                "indexer",
                json!([{"name": "Mein Privater Indexer", "implementation": "Newznab", "protocol": "usenet",
                        "enableRss": true, "fields": [
                            {"name": "baseUrl", "value": "https://indexer.private.example"},
                            {"name": "apiKey", "value": "SECRET-API-KEY"}]}]),
            ),
            (
                "downloadclient",
                json!([{"name": "nzbget", "implementation": "Nzbget", "protocol": "usenet", "enable": true,
                        "fields": [{"name": "host", "value": "nzbget.lan"},
                                   {"name": "password", "value": "geheimes-passwort"}]}]),
            ),
            (
                "notification",
                json!([{"name": "Jellyfin", "implementation": "MediaBrowser", "onDownload": true,
                        "onGrab": false, "onUpgrade": true,
                        "fields": [{"name": "apiKey", "value": "SECRET-API-KEY"}]}]),
            ),
            (
                "rootfolder",
                json!([{"path": "/data/media/movies", "freeSpace": 1}]),
            ),
            ("tag", json!([{"id": 1, "label": "kids"}])),
            (
                "delayprofile",
                json!([{"usenetDelay": 30, "bypassIfHighestQuality": true}]),
            ),
            (
                "config_naming",
                json!({"renameMovies": true, "colonReplacementFormat": "delete",
                       "standardMovieFormat": "{Movie Title} ({Release Year}) {Quality Full}",
                       "movieFolderFormat": "{Movie Title} ({Release Year})"}),
            ),
            (
                "config_mediamanagement",
                json!({"copyUsingHardlinks": true, "recycleBin": "/data/media/movies/.trash",
                       "downloadPropersAndRepacks": "preferAndUpgrade"}),
            ),
            (
                "movie",
                json!([{"title": "Mein Lieblingsfilm", "monitored": true,
                        "path": "/data/media/movies/Mein Lieblingsfilm (2001)"}]),
            ),
        ];
        AppExport {
            app: "radarr",
            files: files.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
        }
    }

    fn rendered() -> String {
        let apps = vec![fake_radarr()];
        render(&apps, &rows(&apps), None)
    }

    #[test]
    fn output_contains_no_secrets() {
        let doc = rendered();
        for (i, secret) in SECRETS.iter().enumerate() {
            // Name the entry by index only: the message must not echo the value.
            assert!(!doc.contains(secret), "SECRETS[{i}] leaked into the matrix");
        }
    }

    #[test]
    fn output_lists_used_features() {
        let doc = rendered();
        for expected in [
            "| Quality Profiles (Cutoff) | radarr: 1 Profile | 2 | offen |",
            "Quality Profile: Upgrades erlaubt",
            "Quality Profile: Quality-Gruppen",
            "Quality Profile: Custom-Format-Scores",
            "Quality Profile: Mindest-/Cutoff-Score",
            "| Quality: WEBRip-1080p |",
            "Profil-Sprache: German",
            "| CF-Bedingung: ReleaseTitleSpecification | radarr: 1× (negiert 0, Pflicht 1) | 2 |",
            "| Indexer: Newznab | radarr: 1 (1 aktiv, usenet) | 3 |",
            "| Download Client: Nzbget |",
            "Auslöser: onDownload, onUpgrade",
            "| Naming-Token `{Release Year}` | radarr: movieFolderFormat, standardMovieFormat | 4 |",
            "Benennung: colonReplacementFormat",
            "Media Management: copyUsingHardlinks",
            "| Root Folders | radarr: 1 | 3 |",
            "| Bibliothek: Filme | radarr: 1 (1 überwacht) | 3 |",
            "Delay Profile: Usenet-Verzögerung",
            "Orakel: radarr 1.2.3",
        ] {
            assert!(doc.contains(expected), "missing {expected}:\n{doc}");
        }
        assert!(!doc.contains("Quality: SDTV"));
    }

    #[test]
    fn sonarr_only_features_get_series_phase() {
        let mut sonarr = fake_radarr();
        sonarr.app = "sonarr";
        sonarr.files.retain(|k, _| k == "tag");
        let mut radarr = fake_radarr();
        radarr.files.retain(|k, _| k == "rootfolder");
        let apps = vec![radarr, sonarr];
        let rows = rows(&apps);
        let tags = rows.iter().find(|r| r.feature == "Tags").expect("tags row");
        assert_eq!(tags.phase, SERIES_PHASE);
        let roots = rows
            .iter()
            .find(|r| r.feature == "Root Folders")
            .expect("row");
        assert_eq!(roots.phase, "3");
    }

    #[test]
    fn regenerating_keeps_status_and_manual_part() {
        let apps = vec![fake_radarr()];
        let rows = rows(&apps);
        let first = render(&apps, &rows, None).replace(
            "| Tags | radarr: 1 | 3 | offen |",
            "| Tags | radarr: 1 | 3 | erledigt |",
        ) + "| Titel | weicht ab | Absicht |\n";
        let second = render(&apps, &rows, Some(&first));
        assert!(second.contains("| Tags | radarr: 1 | 3 | erledigt |"));
        assert!(second.contains("| Titel | weicht ab | Absicht |"));
        assert_eq!(second.matches(MANUAL_MARKER).count(), 1);
        let third = render(&apps, &rows, Some(&second));
        // A Markdown formatter may rewrite the separator row.
        let reformatted = second.replace("|---|---|---|---|", "| --- | --- | --- | --- |");
        let statuses = existing_statuses(&reformatted);
        assert!(!statuses.contains_key("---"));
        assert_eq!(statuses["Tags"], "erledigt");
        assert_eq!(second, third, "regenerating must be stable");
    }

    #[test]
    fn extracts_naming_tokens() {
        assert_eq!(
            naming_tokens("{Movie Title} - {[Quality Full]}{-Release Group} {unclosed"),
            ["{Movie Title}", "{[Quality Full]}", "{-Release Group}"]
        );
    }
}
