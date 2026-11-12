//! Truncated, corrupted, random, huge and failing inputs: the reader never panics, never
//! reads more than READ_LIMIT bytes and never writes.

use std::io::{self, Cursor, Read, Seek, SeekFrom};

use crate::testkit::*;
use crate::{READ_LIMIT, ReadStatus, read, unreadable};

/// Where `part` ends inside `file`.
fn end_of(file: &[u8], part: &[u8]) -> usize {
    file.windows(part.len())
        .position(|window| window == part)
        .expect("part in file")
        + part.len()
}

/// A Matroska file with video, two audio tracks, a subtitle track, tags and a cluster, with
/// the end of its first `TrackEntry` and of its `Tracks`.
fn mkv_sample() -> (Vec<u8>, usize, usize) {
    let video = entry(
        1,
        1,
        "V_MPEGH/ISO/HEVC",
        &[
            el(0x63A2, &hvcc(4, &[])),
            master(
                0xE0,
                &[el_uint(0xB0, 1920), master(0x55B0, &[el_uint(0x55BA, 16)])],
            ),
        ],
    );
    let tracks = master(
        TRACKS,
        &[
            video.clone(),
            entry(
                2,
                2,
                "A_EAC3",
                &[el_str(0x22_B59C, "ger"), master(0xE1, &[el_uint(0x9F, 6)])],
            ),
            entry(3, 2, "A_DTS", &[master(0xE1, &[el_uint(0x9F, 6)])]),
            entry(4, 0x11, "S_TEXT/ASS", &[el_str(0x22_B59D, "de-DE")]),
        ],
    );
    let frame = length_prefixed(
        4,
        &[
            hevc_sei_nal(&x26x_sei("x265 (build 1)")),
            hevc_sei_nal(&hdr10_plus_sei()),
        ],
    );
    let blocks = cluster(&[
        simple_block(1, &frame),
        simple_block(2, &eac3_frame(true, true)),
        simple_block(3, &dts_frame(false, Some([0x41, 0xA2, 0x95, 0x47]))),
    ]);
    let file = mkv("matroska", &[tracks.clone(), blocks]);
    let video_end = end_of(&file, &video);
    let tracks_end = end_of(&file, &tracks);
    (file, video_end, tracks_end)
}

/// An MP4 with the moov first and one AVC sample, with the end of its first `trak` and of
/// its `moov`.
fn mp4_sample() -> (Vec<u8>, usize, usize) {
    let sample = length_prefixed(4, &[vec![0x06, 0x05, 0x01, 0x00, 0x80], vec![0x65, 0x88]]);
    let sample_len = sample.len() as u32;
    let moov = |data_at: u32| {
        boxes(
            b"moov",
            &[
                trak(
                    1,
                    b"vide",
                    "und",
                    visual_entry(b"avc1", 640, 360, &[bx(b"avcC", &avcc(4))]),
                    (data_at, sample_len),
                ),
                trak(
                    2,
                    b"soun",
                    "deu",
                    audio_entry(b"ac-3", 2, &[bx(b"dac3", &[0x10, 0x3C, 0])]),
                    (0, 0),
                ),
            ],
        )
    };
    let head = ftyp(b"isom");
    let data_at = (head.len() + moov(0).len() + 8) as u32;
    let moov = moov(data_at);
    // The first `trak` starts right after the `moov` header.
    let first_trak = u32::from_be_bytes([moov[8], moov[9], moov[10], moov[11]]) as usize;
    let trak_end = head.len() + 8 + first_trak;
    let moov_end = head.len() + moov.len();
    let mut out = head;
    out.extend(moov);
    out.extend(bx(b"mdat", &sample));
    (out, trak_end, moov_end)
}

#[test]
fn complete_fixtures_read_completely() {
    let (mkv, _, _) = mkv_sample();
    let info = read(&mut Cursor::new(&mkv));
    assert_eq!(info.status, ReadStatus::Complete);
    let labels = info.labels();
    assert_eq!(labels.video.as_deref(), Some("x265"));
    assert_eq!(labels.audio.as_deref(), Some("EAC3 Atmos"));
    assert_eq!(labels.dynamic_range.as_deref(), Some("HDR10+"));
    assert_eq!(info.audio[1].codec, "dts-hd ma");
    assert_eq!(info.subtitles[0].language, "ger");
    let mp4 = read(&mut Cursor::new(mp4_sample().0));
    assert_eq!(mp4.status, ReadStatus::Complete);
    assert_eq!(mp4.labels().audio.as_deref(), Some("AC3"));
    assert_eq!(mp4.labels().channels.as_deref(), Some("5.1"));
}

#[test]
fn truncated_files_are_partial_or_damaged_at_every_cut() {
    let (mkv, video_end, tracks_end) = mkv_sample();
    for cut in 0..mkv.len() {
        let info = read(&mut Cursor::new(&mkv[..cut]));
        assert_ne!(info.status, ReadStatus::Unreadable, "cut {cut}");
        if cut < video_end {
            assert!(
                matches!(
                    info.status,
                    ReadStatus::Partial | ReadStatus::Damaged | ReadStatus::Unsupported
                ),
                "mkv cut {cut}: {:?}",
                info.status
            );
            assert!(info.video.is_none() && info.audio.is_empty());
        } else if cut < tracks_end {
            // The complete `TrackEntry`s before the cut are kept (AC-04).
            assert_eq!(info.status, ReadStatus::Partial, "mkv cut {cut}");
            let video = info.video.as_ref().expect("first track kept");
            assert_eq!((video.codec.as_str(), video.width), ("h265", Some(1920)));
            assert!(info.audio.len() <= 2 && info.subtitles.is_empty());
        } else {
            assert_eq!(info.status, ReadStatus::Complete, "mkv cut {cut}");
        }
    }
    let (mp4, trak_end, moov_end) = mp4_sample();
    for cut in 0..mp4.len() {
        let info = read(&mut Cursor::new(&mp4[..cut]));
        assert_ne!(info.status, ReadStatus::Unreadable, "cut {cut}");
        if cut < trak_end {
            assert!(
                info.video.is_none() && info.audio.is_empty(),
                "mp4 cut {cut}"
            );
        } else if cut < moov_end {
            // The complete `trak`s of a cut-off `moov` are kept (AC-04).
            assert_eq!(info.status, ReadStatus::Partial, "mp4 cut {cut}");
            let video = info.video.as_ref().expect("first trak kept");
            assert_eq!((video.codec.as_str(), video.width), ("h264", Some(640)));
            assert!(info.audio.is_empty(), "mp4 cut {cut}");
        } else {
            assert_eq!(info.status, ReadStatus::Complete, "mp4 cut {cut}");
        }
    }
}

#[test]
fn corrupted_and_random_bytes_never_panic() {
    let (mkv, _, _) = mkv_sample();
    let fixtures = [mkv, mp4_sample().0];
    let mut rng = Prng::new(0x5EED_CAFE);
    for round in 0..4000 {
        let mut data = fixtures[round % fixtures.len()].clone();
        for _ in 0..1 + rng.below(8) {
            let at = rng.below(data.len());
            data[at] = rng.next() as u8;
        }
        if rng.below(4) == 0 {
            data.truncate(rng.below(data.len()));
        }
        let mut reader = Counting::new(Cursor::new(&data));
        let _ = read(&mut reader);
        assert!(reader.read <= READ_LIMIT);
    }
    let prefixes: [&[u8]; 4] = [
        &[0x1A, 0x45, 0xDF, 0xA3],
        b"\0\0\0\x18ftyp",
        b"\0\0\0\x08moov",
        b"",
    ];
    for round in 0..4000 {
        let mut data = prefixes[round % prefixes.len()].to_vec();
        let len = rng.below(512);
        data.extend((0..len).map(|_| rng.next() as u8));
        let _ = read(&mut Cursor::new(&data));
    }
}

#[test]
fn huge_input_reads_at_most_the_limit() {
    // An unknown-size Segment followed by endless Void elements with 8-byte sizes of zero.
    let mut prefix = ebml_header("matroska");
    prefix.extend(id_bytes(SEGMENT));
    prefix.extend([0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    let start = prefix.len() as u64;
    let mut sparse = Sparse::new(100 << 30, vec![(0, prefix)])
        .with_pattern(start, vec![0xEC, 0x01, 0, 0, 0, 0, 0, 0, 0]);
    let info = read(&mut sparse);
    assert_eq!(info.status, ReadStatus::Partial);
    assert!(sparse.read <= READ_LIMIT, "read {}", sparse.read);
    assert!(
        sparse.read > READ_LIMIT - 64,
        "the limit was reached: {}",
        sparse.read
    );
}

#[test]
fn reader_errors_are_unreadable() {
    struct Failing {
        fail_seek: bool,
    }
    impl Read for Failing {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("disk on fire"))
        }
    }
    impl Seek for Failing {
        fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
            if self.fail_seek {
                Err(io::Error::other("no seek"))
            } else {
                Ok(1024)
            }
        }
    }
    assert_eq!(read(&mut Failing { fail_seek: true }), unreadable());
    assert_eq!(read(&mut Failing { fail_seek: false }), unreadable());
    assert_eq!(unreadable().status, ReadStatus::Unreadable);
    assert!(unreadable().container.is_none());
}

#[test]
fn empty_and_foreign_inputs_are_unsupported() {
    assert_eq!(
        read(&mut Cursor::new(Vec::new())).status,
        ReadStatus::Unsupported
    );
    assert_eq!(
        read(&mut Cursor::new(b"RIFF\0\0\0\0AVI LIST".to_vec())).status,
        ReadStatus::Unsupported
    );
    assert_eq!(
        read(&mut Cursor::new(vec![0x47; 188])).status,
        ReadStatus::Unsupported
    );
}

#[test]
fn reading_leaves_the_bytes_unchanged() {
    // The API only takes `Read + Seek`; this shows the input is the same afterwards.
    let (mkv, _, _) = mkv_sample();
    let mut cursor = Cursor::new(mkv.clone());
    let _ = read(&mut cursor);
    assert_eq!(cursor.into_inner(), mkv);
}
