use std::io::Cursor;

use crate::testkit::*;
use crate::{Audio, Container, DynamicRange, MediaInfo, ReadStatus, Subtitle, Video, read};

const DOLBY_VISION_DVCC: u64 = 0x6476_6343;

fn info(bytes: &[u8]) -> MediaInfo {
    read(&mut Cursor::new(bytes))
}

fn video_entry(number: u64, codec: &str, private: Vec<u8>, video: &[Vec<u8>]) -> Vec<u8> {
    let mut extra = vec![master(0xE0, video)];
    if !private.is_empty() {
        extra.push(el(0x63A2, &private));
    }
    entry(number, 1, codec, &extra)
}

fn colour(transfer: u64) -> Vec<u8> {
    master(0x55B0, &[el_uint(0x55BA, transfer)])
}

fn audio_entry(number: u64, codec: &str, channels: u64) -> Vec<u8> {
    entry(
        number,
        2,
        codec,
        &[master(0xE1, &[el_uint(0x9F, channels)])],
    )
}

fn audio(codec: &str, channels: u8, object_audio: bool, language: &str) -> Audio {
    Audio {
        codec: codec.to_string(),
        channels: Some(channels),
        object_audio,
        language: language.to_string(),
    }
}

/// A file with one video track (number 1, HEVC, given frame) plus `others`.
fn hevc_file(video_extra: &[Vec<u8>], frame: &[u8]) -> Vec<u8> {
    let tracks = master(
        TRACKS,
        &[video_entry(
            1,
            "V_MPEGH/ISO/HEVC",
            hvcc(4, &[]),
            video_extra,
        )],
    );
    mkv("matroska", &[tracks, cluster(&[simple_block(1, frame)])])
}

#[test]
fn tracks_languages_and_flags() {
    let tracks = master(
        TRACKS,
        &[
            video_entry(
                1,
                "V_MPEG4/ISO/AVC",
                avcc(4),
                &[el_uint(0xB0, 1920), el_uint(0xBA, 1080)],
            ),
            entry(
                2,
                2,
                "A_EAC3",
                &[el_str(0x22_B59C, "ger"), master(0xE1, &[el_uint(0x9F, 6)])],
            ),
            entry(
                3,
                2,
                "A_AAC",
                &[
                    el_str(0x22_B59C, "eng"),
                    el_str(0x22_B59D, "de-DE"),
                    master(0xE1, &[el_uint(0x9F, 2)]),
                ],
            ),
            entry(
                4,
                0x11,
                "S_TEXT/UTF8",
                &[el_uint(0x88, 0), el_uint(0x55AA, 1)],
            ),
            entry(
                5,
                0x11,
                "S_HDMV/PGS",
                &[el_str(0x22_B59C, "und"), el_uint(0x55AB, 1)],
            ),
            // A track type this reader does not report (buttons).
            entry(6, 0x12, "B_VOBBTN", &[]),
        ],
    );
    let info = info(&mkv("matroska", &[tracks]));
    assert_eq!(
        info,
        MediaInfo {
            status: ReadStatus::Complete,
            container: Some(Container::Matroska),
            video: Some(Video {
                codec: "h264".to_string(),
                encoder: None,
                width: Some(1920),
                height: Some(1080),
                dynamic_range: DynamicRange::default(),
            }),
            audio: vec![
                audio("eac3", 6, false, "ger"),
                audio("aac", 2, false, "ger")
            ],
            subtitles: vec![
                // No Language element: RFC 9559's default `eng`.
                Subtitle {
                    language: "eng".to_string(),
                    format: "srt".to_string(),
                    forced: true,
                    default: false,
                    hearing_impaired: false,
                },
                Subtitle {
                    language: "und".to_string(),
                    format: "pgs".to_string(),
                    forced: false,
                    default: true,
                    hearing_impaired: true,
                },
            ],
        }
    );
    let labels = info.labels();
    assert_eq!(labels.video.as_deref(), Some("h264"));
    assert_eq!(labels.audio.as_deref(), Some("EAC3"));
    assert_eq!(labels.channels.as_deref(), Some("5.1"));
    assert_eq!(labels.dynamic_range, None);
}

#[test]
fn tracks_reachable_only_through_seek_head() {
    // An unknown-size cluster cannot be skipped; only the SeekHead leads past it.
    let tracks = master(TRACKS, &[audio_entry(1, "A_FLAC", 2)]);
    let cluster = master_unknown(CLUSTER, &[el_uint(0xE7, 0), simple_block(1, &[0; 32])]);
    let head_len = seek_head(&[(TRACKS, 0)]).len() as u64;
    let tracks_at = head_len + cluster.len() as u64;
    let with_head = mkv(
        "matroska",
        &[
            seek_head(&[(TRACKS, tracks_at)]),
            cluster.clone(),
            tracks.clone(),
        ],
    );
    let found = info(&with_head);
    assert_eq!(found.status, ReadStatus::Complete);
    assert_eq!(found.audio, vec![audio("flac", 2, false, "eng")]);

    let without_head = mkv("matroska", &[cluster, tracks]);
    let missing = info(&without_head);
    assert_eq!(missing.status, ReadStatus::Partial);
    assert!(missing.audio.is_empty());
}

#[test]
fn tracks_after_sized_clusters_are_found_without_seek_head() {
    let tracks = master(TRACKS, &[audio_entry(1, "A_OPUS", 2)]);
    let file = mkv("matroska", &[cluster(&[simple_block(1, &[1, 2])]), tracks]);
    let info = info(&file);
    assert_eq!(info.status, ReadStatus::Complete);
    assert_eq!(info.audio[0].codec, "opus");
}

#[test]
fn colour_marks_hdr10_and_hlg() {
    for (transfer, label) in [(16, Some("HDR10")), (18, Some("HLG")), (1, None)] {
        let info = info(&hevc_file(&[colour(transfer)], &[]));
        assert_eq!(info.labels().dynamic_range.as_deref(), label, "{transfer}");
    }
}

#[test]
fn dolby_vision_from_block_addition_mapping() {
    let cases = [
        ((8, 1), Some(16), "DV HDR10"),
        ((5, 0), Some(2), "DV"),
        ((8, 4), None, "DV HLG"),
    ];
    for ((profile, compatibility), transfer, label) in cases {
        let mapping = master(
            0x41E4,
            &[
                el_uint(0x41E7, DOLBY_VISION_DVCC),
                el(0x41ED, &dv_record(profile, compatibility)),
            ],
        );
        let video: Vec<Vec<u8>> = transfer.map(colour).into_iter().collect();
        let track = entry(
            1,
            1,
            "V_MPEGH/ISO/HEVC",
            &[master(0xE0, &video), el(0x63A2, &hvcc(4, &[])), mapping],
        );
        let info = info(&mkv("matroska", &[master(TRACKS, &[track])]));
        let range = info.video.as_ref().map(|video| video.dynamic_range);
        assert!(range.is_some_and(|range| range.dolby_vision), "{label}");
        assert_eq!(info.labels().dynamic_range.as_deref(), Some(label));
    }
}

#[test]
fn x265_encoder_from_sei_in_first_simple_block() {
    let frame = length_prefixed(
        4,
        &[
            hevc_sei_nal(&x26x_sei(
                "x265 (build 199) - 3.5+1:[Linux][GCC 9.3.0][64 bit] 10bit",
            )),
            vec![0x26, 0x01, 0xAF, 0x00, 0x00],
        ],
    );
    let tracks = master(
        TRACKS,
        &[
            audio_entry(1, "A_AC3", 6),
            video_entry(2, "V_MPEGH/ISO/HEVC", hvcc(4, &[]), &[colour(1)]),
        ],
    );
    // The audio block comes first; the video track's first block follows.
    let file = mkv(
        "matroska",
        &[
            tracks,
            cluster(&[simple_block(1, &[0x0B, 0x77]), simple_block(2, &frame)]),
        ],
    );
    let info = info(&file);
    let video = info.video.as_ref().expect("video track");
    assert_eq!(video.encoder.as_deref(), Some("x265"));
    let labels = info.labels();
    assert_eq!(labels.video.as_deref(), Some("x265"));
    assert_eq!(labels.audio.as_deref(), Some("AC3"));
}

#[test]
fn encoder_from_hvcc_sei_and_from_tags() {
    let sei = hevc_sei_nal(&x26x_sei("x265 (build 207)"));
    let tracks = master(
        TRACKS,
        &[video_entry(1, "V_MPEGH/ISO/HEVC", hvcc(4, &[sei]), &[])],
    );
    let info_hvcc = info(&mkv("matroska", &[tracks]));
    assert_eq!(info_hvcc.labels().video.as_deref(), Some("x265"));

    let tags = master(
        0x1254_C367,
        &[master(
            0x7373,
            &[
                master(0x63C0, &[]),
                master(
                    0x67C8,
                    &[el_str(0x45A3, "ENCODER"), el_str(0x4487, "x264 core 164")],
                ),
            ],
        )],
    );
    let tracks = master(TRACKS, &[video_entry(1, "V_MPEG4/ISO/AVC", avcc(4), &[])]);
    let head_len = seek_head(&[(TRACKS, 0), (0x1254_C367, 0)]).len() as u64;
    let tags_at = head_len + tracks.len() as u64 + cluster(&[]).len() as u64;
    let file = mkv(
        "matroska",
        &[
            seek_head(&[(TRACKS, head_len), (0x1254_C367, tags_at)]),
            tracks,
            cluster(&[]),
            tags,
        ],
    );
    let info_tags = info(&file);
    assert_eq!(info_tags.status, ReadStatus::Complete);
    assert_eq!(info_tags.labels().video.as_deref(), Some("x264"));
}

#[test]
fn hdr10_plus_from_t35_sei() {
    let frame = length_prefixed(4, &[hevc_sei_nal(&hdr10_plus_sei())]);
    let info = info(&hevc_file(&[colour(16)], &frame));
    let range = info.video.as_ref().map(|video| video.dynamic_range);
    assert_eq!(
        range,
        Some(DynamicRange {
            hdr10: true,
            hdr10_plus: true,
            hlg: false,
            dolby_vision: false
        })
    );
    assert_eq!(info.labels().dynamic_range.as_deref(), Some("HDR10Plus"));
}

fn single_audio(codec: &str, channels: u64, extra: &[Vec<u8>], block: Vec<u8>) -> MediaInfo {
    let mut children = vec![master(0xE1, &[el_uint(0x9F, channels)])];
    children.extend_from_slice(extra);
    let tracks = master(TRACKS, &[entry(1, 2, codec, &children)]);
    info(&mkv("matroska", &[tracks, cluster(&[block])]))
}

#[test]
fn eac3_atmos_from_joc() {
    let atmos = single_audio("A_EAC3", 6, &[], simple_block(1, &eac3_frame(true, false)));
    assert_eq!(atmos.labels().audio.as_deref(), Some("EAC3 Atmos"));
    let plain = single_audio("A_EAC3", 6, &[], simple_block(1, &eac3_frame(false, true)));
    assert_eq!(plain.labels().audio.as_deref(), Some("EAC3"));
}

#[test]
fn header_stripping_is_undone_before_frame_analysis() {
    let frame = eac3_frame(true, true);
    let encodings = master(
        0x6D80,
        &[master(
            0x6240,
            &[master(
                0x5034,
                &[el_uint(0x4254, 3), el(0x4255, &frame[..2])],
            )],
        )],
    );
    let atmos = single_audio("A_EAC3", 6, &[encodings], simple_block(1, &frame[2..]));
    assert_eq!(atmos.labels().audio.as_deref(), Some("EAC3 Atmos"));
    // zlib-compressed frames are not analyzed.
    let zlib = master(0x6D80, &[master(0x6240, &[master(0x5034, &[])])]);
    let plain = single_audio("A_EAC3", 6, &[zlib], simple_block(1, &frame));
    assert_eq!(plain.labels().audio.as_deref(), Some("EAC3"));
}

#[test]
fn ac3_channels_from_the_first_frame_override_the_header() {
    // A broadcast recording whose header claims 5.1 for a stereo stream.
    let stereo = single_audio("A_AC3", 6, &[], simple_block(1, &ac3_frame(2, false)));
    assert_eq!(stereo.labels().audio.as_deref(), Some("AC3"));
    assert_eq!(stereo.labels().channels.as_deref(), Some("2.0"));
    let surround = single_audio("A_AC3", 2, &[], simple_block(1, &ac3_frame(7, true)));
    assert_eq!(surround.labels().channels.as_deref(), Some("5.1"));
}

#[test]
fn dts_profiles_and_truehd_atmos_from_first_frames() {
    let ma = single_audio(
        "A_DTS",
        6,
        &[],
        simple_block(1, &dts_frame(false, Some([0x41, 0xA2, 0x95, 0x47]))),
    );
    assert_eq!(ma.audio[0].codec, "dts-hd ma");
    assert_eq!(ma.labels().audio.as_deref(), Some("DTS-HD MA"));
    let es = single_audio("A_DTS", 7, &[], simple_block(1, &dts_frame(true, None)));
    assert_eq!(es.labels().audio.as_deref(), Some("DTS-ES"));
    assert_eq!(es.labels().channels.as_deref(), Some("6.1"));
    // A header that leaves the LFE out: the core frame counts it.
    let lfe = single_audio("A_DTS", 5, &[], simple_block(1, &dts_core(false, true)));
    assert_eq!(lfe.labels().audio.as_deref(), Some("DTS"));
    assert_eq!(lfe.labels().channels.as_deref(), Some("5.1"));
    // A 7.1 DTS-HD MA keeps its header channels over the 5.1 core.
    let wide = single_audio(
        "A_DTS",
        8,
        &[],
        simple_block(1, &dts_frame(false, Some([0x41, 0xA2, 0x95, 0x47]))),
    );
    assert_eq!(wide.labels().channels.as_deref(), Some("7.1"));
    // TrueHD in a BlockGroup.
    let mut block = vec![0x81, 0, 0, 0];
    block.extend(truehd_frame(4));
    let group = master(0xA0, &[el(0xA1, &block)]);
    let truehd = single_audio("A_TRUEHD", 8, &[], group);
    assert_eq!(truehd.labels().audio.as_deref(), Some("TrueHD Atmos"));
    assert_eq!(truehd.labels().channels.as_deref(), Some("7.1"));
}

#[test]
fn webm_is_its_own_container() {
    let tracks = master(
        TRACKS,
        &[
            video_entry(
                1,
                "V_VP9",
                Vec::new(),
                &[el_uint(0xB0, 3840), el_uint(0xBA, 2160)],
            ),
            audio_entry(2, "A_OPUS", 2),
            entry(3, 0x11, "D_WEBVTT/SUBTITLES", &[el_str(0x22_B59D, "en-US")]),
        ],
    );
    let info = info(&mkv("webm", &[tracks]));
    assert_eq!(info.container, Some(Container::Webm));
    assert_eq!(info.status, ReadStatus::Complete);
    let labels = info.labels();
    assert_eq!(labels.video.as_deref(), Some("VP9"));
    assert_eq!(labels.audio.as_deref(), Some("Opus"));
    assert_eq!(info.subtitles[0].format, "webvtt");
    assert_eq!(info.subtitles[0].language, "eng");
}

#[test]
fn other_doc_types_and_invalid_structures() {
    let other = mkv("notmatroska", &[]);
    assert_eq!(info(&other).status, ReadStatus::Unsupported);
    // A TrackEntry child larger than its parent.
    let tracks = el(TRACKS, &el(0xAE, &[0x86, 0x85, b'A']));
    let damaged = info(&mkv("matroska", &[tracks]));
    assert_eq!(damaged.status, ReadStatus::Damaged);
    assert_eq!(damaged.container, Some(Container::Matroska));
}

#[test]
fn legacy_codec_ids() {
    let mut bitmap = vec![0; 16];
    bitmap.extend(b"XVID");
    let tracks = master(
        TRACKS,
        &[
            video_entry(1, "V_MS/VFW/FOURCC", bitmap, &[]),
            audio_entry(2, "A_PCM/INT/LIT", 2),
            audio_entry(3, "A_MPEG/L3", 2),
            audio_entry(4, "A_REAL/COOK", 2),
        ],
    );
    let info = info(&mkv("matroska", &[tracks]));
    assert_eq!(info.labels().video.as_deref(), Some("XviD"));
    let codecs: Vec<_> = info
        .audio
        .iter()
        .map(|audio| audio.codec.as_str())
        .collect();
    assert_eq!(codecs, ["pcm", "mp3", "A_REAL/COOK"]);
}

#[test]
fn a_file_without_video_track_has_the_video_fields_unknown() {
    let tracks = master(TRACKS, &[audio_entry(1, "A_FLAC", 2)]);
    let info = info(&mkv("matroska", &[tracks]));
    assert_eq!(info.status, ReadStatus::Complete);
    assert_eq!(info.video, None);
    assert_eq!(info.audio, vec![audio("flac", 2, false, "eng")]);
    let labels = info.labels();
    assert_eq!(labels.video, None);
    assert_eq!(labels.dynamic_range, None);
    assert_eq!(labels.audio.as_deref(), Some("FLAC"));
}
