//! Subtitles from OpenSubtitles.com (spec subtitles, D17, D25): the REST client written
//! from the provider's public API documentation, the movie hash after OpenSubtitles'
//! published algorithm, the ranking of candidates by match quality, the conversion of a
//! downloaded subtitle to UTF-8 and the sidecar file names of Jellyfin's naming rules.
//!
//! IO only through [`OpenSubtitles`] (HTTP) and the reader handed to [`movie_hash`]; the
//! rest is pure.

mod client;
mod hash;
mod naming;
mod rank;
mod text;

#[cfg(test)]
mod client_tests;

pub use client::{
    DownloadError, Downloaded, Feature, OpenSubtitles, ProviderConfig, ProviderError, SearchQuery,
};
pub use hash::{HASH_CHUNK, movie_hash};
pub use naming::{SUBTITLE_EXTENSIONS, Sidecar, parse_sidecar, renamed_sidecar, sidecar_name};
pub use rank::{Candidate, MatchQuality, Video, Wanted, rank};
pub use text::{TextError, to_utf8};
