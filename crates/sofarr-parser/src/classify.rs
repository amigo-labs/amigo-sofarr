//! Assigns a meaning to each token via keyword tables.

use crate::model::{Language, Modifier, Resolution, Source};
use crate::token::{Sep, Token};

/// Meaning of a single token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    /// Not recognised; may belong to the title or the release group.
    Plain,
    /// Part of a multi-token keyword that started at an earlier token.
    Continued,
    Year(u16),
    Quality {
        source: Option<Source>,
        resolution: Option<Resolution>,
        modifier: Option<Modifier>,
    },
    Proper,
    Repack,
    Real,
    Version(u8),
    Edition,
    Language(Language),
    /// `DL`: dual language, i.e. the named language plus the original audio track.
    DualLanguage,
    /// Codec, audio format, streaming service or scene tag: carries no field
    /// but clearly does not belong to the title.
    Technical,
    /// `AKA`, separating alternative titles.
    Aka,
}

impl Tag {
    /// Tags that reliably mark the start of the metadata part of a title.
    pub fn is_strong(self) -> bool {
        matches!(
            self,
            Self::Quality { .. }
                | Self::Proper
                | Self::Repack
                | Self::Version(_)
                | Self::Technical
                | Self::DualLanguage
        )
    }
}

const fn src(source: Source) -> Tag {
    Tag::Quality {
        source: Some(source),
        resolution: None,
        modifier: None,
    }
}

const fn modi(modifier: Modifier, source: Source) -> Tag {
    Tag::Quality {
        source: Some(source),
        resolution: None,
        modifier: Some(modifier),
    }
}

const fn res(resolution: Resolution) -> Tag {
    Tag::Quality {
        source: None,
        resolution: Some(resolution),
        modifier: None,
    }
}

/// Keyword phrases (lower-case, apostrophes removed). Longer phrases are tried first.
const PHRASES: &[(&[&str], Tag)] = &[
    // Sources
    (&["blu", "ray"], src(Source::BluRay)),
    (&["bluray"], src(Source::BluRay)),
    (&["bdrip"], src(Source::BluRay)),
    (&["brrip"], src(Source::BluRay)),
    (&["bd", "rip"], src(Source::BluRay)),
    (&["br", "rip"], src(Source::BluRay)),
    (&["uhdbluray"], src(Source::BluRay)),
    (&["web", "dl"], src(Source::WebDl)),
    (&["webdl"], src(Source::WebDl)),
    (&["web"], src(Source::WebDl)),
    (&["webhd"], src(Source::WebDl)),
    (&["web", "rip"], src(Source::WebRip)),
    (&["webrip"], src(Source::WebRip)),
    (&["hdtv"], src(Source::Tv)),
    (&["pdtv"], src(Source::Tv)),
    (&["sdtv"], src(Source::Tv)),
    (&["tvrip"], src(Source::Tv)),
    (&["hdtvrip"], src(Source::Tv)),
    (&["satrip"], src(Source::Tv)),
    (&["dvb"], src(Source::Tv)),
    (&["dvd", "rip"], src(Source::Dvd)),
    (&["dvdrip"], src(Source::Dvd)),
    (&["dvd"], src(Source::Dvd)),
    (&["dvdr"], src(Source::Dvd)),
    (&["dvd5"], src(Source::Dvd)),
    (&["dvd9"], src(Source::Dvd)),
    (&["cam"], src(Source::Cam)),
    (&["camrip"], src(Source::Cam)),
    (&["hdcam"], src(Source::Cam)),
    (&["ts"], src(Source::Telesync)),
    (&["hdts"], src(Source::Telesync)),
    (&["telesync"], src(Source::Telesync)),
    (&["tc"], src(Source::Telecine)),
    (&["hdtc"], src(Source::Telecine)),
    (&["telecine"], src(Source::Telecine)),
    (&["workprint"], src(Source::Workprint)),
    // Modifiers (with the source they imply)
    (&["remux"], modi(Modifier::Remux, Source::BluRay)),
    (&["bdremux"], modi(Modifier::Remux, Source::BluRay)),
    (&["br", "disk"], modi(Modifier::BrDisk, Source::BluRay)),
    (&["brdisk"], modi(Modifier::BrDisk, Source::BluRay)),
    (&["bdmv"], modi(Modifier::BrDisk, Source::BluRay)),
    (&["bd25"], modi(Modifier::BrDisk, Source::BluRay)),
    (&["bd50"], modi(Modifier::BrDisk, Source::BluRay)),
    (&["bd66"], modi(Modifier::BrDisk, Source::BluRay)),
    (&["bd100"], modi(Modifier::BrDisk, Source::BluRay)),
    (
        &["complete", "bluray"],
        modi(Modifier::BrDisk, Source::BluRay),
    ),
    (
        &["complete", "uhd", "bluray"],
        modi(Modifier::BrDisk, Source::BluRay),
    ),
    (&["screener"], modi(Modifier::Screener, Source::Dvd)),
    (&["scr"], modi(Modifier::Screener, Source::Dvd)),
    (&["dvdscr"], modi(Modifier::Screener, Source::Dvd)),
    (&["dvd", "scr"], modi(Modifier::Screener, Source::Dvd)),
    (&["bdscr"], modi(Modifier::Screener, Source::BluRay)),
    (&["r5"], modi(Modifier::Regional, Source::Dvd)),
    (&["rawhd"], modi(Modifier::RawHd, Source::Tv)),
    (&["raw", "hd"], modi(Modifier::RawHd, Source::Tv)),
    // Resolutions without a `p` suffix
    (&["4k"], res(Resolution::R2160)),
    (&["uhd"], res(Resolution::R2160)),
    // Revisions
    (&["proper"], Tag::Proper),
    (&["repack"], Tag::Repack),
    (&["rerip"], Tag::Repack),
    (&["real"], Tag::Real),
    // Editions
    (&["directors", "cut"], Tag::Edition),
    (&["directors", "edition"], Tag::Edition),
    (&["extended", "cut"], Tag::Edition),
    (&["extended", "edition"], Tag::Edition),
    (&["extended"], Tag::Edition),
    (&["uncut"], Tag::Edition),
    (&["unrated"], Tag::Edition),
    (&["uncensored"], Tag::Edition),
    (&["theatrical", "cut"], Tag::Edition),
    (&["theatrical", "edition"], Tag::Edition),
    (&["theatrical"], Tag::Edition),
    (&["imax", "edition"], Tag::Edition),
    (&["imax"], Tag::Edition),
    (&["remastered"], Tag::Edition),
    (&["special", "edition"], Tag::Edition),
    (&["collectors", "edition"], Tag::Edition),
    (&["ultimate", "edition"], Tag::Edition),
    (&["ultimate", "cut"], Tag::Edition),
    (&["final", "cut"], Tag::Edition),
    (&["criterion", "collection"], Tag::Edition),
    (&["criterion"], Tag::Edition),
    (&["anniversary", "edition"], Tag::Edition),
    (&["open", "matte"], Tag::Edition),
    (&["despecialized"], Tag::Edition),
    (&["redux"], Tag::Edition),
    (&["fan", "edit"], Tag::Edition),
    // Languages
    (&["english"], Tag::Language(Language::English)),
    (&["eng"], Tag::Language(Language::English)),
    (&["german"], Tag::Language(Language::German)),
    (&["ger"], Tag::Language(Language::German)),
    (&["deutsch"], Tag::Language(Language::German)),
    (&["french"], Tag::Language(Language::French)),
    (&["truefrench"], Tag::Language(Language::French)),
    (&["vff"], Tag::Language(Language::French)),
    (&["vfq"], Tag::Language(Language::French)),
    (&["vf2"], Tag::Language(Language::French)),
    (&["vfi"], Tag::Language(Language::French)),
    (&["spanish"], Tag::Language(Language::Spanish)),
    (&["castellano"], Tag::Language(Language::Spanish)),
    (&["esp"], Tag::Language(Language::Spanish)),
    (&["italian"], Tag::Language(Language::Italian)),
    (&["ita"], Tag::Language(Language::Italian)),
    (&["dutch"], Tag::Language(Language::Dutch)),
    (&["flemish"], Tag::Language(Language::Dutch)),
    (&["danish"], Tag::Language(Language::Danish)),
    (&["swedish"], Tag::Language(Language::Swedish)),
    (&["swe"], Tag::Language(Language::Swedish)),
    (&["norwegian"], Tag::Language(Language::Norwegian)),
    (&["finnish"], Tag::Language(Language::Finnish)),
    (&["polish"], Tag::Language(Language::Polish)),
    (&["portuguese"], Tag::Language(Language::Portuguese)),
    (&["russian"], Tag::Language(Language::Russian)),
    (&["rus"], Tag::Language(Language::Russian)),
    (&["japanese"], Tag::Language(Language::Japanese)),
    (&["korean"], Tag::Language(Language::Korean)),
    (&["chinese"], Tag::Language(Language::Chinese)),
    (&["mandarin"], Tag::Language(Language::Chinese)),
    (&["cantonese"], Tag::Language(Language::Chinese)),
    (&["hindi"], Tag::Language(Language::Hindi)),
    (&["turkish"], Tag::Language(Language::Turkish)),
    (&["czech"], Tag::Language(Language::Czech)),
    (&["hungarian"], Tag::Language(Language::Hungarian)),
    (&["greek"], Tag::Language(Language::Greek)),
    (&["hebrew"], Tag::Language(Language::Hebrew)),
    (&["arabic"], Tag::Language(Language::Arabic)),
    (&["thai"], Tag::Language(Language::Thai)),
    (&["vietnamese"], Tag::Language(Language::Vietnamese)),
    (&["ukrainian"], Tag::Language(Language::Ukrainian)),
    (&["romanian"], Tag::Language(Language::Romanian)),
    (&["bulgarian"], Tag::Language(Language::Bulgarian)),
    (&["lithuanian"], Tag::Language(Language::Lithuanian)),
    (&["icelandic"], Tag::Language(Language::Icelandic)),
    // Codecs written with a separator
    (&["h", "264"], Tag::Technical),
    (&["h", "265"], Tag::Technical),
    (&["x", "264"], Tag::Technical),
    (&["x", "265"], Tag::Technical),
    (&["dts", "hd"], Tag::Technical),
    (&["dts", "x"], Tag::Technical),
    (&["aka"], Tag::Aka),
    (&["dl"], Tag::DualLanguage),
];

/// Single-token technical keywords: codecs, HDR formats, audio, streaming services, scene tags.
const TECHNICAL: &[&str] = &[
    "x264",
    "x265",
    "h264",
    "h265",
    "hevc",
    "avc",
    "xvid",
    "divx",
    "av1",
    "vp9",
    "vc1",
    "mpeg2",
    "8bit",
    "10bit",
    "12bit",
    "hdr",
    "hdr10",
    "hdr10plus",
    "hdr10+",
    "dv",
    "dovi",
    "hlg",
    "sdr",
    "atmos",
    "truehd",
    "dtshd",
    "dtsma",
    "dtsx",
    "lpcm",
    "flac",
    "opus",
    "mp3",
    "amzn",
    "nf",
    "dsnp",
    "hmax",
    "atvp",
    "pcok",
    "hulu",
    "itunes",
    "internal",
    "limited",
    "dubbed",
    "subbed",
    "hardsub",
    "hc",
    "multi",
    "multisub",
    "dual",
    "ac3d",
    "ld",
    "md",
    "nfofix",
    "readnfo",
    "dirfix",
    "syncfix",
    "retail",
    "hybrid",
];

/// Keywords that are also ordinary English words or very short. They only end the title
/// when no year marks the boundary.
const AMBIGUOUS: &[&str] = &[
    "web", "ts", "tc", "cam", "scr", "dvd", "uhd", "dl", "hc", "md", "ld", "nf", "dv", "dual",
    "multi", "limited", "internal", "hybrid", "retail", "proper", "real",
];

/// Whether token `i` reliably marks the start of the metadata part. Multi-token
/// keywords (`WEB-DL`) are never ambiguous.
pub fn is_unambiguous_strong(tokens: &[Token<'_>], tags: &[Tag], i: usize) -> bool {
    let phrase = tags.get(i + 1) == Some(&Tag::Continued);
    tags[i].is_strong() && (phrase || !AMBIGUOUS.contains(&tokens[i].key.as_str()))
}

/// Audio formats that are often glued to a channel count (`DDP5`, `AAC2`).
const AUDIO_PREFIXES: &[&str] = &["dd", "ddp", "dd+", "aac", "ac3", "eac3", "dts", "truehd"];

pub fn classify(tokens: &[Token<'_>]) -> Vec<Tag> {
    let mut tags = vec![Tag::Plain; tokens.len()];
    let mut i = 0;
    while i < tokens.len() {
        if let Some((len, tag)) = match_phrase(tokens, i) {
            tags[i] = tag;
            for t in &mut tags[i + 1..i + len] {
                *t = Tag::Continued;
            }
            i += len;
        } else {
            tags[i] = classify_single(&tokens[i].key);
            i += 1;
        }
    }
    resolve_real(&mut tags);
    mark_ordinal_editions(tokens, &mut tags);
    tags
}

fn match_phrase(tokens: &[Token<'_>], at: usize) -> Option<(usize, Tag)> {
    PHRASES
        .iter()
        .filter(|(words, _)| at + words.len() <= tokens.len())
        .filter(|(words, _)| {
            words.iter().enumerate().all(|(k, word)| {
                let token = &tokens[at + k];
                // Multi-token keywords must not span a bracket boundary.
                token.key == *word && (k == 0 || token.sep != Sep::Bracket)
            })
        })
        .max_by_key(|(words, _)| words.len())
        .map(|(words, tag)| (words.len(), *tag))
}

fn classify_single(key: &str) -> Tag {
    if let Some(year) = parse_year(key) {
        return Tag::Year(year);
    }
    if let Some(resolution) = parse_resolution(key) {
        return res(resolution);
    }
    if let Some(version) = parse_version(key) {
        return Tag::Version(version);
    }
    if TECHNICAL.contains(&key) || is_audio_with_channels(key) {
        return Tag::Technical;
    }
    Tag::Plain
}

fn parse_year(key: &str) -> Option<u16> {
    if key.len() != 4 || !key.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let year: u16 = key.parse().ok()?;
    (1900..=2099).contains(&year).then_some(year)
}

/// `1080p`, `720i`, `1920x1080`.
fn parse_resolution(key: &str) -> Option<Resolution> {
    if let Some((width, height)) = key.split_once('x') {
        let all_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        if all_digits(width) && all_digits(height) {
            return Resolution::from_height(height.parse().ok()?);
        }
        return None;
    }
    let digits = key.strip_suffix('p').or_else(|| key.strip_suffix('i'))?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Resolution::from_height(digits.parse().ok()?)
}

/// `v2` … `v9`.
fn parse_version(key: &str) -> Option<u8> {
    let digit = key.strip_prefix('v')?;
    match digit.as_bytes() {
        [d @ b'2'..=b'9'] => Some(d - b'0'),
        _ => None,
    }
}

fn is_audio_with_channels(key: &str) -> bool {
    let prefix = key.trim_end_matches(|c: char| c.is_ascii_digit());
    prefix.len() < key.len() && AUDIO_PREFIXES.contains(&prefix)
}

/// `REAL` only counts next to `PROPER`/`REPACK`; elsewhere it is an ordinary word.
fn resolve_real(tags: &mut [Tag]) {
    let is_fix = |t: Option<&Tag>| matches!(t, Some(Tag::Proper | Tag::Repack | Tag::Real));
    for i in 0..tags.len() {
        if tags[i] != Tag::Real {
            continue;
        }
        let before = i.checked_sub(1).and_then(|j| tags.get(j));
        // Look past further REALs, so `REAL.REAL.PROPER` counts twice.
        let after = tags[i + 1..].iter().find(|t| **t != Tag::Real);
        if !(is_fix(before) || is_fix(after)) {
            tags[i] = Tag::Plain;
        }
    }
}

/// `25th.Anniversary.Edition`: the ordinal belongs to the edition.
fn mark_ordinal_editions(tokens: &[Token<'_>], tags: &mut [Tag]) {
    for i in 1..tokens.len() {
        if tags[i] == Tag::Edition
            && tokens[i].key == "anniversary"
            && is_ordinal(&tokens[i - 1].key)
        {
            tags[i - 1] = Tag::Edition;
            tags[i] = Tag::Continued;
        }
    }
}

fn is_ordinal(key: &str) -> bool {
    let digits = key.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let suffix = &key[digits.len()..];
    !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && matches!(suffix, "st" | "nd" | "rd" | "th")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::tokenize;

    fn tags(input: &str) -> Vec<Tag> {
        classify(&tokenize(input))
    }

    #[test]
    fn multi_token_source_is_one_tag() {
        assert_eq!(tags("WEB-DL"), [src(Source::WebDl), Tag::Continued],);
        assert_eq!(tags("Blu.Ray"), [src(Source::BluRay), Tag::Continued]);
    }

    #[test]
    fn longest_phrase_wins() {
        assert_eq!(tags("Extended.Edition"), [Tag::Edition, Tag::Continued],);
        assert_eq!(tags("WEB.Rip"), [src(Source::WebRip), Tag::Continued]);
    }

    #[test]
    fn phrases_do_not_cross_brackets() {
        assert_eq!(tags("WEB [DL]")[0], src(Source::WebDl));
        assert_eq!(tags("WEB [DL]")[1], Tag::DualLanguage);
    }

    #[test]
    fn years_and_resolutions() {
        assert_eq!(classify_single("1999"), Tag::Year(1999));
        assert_eq!(classify_single("1899"), Tag::Plain);
        assert_eq!(classify_single("2160p"), res(Resolution::R2160));
        assert_eq!(classify_single("1080i"), res(Resolution::R1080));
        assert_eq!(classify_single("1280x720"), res(Resolution::R720));
        assert_eq!(classify_single("1080"), Tag::Plain);
        assert_eq!(classify_single("p"), Tag::Plain);
        assert_eq!(classify_single("x"), Tag::Plain);
    }

    #[test]
    fn audio_with_channel_count_is_technical() {
        assert_eq!(classify_single("ddp5"), Tag::Technical);
        assert_eq!(classify_single("aac2"), Tag::Technical);
        assert_eq!(classify_single("dd"), Tag::Plain);
    }

    #[test]
    fn real_only_next_to_a_fix() {
        assert_eq!(tags("Real.Life")[0], Tag::Plain);
        assert_eq!(tags("REAL.PROPER"), [Tag::Real, Tag::Proper]);
        assert_eq!(
            tags("REAL.REAL.REPACK"),
            [Tag::Real, Tag::Real, Tag::Repack]
        );
    }

    #[test]
    fn ordinal_anniversary_edition() {
        assert_eq!(
            tags("40th.Anniversary.Edition"),
            [Tag::Edition, Tag::Continued, Tag::Continued]
        );
    }
}
