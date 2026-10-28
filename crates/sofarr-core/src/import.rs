//! Import of a Radarr v3 config: quality definitions, quality profiles and custom
//! formats as returned by the API, plus the settings file of `sofarr-parity
//! export-public`. Pure: takes JSON text, returns the domain model.
//!
//! Field names follow the public API docs and the exported JSON itself.

use serde::Deserialize;
use sofarr_parser::Quality as Parts;

use crate::{
    custom_format::{Condition, CustomFormat, CustomFormatId, Pattern, Specification},
    definition::{QualityDefinition, QualityDefinitions},
    language::LanguageRef,
    profile::{ProfileId, ProfileItem, QualityProfile},
    quality::{NamedQuality, QualityId, parts_from_api},
    settings::{PropersPolicy, Settings},
};

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("{file}: ungültiges JSON: {source}")]
    Json {
        file: &'static str,
        source: serde_json::Error,
    },
    #[error("Quality {id} ({name}): unbekannte Quelle, Auflösung oder Modifier")]
    Quality { id: u32, name: String },
    #[error("Custom Format {format:?}: Bedingungstyp {implementation} wird nicht unterstützt")]
    Implementation {
        format: String,
        implementation: String,
    },
    #[error("Custom Format {format:?}: Bedingung {spec:?} hat keinen gültigen Wert")]
    Value { format: String, spec: String },
    #[error("Custom Format {format:?}: Regex {pattern:?} ungültig: {message}")]
    Regex {
        format: String,
        pattern: String,
        message: String,
    },
    #[error("Sprache {0} ist unbekannt")]
    Language(String),
    #[error("downloadPropersAndRepacks = {0:?} ist unbekannt")]
    Propers(String),
}

/// A complete Radarr config for the decision engine.
#[derive(Debug, Clone)]
pub struct RadarrConfig {
    pub definitions: QualityDefinitions,
    pub profiles: Vec<QualityProfile>,
    pub custom_formats: Vec<CustomFormat>,
    pub settings: Settings,
}

impl RadarrConfig {
    pub fn profile(&self, id: ProfileId) -> Option<&QualityProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }
}

/// Imports the JSON documents; `settings` may be missing (defaults apply).
pub fn radarr(
    definitions: &str,
    profiles: &str,
    custom_formats: &str,
    settings: Option<&str>,
) -> Result<RadarrConfig, ImportError> {
    Ok(RadarrConfig {
        definitions: quality_definitions(definitions)?,
        profiles: quality_profiles(profiles)?,
        custom_formats: self::custom_formats(custom_formats)?,
        settings: settings
            .map(self::settings)
            .transpose()?
            .unwrap_or_default(),
    })
}

fn json<T: for<'de> Deserialize<'de>>(file: &'static str, raw: &str) -> Result<T, ImportError> {
    serde_json::from_str(raw).map_err(|source| ImportError::Json { file, source })
}

#[derive(Deserialize)]
struct QualityDto {
    id: u32,
    name: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    resolution: Option<u32>,
    #[serde(default)]
    modifier: Option<String>,
}

impl QualityDto {
    fn parts(&self) -> Option<Parts> {
        parts_from_api(
            self.source.as_deref()?,
            self.resolution?,
            self.modifier.as_deref()?,
        )
    }

    fn named(&self) -> Result<NamedQuality, ImportError> {
        Ok(NamedQuality {
            id: QualityId(self.id),
            name: self.name.clone(),
            parts: self.parts().ok_or_else(|| ImportError::Quality {
                id: self.id,
                name: self.name.clone(),
            })?,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DefinitionDto {
    quality: QualityDto,
    title: String,
    #[serde(default)]
    min_size: Option<f64>,
    #[serde(default)]
    max_size: Option<f64>,
    #[serde(default)]
    preferred_size: Option<f64>,
}

pub fn quality_definitions(raw: &str) -> Result<QualityDefinitions, ImportError> {
    let dtos: Vec<DefinitionDto> = json("qualitydefinition", raw)?;
    dtos.into_iter()
        .map(|d| {
            Ok(QualityDefinition {
                quality: d.quality.named()?,
                title: d.title,
                min_size: d.min_size.unwrap_or(0.0),
                max_size: d.max_size,
                preferred_size: d.preferred_size,
            })
        })
        .collect::<Result<_, _>>()
        .map(QualityDefinitions)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileDto {
    id: u32,
    name: String,
    upgrade_allowed: bool,
    cutoff: u32,
    items: Vec<ProfileItemDto>,
    #[serde(default)]
    format_items: Vec<FormatItemDto>,
    #[serde(default)]
    min_format_score: i32,
    #[serde(default)]
    cutoff_format_score: i32,
    #[serde(default)]
    min_upgrade_format_score: Option<i32>,
    #[serde(default)]
    language: Option<NamedRefDto>,
}

#[derive(Deserialize)]
struct ProfileItemDto {
    #[serde(default)]
    id: Option<u32>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    quality: Option<QualityDto>,
    #[serde(default)]
    items: Vec<ProfileItemDto>,
    allowed: bool,
}

#[derive(Deserialize)]
struct FormatItemDto {
    format: u32,
    score: i32,
}

#[derive(Deserialize)]
struct NamedRefDto {
    name: String,
}

pub fn quality_profiles(raw: &str) -> Result<Vec<QualityProfile>, ImportError> {
    let dtos: Vec<ProfileDto> = json("qualityprofile", raw)?;
    dtos.into_iter()
        .map(|p| {
            let language = match p.language {
                Some(l) => LanguageRef::from_name(&l.name).ok_or(ImportError::Language(l.name))?,
                None => LanguageRef::Any,
            };
            Ok(QualityProfile {
                id: ProfileId(p.id),
                name: p.name,
                upgrade_allowed: p.upgrade_allowed,
                cutoff: p.cutoff,
                items: p.items.into_iter().filter_map(profile_item).collect(),
                format_scores: p
                    .format_items
                    .into_iter()
                    .map(|f| (CustomFormatId(f.format), f.score))
                    .collect(),
                min_format_score: p.min_format_score,
                cutoff_format_score: p.cutoff_format_score,
                min_upgrade_format_score: p.min_upgrade_format_score,
                language,
            })
        })
        .collect()
}

fn profile_item(dto: ProfileItemDto) -> Option<ProfileItem> {
    match dto.quality {
        Some(q) => Some(ProfileItem::Quality {
            quality: QualityId(q.id),
            allowed: dto.allowed,
        }),
        None => Some(ProfileItem::Group {
            id: dto.id?,
            name: dto.name.unwrap_or_default(),
            allowed: dto.allowed,
            qualities: dto
                .items
                .into_iter()
                .filter_map(|i| i.quality.map(|q| QualityId(q.id)))
                .collect(),
        }),
    }
}

#[derive(Deserialize)]
struct CustomFormatDto {
    id: u32,
    name: String,
    #[serde(default)]
    specifications: Vec<SpecificationDto>,
}

#[derive(Deserialize)]
struct SpecificationDto {
    #[serde(default)]
    name: String,
    implementation: String,
    #[serde(default)]
    negate: bool,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    fields: Vec<FieldDto>,
}

#[derive(Deserialize)]
struct FieldDto {
    name: String,
    #[serde(default)]
    value: serde_json::Value,
}

impl SpecificationDto {
    fn field(&self, name: &str) -> Option<&serde_json::Value> {
        self.fields
            .iter()
            .find(|f| f.name == name)
            .map(|f| &f.value)
    }
}

pub fn custom_formats(raw: &str) -> Result<Vec<CustomFormat>, ImportError> {
    let dtos: Vec<CustomFormatDto> = json("customformat", raw)?;
    dtos.into_iter()
        .map(|cf| {
            let specifications = cf
                .specifications
                .iter()
                .map(|s| specification(&cf.name, s))
                .collect::<Result<_, _>>()?;
            Ok(CustomFormat {
                id: CustomFormatId(cf.id),
                name: cf.name,
                specifications,
            })
        })
        .collect()
}

fn specification(format: &str, spec: &SpecificationDto) -> Result<Specification, ImportError> {
    let invalid = || ImportError::Value {
        format: format.to_owned(),
        spec: spec.name.clone(),
    };
    let pattern = || {
        let source = spec
            .field("value")
            .and_then(|v| v.as_str())
            .ok_or_else(invalid)?;
        Pattern::new(source).map_err(|e| ImportError::Regex {
            format: format.to_owned(),
            pattern: source.to_owned(),
            message: e.to_string(),
        })
    };
    let condition = match spec.implementation.as_str() {
        "LanguageSpecification" => {
            let id = spec
                .field("value")
                .and_then(|v| v.as_i64())
                .ok_or_else(invalid)?;
            Condition::Language {
                language: LanguageRef::from_id(id)
                    .ok_or_else(|| ImportError::Language(format!("id {id}")))?,
                except: spec
                    .field("exceptLanguage")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            }
        }
        "ReleaseGroupSpecification" => Condition::ReleaseGroup(pattern()?),
        "ReleaseTitleSpecification" => Condition::ReleaseTitle(pattern()?),
        other => {
            return Err(ImportError::Implementation {
                format: format.to_owned(),
                implementation: other.to_owned(),
            });
        }
    };
    Ok(Specification {
        name: spec.name.clone(),
        condition,
        negate: spec.negate,
        required: spec.required,
    })
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SettingsDto {
    #[serde(default)]
    indexer: IndexerSettingsDto,
    #[serde(default)]
    media_management: MediaManagementDto,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct IndexerSettingsDto {
    #[serde(default)]
    minimum_age: u32,
    #[serde(default)]
    maximum_size: u32,
    #[serde(default)]
    retention: u32,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct MediaManagementDto {
    #[serde(default)]
    download_propers_and_repacks: Option<String>,
}

pub fn settings(raw: &str) -> Result<Settings, ImportError> {
    let dto: SettingsDto = json("settings", raw)?;
    let propers = match dto.media_management.download_propers_and_repacks {
        Some(v) => PropersPolicy::from_api(&v).ok_or(ImportError::Propers(v))?,
        None => PropersPolicy::default(),
    };
    Ok(Settings {
        minimum_age_minutes: dto.indexer.minimum_age,
        maximum_size_mb: dto.indexer.maximum_size,
        retention_days: dto.indexer.retention,
        propers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sofarr_parser::Language;

    const DEFINITIONS: &str = r#"[
        {"quality": {"id": 7, "name": "Bluray-1080p", "source": "bluray", "resolution": 1080, "modifier": "none"},
         "title": "Bluray-1080p", "minSize": 5.5, "maxSize": null, "preferredSize": 300},
        {"quality": {"id": 3, "name": "WEBDL-1080p", "source": "webdl", "resolution": 1080, "modifier": "none"},
         "title": "WEB 1080p", "minSize": 4, "maxSize": 400, "preferredSize": 200}
    ]"#;

    const PROFILES: &str = r#"[{
        "id": 4, "name": "HD", "upgradeAllowed": true, "cutoff": 1001,
        "items": [
            {"quality": {"id": 1, "name": "SDTV"}, "items": [], "allowed": false},
            {"id": 1001, "name": "WEB 1080p", "allowed": true, "items": [
                {"quality": {"id": 3, "name": "WEBDL-1080p"}, "items": [], "allowed": true},
                {"quality": {"id": 15, "name": "WEBRip-1080p"}, "items": [], "allowed": true}
            ]},
            {"quality": {"id": 7, "name": "Bluray-1080p"}, "items": [], "allowed": true}
        ],
        "formatItems": [{"format": 2, "name": "German", "score": 100}],
        "minFormatScore": 10, "cutoffFormatScore": 200,
        "language": {"id": 4, "name": "German"}
    }]"#;

    const FORMATS: &str = r#"[{
        "id": 2, "name": "German", "specifications": [
            {"name": "German", "implementation": "LanguageSpecification", "negate": false, "required": true,
             "fields": [{"name": "value", "value": 4}, {"name": "exceptLanguage", "value": false}]},
            {"name": "Not 3D", "implementation": "ReleaseTitleSpecification", "negate": true, "required": false,
             "fields": [{"name": "value", "value": "\\b3D\\b"}]},
            {"name": "Group", "implementation": "ReleaseGroupSpecification", "negate": false, "required": false,
             "fields": [{"name": "value", "value": "^(GRP|OTHER)$"}]}
        ]
    }]"#;

    const SETTINGS: &str = r#"{"indexer": {"minimumAge": 0, "maximumSize": 0, "retention": 0},
        "mediaManagement": {"downloadPropersAndRepacks": "doNotUpgrade"}}"#;

    #[test]
    fn imports_a_config() {
        let config = radarr(DEFINITIONS, PROFILES, FORMATS, Some(SETTINGS)).unwrap();

        let defs = &config.definitions;
        assert_eq!(defs.0.len(), 2);
        assert_eq!(defs.get(QualityId(7)).unwrap().max_size, None);
        assert_eq!(defs.get(QualityId(3)).unwrap().max_size, Some(400.0));
        assert!(defs.catalog().by_id(QualityId(0)).is_some());

        let profile = config.profile(ProfileId(4)).unwrap();
        assert_eq!(profile.items.len(), 3);
        assert_eq!(profile.cutoff_rank(), Some(1));
        assert!(!profile.is_allowed(QualityId(1)));
        assert_eq!(profile.rank(QualityId(15)), Some(1));
        assert_eq!(profile.min_upgrade_format_score, None);
        assert_eq!(profile.language, LanguageRef::Language(Language::German));

        let cf = &config.custom_formats[0];
        assert_eq!(cf.specifications.len(), 3);
        assert!(matches!(
            cf.specifications[0].condition,
            Condition::Language {
                language: LanguageRef::Language(Language::German),
                except: false
            }
        ));
        assert_eq!(config.settings.propers, PropersPolicy::DoNotUpgrade);
    }

    #[test]
    fn rejects_unsupported_conditions() {
        let raw = r#"[{"id": 1, "name": "X", "specifications": [
            {"name": "s", "implementation": "SizeSpecification", "fields": []}]}]"#;
        let err = custom_formats(raw).unwrap_err();
        assert!(err.to_string().contains("SizeSpecification"), "{err}");
    }

    #[test]
    fn rejects_unknown_language_ids_and_bad_regex() {
        let raw = r#"[{"id": 1, "name": "X", "specifications": [
            {"name": "s", "implementation": "LanguageSpecification", "fields": [{"name": "value", "value": 9999}]}]}]"#;
        assert!(matches!(custom_formats(raw), Err(ImportError::Language(_))));
        let raw = r#"[{"id": 1, "name": "X", "specifications": [
            {"name": "s", "implementation": "ReleaseTitleSpecification", "fields": [{"name": "value", "value": "("}]}]}]"#;
        assert!(matches!(
            custom_formats(raw),
            Err(ImportError::Regex { .. })
        ));
    }

    #[test]
    fn missing_settings_use_defaults() {
        let config = radarr(DEFINITIONS, PROFILES, FORMATS, None).unwrap();
        assert_eq!(config.settings, Settings::default());
    }
}
