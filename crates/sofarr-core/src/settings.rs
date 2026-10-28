//! Global settings that influence decisions.

/// How propers and repacks are treated (`downloadPropersAndRepacks`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PropersPolicy {
    /// A higher revision is preferred and is an upgrade on its own.
    #[default]
    PreferAndUpgrade,
    /// A higher revision is preferred, but not an upgrade on its own.
    DoNotUpgrade,
    /// Revisions are ignored; custom formats decide.
    DoNotPrefer,
}

impl PropersPolicy {
    pub fn from_api(value: &str) -> Option<Self> {
        match value {
            "preferAndUpgrade" => Some(Self::PreferAndUpgrade),
            "doNotUpgrade" => Some(Self::DoNotUpgrade),
            "doNotPrefer" => Some(Self::DoNotPrefer),
            _ => None,
        }
    }
}

/// Indexer and media management settings; `0` disables a limit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Settings {
    /// Minimum age of a usenet release in minutes.
    pub minimum_age_minutes: u32,
    /// Maximum release size in megabytes.
    pub maximum_size_mb: u32,
    /// Usenet retention in days.
    pub retention_days: u32,
    /// Whether releases with burnt-in subtitles may be grabbed.
    pub allow_hardcoded_subs: bool,
    pub propers: PropersPolicy,
}
