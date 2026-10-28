//! Domain model and decision engine: qualities, quality profiles,
//! custom formats, and grab/upgrade decisions.
//!
//! Depends on `sofarr-parser`; must stay free of IO. Implementation: PLAN.md Phase 2.

pub mod custom_format;
pub mod decision;
pub mod definition;
pub mod import;
pub mod language;
pub mod profile;
pub mod quality;
pub mod settings;
pub mod title_match;

pub use custom_format::{CustomFormat, CustomFormatId, ReleaseFacts};
pub use decision::{Decision, ExistingFile, MovieContext, Rejection, ReleaseInput, decide};
pub use definition::{QualityDefinition, QualityDefinitions};
pub use import::RadarrConfig;
pub use language::LanguageRef;
pub use profile::{ProfileId, ProfileItem, QualityProfile};
pub use quality::{NamedQuality, QualityCatalog, QualityId};
pub use settings::{PropersPolicy, Settings};
