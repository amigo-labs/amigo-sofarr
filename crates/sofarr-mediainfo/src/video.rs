//! Video bitstream details: H.264/H.265 NAL units and SEI messages (encoder string, HDR10+),
//! the Dolby Vision configuration record and MPEG-4 Part 2 vendor strings.

use crate::bits::{self, be16};

/// What the first frames and the decoder configuration of a video track show.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Findings {
    /// `x264` or `x265` from a user-data SEI.
    pub(crate) encoder: Option<&'static str>,
    /// An HDR10+ (SMPTE ST 2094-40) dynamic-metadata SEI.
    pub(crate) hdr10_plus: bool,
    /// A Dolby Vision RPU NAL unit (HEVC NAL type 62).
    pub(crate) dv_rpu: bool,
    /// A mastering-display colour-volume SEI.
    pub(crate) mastering: bool,
}

/// The fields of a `DOVIDecoderConfigurationRecord` (`dvcC`/`dvvC`/`dvwC`) used here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DvConfig {
    pub(crate) profile: u8,
    pub(crate) compatibility: u8,
}

impl DvConfig {
    /// Bytes 0–1 hold the version; then dv_profile (7 bits), dv_level (6 bits) and three flags;
    /// then dv_bl_signal_compatibility_id (4 bits).
    pub(crate) fn parse(record: &[u8]) -> Option<Self> {
        let profile = record.get(2)? >> 1;
        let compatibility = record.get(4)? >> 4;
        Some(Self {
            profile,
            compatibility,
        })
    }
}

const SEI_USER_DATA_REGISTERED: u32 = 4;
const SEI_USER_DATA_UNREGISTERED: u32 = 5;
const SEI_MASTERING_DISPLAY: u32 = 137;
const MAX_NAL_UNITS: usize = 512;

/// NAL length size from an `AVCDecoderConfigurationRecord`.
pub(crate) fn avcc_length_size(config: &[u8]) -> Option<usize> {
    if *config.first()? != 1 {
        return None;
    }
    Some(usize::from(config.get(4)? & 0x03) + 1)
}

/// NAL length size from an `HEVCDecoderConfigurationRecord`; SEI NAL units stored in its
/// arrays (some muxers put the encoder SEI there) are scanned into `findings`.
pub(crate) fn scan_hvcc(config: &[u8], findings: &mut Findings) -> Option<usize> {
    if *config.first()? != 1 {
        return None;
    }
    let length_size = usize::from(config.get(21)? & 0x03) + 1;
    let arrays = *config.get(22)?;
    let mut pos = 23usize;
    'arrays: for _ in 0..arrays {
        let Some(count) = be16(config, pos.saturating_add(1)) else {
            break;
        };
        pos = pos.saturating_add(3);
        for _ in 0..count {
            let Some(len) = be16(config, pos) else {
                break 'arrays;
            };
            let start = pos.saturating_add(2);
            let end = start.saturating_add(usize::from(len));
            let Some(nal) = config.get(start..end) else {
                break 'arrays;
            };
            scan_nal(nal, true, findings);
            pos = end;
        }
    }
    Some(length_size)
}

/// Scans length-prefixed NAL units (the sample format of `avcC`/`hvcC` streams).
pub(crate) fn scan_length_prefixed(
    data: &[u8],
    length_size: usize,
    hevc: bool,
    findings: &mut Findings,
) {
    let mut pos = 0usize;
    for _ in 0..MAX_NAL_UNITS {
        let Some(prefix) = pos
            .checked_add(length_size)
            .and_then(|end| data.get(pos..end))
        else {
            return;
        };
        let Some(len) = bits::be_uint(prefix).and_then(|len| usize::try_from(len).ok()) else {
            return;
        };
        let start = pos + length_size;
        let end = start.saturating_add(len);
        let nal = data.get(start..end.min(data.len())).unwrap_or_default();
        if !nal.is_empty() {
            scan_nal(nal, hevc, findings);
        }
        if end >= data.len() || len == 0 {
            return;
        }
        pos = end;
    }
}

fn scan_nal(nal: &[u8], hevc: bool, findings: &mut Findings) {
    let Some(first) = nal.first() else {
        return;
    };
    if hevc {
        match (first >> 1) & 0x3F {
            39 | 40 => scan_sei(&unescape(nal.get(2..).unwrap_or_default()), findings),
            62 => findings.dv_rpu = true,
            _ => {}
        }
    } else if first & 0x1F == 6 {
        scan_sei(&unescape(nal.get(1..).unwrap_or_default()), findings);
    }
}

/// Removes the emulation-prevention bytes (`00 00 03` becomes `00 00`).
pub(crate) fn unescape(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len());
    let mut zeros = 0usize;
    for byte in payload {
        if zeros >= 2 && *byte == 3 {
            zeros = 0;
            continue;
        }
        zeros = if *byte == 0 { zeros + 1 } else { 0 };
        out.push(*byte);
    }
    out
}

/// Reads a SEI payload type or size: a run of 0xFF bytes plus one final byte.
fn sei_number(rbsp: &[u8], pos: &mut usize) -> Option<u32> {
    let mut value = 0u32;
    loop {
        let byte = *rbsp.get(*pos)?;
        *pos += 1;
        value = value.checked_add(u32::from(byte))?;
        if byte != 0xFF {
            return Some(value);
        }
    }
}

fn scan_sei(rbsp: &[u8], findings: &mut Findings) {
    let mut pos = 0usize;
    // More than the trailing-bits byte must remain for another message.
    while rbsp.len().saturating_sub(pos) > 1 {
        let (Some(kind), Some(size)) = (sei_number(rbsp, &mut pos), sei_number(rbsp, &mut pos))
        else {
            return;
        };
        let end = pos.saturating_add(size as usize).min(rbsp.len());
        let payload = rbsp.get(pos..end).unwrap_or_default();
        match kind {
            SEI_USER_DATA_UNREGISTERED => {
                if let Some(encoder) = payload.get(16..).and_then(encoder_in_sei_text) {
                    findings.encoder.get_or_insert(encoder);
                }
            }
            SEI_USER_DATA_REGISTERED if is_hdr10_plus(payload) => findings.hdr10_plus = true,
            SEI_MASTERING_DISPLAY => findings.mastering = true,
            _ => {}
        }
        pos = end;
    }
}

/// ITU-T T.35 with country code 0xB5 (USA), provider code 0x003C, provider-oriented code
/// 0x0001 and application identifier 4: SMPTE ST 2094-40 dynamic metadata.
fn is_hdr10_plus(payload: &[u8]) -> bool {
    payload.first() == Some(&0xB5)
        && be16(payload, 1) == Some(0x003C)
        && be16(payload, 3) == Some(0x0001)
        && payload.get(5) == Some(&4)
}

/// The encoder that wrote a user-data SEI text (`x264 - core …`, `x265 (build …`).
fn encoder_in_sei_text(text: &[u8]) -> Option<&'static str> {
    let head = text.get(..text.len().min(16)).unwrap_or_default();
    let head = String::from_utf8_lossy(head).to_ascii_lowercase();
    let head = head.trim_start_matches(|c: char| c.is_whitespace() || c == '\0');
    if head.starts_with("x264") {
        Some("x264")
    } else if head.starts_with("x265") {
        Some("x265")
    } else {
        None
    }
}

/// The encoder a Matroska `ENCODER` tag names, if it is x264 or x265.
pub(crate) fn encoder_in_tag(text: &str) -> Option<&'static str> {
    let text = text.to_ascii_lowercase();
    if text.contains("x265") {
        Some("x265")
    } else if text.contains("x264") {
        Some("x264")
    } else {
        None
    }
}

/// `xvid` or `divx` from the user-data start code (`00 00 01 B2`) of MPEG-4 Part 2 data.
pub(crate) fn mpeg4_vendor(data: &[u8]) -> Option<&'static str> {
    let marker = [0x00, 0x00, 0x01, 0xB2];
    data.windows(8).find_map(|window| {
        if window.get(..4) != Some(&marker[..]) {
            return None;
        }
        match window.get(4..8) {
            Some(b"XviD") => Some("xvid"),
            Some(b"DivX") => Some("divx"),
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{hdr10_plus_sei, hevc_sei_nal, length_prefixed, x26x_sei};

    #[test]
    fn unescape_removes_emulation_prevention_bytes() {
        assert_eq!(
            unescape(&[0, 0, 3, 1, 0, 0, 3, 0, 0, 3]),
            [0, 0, 1, 0, 0, 0, 0]
        );
        assert_eq!(unescape(&[0, 3, 0, 0, 0, 3]), [0, 3, 0, 0, 0]);
    }

    #[test]
    fn hevc_sei_names_encoder_and_hdr10_plus() {
        let frame = length_prefixed(
            4,
            &[
                hevc_sei_nal(&x26x_sei("x265 (build 199) - 3.5:[Windows][MSVC]")),
                hevc_sei_nal(&hdr10_plus_sei()),
            ],
        );
        let mut findings = Findings::default();
        scan_length_prefixed(&frame, 4, true, &mut findings);
        assert_eq!(findings.encoder, Some("x265"));
        assert!(findings.hdr10_plus);
        assert!(!findings.dv_rpu);
    }

    #[test]
    fn h264_sei_names_x264() {
        let mut nal = vec![0x06];
        nal.extend(x26x_sei("x264 - core 164 r3095"));
        let frame = length_prefixed(4, &[nal]);
        let mut findings = Findings::default();
        scan_length_prefixed(&frame, 4, false, &mut findings);
        assert_eq!(findings.encoder, Some("x264"));
    }

    #[test]
    fn dv_record_and_rpu() {
        // Version 1.0, profile 8, level 6, rpu+bl, compatibility 1.
        let record = [1, 0, (8 << 1), (6 << 3) | 0b101, 0x10, 0, 0, 0];
        assert_eq!(
            DvConfig::parse(&record),
            Some(DvConfig {
                profile: 8,
                compatibility: 1
            })
        );
        let frame = length_prefixed(4, &[vec![62 << 1, 1, 0x19, 0x08]]);
        let mut findings = Findings::default();
        scan_length_prefixed(&frame, 4, true, &mut findings);
        assert!(findings.dv_rpu);
    }

    #[test]
    fn broken_nal_data_is_ignored() {
        let mut findings = Findings::default();
        scan_length_prefixed(&[0xFF, 0xFF, 0xFF, 0xFF, 0x4E], 4, true, &mut findings);
        scan_length_prefixed(&[0, 0, 0, 3, 0x4E, 0x01, 0xFF], 4, true, &mut findings);
        scan_length_prefixed(&[], 4, true, &mut findings);
        assert_eq!(findings, Findings::default());
    }

    #[test]
    fn tags_and_mpeg4_vendors() {
        assert_eq!(encoder_in_tag("Lavc libx265"), Some("x265"));
        assert_eq!(encoder_in_tag("Lavf58.76.100"), None);
        let mut data = vec![0, 0, 1, 0xB2];
        data.extend(b"XviD0050");
        assert_eq!(mpeg4_vendor(&data), Some("xvid"));
        assert_eq!(mpeg4_vendor(b"DivX"), None);
    }
}
