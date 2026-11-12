//! The first frame of each video and audio track that needs one: `Cluster` → `SimpleBlock`
//! or `BlockGroup` → `Block`, with the lacing of RFC 9559 §10.

use std::io::{Read, Seek};

use super::tracks::MkvTrack;
use super::{CLUSTER, CRC32, VOID, header_at, is_level1};
use crate::ebml::{self, Header};
use crate::source::{Res, Source, Stop};
use crate::track::{FRAME_CAP, Probe};

const SIMPLE_BLOCK: u32 = 0xA3;
const BLOCK_GROUP: u32 = 0xA0;
const BLOCK: u32 = 0xA1;
const MAX_CLUSTERS: usize = 64;
const MAX_BLOCKS: usize = 4096;
/// Enough of a block for the track number, timestamp, flags and lace count.
const BLOCK_HEAD: u64 = 12;

struct Scan {
    /// Indexes of the tracks still waiting for their first frame.
    pending: Vec<usize>,
    blocks: usize,
}

impl Scan {
    fn done(&self) -> bool {
        self.pending.is_empty() || self.blocks >= MAX_BLOCKS
    }
}

/// Reads the first frame of every track with a frame probe, starting at the first cluster.
pub(super) fn scan<R: Read + Seek>(
    src: &mut Source<'_, R>,
    first_cluster: u64,
    seg_end: u64,
    tracks: &mut [MkvTrack],
) -> Res<()> {
    let pending = tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| track.frames_readable && track.draft.probe != Probe::None)
        .map(|(index, _)| index)
        .collect();
    let mut scan = Scan { pending, blocks: 0 };
    let mut pos = first_cluster;
    let mut clusters = 0;
    while !scan.done() && clusters < MAX_CLUSTERS && pos < seg_end {
        let header = header_at(src, pos)?;
        if header.id != CLUSTER {
            match header.end()? {
                Some(end) if is_level1(header.id) || header.id == VOID || header.id == CRC32 => {
                    pos = end;
                    continue;
                }
                _ => return Ok(()),
            }
        }
        clusters += 1;
        pos = cluster(src, &header, tracks, &mut scan)?;
    }
    Ok(())
}

/// Scans one cluster; returns the position after it.
fn cluster<R: Read + Seek>(
    src: &mut Source<'_, R>,
    header: &Header,
    tracks: &mut [MkvTrack],
    scan: &mut Scan,
) -> Res<u64> {
    let end = header.end()?;
    let mut pos = header.data;
    loop {
        if end.is_some_and(|end| pos >= end) || pos >= src.len() || scan.done() {
            return Ok(end.unwrap_or(pos));
        }
        let child = header_at(src, pos)?;
        // A cluster of unknown size ends at the next level-1 element.
        if end.is_none() && is_level1(child.id) {
            return Ok(pos);
        }
        let child_end = child.end()?.ok_or(Stop::Damaged)?;
        match child.id {
            SIMPLE_BLOCK => block(src, &child, tracks, scan)?,
            BLOCK_GROUP => {
                let mut inner = child.data;
                while inner < child_end && !scan.done() {
                    let part = header_at(src, inner)?;
                    if part.id == BLOCK {
                        block(src, &part, tracks, scan)?;
                    }
                    inner = part.end()?.ok_or(Stop::Damaged)?;
                }
            }
            _ => {}
        }
        pos = child_end;
    }
}

fn block<R: Read + Seek>(
    src: &mut Source<'_, R>,
    header: &Header,
    tracks: &mut [MkvTrack],
    scan: &mut Scan,
) -> Res<()> {
    scan.blocks += 1;
    let size = header.size.ok_or(Stop::Damaged)?;
    src.seek(header.data);
    let mut data = src.read_vec(size.min(BLOCK_HEAD))?;
    let Some((number, number_len)) = ebml::vint(&data) else {
        return Ok(());
    };
    let Some(slot) = scan
        .pending
        .iter()
        .position(|index| tracks.get(*index).is_some_and(|t| t.number == number))
    else {
        return Ok(());
    };
    let index = scan.pending.remove(slot);
    let wanted = size.min(FRAME_CAP);
    if wanted > BLOCK_HEAD {
        data.extend(src.read_some(wanted - BLOCK_HEAD)?);
    }
    let Some(track) = tracks.get_mut(index) else {
        return Ok(());
    };
    if let Some(frame) = first_frame(&data, number_len, size) {
        let mut bytes = track.strip.clone();
        bytes.extend_from_slice(frame);
        track.draft.analyze_frame(&bytes);
    }
    Ok(())
}

/// The first frame of a block: after the track number, the 16-bit timestamp, the flags and,
/// for laced blocks, the lace sizes. Clamped to the bytes that were read.
fn first_frame(data: &[u8], number_len: usize, block_size: u64) -> Option<&[u8]> {
    let flags = *data.get(number_len + 2)?;
    let mut pos = number_len + 3;
    let lacing = (flags >> 1) & 0x03;
    if lacing == 0 {
        return data.get(pos..);
    }
    let count = usize::from(*data.get(pos)?) + 1;
    pos += 1;
    let first_size = match lacing {
        // Xiph lacing: each size is a run of 255s plus a final byte.
        1 => {
            let mut sizes = Vec::with_capacity(count - 1);
            for _ in 1..count {
                let mut size = 0usize;
                loop {
                    let byte = *data.get(pos)?;
                    pos += 1;
                    size += usize::from(byte);
                    if byte != 255 {
                        break;
                    }
                }
                sizes.push(size);
            }
            *sizes.first()?
        }
        // Fixed-size lacing: all frames share the rest of the block.
        2 => {
            let rest = usize::try_from(block_size).ok()?.checked_sub(pos)?;
            rest / count
        }
        // EBML lacing: the first size, then signed differences for the others.
        _ => {
            let (first, len) = ebml::vint(data.get(pos..)?)?;
            pos += len;
            for _ in 2..count {
                let (_, len) = ebml::vint(data.get(pos..)?)?;
                pos += len;
            }
            usize::try_from(first).ok()?
        }
    };
    let end = pos.saturating_add(first_size).min(data.len());
    data.get(pos..end)
}

#[cfg(test)]
mod tests {
    use super::first_frame;

    #[test]
    fn lacing_variants_yield_the_first_frame() {
        // Track 1, timestamp 0, no lacing.
        let plain = [0x81, 0, 0, 0x80, 1, 2, 3];
        assert_eq!(first_frame(&plain, 1, 7), Some(&[1u8, 2, 3][..]));
        // Xiph lacing, three frames of 2, 1 and 1 bytes.
        let xiph = [0x81, 0, 0, 0x82, 2, 2, 1, 9, 9, 8, 7];
        assert_eq!(first_frame(&xiph, 1, 11), Some(&[9u8, 9][..]));
        // Fixed lacing, two frames of 2 bytes.
        let fixed = [0x81, 0, 0, 0x84, 1, 5, 5, 6, 6];
        assert_eq!(first_frame(&fixed, 1, 9), Some(&[5u8, 5][..]));
        // EBML lacing, three frames: first 2 bytes, then a difference.
        let ebml = [0x81, 0, 0, 0x86, 2, 0x82, 0xBF, 4, 4, 5, 6];
        assert_eq!(first_frame(&ebml, 1, 11), Some(&[4u8, 4][..]));
        assert_eq!(first_frame(&[0x81, 0], 1, 2), None);
    }
}
