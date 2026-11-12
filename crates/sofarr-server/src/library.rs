//! The movie library: rules from the database, import from Radarr, wanted lists.

use std::{collections::BTreeMap, path::Path};

use anyhow::Context;
use jiff::Timestamp;
use serde::Serialize;
use serde_json::Value;
use sofarr_core::{
    ExistingFile, MovieContext, PropersPolicy, QualityCatalog, QualityId, QualityProfile,
    RadarrConfig, ReleaseFacts, language::LanguageRef, profile::ProfileId, quality::UNKNOWN,
};
use sofarr_db::{Availability, Db, Grab, Movie, MovieFile, NewMovie};
use sofarr_parser::{Language, Revision};

use crate::radarr::Radarr;

/// Configuration documents kept in the `settings` table, named like the export files.
pub const SETTING_KEYS: &[&str] = &[
    "qualityprofile",
    "qualitydefinition",
    "customformat",
    "config_mediamanagement",
    "config_indexer",
    "config_naming",
    "delayprofile",
    "rootfolder",
    "config_downloadclient",
    "importlist",
    "exclusions",
    "tag",
];

/// API path of each setting key.
fn api_path(key: &str) -> String {
    key.replacen("config_", "config/", 1)
}

/// The decision rules (the engine of `sofarr-core`) built from stored settings.
#[derive(Debug, Clone)]
pub struct Rules {
    pub config: RadarrConfig,
    pub catalog: QualityCatalog,
}

impl Rules {
    /// Builds the rules from the API documents (`qualityprofile`, `qualitydefinition`,
    /// `customformat`, `config/mediamanagement`, `config/indexer`).
    pub fn from_documents(
        profiles: &Value,
        definitions: &Value,
        formats: &Value,
        media_management: Option<&Value>,
        indexer: Option<&Value>,
    ) -> anyhow::Result<Self> {
        let settings = serde_json::json!({
            "indexer": indexer.cloned().unwrap_or_else(|| serde_json::json!({})),
            "mediaManagement": media_management.cloned().unwrap_or_else(|| serde_json::json!({})),
        });
        let config = sofarr_core::import::radarr(
            &definitions.to_string(),
            &profiles.to_string(),
            &formats.to_string(),
            Some(&settings.to_string()),
        )?;
        let catalog = config.definitions.catalog();
        Ok(Self { config, catalog })
    }

    pub fn profile(&self, id: i64) -> Option<&QualityProfile> {
        u32::try_from(id)
            .ok()
            .and_then(|id| self.config.profile(ProfileId(id)))
    }

    pub fn quality_id(&self, name: &str) -> QualityId {
        self.catalog.by_name(name).map_or(UNKNOWN, |q| q.id)
    }

    pub fn propers(&self) -> PropersPolicy {
        self.config.settings.propers
    }

    /// Custom format score of a release name under a profile.
    pub fn score(&self, profile_id: i64, name: &str, original: Option<Language>) -> i32 {
        let Some(profile) = self.profile(profile_id) else {
            return 0;
        };
        let parsed = sofarr_parser::parse(name);
        let facts = ReleaseFacts {
            title: name,
            parsed: &parsed,
            languages: &parsed.languages,
            original_language: original,
        };
        let ids: Vec<_> = self
            .config
            .custom_formats
            .iter()
            .filter(|cf| cf.matches(&facts))
            .map(|cf| cf.id)
            .collect();
        profile.score(&ids)
    }

    pub fn existing(&self, file: &MovieFile) -> ExistingFile {
        ExistingFile {
            quality: self.quality_id(&file.quality),
            revision: Revision {
                version: file.revision_version,
                real: file.revision_real,
                ..Revision::default()
            },
            custom_format_score: file.custom_format_score,
            release_group: file.release_group.clone(),
        }
    }

    /// The pending grab as if it were the file.
    pub fn pending(&self, quality: &str, title: &str, score: i32) -> ExistingFile {
        let parsed = sofarr_parser::parse(title);
        ExistingFile {
            quality: self.quality_id(quality),
            revision: parsed.revision,
            custom_format_score: score,
            release_group: parsed.release_group,
        }
    }
}

/// Loads the movie rules from the database.
pub async fn load_rules(db: &Db) -> anyhow::Result<Rules> {
    load_prefixed_rules(db, "", "import-radarr").await
}

/// Loads rules stored under `prefix` (empty for Radarr, `sonarr_` for series).
pub async fn load_prefixed_rules(db: &Db, prefix: &str, command: &str) -> anyhow::Result<Rules> {
    let get =
        |key: &'static str| async move {
            db.setting(&format!("{prefix}{key}")).await?.with_context(|| {
            format!("Einstellung `{prefix}{key}` fehlt – zuerst `amigo-sofarr {command}` ausführen")
        })
        };
    let profiles = get("qualityprofile").await?;
    let definitions = get("qualitydefinition").await?;
    let formats = get("customformat").await?;
    let media = db
        .setting(&format!("{prefix}config_mediamanagement"))
        .await?;
    let indexer = db.setting(&format!("{prefix}config_indexer")).await?;
    Rules::from_documents(
        &profiles,
        &definitions,
        &formats,
        media.as_ref(),
        indexer.as_ref(),
    )
}

pub fn original_language(movie: &Movie) -> Option<Language> {
    movie
        .original_language
        .as_deref()
        .and_then(LanguageRef::from_name)
        .and_then(|l| match l {
            LanguageRef::Language(l) => Some(l),
            _ => None,
        })
}

/// The decision context of a library movie. A pending grab (shadow or still
/// downloading) stands in for the file: only releases that beat it are upgrades.
pub fn context(movie: &Movie, pending: Option<&Grab>, rules: &Rules) -> MovieContext {
    MovieContext {
        years: movie.year.into_iter().collect(),
        runtime_minutes: movie.runtime_minutes,
        original_language: original_language(movie),
        file: match pending {
            Some(g) => Some(rules.pending(&g.quality, &g.title, g.custom_format_score)),
            None => movie.file.as_ref().map(|f| rules.existing(f)),
        },
    }
}

fn date_part(raw: &str) -> String {
    raw.get(..10).unwrap_or(raw).to_owned()
}

/// Tag ids of an API object (`tags: [1, 2]`).
pub fn tag_ids(value: &Value) -> Vec<i64> {
    value
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(Value::as_i64)
        .collect()
}

/// Labels of tag ids, from a `tag` setting (`[{"id": 1, "label": "kids"}]`).
pub fn tag_labels(tags: Option<&Value>, ids: &[i64]) -> Vec<String> {
    tags.and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter(|t| t["id"].as_i64().is_some_and(|id| ids.contains(&id)))
        .filter_map(|t| t["label"].as_str())
        .map(str::to_lowercase)
        .collect()
}

/// Whether a movie may be grabbed automatically today (`minimumAvailability` plus
/// `availabilityDelay`).
pub fn is_available(movie: &Movie, rules: &Rules, today: jiff::civil::Date) -> bool {
    use sofarr_core::availability::{self, ReleaseInfo, Stage};
    let a = &movie.availability;
    let date = |d: &Option<String>| d.as_deref().and_then(availability::parse_date);
    let info = ReleaseInfo {
        status: a.status.as_deref().and_then(Stage::from_api),
        in_cinemas: date(&a.in_cinemas),
        digital: date(&a.digital_release),
        physical: date(&a.physical_release),
    };
    availability::is_available(
        Stage::from_api(&a.minimum).unwrap_or_default(),
        &info,
        rules.config.settings.availability_delay_days,
        today,
    )
}

/// Reads one movie of Radarr's `/api/v3/movie` answer.
pub fn movie_from_radarr(
    value: &Value,
    rules: Option<&Rules>,
) -> Option<(NewMovie, Option<MovieFile>)> {
    let text = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    let tmdb_id = value["tmdbId"].as_i64().filter(|id| *id > 0)?;
    let profile_id = value["qualityProfileId"].as_i64().unwrap_or(0);
    let original_language = text(&value["originalLanguage"]["name"]);
    let movie = NewMovie {
        tmdb_id,
        imdb_id: text(&value["imdbId"]),
        title: text(&value["title"])?,
        original_title: text(&value["originalTitle"]),
        title_de: None,
        year: value["year"]
            .as_u64()
            .and_then(|y| u16::try_from(y).ok())
            .filter(|y| *y > 0),
        runtime_minutes: value["runtime"]
            .as_u64()
            .and_then(|r| u32::try_from(r).ok())
            .unwrap_or(0),
        original_language: original_language.clone(),
        monitored: value["monitored"].as_bool().unwrap_or(false),
        quality_profile_id: profile_id,
        path: text(&value["path"]).unwrap_or_default(),
        radarr_id: value["id"].as_i64(),
        alternative_titles: value["alternateTitles"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .filter_map(|t| text(&t["title"]))
            .collect(),
        availability: Availability {
            minimum: text(&value["minimumAvailability"]).unwrap_or_else(|| "released".to_owned()),
            status: text(&value["status"]),
            in_cinemas: text(&value["inCinemas"]).map(|d| date_part(&d)),
            digital_release: text(&value["digitalRelease"]).map(|d| date_part(&d)),
            physical_release: text(&value["physicalRelease"]).map(|d| date_part(&d)),
        },
        tags: tag_ids(&value["tags"]),
    };
    let file = value["movieFile"].as_object().map(|_| {
        let f = &value["movieFile"];
        let relative_path = text(&f["relativePath"]).unwrap_or_default();
        let scene_name = text(&f["sceneName"]);
        let size = f["size"].as_u64().unwrap_or(0);
        let score = f["customFormatScore"]
            .as_i64()
            .and_then(|s| i32::try_from(s).ok())
            .unwrap_or_else(|| {
                let name = scene_name.as_deref().unwrap_or(&relative_path);
                let original = original_language
                    .as_deref()
                    .and_then(LanguageRef::from_name)
                    .and_then(|l| match l {
                        LanguageRef::Language(l) => Some(l),
                        _ => None,
                    });
                rules.map_or(0, |r| r.score(profile_id, name, original))
            });
        let revision = &f["quality"]["revision"];
        let small = |v: &Value, default| {
            v.as_u64()
                .and_then(|n| u8::try_from(n).ok())
                .unwrap_or(default)
        };
        MovieFile {
            relative_path,
            size,
            quality: text(&f["quality"]["quality"]["name"]).unwrap_or_else(|| "Unknown".to_owned()),
            revision_version: small(&revision["version"], 1),
            revision_real: small(&revision["real"], 0),
            release_group: text(&f["releaseGroup"]),
            scene_name,
            custom_format_score: score,
        }
    });
    Some((movie, file))
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct ImportSummary {
    pub settings: usize,
    pub movies: usize,
    pub files: usize,
    pub skipped: usize,
}

/// Imports configuration documents and the movie list (`movie`).
pub async fn import_documents(
    db: &Db,
    docs: &BTreeMap<String, Value>,
    now: Timestamp,
) -> anyhow::Result<ImportSummary> {
    let mut summary = ImportSummary::default();
    for key in SETTING_KEYS {
        if let Some(value) = docs.get(*key) {
            db.put_setting(key, value).await?;
            summary.settings += 1;
        }
    }
    let rules = load_rules(db).await.ok();
    for value in docs
        .get("movie")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
    {
        let Some((movie, file)) = movie_from_radarr(value, rules.as_ref()) else {
            summary.skipped += 1;
            continue;
        };
        let id = db.upsert_movie(&movie, now).await?;
        db.set_movie_presentation(
            id,
            crate::images::poster_of(value, crate::images::tmdb_path).as_deref(),
            value["overview"].as_str().filter(|o| !o.trim().is_empty()),
        )
        .await?;
        summary.movies += 1;
        match file {
            Some(file) => {
                db.set_file(id, &file, now).await?;
                summary.files += 1;
            }
            None => db.remove_file(id).await?,
        }
    }
    Ok(summary)
}

/// Reads a config export directory (`fixtures/private/config/radarr` layout).
pub fn read_export_dir(dir: &Path) -> anyhow::Result<BTreeMap<String, Value>> {
    read_dir_documents(dir, &["movie"])
}

/// Reads the setting documents plus `extra` (e.g. `series`) from an export directory.
pub fn read_dir_documents(dir: &Path, extra: &[&str]) -> anyhow::Result<BTreeMap<String, Value>> {
    let mut docs = BTreeMap::new();
    for key in SETTING_KEYS.iter().chain(extra) {
        let path = dir.join(format!("{key}.json"));
        match std::fs::read_to_string(&path) {
            Ok(raw) => {
                let value = serde_json::from_str(&raw)
                    .with_context(|| format!("{}: ungültiges JSON", path.display()))?;
                docs.insert((*key).to_owned(), value);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("{} nicht lesbar", path.display())),
        }
    }
    anyhow::ensure!(
        !docs.is_empty(),
        "{} enthält keinen Radarr-Export",
        dir.display()
    );
    Ok(docs)
}

/// Fetches the same documents from a running Radarr.
pub async fn fetch_from_radarr(radarr: &Radarr) -> anyhow::Result<BTreeMap<String, Value>> {
    let mut docs = BTreeMap::new();
    for key in SETTING_KEYS.iter().copied().chain(["movie"]) {
        docs.insert(key.to_owned(), radarr.get(&api_path(key), &[]).await?);
    }
    Ok(docs)
}

/// Movies Radarr would list as missing or cutoff unmet.
#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct Wanted {
    /// TMDB ids of monitored movies without file.
    pub missing: Vec<i64>,
    /// TMDB ids of monitored movies whose file is below the cutoff.
    pub cutoff_unmet: Vec<i64>,
}

pub fn wanted(movies: &[Movie], rules: &Rules) -> Wanted {
    let mut out = Wanted::default();
    for movie in movies.iter().filter(|m| m.monitored) {
        match &movie.file {
            None => out.missing.push(movie.tmdb_id),
            Some(file) => {
                let met = rules.profile(movie.quality_profile_id).is_some_and(|p| {
                    p.cutoff_met(rules.quality_id(&file.quality), file.custom_format_score)
                });
                if !met {
                    out.cutoff_unmet.push(movie.tmdb_id);
                }
            }
        }
    }
    out.missing.sort_unstable();
    out.cutoff_unmet.sort_unstable();
    out
}

#[cfg(test)]
pub mod tests {
    use serde_json::json;

    use super::*;

    pub fn documents() -> BTreeMap<String, Value> {
        let mut docs = BTreeMap::new();
        docs.insert(
            "qualityprofile".to_owned(),
            json!([{"id": 1, "name": "HD", "upgradeAllowed": true, "cutoff": 7,
            "cutoffFormatScore": 0, "language": {"id": -1, "name": "Any"},
            "formatItems": [{"format": 1, "score": 10}],
            "items": [
                {"quality": {"id": 3, "name": "WEBDL-1080p"}, "allowed": true},
                {"quality": {"id": 7, "name": "Bluray-1080p"}, "allowed": true}
            ]}]),
        );
        docs.insert(
            "qualitydefinition".to_owned(),
            serde_json::from_str(include_str!(
                "../../../fixtures/public/config/radarr/qualitydefinition.json"
            ))
            .expect("public definitions"),
        );
        docs.insert(
            "customformat".to_owned(),
            json!([{"id": 1, "name": "German", "specifications": [
                {"implementation": "ReleaseTitleSpecification", "fields": [{"name": "value", "value": "\\bGerman\\b"}]}]}]),
        );
        docs.insert(
            "movie".to_owned(),
            json!([
                {"id": 1, "tmdbId": 101, "title": "Erster Film", "year": 2001, "monitored": true,
                 "qualityProfileId": 1, "path": "/data/media/movies/Erster Film (2001)", "runtime": 90,
                 "originalLanguage": {"id": 1, "name": "English"},
                 "alternateTitles": [{"title": "First Film"}]},
                {"id": 2, "tmdbId": 102, "title": "Zweiter Film", "year": 2002, "monitored": true,
                 "qualityProfileId": 1, "path": "/data/media/movies/Zweiter Film (2002)",
                 "movieFile": {"relativePath": "Zweiter Film (2002).mkv", "size": 1000,
                    "sceneName": "Zweiter.Film.2002.German.1080p.WEB-DL.x264-GRP",
                    "quality": {"quality": {"name": "WEBDL-1080p"}, "revision": {"version": 1, "real": 0}}}},
                {"id": 3, "tmdbId": 103, "title": "Dritter Film", "year": 2003, "monitored": true,
                 "qualityProfileId": 1, "path": "/data/media/movies/Dritter Film (2003)",
                 "movieFile": {"relativePath": "Dritter Film (2003).mkv", "size": 1000, "customFormatScore": 0,
                    "quality": {"quality": {"name": "Bluray-1080p"}, "revision": {"version": 1, "real": 0}}}},
                {"id": 4, "tmdbId": 104, "title": "Vierter Film", "monitored": false, "qualityProfileId": 1,
                 "path": "/x"},
                {"id": 5, "tmdbId": 0, "title": "ohne TMDB"}
            ]),
        );
        docs
    }

    #[tokio::test]
    async fn imports_library_and_computes_wanted() {
        let db = Db::in_memory().await.expect("db");
        let now: Timestamp = "2026-11-01T00:00:00Z".parse().expect("ts");
        let summary = import_documents(&db, &documents(), now)
            .await
            .expect("import");
        assert_eq!(
            summary,
            ImportSummary {
                settings: 3,
                movies: 4,
                files: 2,
                skipped: 1
            }
        );
        let movies = db.movies().await.expect("movies");
        let second = movies.iter().find(|m| m.tmdb_id == 102).expect("second");
        // Score computed from the scene name: the German format matches.
        assert_eq!(
            second.file.as_ref().map(|f| f.custom_format_score),
            Some(10)
        );
        let rules = load_rules(&db).await.expect("rules");
        assert_eq!(
            wanted(&movies, &rules),
            Wanted {
                missing: vec![101],
                cutoff_unmet: vec![102]
            }
        );
        let first = movies.iter().find(|m| m.tmdb_id == 101).expect("first");
        assert_eq!(first.alternative_titles, ["First Film"]);
        let c = context(first, None, &rules);
        assert_eq!(c.years, [2001]);
        assert_eq!(c.original_language, Some(Language::English));
        assert_eq!(c.file, None);
    }
}
