//! `sofarr-parity export-public`: copies the parts of the config export the decision
//! engine needs into `fixtures/public/config/`, so CI can test against the real setup.
//!
//! Like `matrix`, this works on a whitelist: every object is rebuilt from the listed
//! keys only. Indexers, download clients, notifications, paths, URLs and keys are never
//! read, and unknown keys in the export never reach the output.

use std::{fs, path::Path};

use anyhow::Context;
use serde_json::{Map, Value};

use crate::matrix::{self, AppExport};

const QUALITY_KEYS: &[&str] = &["id", "name", "source", "resolution", "modifier"];
const DEFINITION_KEYS: &[&str] = &[
    "id",
    "title",
    "weight",
    "minSize",
    "maxSize",
    "preferredSize",
];
const PROFILE_KEYS: &[&str] = &[
    "id",
    "name",
    "upgradeAllowed",
    "cutoff",
    "minFormatScore",
    "cutoffFormatScore",
    "minUpgradeFormatScore",
];
const PROFILE_ITEM_KEYS: &[&str] = &["id", "name", "allowed"];
const FORMAT_ITEM_KEYS: &[&str] = &["format", "name", "score"];
const LANGUAGE_KEYS: &[&str] = &["id", "name"];
const CUSTOM_FORMAT_KEYS: &[&str] = &["id", "name", "includeCustomFormatWhenRenaming"];
const SPECIFICATION_KEYS: &[&str] = &["name", "implementation", "negate", "required"];
/// Condition parameters; custom formats never hold credentials, but stay strict anyway.
const SPECIFICATION_FIELDS: &[&str] = &["value", "exceptLanguage", "min", "max"];
const INDEXER_SETTING_KEYS: &[&str] = &[
    "minimumAge",
    "maximumSize",
    "retention",
    "preferIndexerFlags",
    "availabilityDelay",
    "allowHardcodedSubs",
];
const MEDIA_MANAGEMENT_SETTING_KEYS: &[&str] = &["downloadPropersAndRepacks"];

/// One output file: name and content.
pub struct PublicFile {
    pub name: &'static str,
    pub content: Value,
}

/// Builds the public files for Radarr. Files whose source is missing are skipped.
pub fn public_files(export: &AppExport) -> Vec<PublicFile> {
    let mut files = Vec::new();
    if let Some(list) = export.files.get("qualitydefinition") {
        files.push(PublicFile {
            name: "qualitydefinition.json",
            content: map_list(list, definition),
        });
    }
    if let Some(list) = export.files.get("qualityprofile") {
        files.push(PublicFile {
            name: "qualityprofile.json",
            content: map_list(list, profile),
        });
    }
    if let Some(list) = export.files.get("customformat") {
        files.push(PublicFile {
            name: "customformat.json",
            content: map_list(list, custom_format),
        });
    }
    let mut settings = Map::new();
    if let Some(indexer) = export.files.get("config_indexer") {
        settings.insert("indexer".into(), pick(indexer, INDEXER_SETTING_KEYS));
    }
    if let Some(mm) = export.files.get("config_mediamanagement") {
        settings.insert(
            "mediaManagement".into(),
            pick(mm, MEDIA_MANAGEMENT_SETTING_KEYS),
        );
    }
    if !settings.is_empty() {
        files.push(PublicFile {
            name: "settings.json",
            content: Value::Object(settings),
        });
    }
    files
}

/// Reads the export and writes the public files; returns the written file names.
pub fn write(exports: &Path, out: &Path) -> anyhow::Result<Vec<&'static str>> {
    let apps = matrix::load_exports(exports)?;
    let radarr = apps.iter().find(|a| a.app == "radarr").with_context(|| {
        format!(
            "kein Radarr-Export unter {} – zuerst scripts/export-arr-config.sh ausführen",
            exports.display()
        )
    })?;
    let files = public_files(radarr);
    anyhow::ensure!(
        !files.is_empty(),
        "Radarr-Export enthält weder Quality Definitions noch Profile oder Custom Formats"
    );
    fs::create_dir_all(out).with_context(|| format!("{} nicht anlegbar", out.display()))?;
    let mut names = Vec::new();
    for file in files {
        let path = out.join(file.name);
        let mut json = serde_json::to_string_pretty(&file.content)?;
        json.push('\n');
        fs::write(&path, json).with_context(|| format!("{} nicht schreibbar", path.display()))?;
        names.push(file.name);
    }
    Ok(names)
}

fn map_list(list: &Value, f: fn(&Value) -> Value) -> Value {
    Value::Array(
        list.as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .map(f)
            .collect(),
    )
}

/// Copies only `keys` from `value` (an object); everything else is dropped.
fn pick(value: &Value, keys: &[&str]) -> Value {
    let mut out = Map::new();
    for key in keys {
        if let Some(v) = value.get(*key) {
            out.insert((*key).to_owned(), v.clone());
        }
    }
    Value::Object(out)
}

fn insert(target: &mut Value, key: &str, value: Value) {
    if let Value::Object(map) = target {
        map.insert(key.to_owned(), value);
    }
}

fn definition(value: &Value) -> Value {
    let mut out = pick(value, DEFINITION_KEYS);
    if let Some(quality) = value.get("quality") {
        insert(&mut out, "quality", pick(quality, QUALITY_KEYS));
    }
    out
}

fn profile(value: &Value) -> Value {
    let mut out = pick(value, PROFILE_KEYS);
    if let Some(items) = value.get("items") {
        insert(&mut out, "items", map_list(items, profile_item));
    }
    if let Some(items) = value.get("formatItems") {
        insert(
            &mut out,
            "formatItems",
            map_list(items, |i| pick(i, FORMAT_ITEM_KEYS)),
        );
    }
    if let Some(language) = value.get("language") {
        insert(&mut out, "language", pick(language, LANGUAGE_KEYS));
    }
    out
}

fn profile_item(value: &Value) -> Value {
    let mut out = pick(value, PROFILE_ITEM_KEYS);
    if let Some(quality) = value.get("quality") {
        insert(&mut out, "quality", pick(quality, QUALITY_KEYS));
    }
    if let Some(items) = value.get("items") {
        insert(&mut out, "items", map_list(items, profile_item));
    }
    out
}

fn custom_format(value: &Value) -> Value {
    let mut out = pick(value, CUSTOM_FORMAT_KEYS);
    if let Some(specs) = value.get("specifications") {
        insert(&mut out, "specifications", map_list(specs, specification));
    }
    out
}

fn specification(value: &Value) -> Value {
    let mut out = pick(value, SPECIFICATION_KEYS);
    let fields: Vec<Value> = value
        .get("fields")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter(|f| {
            f.get("name")
                .and_then(Value::as_str)
                .is_some_and(|n| SPECIFICATION_FIELDS.contains(&n))
        })
        .map(|f| pick(f, &["name", "value"]))
        .collect();
    insert(&mut out, "fields", Value::Array(fields));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SECRETS: &[&str] = &[
        "SECRET-API-KEY",
        "indexer.private.example",
        "/data/media/movies",
        "Mein Privater Indexer",
    ];

    /// Invented export with secrets in places a real one has them, plus extra keys.
    fn fake_radarr() -> AppExport {
        let files = [
            (
                "qualitydefinition",
                json!([{"id": 1, "title": "Bluray-1080p", "weight": 20, "minSize": 5.0,
                        "maxSize": 400.0, "preferredSize": 300.0, "secretNote": "SECRET-API-KEY",
                        "quality": {"id": 7, "name": "Bluray-1080p", "source": "bluray",
                                    "resolution": 1080, "modifier": "none"}}]),
            ),
            (
                "qualityprofile",
                json!([{"id": 4, "name": "HD", "upgradeAllowed": true, "cutoff": 7,
                "minFormatScore": 10, "cutoffFormatScore": 0,
                "language": {"id": 4, "name": "German"},
                "formatItems": [{"format": 1, "name": "X", "score": 50}],
                "items": [
                    {"quality": {"id": 7, "name": "Bluray-1080p", "source": "bluray",
                                 "resolution": 1080, "modifier": "none"},
                     "items": [], "allowed": true},
                    {"id": 1000, "name": "WEB 1080p", "allowed": true, "items": [
                        {"quality": {"id": 3, "name": "WEBDL-1080p"}, "items": [], "allowed": true}
                    ]}
                ]}]),
            ),
            (
                "customformat",
                json!([{"id": 1, "name": "X", "includeCustomFormatWhenRenaming": false,
                        "specifications": [
                    {"name": "x265", "implementation": "ReleaseTitleSpecification",
                     "implementationName": "Release Title", "infoLink": "https://indexer.private.example",
                     "negate": false, "required": true,
                     "fields": [{"name": "value", "value": "x265", "label": "Regular Expression",
                                 "helpText": "SECRET-API-KEY"},
                                {"name": "apiKey", "value": "SECRET-API-KEY"}]}
                ]}]),
            ),
            (
                "indexer",
                json!([{"name": "Mein Privater Indexer", "fields": [
                    {"name": "baseUrl", "value": "https://indexer.private.example"}]}]),
            ),
            ("rootfolder", json!([{"path": "/data/media/movies"}])),
            (
                "config_indexer",
                json!({"minimumAge": 0, "maximumSize": 0, "retention": 0,
                       "allowHardcodedSubs": false, "whitelistedHardcodedSubs": "SECRET-API-KEY"}),
            ),
            (
                "config_mediamanagement",
                json!({"downloadPropersAndRepacks": "preferAndUpgrade",
                       "recycleBin": "/data/media/movies/.trash"}),
            ),
        ];
        AppExport {
            app: "radarr",
            files: files.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
        }
    }

    fn rendered() -> String {
        public_files(&fake_radarr())
            .iter()
            .map(|f| f.content.to_string())
            .collect()
    }

    #[test]
    fn output_contains_no_secrets() {
        let out = rendered();
        for (i, secret) in SECRETS.iter().enumerate() {
            // Name the entry by index only: the message must not echo the value.
            assert!(!out.contains(secret), "SECRETS[{i}] leaked into the export");
        }
    }

    #[test]
    fn keeps_what_the_decision_engine_needs() {
        let files = public_files(&fake_radarr());
        let names: Vec<_> = files.iter().map(|f| f.name).collect();
        assert_eq!(
            names,
            [
                "qualitydefinition.json",
                "qualityprofile.json",
                "customformat.json",
                "settings.json"
            ]
        );
        let get = |name| &files.iter().find(|f| f.name == name).unwrap().content;

        let definition = &get("qualitydefinition.json")[0];
        assert_eq!(definition["maxSize"], 400.0);
        assert_eq!(definition["quality"]["source"], "bluray");

        let profile = &get("qualityprofile.json")[0];
        assert_eq!(profile["cutoff"], 7);
        assert_eq!(profile["language"]["name"], "German");
        assert_eq!(profile["formatItems"][0]["score"], 50);
        assert_eq!(profile["items"][1]["name"], "WEB 1080p");
        assert_eq!(profile["items"][1]["items"][0]["quality"]["id"], 3);

        let spec = &get("customformat.json")[0]["specifications"][0];
        assert_eq!(spec["implementation"], "ReleaseTitleSpecification");
        assert_eq!(spec["required"], true);
        assert_eq!(spec["fields"], json!([{"name": "value", "value": "x265"}]));

        let settings = get("settings.json");
        assert_eq!(
            settings["mediaManagement"]["downloadPropersAndRepacks"],
            "preferAndUpgrade"
        );
        assert_eq!(settings["indexer"]["maximumSize"], 0);
        assert!(
            settings["indexer"]
                .get("whitelistedHardcodedSubs")
                .is_none()
        );
    }
}
