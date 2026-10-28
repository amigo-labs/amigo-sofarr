//! Output types of the parser.

use std::fmt;

/// Structured information extracted from a release title.
///
/// Everything that could not be recognised stays `None`, empty or `Unknown`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedRelease {
    /// Title candidates in order of preference (several if the name contains `AKA`).
    pub titles: Vec<String>,
    pub year: Option<u16>,
    pub quality: Quality,
    pub revision: Revision,
    pub edition: Option<String>,
    pub release_group: Option<String>,
    pub languages: Vec<Language>,
}

impl ParsedRelease {
    /// The preferred title candidate.
    pub fn title(&self) -> Option<&str> {
        self.titles.first().map(String::as_str)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Quality {
    pub source: Source,
    pub resolution: Resolution,
    pub modifier: Modifier,
}

/// Where the video was taken from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Source {
    #[default]
    Unknown,
    Cam,
    Telesync,
    Telecine,
    Workprint,
    Dvd,
    Tv,
    WebDl,
    WebRip,
    BluRay,
}

impl Source {
    /// Lower-case identifier, matching the vocabulary of the public Radarr API docs.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Cam => "cam",
            Self::Telesync => "telesync",
            Self::Telecine => "telecine",
            Self::Workprint => "workprint",
            Self::Dvd => "dvd",
            Self::Tv => "tv",
            Self::WebDl => "webdl",
            Self::WebRip => "webrip",
            Self::BluRay => "bluray",
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Vertical resolution class.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Resolution {
    #[default]
    Unknown,
    R360,
    R480,
    R540,
    R576,
    R720,
    R1080,
    R2160,
}

impl Resolution {
    /// Height in lines, `None` if unknown.
    pub fn height(self) -> Option<u16> {
        match self {
            Self::Unknown => None,
            Self::R360 => Some(360),
            Self::R480 => Some(480),
            Self::R540 => Some(540),
            Self::R576 => Some(576),
            Self::R720 => Some(720),
            Self::R1080 => Some(1080),
            Self::R2160 => Some(2160),
        }
    }

    pub fn from_height(height: u32) -> Option<Self> {
        Some(match height {
            360 => Self::R360,
            480 => Self::R480,
            540 => Self::R540,
            576 => Self::R576,
            720 => Self::R720,
            1080 => Self::R1080,
            2160 => Self::R2160,
            _ => return None,
        })
    }
}

/// Additional property of a quality that is not covered by source and resolution.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Modifier {
    #[default]
    None,
    Regional,
    Screener,
    RawHd,
    BrDisk,
    Remux,
}

impl Modifier {
    /// Lower-case identifier, matching the vocabulary of the public Radarr API docs.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Regional => "regional",
            Self::Screener => "screener",
            Self::RawHd => "rawhd",
            Self::BrDisk => "brdisk",
            Self::Remux => "remux",
        }
    }
}

impl fmt::Display for Modifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Re-release information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Revision {
    /// 1 for a first release, higher for fixed re-releases.
    pub version: u8,
    pub proper: bool,
    pub repack: bool,
    /// How often the release was marked as `REAL`.
    pub real: u8,
}

impl Default for Revision {
    fn default() -> Self {
        Self {
            version: 1,
            proper: false,
            repack: false,
            real: 0,
        }
    }
}

/// Audio language of a release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Language {
    Arabic,
    Bulgarian,
    Chinese,
    Czech,
    Danish,
    Dutch,
    English,
    Finnish,
    French,
    German,
    Greek,
    Hebrew,
    Hindi,
    Hungarian,
    Icelandic,
    Italian,
    Japanese,
    Korean,
    Lithuanian,
    Norwegian,
    Polish,
    Portuguese,
    Romanian,
    Russian,
    Spanish,
    Swedish,
    Thai,
    Turkish,
    Ukrainian,
    Vietnamese,
    /// The film's original audio track, as in dual-language (`DL`) releases.
    Original,
}

impl Language {
    /// English name of the language.
    pub fn name(self) -> &'static str {
        match self {
            Self::Arabic => "Arabic",
            Self::Bulgarian => "Bulgarian",
            Self::Chinese => "Chinese",
            Self::Czech => "Czech",
            Self::Danish => "Danish",
            Self::Dutch => "Dutch",
            Self::English => "English",
            Self::Finnish => "Finnish",
            Self::French => "French",
            Self::German => "German",
            Self::Greek => "Greek",
            Self::Hebrew => "Hebrew",
            Self::Hindi => "Hindi",
            Self::Hungarian => "Hungarian",
            Self::Icelandic => "Icelandic",
            Self::Italian => "Italian",
            Self::Japanese => "Japanese",
            Self::Korean => "Korean",
            Self::Lithuanian => "Lithuanian",
            Self::Norwegian => "Norwegian",
            Self::Polish => "Polish",
            Self::Portuguese => "Portuguese",
            Self::Romanian => "Romanian",
            Self::Russian => "Russian",
            Self::Spanish => "Spanish",
            Self::Swedish => "Swedish",
            Self::Thai => "Thai",
            Self::Turkish => "Turkish",
            Self::Ukrainian => "Ukrainian",
            Self::Vietnamese => "Vietnamese",
            Self::Original => "Original",
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
