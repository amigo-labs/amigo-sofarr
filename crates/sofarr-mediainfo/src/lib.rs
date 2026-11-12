//! Media info from Matroska/WebM and MP4/QuickTime files: codecs, encoder, resolution,
//! dynamic range, audio and subtitle tracks with their languages.
//!
//! Pure: the reader works on any `Read + Seek`, does no file-system access of its own, never
//! writes and never panics. It reads at most [`READ_LIMIT`] bytes per file; bytes skipped by
//! seeking do not count. Written from the public specifications (Matroska RFC 9559, EBML
//! RFC 8794, ISO/IEC 14496-12 and -15, ETSI TS 102 366 and TS 102 114, H.264/H.265 SEI syntax,
//! the Dolby Vision configuration record); design: docs/specs/mediainfo.md.

mod audio;
mod bits;
mod ebml;
mod labels;
mod language;
mod matroska;
mod mp4;
mod source;
mod track;
mod video;

#[cfg(test)]
mod robustness_tests;
#[cfg(test)]
mod testkit;

use std::io::{Read, Seek};

pub use language::{language_code, language_from_name};

/// The most bytes [`read`] reads from one input.
pub const READ_LIMIT: u64 = 16 * 1024 * 1024;

/// How far the headers of a file could be read.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadStatus {
    /// The headers of every track were read.
    Complete,
    /// The input ended or the read limit was reached before every track header was read.
    Partial,
    /// The container structure is invalid.
    Damaged,
    /// The content is no supported container.
    Unsupported,
    /// The input could not be opened or read.
    Unreadable,
}

/// The container format, recognized by content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Container {
    Matroska,
    Webm,
    Mp4,
    QuickTime,
}

/// The dynamic range formats a video track carries; all false is SDR.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DynamicRange {
    pub hdr10: bool,
    pub hdr10_plus: bool,
    pub hlg: bool,
    pub dolby_vision: bool,
}

/// The first video track in file order.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Video {
    pub codec: String,
    pub encoder: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub dynamic_range: DynamicRange,
}

/// One audio track.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Audio {
    pub codec: String,
    pub channels: Option<u8>,
    pub object_audio: bool,
    /// ISO 639-2/B code, `und` when unknown.
    pub language: String,
}

/// One subtitle track.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Subtitle {
    /// ISO 639-2/B code, `und` when unknown.
    pub language: String,
    pub format: String,
    pub forced: bool,
    pub default: bool,
    pub hearing_impaired: bool,
}

/// What sofarr records per file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MediaInfo {
    pub status: ReadStatus,
    pub container: Option<Container>,
    pub video: Option<Video>,
    pub audio: Vec<Audio>,
    pub subtitles: Vec<Subtitle>,
}

/// The texts derived from media info for the naming tokens.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Labels {
    pub video: Option<String>,
    pub audio: Option<String>,
    pub channels: Option<String>,
    pub dynamic_range: Option<String>,
}

/// Reads at most READ_LIMIT bytes in total (bytes skipped by seeking don't count); never panics.
pub fn read<R: Read + Seek>(reader: &mut R) -> MediaInfo {
    let Ok(mut src) = source::Source::new(reader) else {
        return unreadable();
    };
    let magic = match src.read_some(8) {
        Ok(magic) => magic,
        Err(stop) => return track::empty(track::status_of(stop), None),
    };
    src.seek(0);
    if magic.starts_with(&ebml::MAGIC) {
        matroska::read(&mut src)
    } else if mp4::looks_like(&magic) {
        mp4::read(&mut src)
    } else {
        track::empty(ReadStatus::Unsupported, None)
    }
}

/// A MediaInfo with status Unreadable and nothing else (the server uses it when the file cannot be opened).
pub fn unreadable() -> MediaInfo {
    track::empty(ReadStatus::Unreadable, None)
}
