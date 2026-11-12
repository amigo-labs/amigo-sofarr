//! Labels: the vocabulary the naming tokens are built from (spec "Starting vocabulary").

use crate::{Audio, DynamicRange, Labels, MediaInfo, Video};

impl MediaInfo {
    /// The labels of the first video track and the first audio track in file order.
    pub fn labels(&self) -> Labels {
        let first_audio = self.audio.first();
        Labels {
            video: self.video.as_ref().map(video_label),
            audio: first_audio.map(audio_label),
            channels: first_audio
                .and_then(|audio| audio.channels)
                .and_then(channels_label),
            dynamic_range: self
                .video
                .as_ref()
                .and_then(|video| dynamic_range_label(&video.dynamic_range)),
        }
    }
}

fn video_label(video: &Video) -> String {
    if let Some(encoder @ ("x264" | "x265")) = video.encoder.as_deref() {
        return encoder.to_string();
    }
    let label = match video.codec.as_str() {
        "h264" => "h264",
        "h265" => "h265",
        "av1" => "AV1",
        "vp9" => "VP9",
        "mpeg2" => "MPEG2",
        "vc1" => "VC1",
        "xvid" => "XviD",
        "divx" => "DivX",
        other => other,
    };
    label.to_string()
}

fn audio_label(audio: &Audio) -> String {
    let codec = match audio.codec.as_str() {
        "aac" => "AAC",
        "ac3" => "AC3",
        "eac3" => "EAC3",
        "dts" => "DTS",
        "dts-es" => "DTS-ES",
        "dts-hd hra" => "DTS-HD HRA",
        "dts-hd ma" => "DTS-HD MA",
        "dts-x" => "DTS-X",
        "truehd" => "TrueHD",
        "flac" => "FLAC",
        "opus" => "Opus",
        "vorbis" => "Vorbis",
        "mp3" => "MP3",
        "pcm" => "PCM",
        other => other,
    };
    if audio.object_audio {
        format!("{codec} Atmos")
    } else {
        codec.to_string()
    }
}

fn channels_label(channels: u8) -> Option<String> {
    let label = match channels {
        1 => "1.0",
        2 => "2.0",
        3 => "2.1",
        4 => "4.0",
        5 => "5.0",
        6 => "5.1",
        7 => "6.1",
        8 => "7.1",
        _ => return None,
    };
    Some(label.to_string())
}

/// `None` for SDR.
fn dynamic_range_label(range: &DynamicRange) -> Option<String> {
    let label = if range.dolby_vision {
        if range.hdr10_plus {
            "DV HDR10+"
        } else if range.hdr10 {
            "DV HDR10"
        } else if range.hlg {
            "DV HLG"
        } else {
            "DV"
        }
    } else if range.hdr10_plus {
        "HDR10+"
    } else if range.hdr10 {
        "HDR"
    } else if range.hlg {
        "HLG"
    } else {
        return None;
    };
    Some(label.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReadStatus;

    fn info(video: Option<Video>, audio: Vec<Audio>) -> MediaInfo {
        MediaInfo {
            status: ReadStatus::Complete,
            container: None,
            video,
            audio,
            subtitles: Vec::new(),
        }
    }

    fn video(codec: &str, encoder: Option<&str>) -> Video {
        Video {
            codec: codec.to_string(),
            encoder: encoder.map(str::to_string),
            width: None,
            height: None,
            dynamic_range: DynamicRange::default(),
        }
    }

    fn audio(codec: &str, channels: Option<u8>, object_audio: bool) -> Audio {
        Audio {
            codec: codec.to_string(),
            channels,
            object_audio,
            language: "und".to_string(),
        }
    }

    #[test]
    fn video_labels() {
        let cases = [
            ("h265", Some("x265"), "x265"),
            ("h264", Some("x264"), "x264"),
            ("h264", None, "h264"),
            ("h265", Some("NVENC"), "h265"),
            ("av1", None, "AV1"),
            ("vp9", None, "VP9"),
            ("mpeg2", None, "MPEG2"),
            ("vc1", None, "VC1"),
            ("xvid", None, "XviD"),
            ("divx", None, "DivX"),
            ("V_PRORES", None, "V_PRORES"),
        ];
        for (codec, encoder, expected) in cases {
            let labels = info(Some(video(codec, encoder)), Vec::new()).labels();
            assert_eq!(labels.video.as_deref(), Some(expected), "{codec}");
        }
    }

    #[test]
    fn audio_and_channel_labels_use_the_first_track() {
        let cases = [
            ("aac", false, "AAC"),
            ("ac3", false, "AC3"),
            ("eac3", true, "EAC3 Atmos"),
            ("dts", false, "DTS"),
            ("dts-es", false, "DTS-ES"),
            ("dts-hd hra", false, "DTS-HD HRA"),
            ("dts-hd ma", false, "DTS-HD MA"),
            ("dts-x", false, "DTS-X"),
            ("truehd", true, "TrueHD Atmos"),
            ("flac", false, "FLAC"),
            ("opus", false, "Opus"),
            ("vorbis", false, "Vorbis"),
            ("mp3", false, "MP3"),
            ("pcm", false, "PCM"),
            ("A_REAL/COOK", false, "A_REAL/COOK"),
        ];
        for (codec, atmos, expected) in cases {
            let tracks = vec![audio(codec, Some(6), atmos), audio("aac", Some(2), false)];
            let labels = info(None, tracks).labels();
            assert_eq!(labels.audio.as_deref(), Some(expected), "{codec}");
            assert_eq!(labels.channels.as_deref(), Some("5.1"));
        }
        let channels = [
            (1, Some("1.0")),
            (2, Some("2.0")),
            (3, Some("2.1")),
            (4, Some("4.0")),
            (5, Some("5.0")),
            (6, Some("5.1")),
            (7, Some("6.1")),
            (8, Some("7.1")),
            (0, None),
            (9, None),
        ];
        for (count, expected) in channels {
            assert_eq!(channels_label(count).as_deref(), expected);
        }
        let none = info(None, vec![audio("aac", None, false)]).labels();
        assert_eq!(none.channels, None);
    }

    #[test]
    fn dynamic_range_labels() {
        let range = |hdr10, hdr10_plus, hlg, dolby_vision| DynamicRange {
            hdr10,
            hdr10_plus,
            hlg,
            dolby_vision,
        };
        let cases = [
            (range(false, false, false, false), None),
            (range(true, false, false, false), Some("HDR")),
            (range(true, true, false, false), Some("HDR10+")),
            (range(false, false, true, false), Some("HLG")),
            (range(false, false, false, true), Some("DV")),
            (range(true, false, false, true), Some("DV HDR10")),
            (range(true, true, false, true), Some("DV HDR10+")),
            (range(false, false, true, true), Some("DV HLG")),
        ];
        for (dynamic_range, expected) in cases {
            let mut video = video("h265", None);
            video.dynamic_range = dynamic_range;
            let labels = info(Some(video), Vec::new()).labels();
            assert_eq!(
                labels.dynamic_range.as_deref(),
                expected,
                "{dynamic_range:?}"
            );
        }
        assert_eq!(info(None, Vec::new()).labels(), Labels::default());
    }
}
