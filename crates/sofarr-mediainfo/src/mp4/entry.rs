//! `stsd` sample entries (ISO/IEC 14496-12 §12, 14496-14, 14496-15, ETSI TS 102 366 Annex F,
//! the Dolby Vision ISO BMFF mapping) and the codec names derived from them.

use super::Boxes;
use crate::audio;
use crate::bits::{be16, be32};
use crate::source::Res;
use crate::track::{Draft, Kind, Probe};
use crate::video::{self, DvConfig};

/// Size of the fields of a `VisualSampleEntry` before its child boxes.
const VISUAL_FIELDS: usize = 78;

/// Fills `draft` from the first sample entry of an `stsd` box.
pub(super) fn parse_stsd(stsd: &[u8], draft: &mut Draft) -> Res<()> {
    let Some(entries) = stsd.get(8..) else {
        return Ok(());
    };
    let Some(first) = Boxes::new(entries).next() else {
        return Ok(());
    };
    let (format, body) = first?;
    match draft.kind {
        Kind::Video => visual(format, body, draft),
        Kind::Audio => sound(format, body, draft),
        Kind::Subtitle => subtitle(format, body, draft),
    }
    Ok(())
}

/// The original format of a protected sample entry (`sinf` → `frma`).
fn original_format(sinf: &[u8]) -> Option<[u8; 4]> {
    Boxes::new(sinf).flatten().find_map(|(kind, body)| {
        (&kind == b"frma")
            .then(|| body.get(..4).and_then(|format| format.try_into().ok()))
            .flatten()
    })
}

fn fourcc(format: [u8; 4]) -> String {
    String::from_utf8_lossy(&format).trim().to_string()
}

fn visual(mut format: [u8; 4], body: &[u8], draft: &mut Draft) {
    draft.width = be16(body, 24).map(u32::from).filter(|width| *width > 0);
    draft.height = be16(body, 26).map(u32::from).filter(|height| *height > 0);
    let mut object_type = None;
    let mut probe = Probe::None;
    for (kind, child) in Boxes::new(body.get(VISUAL_FIELDS..).unwrap_or_default()).flatten() {
        match &kind {
            b"avcC" => {
                let length_size = video::avcc_length_size(child).unwrap_or(4);
                probe = Probe::Nal {
                    hevc: false,
                    length_size,
                };
            }
            b"hvcC" => {
                let length_size = video::scan_hvcc(child, &mut draft.findings).unwrap_or(4);
                probe = Probe::Nal {
                    hevc: true,
                    length_size,
                };
            }
            b"dvcC" | b"dvvC" | b"dvwC" => draft.dv = DvConfig::parse(child),
            b"colr" => {
                if matches!(child.get(..4), Some(b"nclx" | b"nclc")) {
                    draft.transfer = be16(child, 6).map(u64::from);
                }
            }
            b"mdcv" | b"SmDm" => draft.mastering = true,
            b"esds" => object_type = es_descriptor(child).map(|es| es.object_type),
            b"sinf" => format = original_format(child).unwrap_or(format),
            _ => {}
        }
    }
    let codec = match &format {
        b"avc1" | b"avc3" => "h264",
        b"hvc1" | b"hev1" => "h265",
        b"dvh1" | b"dvhe" | b"dva1" | b"dvav" | b"dav1" => {
            // Dolby Vision sample entries; the configuration box, if any, adds the profile.
            draft.findings.dv_rpu = true;
            match &format {
                b"dva1" | b"dvav" => "h264",
                b"dav1" => "av1",
                _ => "h265",
            }
        }
        b"av01" => "av1",
        b"vp09" => "vp9",
        b"vp08" => "vp8",
        b"mp2v" | b"m2v1" => "mpeg2",
        b"vc-1" => "vc1",
        b"xvid" | b"XVID" => "xvid",
        b"divx" | b"DIVX" | b"DX50" => "divx",
        b"mp4v" => match object_type {
            Some(0x60..=0x65) => "mpeg2",
            Some(0x6A) => "mpeg1",
            Some(0x21) => "h264",
            _ => {
                probe = Probe::Mpeg4Part2;
                "mpeg4"
            }
        },
        _ => "",
    };
    draft.codec = if codec.is_empty() {
        fourcc(format)
    } else {
        codec.to_string()
    };
    if matches!(draft.codec.as_str(), "h264" | "h265" | "mpeg4") {
        draft.probe = probe;
    }
}

fn sound(mut format: [u8; 4], body: &[u8], draft: &mut Draft) {
    // QuickTime sound descriptions version 1 and 2 have more fields before the boxes.
    let (channels, boxes_at) = match be16(body, 8) {
        Some(1) => (be16(body, 16).map(u32::from), 44),
        Some(2) => (be32(body, 40), 64),
        _ => (be16(body, 16).map(u32::from), 28),
    };
    draft.channels = channels
        .and_then(|channels| u8::try_from(channels).ok())
        .filter(|channels| *channels > 0);
    let mut es = None;
    for (kind, child) in Boxes::new(body.get(boxes_at..).unwrap_or_default()).flatten() {
        match &kind {
            b"esds" => es = es_descriptor(child),
            b"wave" => {
                for (inner, value) in Boxes::new(child).flatten() {
                    if &inner == b"esds" {
                        es = es_descriptor(value);
                    }
                }
            }
            b"dac3" => draft.channels = audio::dac3_channels(child).or(draft.channels),
            b"dec3" => {
                if let Some(dec3) = audio::parse_dec3(child) {
                    draft.channels = Some(dec3.channels);
                    draft.object_audio = dec3.joc;
                }
            }
            b"dOps" => draft.channels = child.get(1).copied().or(draft.channels),
            b"sinf" => format = original_format(child).unwrap_or(format),
            _ => {}
        }
    }
    let codec = match &format {
        b"mp4a" => {
            let object_type = es.as_ref().map(|es| es.object_type);
            if matches!(object_type, Some(0x40 | 0x66 | 0x67 | 0x68))
                && let Some(channels) = es.as_ref().and_then(|es| aac_channels(&es.config))
            {
                draft.channels = Some(channels);
            }
            match object_type {
                Some(0x40 | 0x66 | 0x67 | 0x68) => "aac",
                Some(0x69 | 0x6B) => "mp3",
                Some(0xA5) => "ac3",
                Some(0xA6) => "eac3",
                Some(0xA9) => "dts",
                Some(0xAD) => "opus",
                Some(0xDD) => "vorbis",
                _ => "",
            }
        }
        b"ac-3" => "ac3",
        b"ec-3" => {
            draft.probe = Probe::Eac3;
            "eac3"
        }
        b"Opus" => "opus",
        b"fLaC" => "flac",
        b"mlpa" => {
            draft.probe = Probe::TrueHd;
            "truehd"
        }
        b"dtsc" | b"dtsh" => {
            draft.probe = Probe::Dts;
            "dts"
        }
        b"dtsl" => "dts-hd ma",
        b"dtsx" => "dts-x",
        b".mp3" => "mp3",
        b"lpcm" | b"sowt" | b"twos" | b"in24" | b"in32" | b"fl32" | b"fl64" | b"ipcm" | b"fpcm"
        | b"raw " => "pcm",
        _ => "",
    };
    draft.codec = if codec.is_empty() {
        fourcc(format)
    } else {
        codec.to_string()
    };
}

fn subtitle(format: [u8; 4], body: &[u8], draft: &mut Draft) {
    let codec = match &format {
        b"tx3g" => {
            // displayFlags 0x80000000: all samples are forced.
            draft.forced = be32(body, 8).is_some_and(|flags| flags & 0x8000_0000 != 0);
            "tx3g"
        }
        b"wvtt" => "webvtt",
        b"mp4s" => {
            let object_type = Boxes::new(body.get(8..).unwrap_or_default())
                .flatten()
                .find(|(kind, _)| kind == b"esds")
                .and_then(|(_, esds)| es_descriptor(esds))
                .map(|es| es.object_type);
            if object_type == Some(0xE0) {
                "vobsub"
            } else {
                ""
            }
        }
        _ => "",
    };
    draft.codec = if codec.is_empty() {
        fourcc(format)
    } else {
        codec.to_string()
    };
}

/// What an `esds` box (ISO/IEC 14496-1 ES_Descriptor) holds.
struct EsDescriptor {
    object_type: u8,
    /// DecoderSpecificInfo, e.g. the AAC AudioSpecificConfig.
    config: Vec<u8>,
}

/// A descriptor tag and its expandable size (up to four 7-bit groups).
fn descriptor(data: &[u8], pos: &mut usize) -> Option<(u8, usize)> {
    let tag = *data.get(*pos)?;
    *pos += 1;
    let mut size = 0usize;
    for _ in 0..4 {
        let byte = *data.get(*pos)?;
        *pos += 1;
        size = (size << 7) | usize::from(byte & 0x7F);
        if byte & 0x80 == 0 {
            break;
        }
    }
    Some((tag, size))
}

fn es_descriptor(esds: &[u8]) -> Option<EsDescriptor> {
    // Full box: version and flags.
    let mut pos = 4;
    let (tag, _) = descriptor(esds, &mut pos)?;
    if tag != 0x03 {
        return None;
    }
    pos += 2;
    let flags = *esds.get(pos)?;
    pos += 1;
    if flags & 0x80 != 0 {
        pos += 2;
    }
    if flags & 0x40 != 0 {
        pos += usize::from(*esds.get(pos)?) + 1;
    }
    if flags & 0x20 != 0 {
        pos += 2;
    }
    let (tag, _) = descriptor(esds, &mut pos)?;
    if tag != 0x04 {
        return None;
    }
    let object_type = *esds.get(pos)?;
    // objectTypeIndication, streamType, bufferSizeDB, maxBitrate, avgBitrate.
    pos += 13;
    let config = match descriptor(esds, &mut pos) {
        Some((0x05, size)) => esds
            .get(pos..pos.saturating_add(size).min(esds.len()))
            .unwrap_or_default()
            .to_vec(),
        _ => Vec::new(),
    };
    Some(EsDescriptor {
        object_type,
        config,
    })
}

/// Channels from the channelConfiguration of an AAC AudioSpecificConfig.
fn aac_channels(config: &[u8]) -> Option<u8> {
    let mut bits = crate::bits::BitReader::new(config);
    if bits.read(5)? == 31 {
        bits.skip(6)?;
    }
    if bits.read(4)? == 15 {
        bits.skip(24)?;
    }
    match bits.read(4)? {
        channels @ 1..=6 => Some(channels as u8),
        7 => Some(8),
        _ => None,
    }
}
