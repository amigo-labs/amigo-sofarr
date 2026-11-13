use std::io::Cursor;

use crate::testkit::*;
use crate::{Container, DynamicRange, MediaInfo, ReadStatus, Subtitle, read};

fn colr(transfer: u16) -> Vec<u8> {
    let mut payload = b"nclx".to_vec();
    for value in [9u16, transfer, 9] {
        payload.extend(value.to_be_bytes());
    }
    payload.push(0);
    bx(b"colr", &payload)
}

fn tx3g(forced: bool) -> Vec<u8> {
    let mut payload = vec![0, 0, 0, 0, 0, 0, 0, 1];
    let flags: u32 = if forced { 0x8000_0000 } else { 0 };
    payload.extend(flags.to_be_bytes());
    payload.extend([0; 30]);
    bx(b"tx3g", &payload)
}

fn esds(object_type: u8, config: &[u8]) -> Vec<u8> {
    let mut decoder = vec![object_type, 0x15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    decoder.extend([0x05, config.len() as u8]);
    decoder.extend(config);
    let mut es = vec![0, 1, 0, 0x04, decoder.len() as u8];
    es.extend(decoder);
    let mut payload = vec![0x03, es.len() as u8];
    payload.extend(es);
    full_bx(b"esds", 0, 0, &payload)
}

/// ftyp, moov, mdat ("fast start"); `moov` gets the offset where the mdat data starts.
fn fast_start(brand: &[u8; 4], moov: impl Fn(u32) -> Vec<u8>, mdat: &[u8]) -> Vec<u8> {
    let head = ftyp(brand);
    let moov_len = moov(0).len();
    let data_at = (head.len() + moov_len + 8) as u32;
    let mut out = head;
    out.extend(moov(data_at));
    out.extend(bx(b"mdat", mdat));
    out
}

/// Inserts `extra` as the last child of a box.
fn with_child(parent: Vec<u8>, extra: Vec<u8>) -> Vec<u8> {
    let kind: [u8; 4] = parent[4..8].try_into().expect("box type");
    let mut body = parent[8..].to_vec();
    body.extend(extra);
    bx(&kind, &body)
}

#[test]
fn moov_after_a_huge_mdat_is_reached_by_seeking() {
    let mdat_size: u64 = 8 << 30;
    let video_at: u64 = 24 + 16;
    let audio_at: u64 = video_at + 4096;
    let moov_at = 24 + mdat_size;
    let video_sample = length_prefixed(
        4,
        &[
            hevc_sei_nal(&x26x_sei("x265 (build 199)")),
            vec![0x26, 0x01, 0xAF],
        ],
    );
    let audio_sample = eac3_frame(true, false);
    let video = visual_entry(
        b"hvc1",
        3840,
        2160,
        &[
            bx(b"hvcC", &hvcc(4, &[])),
            colr(16),
            bx(b"dvcC", &dv_record(8, 1)),
        ],
    );
    let audio = audio_entry(
        b"ec-3",
        2,
        &[bx(b"dec3", &[0x14, 0x00, 0x20, 0x0F, 0x00, 0x01, 0x10])],
    );
    let moov = boxes(
        b"moov",
        &[
            trak(
                1,
                b"vide",
                "und",
                video,
                (video_at as u32, video_sample.len() as u32),
            ),
            trak(
                2,
                b"soun",
                "deu",
                audio,
                (audio_at as u32, audio_sample.len() as u32),
            ),
            trak(3, b"sbtl", "eng", tx3g(true), (0, 0)),
        ],
    );
    let mut head = ftyp(b"isom");
    head.extend([0, 0, 0, 1]);
    head.extend(b"mdat");
    head.extend(mdat_size.to_be_bytes());
    let len = moov_at + moov.len() as u64;
    let mut sparse = Sparse::new(
        len,
        vec![
            (0, head),
            (video_at, video_sample),
            (audio_at, audio_sample),
            (moov_at, moov),
        ],
    );
    let info = read(&mut sparse);
    assert!(sparse.read < 64 * 1024, "read {} bytes", sparse.read);
    assert_eq!(info.status, ReadStatus::Complete);
    assert_eq!(info.container, Some(Container::Mp4));
    let video = info.video.as_ref().expect("video track");
    assert_eq!(video.codec, "h265");
    assert_eq!((video.width, video.height), (Some(3840), Some(2160)));
    assert_eq!(video.encoder.as_deref(), Some("x265"));
    assert_eq!(
        video.dynamic_range,
        DynamicRange {
            hdr10: true,
            hdr10_plus: false,
            hlg: false,
            dolby_vision: true
        }
    );
    assert_eq!(info.audio[0].language, "ger");
    assert_eq!(
        info.subtitles,
        vec![Subtitle {
            language: "eng".to_string(),
            format: "tx3g".to_string(),
            forced: true,
            default: true,
            hearing_impaired: false,
        }]
    );
    let labels = info.labels();
    assert_eq!(labels.video.as_deref(), Some("x265"));
    assert_eq!(labels.audio.as_deref(), Some("EAC3 Atmos"));
    assert_eq!(labels.channels.as_deref(), Some("5.1"));
    assert_eq!(labels.dynamic_range.as_deref(), Some("DV HDR10"));
}

fn aac_file() -> Vec<u8> {
    // AudioSpecificConfig: AAC LC, 48 kHz, channelConfiguration 6.
    let audio = audio_entry(b"mp4a", 2, &[esds(0x40, &[0x11, 0xB0])]);
    let x264 = {
        let mut nal = vec![0x06];
        nal.extend(x26x_sei("x264 - core 164 r3095"));
        length_prefixed(4, &[nal, vec![0x65, 0x88, 0x84]])
    };
    let sample_len = x264.len() as u32;
    let moov = |data_at: u32| {
        let video = visual_entry(b"avc1", 1920, 800, &[bx(b"avcC", &avcc(4))]);
        boxes(
            b"moov",
            &[
                trak(1, b"vide", "und", video, (data_at, sample_len)),
                trak(2, b"soun", "fra", audio.clone(), (0, 0)),
            ],
        )
    };
    fast_start(b"mp42", moov, &x264)
}

#[test]
fn aac_channels_language_and_x264_sample() {
    let info = read(&mut Cursor::new(aac_file()));
    assert_eq!(info.status, ReadStatus::Complete);
    assert_eq!(info.audio[0].codec, "aac");
    assert_eq!(info.audio[0].channels, Some(6));
    assert_eq!(info.audio[0].language, "fre");
    let labels = info.labels();
    assert_eq!(labels.video.as_deref(), Some("x264"));
    assert_eq!(labels.audio.as_deref(), Some("AAC"));
    assert_eq!(labels.channels.as_deref(), Some("5.1"));
}

#[test]
fn content_decides_not_the_extension() {
    // The bytes of an MP4 stored as "movie.mkv": the reader never sees the name.
    let info = read(&mut Cursor::new(aac_file()));
    assert_eq!(info.container, Some(Container::Mp4));
}

#[test]
fn quicktime_brand_and_chapter_text_tracks() {
    let moov = |_| {
        let video = visual_entry(b"apch", 1920, 1080, &[]);
        let video = with_child(trak(1, b"vide", "und", video, (0, 0)), {
            boxes(b"tref", &[bx(b"chap", &2u32.to_be_bytes())])
        });
        boxes(
            b"moov",
            &[
                video,
                trak(2, b"text", "eng", tx3g(false), (0, 0)),
                trak(3, b"soun", "eng", audio_entry(b"lpcm", 2, &[]), (0, 0)),
            ],
        )
    };
    let info = read(&mut Cursor::new(fast_start(b"qt  ", moov, &[])));
    assert_eq!(info.container, Some(Container::QuickTime));
    assert_eq!(info.labels().video.as_deref(), Some("apch"));
    assert_eq!(info.labels().audio.as_deref(), Some("PCM"));
    assert!(info.subtitles.is_empty());
}

#[test]
fn moov_beyond_the_limit_is_partial_without_reading_it() {
    let moov_size: u32 = 1 << 30;
    let mut head = ftyp(b"isom");
    head.extend(moov_size.to_be_bytes());
    head.extend(b"moov");
    let len = 24 + u64::from(moov_size);
    let mut sparse = Sparse::new(len, vec![(0, head)]);
    let info = read(&mut sparse);
    assert_eq!(info.status, ReadStatus::Partial);
    assert!(sparse.read < 1024);
}

#[test]
fn invalid_box_sizes_are_damage_and_missing_moov_is_partial() {
    let mut tiny = ftyp(b"isom");
    tiny.extend([0, 0, 0, 4]);
    tiny.extend(b"free");
    assert_eq!(read(&mut Cursor::new(tiny)).status, ReadStatus::Damaged);

    let broken_trak = boxes(
        b"moov",
        &[bx(b"trak", &[0, 0, 0, 4, b'm', b'd', b'i', b'a'])],
    );
    let mut file = ftyp(b"isom");
    file.extend(broken_trak);
    let info = read(&mut Cursor::new(file));
    assert_eq!(info.status, ReadStatus::Damaged);
    assert_eq!(info.container, Some(Container::Mp4));

    let mut no_moov = ftyp(b"isom");
    no_moov.extend(bx(b"mdat", &[0; 64]));
    assert_eq!(
        read(&mut Cursor::new(no_moov)),
        MediaInfo {
            status: ReadStatus::Partial,
            container: Some(Container::Mp4),
            video: None,
            audio: Vec::new(),
            subtitles: Vec::new(),
        }
    );
}
