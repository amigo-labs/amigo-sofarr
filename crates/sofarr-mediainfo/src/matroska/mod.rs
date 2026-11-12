//! Matroska and WebM (RFC 9559): the EBML header, the level-1 elements of the `Segment`,
//! `SeekHead` pointers to `Tracks` and `Tags`, then the first frames of the first clusters.

mod frames;
mod tracks;

#[cfg(test)]
mod tests;

use std::io::{Read, Seek};

use crate::ebml::{self, Children, Header};
use crate::source::{Res, Source, Stop};
use crate::track::{self, Draft};
use crate::{Container, MediaInfo, ReadStatus, video};

const EBML_HEADER: u32 = 0x1A45_DFA3;
const DOC_TYPE: u32 = 0x4282;
const SEGMENT: u32 = 0x1853_8067;
const SEEK_HEAD: u32 = 0x114D_9B74;
const SEEK: u32 = 0x4DBB;
const SEEK_ID: u32 = 0x53AB;
const SEEK_POSITION: u32 = 0x53AC;
const INFO: u32 = 0x1549_A966;
const TRACKS: u32 = 0x1654_AE6B;
const TAGS: u32 = 0x1254_C367;
const TAG: u32 = 0x7373;
const SIMPLE_TAG: u32 = 0x67C8;
const TAG_NAME: u32 = 0x45A3;
const TAG_STRING: u32 = 0x4487;
const CLUSTER: u32 = 0x1F43_B675;
const CUES: u32 = 0x1C53_BB6B;
const CHAPTERS: u32 = 0x1043_A770;
const ATTACHMENTS: u32 = 0x1941_A469;
const VOID: u32 = 0xEC;
const CRC32: u32 = 0xBF;

const MAX_EBML_HEADER: u64 = 64 * 1024;
const MAX_SEEK_HEAD: u64 = 1024 * 1024;
const MAX_TAGS: u64 = 4 * 1024 * 1024;
/// Elements tolerated between the EBML header and the `Segment`.
const MAX_LEADING_ELEMENTS: usize = 16;
const MAX_SEEK_HEADS: usize = 4;

fn is_level1(id: u32) -> bool {
    matches!(
        id,
        SEEK_HEAD | INFO | TRACKS | TAGS | CLUSTER | CUES | CHAPTERS | ATTACHMENTS | SEGMENT
    )
}

#[derive(Debug, Default)]
struct Walk {
    seg_start: u64,
    /// The declared end of the `Segment`, `None` for an unknown size.
    seg_declared_end: Option<u64>,
    tracks: Option<Vec<tracks::MkvTrack>>,
    tags_read: bool,
    tag_encoder: Option<&'static str>,
    seek_tracks: Option<u64>,
    seek_tags: Option<u64>,
    seek_cluster: Option<u64>,
    seek_heads: Vec<u64>,
    parsed_heads: Vec<u64>,
    first_cluster: Option<u64>,
}

impl Walk {
    fn seg_end(&self, len: u64) -> u64 {
        self.seg_declared_end.unwrap_or(len)
    }
}

pub(crate) fn read<R: Read + Seek>(src: &mut Source<'_, R>) -> MediaInfo {
    let container = match doc_type(src) {
        Ok(Some(container)) => container,
        Ok(None) => return track::empty(ReadStatus::Unsupported, None),
        Err(stop) => return track::empty(track::status_of(stop), None),
    };
    let mut walk = Walk::default();
    let error = segment(src, &mut walk).err();
    let Some(mut entries) = walk.tracks.take() else {
        let status = error.map_or(ReadStatus::Partial, track::status_of);
        return track::empty(status, Some(container));
    };
    if let Some(first) = walk.first_cluster.or(walk.seek_cluster) {
        let seg_end = walk.seg_end(src.len());
        // The frames only add details; failing to read them leaves the headers complete.
        let _ = frames::scan(src, first, seg_end, &mut entries);
    }
    let drafts: Vec<Draft> = entries.into_iter().map(|entry| entry.draft).collect();
    track::finish(
        ReadStatus::Complete,
        Some(container),
        drafts,
        walk.tag_encoder,
    )
}

/// The container named by the DocType of the EBML header; `None` for another document type.
fn doc_type<R: Read + Seek>(src: &mut Source<'_, R>) -> Res<Option<Container>> {
    let header = ebml::read_header(src)?;
    if header.id != EBML_HEADER {
        return Err(Stop::Damaged);
    }
    let size = header.size.ok_or(Stop::Damaged)?;
    if size > MAX_EBML_HEADER {
        return Err(Stop::Damaged);
    }
    let data = src.read_vec(size)?;
    let mut doc_type = None;
    for child in Children::new(&data) {
        let (id, body) = child?;
        if id == DOC_TYPE {
            doc_type = Some(ebml::string(body));
        }
    }
    // A missing DocType means Matroska (its default value).
    Ok(match doc_type.as_deref().unwrap_or("matroska") {
        "matroska" => Some(Container::Matroska),
        "webm" => Some(Container::Webm),
        _ => None,
    })
}

fn header_at<R: Read + Seek>(src: &mut Source<'_, R>, pos: u64) -> Res<Header> {
    if pos >= src.len() {
        return Err(Stop::Truncated);
    }
    src.seek(pos);
    ebml::read_header(src)
}

fn payload<R: Read + Seek>(src: &mut Source<'_, R>, header: &Header, cap: u64) -> Res<Vec<u8>> {
    let size = header.size.ok_or(Stop::Damaged)?;
    if size > cap {
        return Err(Stop::Limit);
    }
    src.seek(header.data);
    src.read_vec(size)
}

/// Finds the `Segment`, walks its level-1 elements and follows `SeekHead` pointers.
fn segment<R: Read + Seek>(src: &mut Source<'_, R>, walk: &mut Walk) -> Res<()> {
    let mut pos = src.pos();
    let mut leading = 0;
    let seg = loop {
        let header = header_at(src, pos)?;
        if header.id == SEGMENT {
            break header;
        }
        leading += 1;
        if leading > MAX_LEADING_ELEMENTS {
            return Err(Stop::Damaged);
        }
        pos = header.end()?.ok_or(Stop::Damaged)?;
    };
    walk.seg_start = seg.data;
    walk.seg_declared_end = seg.end()?;
    let walked = walk_level1(src, walk);
    if walk.tracks.is_none() {
        let followed = follow_seek_heads(src, walk);
        if walk.tracks.is_none() {
            return walked.and(followed);
        }
    }
    if !walk.tags_read
        && let Some(pos) = walk.seek_tags
    {
        // Tags only add the encoder fallback.
        let _ = header_at(src, pos).and_then(|header| read_tags(src, walk, &header));
    }
    Ok(())
}

fn walk_level1<R: Read + Seek>(src: &mut Source<'_, R>, walk: &mut Walk) -> Res<()> {
    let seg_end = walk.seg_end(src.len());
    let mut pos = walk.seg_start;
    while pos < seg_end {
        let header = header_at(src, pos)?;
        let end = header.end()?;
        if let (Some(end), Some(declared)) = (end, walk.seg_declared_end)
            && end > declared
        {
            return Err(Stop::Damaged);
        }
        match header.id {
            SEEK_HEAD => {
                walk.parsed_heads.push(header.start);
                if let Ok(data) = payload(src, &header, MAX_SEEK_HEAD) {
                    parse_seek_head(&data, walk);
                }
            }
            TRACKS if walk.tracks.is_none() => {
                let data = payload(src, &header, u64::MAX)?;
                walk.tracks = Some(tracks::parse(&data)?);
            }
            TAGS => read_tags(src, walk, &header)?,
            CLUSTER => {
                walk.first_cluster.get_or_insert(header.start);
                if walk.tracks.is_some() || walk.seek_tracks.is_some() {
                    return Ok(());
                }
            }
            _ => {}
        }
        pos = match end {
            Some(end) => end,
            None if header.id == CLUSTER => return Ok(()),
            None => return Err(Stop::Damaged),
        };
    }
    Ok(())
}

fn follow_seek_heads<R: Read + Seek>(src: &mut Source<'_, R>, walk: &mut Walk) -> Res<()> {
    for _ in 0..MAX_SEEK_HEADS {
        let Some(pos) = walk
            .seek_heads
            .iter()
            .copied()
            .find(|pos| !walk.parsed_heads.contains(pos))
        else {
            break;
        };
        walk.parsed_heads.push(pos);
        let header = header_at(src, pos)?;
        if header.id != SEEK_HEAD {
            return Err(Stop::Damaged);
        }
        let data = payload(src, &header, MAX_SEEK_HEAD)?;
        parse_seek_head(&data, walk);
    }
    let Some(pos) = walk.seek_tracks else {
        return Ok(());
    };
    let header = header_at(src, pos)?;
    if header.id != TRACKS {
        return Err(Stop::Damaged);
    }
    let data = payload(src, &header, u64::MAX)?;
    walk.tracks = Some(tracks::parse(&data)?);
    Ok(())
}

/// Records the positions a `SeekHead` names; a damaged `SeekHead` is ignored from there on.
fn parse_seek_head(data: &[u8], walk: &mut Walk) {
    for seek in Children::new(data) {
        let Ok((SEEK, body)) = seek else {
            if seek.is_err() {
                return;
            }
            continue;
        };
        let mut id = None;
        let mut position = None;
        for child in Children::new(body) {
            match child {
                Ok((SEEK_ID, value)) => id = crate::bits::be_uint(value),
                Ok((SEEK_POSITION, value)) => position = ebml::uint(value).ok(),
                Ok(_) => {}
                Err(_) => return,
            }
        }
        let (Some(id), Some(position)) = (id, position) else {
            continue;
        };
        let Some(pos) = walk.seg_start.checked_add(position) else {
            continue;
        };
        match u32::try_from(id) {
            Ok(TRACKS) => {
                walk.seek_tracks.get_or_insert(pos);
            }
            Ok(TAGS) => {
                walk.seek_tags.get_or_insert(pos);
            }
            Ok(CLUSTER) => {
                let first = walk.seek_cluster.map_or(pos, |known| known.min(pos));
                walk.seek_cluster = Some(first);
            }
            Ok(SEEK_HEAD) if !walk.seek_heads.contains(&pos) => walk.seek_heads.push(pos),
            _ => {}
        }
    }
}

/// Reads `Tags` for an `ENCODER` tag that names x264 or x265; oversized or damaged tags are
/// skipped.
fn read_tags<R: Read + Seek>(src: &mut Source<'_, R>, walk: &mut Walk, header: &Header) -> Res<()> {
    if header.id != TAGS {
        return Ok(());
    }
    walk.tags_read = true;
    let data = match payload(src, header, MAX_TAGS) {
        Ok(data) => data,
        Err(Stop::Limit | Stop::Damaged) => return Ok(()),
        Err(stop) => return Err(stop),
    };
    for tag in Children::new(&data) {
        let Ok((TAG, body)) = tag else {
            continue;
        };
        for simple in Children::new(body) {
            let Ok((SIMPLE_TAG, fields)) = simple else {
                continue;
            };
            let mut name = String::new();
            let mut value = String::new();
            for field in Children::new(fields).flatten() {
                match field {
                    (TAG_NAME, text) => name = ebml::string(text),
                    (TAG_STRING, text) => value = ebml::string(text),
                    _ => {}
                }
            }
            if name.eq_ignore_ascii_case("ENCODER")
                && let Some(encoder) = video::encoder_in_tag(&value)
            {
                walk.tag_encoder.get_or_insert(encoder);
            }
        }
    }
    Ok(())
}
