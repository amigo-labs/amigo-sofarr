//! Domain model and decision engine: qualities, quality profiles,
//! custom formats, and grab/upgrade decisions.
//!
//! Depends on `sofarr-parser`; must stay free of IO. Implementation: PLAN.md Phase 2.

pub mod quality;

pub use quality::{NamedQuality, QualityCatalog, QualityId};
