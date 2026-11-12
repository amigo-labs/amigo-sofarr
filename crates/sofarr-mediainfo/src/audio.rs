//! Audio bitstream details: AC-3/E-AC-3 configuration boxes and syncframes (ETSI TS 102 366),
//! the TrueHD major sync and DTS core and extension substream headers (ETSI TS 102 114).

use crate::bits::{self, BitReader, be16, be32};

/// Channels (without LFE) per AC-3 `acmod`.
fn acmod_channels(acmod: u32) -> u8 {
    match acmod {
        1 => 1,
        0 | 2 => 2,
        3 | 4 => 3,
        5 | 6 => 4,
        _ => 5,
    }
}

/// Channel count from a `dac3` box (Annex F.4): fscod 2, bsid 5, bsmod 3, acmod 3, lfeon 1.
pub(crate) fn dac3_channels(data: &[u8]) -> Option<u8> {
    let mut bits = BitReader::new(data);
    bits.skip(10)?;
    let acmod = bits.read(3)?;
    let lfe = bits.read(1)?;
    Some(acmod_channels(acmod) + lfe as u8)
}

/// What a `dec3` box (Annex F.6) shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Dec3 {
    pub(crate) channels: u8,
    /// `flag_ec3_extension_type_a`: Joint Object Coding (Dolby Atmos).
    pub(crate) joc: bool,
}

/// Channels of the `chan_loc` bits of a dependent substream; bit 0 is the most significant.
fn chan_loc_channels(chan_loc: u32) -> u8 {
    // Pairs: Lc/Rc, Lrs/Rrs, Lsd/Rsd, Lw/Rw, Lvh/Rvh; singles: Cs, Ts, Cvh, LFE2.
    let pairs = [0x100, 0x080, 0x010, 0x008, 0x004];
    let singles = [0x040, 0x020, 0x002, 0x001];
    let count = |masks: &[u32]| masks.iter().filter(|mask| chan_loc & **mask != 0).count();
    (count(&pairs) * 2 + count(&singles)) as u8
}

pub(crate) fn parse_dec3(data: &[u8]) -> Option<Dec3> {
    let mut bits = BitReader::new(data);
    bits.skip(13)?;
    let substreams = bits.read(3)? + 1;
    let mut channels = 0u8;
    for index in 0..substreams {
        bits.skip(2 + 5 + 1 + 1 + 3)?;
        let acmod = bits.read(3)?;
        let lfe = bits.read(1)?;
        bits.skip(3)?;
        let dependents = bits.read(4)?;
        let extra = if dependents > 0 {
            chan_loc_channels(bits.read(9)?)
        } else {
            bits.skip(1)?;
            0
        };
        // Further independent substreams carry other programs; the first one is the main mix.
        if index == 0 {
            channels = acmod_channels(acmod) + lfe as u8 + extra;
        }
    }
    let joc = match bits.read(8) {
        Some(byte) => byte & 1 == 1,
        None => false,
    };
    Some(Dec3 { channels, joc })
}

/// The channels of an AC-3 syncframe: `acmod` plus `lfeon` (ETSI TS 102 366, 5.4.2).
pub(crate) fn ac3_channels(frame: &[u8]) -> Option<u8> {
    if frame.get(..2)? != [0x0B, 0x77] {
        return None;
    }
    let mut bits = BitReader::new(frame.get(2..)?);
    bits.skip(16 + 2 + 6)?;
    // bsid up to 8 is AC-3, 9 and 10 its reduced-rate variants; above is E-AC-3.
    if bits.read(5)? > 10 {
        return None;
    }
    bits.skip(3)?;
    let acmod = bits.read(3)?;
    if acmod & 1 == 1 && acmod != 1 {
        bits.skip(2)?;
    }
    if acmod & 4 != 0 {
        bits.skip(2)?;
    }
    if acmod == 2 {
        bits.skip(2)?;
    }
    let lfe = bits.read(1)?;
    Some(acmod_channels(acmod) + lfe as u8)
}

/// Whether the first E-AC-3 syncframe of `frame` signals Joint Object Coding in its
/// additional bit stream information; `None` when `frame` starts with no E-AC-3 syncframe.
pub(crate) fn eac3_joc(frame: &[u8]) -> Option<bool> {
    if frame.get(..2)? != [0x0B, 0x77] {
        return None;
    }
    let mut bits = BitReader::new(frame.get(2..)?);
    let stream_type = bits.read(2)?;
    bits.skip(3 + 11)?;
    let fscod = bits.read(2)?;
    let numblkscod = if fscod == 3 {
        bits.skip(2)?;
        3
    } else {
        bits.read(2)?
    };
    let acmod = bits.read(3)?;
    let lfe = bits.flag()?;
    let bsid = bits.read(5)?;
    if bsid <= 10 {
        // An AC-3 syncframe: no additional bit stream information of this kind.
        return Some(false);
    }
    if bsid > 16 {
        return None;
    }
    bits.skip(5)?;
    skip_optional(&mut bits, 8)?;
    if acmod == 0 {
        bits.skip(5)?;
        skip_optional(&mut bits, 8)?;
    }
    if stream_type == 1 {
        skip_optional(&mut bits, 16)?;
    }
    if bits.flag()? {
        skip_mix_metadata(&mut bits, stream_type, acmod, lfe, numblkscod)?;
    }
    if bits.flag()? {
        skip_info_metadata(&mut bits, acmod, fscod)?;
    }
    if stream_type == 0 && numblkscod != 3 {
        bits.skip(1)?;
    }
    if stream_type == 2 {
        let blkid = if numblkscod == 3 { true } else { bits.flag()? };
        if blkid {
            bits.skip(6)?;
        }
    }
    if !bits.flag()? {
        return Some(false);
    }
    bits.skip(6)?;
    bits.flag()
}

/// A flag followed by `len` bits when it is set.
fn skip_optional(bits: &mut BitReader<'_>, len: usize) -> Option<()> {
    if bits.flag()? {
        bits.skip(len)?;
    }
    Some(())
}

fn skip_mix_metadata(
    bits: &mut BitReader<'_>,
    stream_type: u32,
    acmod: u32,
    lfe: bool,
    numblkscod: u32,
) -> Option<()> {
    if acmod > 2 {
        bits.skip(2)?;
    }
    if acmod & 1 == 1 && acmod > 2 {
        bits.skip(6)?;
    }
    if acmod & 4 == 4 {
        bits.skip(6)?;
    }
    if lfe {
        skip_optional(bits, 5)?;
    }
    if stream_type != 0 {
        return Some(());
    }
    skip_optional(bits, 6)?;
    if acmod == 0 {
        skip_optional(bits, 6)?;
    }
    skip_optional(bits, 6)?;
    match bits.read(2)? {
        1 => bits.skip(5)?,
        2 => bits.skip(12)?,
        3 => {
            let len = bits.read(5)? as usize;
            bits.skip((len + 2) * 8)?;
        }
        _ => {}
    }
    if acmod < 2 {
        skip_optional(bits, 14)?;
        if acmod == 0 {
            skip_optional(bits, 14)?;
        }
    }
    if bits.flag()? {
        if numblkscod == 0 {
            bits.skip(5)?;
        } else {
            let blocks = match numblkscod {
                1 => 2,
                2 => 3,
                _ => 6,
            };
            for _ in 0..blocks {
                skip_optional(bits, 5)?;
            }
        }
    }
    Some(())
}

fn skip_info_metadata(bits: &mut BitReader<'_>, acmod: u32, fscod: u32) -> Option<()> {
    bits.skip(3 + 1 + 1)?;
    if acmod == 2 {
        bits.skip(4)?;
    }
    if acmod >= 6 {
        bits.skip(2)?;
    }
    skip_optional(bits, 8)?;
    if acmod == 0 {
        skip_optional(bits, 8)?;
    }
    if fscod < 3 {
        bits.skip(1)?;
    }
    Some(())
}

/// Whether a TrueHD access unit with a major sync carries a fourth substream, the one with
/// the 16-channel (object) presentation of Dolby Atmos; `None` without a TrueHD major sync.
pub(crate) fn truehd_atmos(frame: &[u8]) -> Option<bool> {
    if be32(frame, 4)? != 0xF872_6FBA || be16(frame, 12)? != 0xB752 {
        return None;
    }
    let substreams = frame.get(20)? >> 4;
    Some(substreams >= 4)
}

const DTS_CORE_SYNC: u32 = 0x7FFE_8001;
const DTS_EXT_SYNC: u32 = 0x6458_2025;
const DTS_XLL_SYNC: [u8; 4] = [0x41, 0xA2, 0x95, 0x47];
const DTS_XBR_SYNC: [u8; 4] = [0x65, 0x5E, 0x31, 0x5E];

/// The DTS profile of a frame: `dts`, `dts-es` (core with a channel extension), `dts-hd hra`
/// (extension substream with XBR) or `dts-hd ma` (extension substream with XLL).
pub(crate) fn dts_profile(frame: &[u8]) -> Option<&'static str> {
    let mut profile = None;
    let mut rest = frame;
    if be32(frame, 0)? == DTS_CORE_SYNC {
        let mut bits = BitReader::new(frame);
        bits.skip(32 + 1 + 5 + 1 + 7)?;
        let core_size = bits.read(14)? as usize + 1;
        bits.skip(6 + 4 + 5 + 1 + 1 + 1 + 1 + 1)?;
        let extension_id = bits.read(3)?;
        let extension = bits.flag()?;
        profile = Some(if extension && matches!(extension_id, 0 | 3 | 6) {
            "dts-es"
        } else {
            "dts"
        });
        rest = frame.get(core_size..).unwrap_or_default();
    }
    if be32(rest, 0) == Some(DTS_EXT_SYNC)
        && let Some(hd) = extension_substream_profile(rest)
    {
        return Some(hd);
    }
    profile
}

/// The channels of a DTS core frame: AMODE plus the LFE (ETSI TS 102 114, 5.3.1).
pub(crate) fn dts_core_channels(frame: &[u8]) -> Option<u8> {
    if be32(frame, 0)? != DTS_CORE_SYNC {
        return None;
    }
    let mut bits = BitReader::new(frame);
    bits.skip(32 + 1 + 5 + 1 + 7 + 14)?;
    let amode = bits.read(6)?;
    bits.skip(4 + 5 + 1 + 1 + 1 + 1 + 1 + 3 + 1 + 1)?;
    let lfe = bits.read(2)?;
    let channels: u8 = match amode {
        0 => 1,
        1..=4 => 2,
        5 | 6 => 3,
        7 | 8 => 4,
        9 => 5,
        10..=12 => 6,
        13 => 7,
        14 | 15 => 8,
        // User-defined layouts say nothing about the channel count.
        _ => return None,
    };
    Some(channels + u8::from(matches!(lfe, 1 | 2)))
}

fn extension_substream_profile(data: &[u8]) -> Option<&'static str> {
    let mut bits = BitReader::new(data);
    bits.skip(32 + 8 + 2)?;
    let (header_bits, size_bits) = if bits.flag()? { (12, 20) } else { (8, 16) };
    let header_size = bits.read(header_bits)? as usize + 1;
    let frame_size = bits.read(size_bits)? as usize + 1;
    let assets = data.get(header_size..frame_size.min(data.len()))?;
    if bits::contains(assets, &DTS_XLL_SYNC) {
        Some("dts-hd ma")
    } else if bits::contains(assets, &DTS_XBR_SYNC) {
        Some("dts-hd hra")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{ac3_frame, dts_core, dts_frame, eac3_frame, truehd_frame};

    #[test]
    fn dac3_and_dec3_channels() {
        // fscod 0, bsid 8, bsmod 0, acmod 7, lfeon 1.
        let dac3 = [0b0001_0000, 0b0011_1100, 0];
        assert_eq!(dac3_channels(&dac3), Some(6));
        // data_rate 640, one substream: acmod 7, lfe 1, no dependents, then the JOC flag byte.
        let dec3 = [0x14, 0x00, 0x20, 0x0F, 0x00, 0x01, 0x10];
        assert_eq!(
            parse_dec3(&dec3),
            Some(Dec3 {
                channels: 6,
                joc: true
            })
        );
        assert_eq!(
            parse_dec3(&dec3[..5]),
            Some(Dec3 {
                channels: 6,
                joc: false
            })
        );
        // One dependent substream with Lrs/Rrs: 7.1.
        let dec3 = [0x14, 0x00, 0x20, 0x0F, 0x02, 0x80, 0x00];
        assert_eq!(parse_dec3(&dec3).map(|d| d.channels), Some(8));
    }

    #[test]
    fn eac3_joc_in_additional_bsi() {
        assert_eq!(eac3_joc(&eac3_frame(true, false)), Some(true));
        assert_eq!(eac3_joc(&eac3_frame(false, false)), Some(false));
        assert_eq!(eac3_joc(&eac3_frame(true, true)), Some(true));
        assert_eq!(eac3_joc(&eac3_frame(false, true)), Some(false));
        assert_eq!(eac3_joc(&[0x0B, 0x77]), None);
        assert_eq!(eac3_joc(b"nope"), None);
    }

    #[test]
    fn truehd_substreams() {
        assert_eq!(truehd_atmos(&truehd_frame(4)), Some(true));
        assert_eq!(truehd_atmos(&truehd_frame(3)), Some(false));
        assert_eq!(truehd_atmos(&[0; 30]), None);
    }

    #[test]
    fn dts_profiles() {
        assert_eq!(dts_profile(&dts_frame(false, None)), Some("dts"));
        assert_eq!(dts_profile(&dts_frame(true, None)), Some("dts-es"));
        assert_eq!(
            dts_profile(&dts_frame(false, Some(DTS_XLL_SYNC))),
            Some("dts-hd ma")
        );
        assert_eq!(
            dts_profile(&dts_frame(false, Some(DTS_XBR_SYNC))),
            Some("dts-hd hra")
        );
        assert_eq!(dts_profile(&[0x7F, 0xFE, 0x80, 0x01]), None);
    }

    #[test]
    fn ac3_channels_from_the_syncframe() {
        assert_eq!(ac3_channels(&ac3_frame(2, false)), Some(2));
        assert_eq!(ac3_channels(&ac3_frame(7, true)), Some(6));
        assert_eq!(ac3_channels(&ac3_frame(1, false)), Some(1));
        assert_eq!(ac3_channels(&ac3_frame(3, true)), Some(4));
        // E-AC-3 (bsid 16) and anything without a syncword are not AC-3.
        assert_eq!(ac3_channels(&eac3_frame(false, false)), None);
        assert_eq!(ac3_channels(&[0; 16]), None);
    }

    #[test]
    fn dts_core_channels_count_the_lfe() {
        assert_eq!(dts_core_channels(&dts_core(false, false)), Some(5));
        assert_eq!(dts_core_channels(&dts_core(false, true)), Some(6));
        assert_eq!(dts_core_channels(&[0x7F, 0xFE, 0x80, 0x01]), None);
        assert_eq!(dts_core_channels(&[0; 16]), None);
    }
}
