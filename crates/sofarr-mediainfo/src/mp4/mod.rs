//! The ISO base media file format (ISO/IEC 14496-12) and QuickTime: top-level boxes are
//! skipped by seeking until `moov`, which is read whole; then the first sample of each track
//! that needs one.

mod entry;

#[cfg(test)]
mod tests;

use std::io::{Read, Seek};

use crate::bits::{be16, be32, be64};
use crate::source::{Res, Source, Stop};
use crate::track::{self, Draft, FRAME_CAP, Kind, Probe};
use crate::{Container, MediaInfo, ReadStatus, language_code};

/// Box types that may start a file of this family.
const FIRST_BOXES: [&[u8; 4]; 6] = [b"ftyp", b"moov", b"mdat", b"free", b"skip", b"wide"];
const MAX_FTYP: u64 = 64;

/// Whether the first bytes are the header of a plausible first box.
pub(crate) fn looks_like(magic: &[u8]) -> bool {
    let (Some(size), Some(kind)) = (be32(magic, 0), magic.get(4..8)) else {
        return false;
    };
    (size <= 1 || size >= 8) && FIRST_BOXES.iter().any(|first| first[..] == *kind)
}

pub(crate) fn read<R: Read + Seek>(src: &mut Source<'_, R>) -> MediaInfo {
    let mut container = Container::Mp4;
    let moov = match find_moov(src, &mut container) {
        Ok(Some(moov)) => moov,
        Ok(None) => return track::empty(ReadStatus::Partial, Some(container)),
        Err(stop) => return track::empty(track::status_of(stop), Some(container)),
    };
    let mut tracks = match parse_moov(&moov) {
        Ok(tracks) => tracks,
        Err(stop) => return track::empty(track::status_of(stop), Some(container)),
    };
    for track in &mut tracks {
        let Some((offset, size)) = track.first_sample else {
            continue;
        };
        if track.draft.probe == Probe::None || size == 0 {
            continue;
        }
        src.seek(offset);
        // The samples only add details; failing to read them leaves the headers complete.
        if let Ok(sample) = src.read_some(u64::from(size).min(FRAME_CAP)) {
            track.draft.analyze_frame(&sample);
        }
    }
    let drafts = tracks.into_iter().map(|track| track.draft).collect();
    track::finish(ReadStatus::Complete, Some(container), drafts, None)
}

/// Walks the top-level boxes and returns the payload of `moov`.
fn find_moov<R: Read + Seek>(
    src: &mut Source<'_, R>,
    container: &mut Container,
) -> Res<Option<Vec<u8>>> {
    let len = src.len();
    let mut pos = 0u64;
    while pos < len {
        src.seek(pos);
        let head = src.read_vec(8)?;
        let size32 = be32(&head, 0).ok_or(Stop::Truncated)?;
        let kind = head.get(4..8).ok_or(Stop::Truncated)?;
        let (header_len, size) = match size32 {
            0 => (8, len - pos),
            1 => {
                let large = src.read_vec(8)?;
                (16, be64(&large, 0).ok_or(Stop::Truncated)?)
            }
            size => (8, u64::from(size)),
        };
        if size < header_len {
            return Err(Stop::Damaged);
        }
        let end = pos.checked_add(size).ok_or(Stop::Damaged)?;
        match kind {
            b"ftyp" => {
                let brand = src.read_vec((size - header_len).min(MAX_FTYP))?;
                if brand.get(..4) == Some(b"qt  ") {
                    *container = Container::QuickTime;
                }
            }
            b"moov" => {
                if end > len {
                    return Err(Stop::Truncated);
                }
                return src.read_vec(size - header_len).map(Some);
            }
            _ => {}
        }
        pos = end;
    }
    Ok(None)
}

/// Iterates the boxes of a fully read payload. A box that does not fit is damage; fewer
/// than 8 trailing bytes (QuickTime terminators, padding) end the iteration.
pub(super) struct Boxes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Boxes<'a> {
    pub(super) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn next_box(&mut self) -> Res<([u8; 4], &'a [u8])> {
        let rest = self.data.get(self.pos..).ok_or(Stop::Damaged)?;
        let size32 = be32(rest, 0).ok_or(Stop::Damaged)?;
        let kind: [u8; 4] = rest
            .get(4..8)
            .and_then(|kind| kind.try_into().ok())
            .ok_or(Stop::Damaged)?;
        let (header_len, size) = match size32 {
            0 => (8, rest.len()),
            1 => {
                let large = be64(rest, 8).ok_or(Stop::Damaged)?;
                (16, usize::try_from(large).map_err(|_| Stop::Damaged)?)
            }
            size => (8, usize::try_from(size).map_err(|_| Stop::Damaged)?),
        };
        if size < header_len {
            return Err(Stop::Damaged);
        }
        let body = rest.get(header_len..size).ok_or(Stop::Damaged)?;
        self.pos += size;
        Ok((kind, body))
    }
}

impl<'a> Iterator for Boxes<'a> {
    type Item = Res<([u8; 4], &'a [u8])>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.len().saturating_sub(self.pos) < 8 {
            return None;
        }
        let item = self.next_box();
        if item.is_err() {
            self.pos = self.data.len();
        }
        Some(item)
    }
}

/// One `trak` as far as it matters here.
struct Mp4Track {
    id: u32,
    draft: Draft,
    /// File offset and size of the first sample.
    first_sample: Option<(u64, u32)>,
}

/// The tracks of `moov` in file order; text tracks that serve as chapter lists are left out.
fn parse_moov(moov: &[u8]) -> Res<Vec<Mp4Track>> {
    let mut tracks = Vec::new();
    let mut chapters = Vec::new();
    for item in Boxes::new(moov) {
        let (kind, body) = item?;
        if &kind == b"trak"
            && let Some(track) = parse_trak(body, &mut chapters)?
        {
            tracks.push(track);
        }
    }
    tracks.retain(|track| !(track.draft.kind == Kind::Subtitle && chapters.contains(&track.id)));
    Ok(tracks)
}

#[derive(Default)]
struct Media<'a> {
    handler: Option<[u8; 4]>,
    language: Option<String>,
    stsd: Option<&'a [u8]>,
    chunk_offset: Option<u64>,
    sample_size: Option<u32>,
}

fn parse_trak(trak: &[u8], chapters: &mut Vec<u32>) -> Res<Option<Mp4Track>> {
    let mut id = 0;
    let mut enabled = true;
    let mut media = Media::default();
    for item in Boxes::new(trak) {
        let (kind, body) = item?;
        match &kind {
            b"tkhd" => {
                enabled = body.get(3).is_some_and(|flags| flags & 1 == 1);
                let at = if body.first() == Some(&1) { 20 } else { 12 };
                id = be32(body, at).unwrap_or(0);
            }
            b"tref" => {
                for reference in Boxes::new(body) {
                    let (kind, ids) = reference?;
                    if &kind == b"chap" {
                        chapters.extend(ids.as_chunks::<4>().0.iter().filter_map(|id| be32(id, 0)));
                    }
                }
            }
            b"mdia" => parse_mdia(body, &mut media)?,
            _ => {}
        }
    }
    let kind = match media.handler.as_ref() {
        Some(b"vide") => Kind::Video,
        Some(b"soun") => Kind::Audio,
        Some(b"subt" | b"sbtl" | b"text" | b"clcp") => Kind::Subtitle,
        _ => return Ok(None),
    };
    let mut draft = Draft::new(kind);
    if let Some(stsd) = media.stsd {
        entry::parse_stsd(stsd, &mut draft)?;
    }
    draft.language = language_code(media.language.as_deref().unwrap_or("und"));
    draft.default = enabled;
    let first_sample = media.chunk_offset.zip(media.sample_size);
    Ok(Some(Mp4Track {
        id,
        draft,
        first_sample,
    }))
}

fn parse_mdia<'a>(mdia: &'a [u8], media: &mut Media<'a>) -> Res<()> {
    for item in Boxes::new(mdia) {
        let (kind, body) = item?;
        match &kind {
            b"mdhd" => {
                let at = if body.first() == Some(&1) { 32 } else { 20 };
                if let Some(packed) = be16(body, at) {
                    media
                        .language
                        .get_or_insert_with(|| packed_language(packed));
                }
            }
            b"elng" => {
                let tag = crate::ebml::string(body.get(4..).unwrap_or_default());
                if !tag.is_empty() {
                    media.language = Some(tag);
                }
            }
            b"hdlr" => {
                media.handler = body.get(8..12).and_then(|kind| kind.try_into().ok());
            }
            b"minf" => {
                for item in Boxes::new(body) {
                    let (kind, stbl) = item?;
                    if &kind == b"stbl" {
                        parse_stbl(stbl, media)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_stbl<'a>(stbl: &'a [u8], media: &mut Media<'a>) -> Res<()> {
    for item in Boxes::new(stbl) {
        let (kind, body) = item?;
        let has_entries = be32(body, 4).is_some_and(|count| count > 0);
        match &kind {
            b"stsd" => media.stsd = Some(body),
            b"stco" if has_entries => media.chunk_offset = be32(body, 8).map(u64::from),
            b"co64" if has_entries => media.chunk_offset = be64(body, 8),
            b"stsz" => {
                media.sample_size = match be32(body, 4) {
                    Some(0) if be32(body, 8).is_some_and(|count| count > 0) => be32(body, 12),
                    Some(0) | None => None,
                    size => size,
                };
            }
            b"stz2" if be32(body, 8).is_some_and(|count| count > 0) => {
                media.sample_size = match body.get(7) {
                    Some(4) => body.get(12).map(|byte| u32::from(byte >> 4)),
                    Some(8) => body.get(12).map(|byte| u32::from(*byte)),
                    Some(16) => be16(body, 12).map(u32::from),
                    _ => None,
                };
            }
            _ => {}
        }
    }
    Ok(())
}

/// The `mdhd` language: three 5-bit letters (offset 0x60) of ISO 639-2/T; QuickTime files
/// may hold a Macintosh language code (below 0x400) instead.
fn packed_language(packed: u16) -> String {
    if packed < 0x400 {
        const MACINTOSH: [&str; 24] = [
            "eng", "fre", "ger", "ita", "dut", "swe", "spa", "dan", "por", "nor", "heb", "jpn",
            "ara", "fin", "gre", "ice", "mlt", "tur", "hrv", "chi", "urd", "hin", "tha", "kor",
        ];
        return MACINTOSH
            .get(usize::from(packed))
            .copied()
            .unwrap_or("und")
            .to_string();
    }
    let letter = |shift: u16| char::from(((packed >> shift) & 0x1F) as u8 + 0x60);
    [letter(10), letter(5), letter(0)].iter().collect()
}
