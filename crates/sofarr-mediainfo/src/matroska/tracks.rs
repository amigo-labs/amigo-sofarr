//! `Tracks` → `TrackEntry` (RFC 9559 §5.1.4) and the Matroska codec IDs.

use crate::ebml::{self, Children};
use crate::language_code;
use crate::source::Res;
use crate::track::{Draft, Kind, Probe};
use crate::video::{self, DvConfig};

const TRACK_ENTRY: u32 = 0xAE;
const TRACK_NUMBER: u32 = 0xD7;
const TRACK_TYPE: u32 = 0x83;
const FLAG_DEFAULT: u32 = 0x88;
const FLAG_FORCED: u32 = 0x55AA;
const FLAG_HEARING_IMPAIRED: u32 = 0x55AB;
const CODEC_ID: u32 = 0x86;
const CODEC_PRIVATE: u32 = 0x63A2;
const LANGUAGE: u32 = 0x22_B59C;
const LANGUAGE_BCP47: u32 = 0x22_B59D;
const VIDEO: u32 = 0xE0;
const PIXEL_WIDTH: u32 = 0xB0;
const PIXEL_HEIGHT: u32 = 0xBA;
const COLOUR: u32 = 0x55B0;
const TRANSFER_CHARACTERISTICS: u32 = 0x55BA;
const MASTERING_METADATA: u32 = 0x55D0;
const AUDIO: u32 = 0xE1;
const CHANNELS: u32 = 0x9F;
const BLOCK_ADDITION_MAPPING: u32 = 0x41E4;
const BLOCK_ADD_ID_TYPE: u32 = 0x41E7;
const BLOCK_ADD_ID_EXTRA_DATA: u32 = 0x41ED;
const CONTENT_ENCODINGS: u32 = 0x6D80;
const CONTENT_ENCODING: u32 = 0x6240;
const CONTENT_ENCODING_TYPE: u32 = 0x5033;
const CONTENT_COMPRESSION: u32 = 0x5034;
const CONTENT_COMP_ALGO: u32 = 0x4254;
const CONTENT_COMP_SETTINGS: u32 = 0x4255;
const CONTENT_ENCRYPTION: u32 = 0x5035;

/// `dvcC`, `dvvC` and `dvwC` as BlockAddIDType values.
const DOLBY_VISION_TYPES: [u64; 3] = [0x6476_6343, 0x6476_7643, 0x6476_7743];
/// ContentCompAlgo 3: header stripping.
const HEADER_STRIPPING: u64 = 3;

/// A track with what the frame scan needs to find its frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MkvTrack {
    pub(super) number: u64,
    /// Bytes removed from every frame by header-stripping compression.
    pub(super) strip: Vec<u8>,
    /// False when frames are compressed (other than header stripping) or encrypted.
    pub(super) frames_readable: bool,
    pub(super) draft: Draft,
}

#[derive(Debug, Default)]
struct Fields {
    number: u64,
    kind: u64,
    codec_id: String,
    private: Vec<u8>,
    language: Option<String>,
    language_bcp47: Option<String>,
    default: Option<bool>,
    forced: bool,
    hearing_impaired: bool,
    width: Option<u32>,
    height: Option<u32>,
    transfer: Option<u64>,
    mastering: bool,
    channels: Option<u64>,
    dv: Option<DvConfig>,
    strip: Vec<u8>,
    unreadable_frames: bool,
}

/// The tracks of a `Tracks` element in file order; track types other than video, audio and
/// subtitles are left out.
pub(super) fn parse(data: &[u8]) -> Res<Vec<MkvTrack>> {
    let mut tracks = Vec::new();
    for child in Children::new(data) {
        let (id, body) = child?;
        if id == TRACK_ENTRY
            && let Some(track) = parse_entry(body)?
        {
            tracks.push(track);
        }
    }
    Ok(tracks)
}

/// The complete `TrackEntry`s at the start of a `Tracks` element the input ends inside: the
/// cut-off child ends the list, a damaged complete entry is still damage.
pub(super) fn parse_complete(data: &[u8]) -> Res<Vec<MkvTrack>> {
    let mut tracks = Vec::new();
    for child in Children::new(data) {
        let Ok((id, body)) = child else {
            break;
        };
        if id == TRACK_ENTRY
            && let Some(track) = parse_entry(body)?
        {
            tracks.push(track);
        }
    }
    Ok(tracks)
}

fn parse_entry(data: &[u8]) -> Res<Option<MkvTrack>> {
    let mut fields = Fields::default();
    for child in Children::new(data) {
        let (id, body) = child?;
        match id {
            TRACK_NUMBER => fields.number = ebml::uint(body)?,
            TRACK_TYPE => fields.kind = ebml::uint(body)?,
            CODEC_ID => fields.codec_id = ebml::string(body),
            CODEC_PRIVATE => fields.private = body.to_vec(),
            LANGUAGE => fields.language = Some(ebml::string(body)),
            LANGUAGE_BCP47 => fields.language_bcp47 = Some(ebml::string(body)),
            FLAG_DEFAULT => fields.default = Some(ebml::uint(body)? != 0),
            FLAG_FORCED => fields.forced = ebml::uint(body)? != 0,
            FLAG_HEARING_IMPAIRED => fields.hearing_impaired = ebml::uint(body)? != 0,
            VIDEO => parse_video(body, &mut fields)?,
            AUDIO => {
                for child in Children::new(body) {
                    let (id, value) = child?;
                    if id == CHANNELS {
                        fields.channels = Some(ebml::uint(value)?);
                    }
                }
            }
            BLOCK_ADDITION_MAPPING => parse_block_addition(body, &mut fields)?,
            CONTENT_ENCODINGS => parse_encodings(body, &mut fields)?,
            _ => {}
        }
    }
    let kind = match fields.kind {
        1 => Kind::Video,
        2 => Kind::Audio,
        0x11 => Kind::Subtitle,
        _ => return Ok(None),
    };
    Ok(Some(build(kind, fields)))
}

fn parse_video(data: &[u8], fields: &mut Fields) -> Res<()> {
    for child in Children::new(data) {
        let (id, body) = child?;
        match id {
            PIXEL_WIDTH => fields.width = u32::try_from(ebml::uint(body)?).ok(),
            PIXEL_HEIGHT => fields.height = u32::try_from(ebml::uint(body)?).ok(),
            COLOUR => {
                for colour in Children::new(body) {
                    match colour? {
                        (TRANSFER_CHARACTERISTICS, value) => {
                            fields.transfer = Some(ebml::uint(value)?);
                        }
                        (MASTERING_METADATA, _) => fields.mastering = true,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_block_addition(data: &[u8], fields: &mut Fields) -> Res<()> {
    let mut kind = 0;
    let mut extra: &[u8] = &[];
    for child in Children::new(data) {
        match child? {
            (BLOCK_ADD_ID_TYPE, value) => kind = ebml::uint(value)?,
            (BLOCK_ADD_ID_EXTRA_DATA, value) => extra = value,
            _ => {}
        }
    }
    if DOLBY_VISION_TYPES.contains(&kind)
        && let Some(config) = DvConfig::parse(extra)
    {
        fields.dv.get_or_insert(config);
    }
    Ok(())
}

fn parse_encodings(data: &[u8], fields: &mut Fields) -> Res<()> {
    for encoding in Children::new(data) {
        let (id, body) = encoding?;
        if id != CONTENT_ENCODING {
            continue;
        }
        let mut encryption = false;
        let mut algo = None;
        let mut settings: &[u8] = &[];
        for child in Children::new(body) {
            match child? {
                (CONTENT_ENCODING_TYPE, value) => encryption |= ebml::uint(value)? == 1,
                (CONTENT_ENCRYPTION, _) => encryption = true,
                (CONTENT_COMPRESSION, compression) => {
                    // ContentCompAlgo defaults to 0 (zlib).
                    algo = Some(0);
                    for field in Children::new(compression) {
                        match field? {
                            (CONTENT_COMP_ALGO, value) => algo = Some(ebml::uint(value)?),
                            (CONTENT_COMP_SETTINGS, value) => settings = value,
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        match algo {
            _ if encryption => fields.unreadable_frames = true,
            Some(HEADER_STRIPPING) => fields.strip.extend_from_slice(settings),
            Some(_) => fields.unreadable_frames = true,
            None => {}
        }
    }
    Ok(())
}

fn build(kind: Kind, fields: Fields) -> MkvTrack {
    let mut draft = Draft::new(kind);
    // LanguageBCP47 overrides Language; missing or `und` both mean undetermined.
    let raw_language = fields
        .language_bcp47
        .filter(|tag| !tag.is_empty())
        .or(fields.language)
        .unwrap_or_default();
    draft.language = language_code(&raw_language);
    draft.default = fields.default.unwrap_or(true);
    draft.forced = fields.forced;
    draft.hearing_impaired = fields.hearing_impaired;
    draft.width = fields.width;
    draft.height = fields.height;
    draft.transfer = fields.transfer;
    draft.mastering = fields.mastering;
    draft.dv = fields.dv;
    draft.channels = fields
        .channels
        .and_then(|channels| u8::try_from(channels).ok())
        .filter(|channels| *channels > 0);
    match kind {
        Kind::Video => video_codec(&fields.codec_id, &fields.private, &mut draft),
        Kind::Audio => audio_codec(&fields.codec_id, &fields.private, &mut draft),
        Kind::Subtitle => draft.codec = subtitle_format(&fields.codec_id),
    }
    MkvTrack {
        number: fields.number,
        strip: fields.strip,
        frames_readable: !fields.unreadable_frames,
        draft,
    }
}

fn video_codec(codec_id: &str, private: &[u8], draft: &mut Draft) {
    let codec = match codec_id {
        "V_MPEG4/ISO/AVC" => {
            let length_size = video::avcc_length_size(private).unwrap_or(4);
            draft.probe = Probe::Nal {
                hevc: false,
                length_size,
            };
            "h264"
        }
        "V_MPEGH/ISO/HEVC" => {
            let length_size = video::scan_hvcc(private, &mut draft.findings).unwrap_or(4);
            draft.probe = Probe::Nal {
                hevc: true,
                length_size,
            };
            "h265"
        }
        "V_AV1" => "av1",
        "V_VP9" => "vp9",
        "V_VP8" => "vp8",
        "V_MPEG2" => "mpeg2",
        "V_MPEG1" => "mpeg1",
        "V_MPEG4/ISO/ASP" | "V_MPEG4/ISO/SP" | "V_MPEG4/ISO/AP" => mpeg4(private, draft),
        "V_MPEG4/MS/V3" => "divx",
        "V_MS/VFW/FOURCC" => match private.get(16..20) {
            Some(b"WVC1" | b"wvc1") => "vc1",
            Some(b"XVID" | b"xvid") => "xvid",
            Some(b"DIVX" | b"divx" | b"DX50" | b"DIV3" | b"DIV4" | b"DIV5") => "divx",
            Some(b"H264" | b"h264" | b"AVC1" | b"avc1" | b"X264" | b"x264") => "h264",
            Some(b"MP4V" | b"mp4v" | b"FMP4" | b"fmp4") => mpeg4(private, draft),
            _ => codec_id,
        },
        _ => codec_id,
    };
    draft.codec = codec.to_string();
}

/// MPEG-4 Part 2: XviD or DivX when the decoder configuration names it, otherwise decided
/// by the first frame.
fn mpeg4(private: &[u8], draft: &mut Draft) -> &'static str {
    match video::mpeg4_vendor(private) {
        Some(vendor) => vendor,
        None => {
            draft.probe = Probe::Mpeg4Part2;
            "mpeg4"
        }
    }
}

fn audio_codec(codec_id: &str, private: &[u8], draft: &mut Draft) {
    let codec = match codec_id {
        "A_AC3" | "A_AC3/BSID9" | "A_AC3/BSID10" => "ac3",
        "A_EAC3" => {
            draft.probe = Probe::Eac3;
            "eac3"
        }
        "A_DTS" => {
            draft.probe = Probe::Dts;
            "dts"
        }
        "A_DTS/LOSSLESS" => "dts-hd ma",
        "A_TRUEHD" => {
            draft.probe = Probe::TrueHd;
            "truehd"
        }
        "A_FLAC" => "flac",
        "A_OPUS" => "opus",
        "A_VORBIS" => "vorbis",
        "A_MPEG/L3" => "mp3",
        "A_MPEG/L2" => "mp2",
        "A_MS/ACM" => match private
            .get(..2)
            .and_then(|tag| tag.try_into().ok())
            .map(u16::from_le_bytes)
        {
            Some(0x0001 | 0x0003 | 0xFFFE) => "pcm",
            Some(0x0055) => "mp3",
            Some(0x2000) => "ac3",
            Some(0x2001) => "dts",
            Some(0x00FF | 0x1610) => "aac",
            _ => codec_id,
        },
        _ if codec_id == "A_AAC" || codec_id.starts_with("A_AAC/") => "aac",
        _ if codec_id.starts_with("A_PCM/") => "pcm",
        _ => codec_id,
    };
    draft.codec = codec.to_string();
}

fn subtitle_format(codec_id: &str) -> String {
    let format = match codec_id {
        "S_TEXT/UTF8" | "S_TEXT/ASCII" => "srt",
        "S_TEXT/ASS" | "S_ASS" => "ass",
        "S_TEXT/SSA" | "S_SSA" => "ssa",
        "S_HDMV/PGS" => "pgs",
        "S_VOBSUB" => "vobsub",
        "S_TEXT/WEBVTT" => "webvtt",
        _ if codec_id.starts_with("D_WEBVTT/") => "webvtt",
        _ => codec_id,
    };
    format.to_string()
}
