//! Test-only builders for synthetic EBML/ISO BMFF inputs and bitstreams, and instrumented
//! readers. No media files: every fixture is built here from the specifications' syntax.

use std::io::{self, Read, Seek, SeekFrom};

// --- EBML ---------------------------------------------------------------------------------

pub(crate) fn id_bytes(id: u32) -> Vec<u8> {
    let bytes = id.to_be_bytes();
    let skip = bytes.iter().take_while(|byte| **byte == 0).count().min(3);
    bytes[skip..].to_vec()
}

pub(crate) fn size_vint(size: u64) -> Vec<u8> {
    let len = (1..=8u32)
        .find(|len| size < (1u64 << (7 * len)) - 1)
        .expect("size fits a vint");
    let marked = size | (1u64 << (7 * len));
    marked.to_be_bytes()[8 - len as usize..].to_vec()
}

pub(crate) fn el(id: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = id_bytes(id);
    out.extend(size_vint(payload.len() as u64));
    out.extend_from_slice(payload);
    out
}

pub(crate) fn el_uint(id: u32, value: u64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let skip = bytes.iter().take_while(|byte| **byte == 0).count().min(7);
    el(id, &bytes[skip..])
}

pub(crate) fn el_str(id: u32, value: &str) -> Vec<u8> {
    el(id, value.as_bytes())
}

pub(crate) fn master(id: u32, children: &[Vec<u8>]) -> Vec<u8> {
    el(id, &children.concat())
}

/// An element of unknown size (all eight size bytes' value bits set).
pub(crate) fn master_unknown(id: u32, children: &[Vec<u8>]) -> Vec<u8> {
    let mut out = id_bytes(id);
    out.extend([0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    out.extend(children.concat());
    out
}

pub(crate) const SEGMENT: u32 = 0x1853_8067;
pub(crate) const TRACKS: u32 = 0x1654_AE6B;
pub(crate) const CLUSTER: u32 = 0x1F43_B675;

pub(crate) fn ebml_header(doc_type: &str) -> Vec<u8> {
    master(
        0x1A45_DFA3,
        &[
            el_uint(0x4286, 1),
            el_str(0x4282, doc_type),
            el_uint(0x4287, 4),
        ],
    )
}

pub(crate) fn mkv(doc_type: &str, segment: &[Vec<u8>]) -> Vec<u8> {
    let mut out = ebml_header(doc_type);
    out.extend(master(SEGMENT, segment));
    out
}

/// A `TrackEntry` with number, UID, type and codec ID, plus `extra` children.
pub(crate) fn entry(number: u64, kind: u64, codec: &str, extra: &[Vec<u8>]) -> Vec<u8> {
    let mut children = vec![
        el_uint(0xD7, number),
        el_uint(0x73C5, number + 1000),
        el_uint(0x83, kind),
        el_str(0x86, codec),
    ];
    children.extend_from_slice(extra);
    master(0xAE, &children)
}

pub(crate) fn simple_block(track: u8, frame: &[u8]) -> Vec<u8> {
    let mut payload = vec![0x80 | track, 0, 0, 0x80];
    payload.extend_from_slice(frame);
    el(0xA3, &payload)
}

pub(crate) fn cluster(blocks: &[Vec<u8>]) -> Vec<u8> {
    let mut children = vec![el_uint(0xE7, 0)];
    children.extend_from_slice(blocks);
    master(CLUSTER, &children)
}

/// A `SeekHead` whose positions are written with eight bytes, so its size does not depend
/// on them.
pub(crate) fn seek_head(entries: &[(u32, u64)]) -> Vec<u8> {
    let seeks: Vec<Vec<u8>> = entries
        .iter()
        .map(|(id, pos)| {
            master(
                0x4DBB,
                &[el(0x53AB, &id_bytes(*id)), el(0x53AC, &pos.to_be_bytes())],
            )
        })
        .collect();
    master(0x114D_9B74, &seeks)
}

// --- ISO BMFF -----------------------------------------------------------------------------

pub(crate) fn bx(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(payload);
    out
}

pub(crate) fn full_bx(kind: &[u8; 4], version: u8, flags: u32, payload: &[u8]) -> Vec<u8> {
    let mut body = vec![version];
    body.extend_from_slice(&flags.to_be_bytes()[1..]);
    body.extend_from_slice(payload);
    bx(kind, &body)
}

pub(crate) fn boxes(kind: &[u8; 4], children: &[Vec<u8>]) -> Vec<u8> {
    bx(kind, &children.concat())
}

pub(crate) fn ftyp(brand: &[u8; 4]) -> Vec<u8> {
    let mut payload = brand.to_vec();
    payload.extend([0, 0, 0, 0]);
    payload.extend(brand);
    payload.extend(b"iso6");
    bx(b"ftyp", &payload)
}

pub(crate) fn packed_language(code: &str) -> u16 {
    code.bytes()
        .fold(0u16, |acc, letter| (acc << 5) | u16::from(letter - 0x60))
}

pub(crate) fn visual_entry(
    format: &[u8; 4],
    width: u16,
    height: u16,
    children: &[Vec<u8>],
) -> Vec<u8> {
    let mut payload = vec![0, 0, 0, 0, 0, 0, 0, 1];
    payload.extend([0; 16]);
    payload.extend(width.to_be_bytes());
    payload.extend(height.to_be_bytes());
    payload.extend([0, 0x48, 0, 0, 0, 0x48, 0, 0, 0, 0, 0, 0, 0, 1]);
    payload.extend([0; 32]);
    payload.extend([0, 0x18, 0xFF, 0xFF]);
    payload.extend(children.concat());
    bx(format, &payload)
}

pub(crate) fn audio_entry(format: &[u8; 4], channels: u16, children: &[Vec<u8>]) -> Vec<u8> {
    let mut payload = vec![0, 0, 0, 0, 0, 0, 0, 1];
    payload.extend([0; 8]);
    payload.extend(channels.to_be_bytes());
    payload.extend([0, 16, 0, 0, 0, 0, 0xBB, 0x80, 0, 0]);
    payload.extend(children.concat());
    bx(format, &payload)
}

/// A `trak` with `tkhd`, `mdhd` language, `hdlr` and an `stbl` with one sample in one chunk.
pub(crate) fn trak(
    id: u32,
    handler: &[u8; 4],
    language: &str,
    sample_entry: Vec<u8>,
    sample: (u32, u32),
) -> Vec<u8> {
    let mut tkhd = vec![0; 8];
    tkhd.extend(id.to_be_bytes());
    tkhd.extend([0; 68]);
    let mut mdhd = vec![0; 16];
    mdhd.extend(packed_language(language).to_be_bytes());
    mdhd.extend([0, 0]);
    let mut hdlr = vec![0; 4];
    hdlr.extend(handler);
    hdlr.extend([0; 13]);
    let mut stsd = 1u32.to_be_bytes().to_vec();
    stsd.extend(sample_entry);
    let mut stsz = 0u32.to_be_bytes().to_vec();
    stsz.extend(1u32.to_be_bytes());
    stsz.extend(sample.1.to_be_bytes());
    let mut stco = 1u32.to_be_bytes().to_vec();
    stco.extend(sample.0.to_be_bytes());
    let stbl = boxes(
        b"stbl",
        &[
            full_bx(b"stsd", 0, 0, &stsd),
            full_bx(b"stsz", 0, 0, &stsz),
            full_bx(b"stco", 0, 0, &stco),
        ],
    );
    boxes(
        b"trak",
        &[
            full_bx(b"tkhd", 0, 3, &tkhd),
            boxes(
                b"mdia",
                &[
                    full_bx(b"mdhd", 0, 0, &mdhd),
                    full_bx(b"hdlr", 0, 0, &hdlr),
                    boxes(b"minf", &[stbl]),
                ],
            ),
        ],
    )
}

// --- Bitstreams ---------------------------------------------------------------------------

pub(crate) struct BitWriter {
    bytes: Vec<u8>,
    bits: usize,
}

impl BitWriter {
    pub(crate) fn new() -> Self {
        Self {
            bytes: Vec::new(),
            bits: 0,
        }
    }

    pub(crate) fn put(&mut self, n: u32, value: u64) -> &mut Self {
        for index in (0..n).rev() {
            if self.bits.is_multiple_of(8) {
                self.bytes.push(0);
            }
            let bit = ((value >> index) & 1) as u8;
            let last = self.bytes.len() - 1;
            self.bytes[last] |= bit << (7 - self.bits % 8);
            self.bits += 1;
        }
        self
    }

    pub(crate) fn finish(&self, min_len: usize) -> Vec<u8> {
        let mut out = self.bytes.clone();
        out.resize(out.len().max(min_len), 0);
        out
    }
}

pub(crate) fn length_prefixed(size: usize, nals: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for nal in nals {
        out.extend(&(nal.len() as u64).to_be_bytes()[8 - size..]);
        out.extend(nal);
    }
    out
}

fn sei_message(kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![kind];
    let mut size = payload.len();
    while size >= 255 {
        out.push(0xFF);
        size -= 255;
    }
    out.push(size as u8);
    out.extend_from_slice(payload);
    out
}

/// SEI RBSP with a user_data_unregistered message carrying `text`.
pub(crate) fn x26x_sei(text: &str) -> Vec<u8> {
    let mut payload = vec![0x2C; 16];
    payload.extend(text.as_bytes());
    payload.push(0);
    let mut out = sei_message(5, &payload);
    out.push(0x80);
    out
}

/// SEI RBSP with an ST 2094-40 (HDR10+) user_data_registered_itu_t_t35 message; the
/// metadata contains `00 00 01`, which needs an emulation-prevention byte.
pub(crate) fn hdr10_plus_sei() -> Vec<u8> {
    let payload = [
        0xB5, 0x00, 0x3C, 0x00, 0x01, 0x04, 0x01, 0x40, 0x00, 0x00, 0x01, 0x20,
    ];
    let mut out = sei_message(4, &payload);
    out.push(0x80);
    out
}

/// A prefix SEI NAL unit (type 39) from an RBSP, with emulation prevention applied.
pub(crate) fn hevc_sei_nal(rbsp: &[u8]) -> Vec<u8> {
    let mut out = vec![39 << 1, 0x01];
    let mut zeros = 0;
    for byte in rbsp {
        if zeros >= 2 && *byte <= 3 {
            out.push(3);
            zeros = 0;
        }
        zeros = if *byte == 0 { zeros + 1 } else { 0 };
        out.push(*byte);
    }
    out
}

pub(crate) fn avcc(length_size: u8) -> Vec<u8> {
    vec![1, 0x64, 0, 0x28, 0xFC | (length_size - 1), 0xE0, 0]
}

/// An `HEVCDecoderConfigurationRecord` with one array of the given NAL units.
pub(crate) fn hvcc(length_size: u8, nals: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![
        1, 0x02, 0x20, 0, 0, 0, 0x90, 0, 0, 0, 0, 0, 0x99, 0xF0, 0, 0xFC, 0xFD,
    ];
    out.extend([0xFA, 0xFA, 0, 0, 0x0C | (length_size - 1)]);
    if nals.is_empty() {
        out.push(0);
        return out;
    }
    out.push(1);
    out.push(0x80 | 39);
    out.extend((nals.len() as u16).to_be_bytes());
    for nal in nals {
        out.extend((nal.len() as u16).to_be_bytes());
        out.extend(nal);
    }
    out
}

/// A `DOVIDecoderConfigurationRecord` (24 bytes) with level 6, RPU and base layer present.
pub(crate) fn dv_record(profile: u8, compatibility: u8) -> Vec<u8> {
    let mut out = vec![1, 0, profile << 1, (6 << 3) | 0b101, compatibility << 4];
    out.resize(24, 0);
    out
}

/// An AC-3 syncframe header: fscod 0, frmsizecod 0, bsid 8, bsmod 0, `acmod` and `lfe`.
pub(crate) fn ac3_frame(acmod: u64, lfe: bool) -> Vec<u8> {
    let mut bits = BitWriter::new();
    bits.put(16, 0x0B77)
        .put(16, 0)
        .put(2, 0)
        .put(6, 0)
        .put(5, 8)
        .put(3, 0)
        .put(3, acmod);
    if acmod & 1 == 1 && acmod != 1 {
        bits.put(2, 0);
    }
    if acmod & 4 != 0 {
        bits.put(2, 0);
    }
    if acmod == 2 {
        bits.put(2, 0);
    }
    bits.put(1, u64::from(lfe));
    bits.finish(32)
}

/// An E-AC-3 syncframe header (5.1, six blocks); `metadata` adds mixing and informational
/// metadata before the additional bit stream information that carries the JOC flag.
pub(crate) fn eac3_frame(joc: bool, metadata: bool) -> Vec<u8> {
    let mut bits = BitWriter::new();
    bits.put(16, 0x0B77).put(2, 0).put(3, 0).put(11, 0x2FF);
    bits.put(2, 0)
        .put(2, 3)
        .put(3, 7)
        .put(1, 1)
        .put(5, 16)
        .put(5, 27)
        .put(1, 0);
    if metadata {
        // mixmdate: dmixmod, ltrt/loro centre levels, ltrt/loro surround levels,
        // lfemixlevcode, pgmscle, extpgmscle, mixdef 3 with 2 bytes, frmmixcfginfoe.
        bits.put(1, 1)
            .put(2, 1)
            .put(6, 0b100_100)
            .put(6, 0b100_100)
            .put(1, 0);
        bits.put(1, 0)
            .put(1, 0)
            .put(2, 3)
            .put(5, 0)
            .put(16, 0xFFFF)
            .put(1, 0);
        // infomdate: bsmod, copyrightb, origbs, dsurexmod, audprodie with fields, sourcefscod.
        bits.put(1, 1)
            .put(3, 0)
            .put(1, 1)
            .put(1, 1)
            .put(2, 0)
            .put(1, 1)
            .put(8, 0xA5)
            .put(1, 0);
    } else {
        bits.put(1, 0).put(1, 0);
    }
    if joc {
        bits.put(1, 1).put(6, 1).put(1, 1).put(8, 16).put(7, 0);
    } else {
        bits.put(1, 0);
    }
    bits.finish(64)
}

/// A TrueHD access unit starting with a major sync that announces `substreams`.
pub(crate) fn truehd_frame(substreams: u8) -> Vec<u8> {
    let mut out = vec![0xF0, 0x40, 0x00, 0x00, 0xF8, 0x72, 0x6F, 0xBA];
    out.extend([
        0x00, 0x00, 0x00, 0x00, 0xB7, 0x52, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00,
    ]);
    out.push(substreams << 4);
    out.resize(64, 0);
    out
}

/// A 64-byte DTS core frame, optionally followed by a 64-byte extension substream whose
/// first asset component starts with `component`.
pub(crate) fn dts_frame(es: bool, component: Option<[u8; 4]>) -> Vec<u8> {
    let mut out = dts_core(es, false);
    if let Some(sync) = component {
        let mut ext = BitWriter::new();
        ext.put(32, 0x6458_2025)
            .put(8, 0)
            .put(2, 0)
            .put(1, 0)
            .put(8, 15)
            .put(16, 63);
        let mut substream = ext.finish(16);
        substream.extend([0, 0, 0, 0]);
        substream.extend(sync);
        substream.resize(64, 0);
        out.extend(substream);
    }
    out
}

/// A 64-byte DTS core frame with AMODE 9 (C, L, R, SL, SR), a channel extension flag and
/// an LFE flag.
pub(crate) fn dts_core(es: bool, lfe: bool) -> Vec<u8> {
    let mut core = BitWriter::new();
    core.put(32, 0x7FFE_8001)
        .put(1, 1)
        .put(5, 31)
        .put(1, 0)
        .put(7, 15)
        .put(14, 63);
    core.put(6, 9)
        .put(4, 13)
        .put(5, 15)
        .put(5, 0)
        .put(3, 0)
        .put(1, u64::from(es))
        .put(1, 0)
        .put(2, u64::from(lfe));
    core.finish(64)
}

// --- Readers ------------------------------------------------------------------------------

/// Counts the bytes handed out by `read`.
pub(crate) struct Counting<R> {
    inner: R,
    pub(crate) read: u64,
}

impl<R> Counting<R> {
    pub(crate) fn new(inner: R) -> Self {
        Self { inner, read: 0 }
    }
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read += n as u64;
        Ok(n)
    }
}

impl<R: Seek> Seek for Counting<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

/// A large input that exists only as a few byte runs: `parts` at their offsets, `pattern`
/// repeated from `pattern_start` on, zeros elsewhere. Counts the bytes read.
pub(crate) struct Sparse {
    len: u64,
    parts: Vec<(u64, Vec<u8>)>,
    pattern: Vec<u8>,
    pattern_start: u64,
    pos: u64,
    pub(crate) read: u64,
}

impl Sparse {
    pub(crate) fn new(len: u64, parts: Vec<(u64, Vec<u8>)>) -> Self {
        Self {
            len,
            parts,
            pattern: Vec::new(),
            pattern_start: u64::MAX,
            pos: 0,
            read: 0,
        }
    }

    pub(crate) fn with_pattern(mut self, start: u64, pattern: Vec<u8>) -> Self {
        self.pattern_start = start;
        self.pattern = pattern;
        self
    }

    fn byte_at(&self, pos: u64) -> u8 {
        for (offset, bytes) in &self.parts {
            if pos >= *offset && pos - offset < bytes.len() as u64 {
                return bytes[(pos - offset) as usize];
            }
        }
        if pos >= self.pattern_start && !self.pattern.is_empty() {
            return self.pattern[((pos - self.pattern_start) % self.pattern.len() as u64) as usize];
        }
        0
    }
}

impl Read for Sparse {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = (self.len.saturating_sub(self.pos)).min(buf.len() as u64) as usize;
        for (index, slot) in buf.iter_mut().take(n).enumerate() {
            *slot = self.byte_at(self.pos + index as u64);
        }
        self.pos += n as u64;
        self.read += n as u64;
        Ok(n)
    }
}

impl Seek for Sparse {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.pos = match pos {
            SeekFrom::Start(pos) => pos,
            SeekFrom::End(delta) => self.len.checked_add_signed(delta).expect("seek in range"),
            SeekFrom::Current(delta) => self.pos.checked_add_signed(delta).expect("seek in range"),
        };
        Ok(self.pos)
    }
}

/// A small deterministic xorshift PRNG for the fuzz-like tests.
pub(crate) struct Prng(u64);

impl Prng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub(crate) fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub(crate) fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound.max(1) as u64) as usize
    }
}
