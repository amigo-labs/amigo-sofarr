//! Lossless audio to E-AC-3 in the same encode (spec reencode-audio): which tracks are
//! converted, at which rate, and whether an efficient (HEVC) video is copied instead.

use sofarr_mediainfo::{Audio, MediaInfo};

/// The audio settings of `[reencode]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSettings {
    pub convert: bool,
    /// For converted tracks with 3 and more channels (or an unknown number).
    pub bitrate_kbps: u32,
}

/// The rate of a converted track with 1 or 2 channels.
pub const STEREO_KBPS: u32 = 256;
/// The most channels ffmpeg's E-AC-3 encoder writes.
pub const MAX_CHANNELS: u8 = 6;

/// What happens to one audio track of the original.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPlan {
    Copy,
    /// To E-AC-3 at this rate; `downmix` mixes more than 6 channels down to 5.1.
    Convert {
        kbps: u32,
        downmix: bool,
    },
}

/// Lossless without object audio: TrueHD without Atmos, PCM, FLAC. DTS (also DTS-HD MA,
/// which may carry DTS:X) and every lossy codec stay as they are.
pub fn convertible(audio: &Audio) -> bool {
    !audio.object_audio && matches!(audio.codec.as_str(), "truehd" | "pcm" | "flac")
}

/// The plan of every audio track of the original, in its order.
pub fn plan(info: &MediaInfo, settings: AudioSettings) -> Vec<AudioPlan> {
    info.audio
        .iter()
        .map(|audio| {
            if !settings.convert || !convertible(audio) {
                return AudioPlan::Copy;
            }
            let kbps = match audio.channels {
                Some(1 | 2) => STEREO_KBPS,
                _ => settings.bitrate_kbps,
            };
            AudioPlan::Convert {
                kbps,
                downmix: audio.channels.is_some_and(|c| c > MAX_CHANNELS),
            }
        })
        .collect()
}

/// Whether the first video track is HEVC and the plan converts a track: then the video is
/// copied and only the audio is encoded (AC-06).
pub fn copies_video(info: &MediaInfo, plans: &[AudioPlan]) -> bool {
    info.video.as_ref().is_some_and(|v| v.codec == "h265")
        && plans.iter().any(|p| matches!(p, AudioPlan::Convert { .. }))
}

#[cfg(test)]
mod tests {
    use sofarr_mediainfo::{Container, DynamicRange, ReadStatus, Video};

    use super::*;

    fn audio(codec: &str, channels: Option<u8>, object_audio: bool) -> Audio {
        Audio {
            codec: codec.to_owned(),
            channels,
            object_audio,
            language: "ger".to_owned(),
        }
    }

    fn media(codec: &str, audio: Vec<Audio>) -> MediaInfo {
        MediaInfo {
            status: ReadStatus::Complete,
            container: Some(Container::Matroska),
            video: Some(Video {
                codec: codec.to_owned(),
                encoder: None,
                width: Some(3840),
                height: Some(2160),
                dynamic_range: DynamicRange::default(),
            }),
            audio,
            subtitles: Vec::new(),
            duration_ms: Some(6_000_000),
        }
    }

    const ON: AudioSettings = AudioSettings {
        convert: true,
        bitrate_kbps: 640,
    };

    #[test]
    fn only_lossless_tracks_without_object_audio_are_converted() {
        // AC-01, AC-02.
        let original = media(
            "h264",
            vec![
                audio("truehd", Some(8), true),
                audio("truehd", Some(8), false),
                audio("dts-hd ma", Some(6), false),
                audio("dts-hd hra", Some(6), false),
                audio("dts-es", Some(7), false),
                audio("dts", Some(6), false),
                audio("pcm", Some(2), false),
                audio("flac", Some(6), false),
                audio("ac3", Some(6), false),
                audio("eac3", Some(6), true),
                audio("aac", Some(2), false),
                audio("opus", Some(2), false),
                audio("pcm", None, false),
            ],
        );
        let convert = |kbps, downmix| AudioPlan::Convert { kbps, downmix };
        assert_eq!(
            plan(&original, ON),
            [
                AudioPlan::Copy,
                convert(640, true),
                AudioPlan::Copy,
                AudioPlan::Copy,
                AudioPlan::Copy,
                AudioPlan::Copy,
                convert(256, false),
                convert(640, false),
                AudioPlan::Copy,
                AudioPlan::Copy,
                AudioPlan::Copy,
                AudioPlan::Copy,
                // Unknown channels: the configured rate, no mix-down.
                convert(640, false),
            ]
        );
        assert!(convertible(&audio("flac", Some(1), false)));
        assert!(!convertible(&audio("truehd", Some(8), true)));
    }

    #[test]
    fn the_switch_and_the_rate_apply() {
        // AC-03, AC-04.
        let original = media(
            "h264",
            vec![audio("flac", Some(6), false), audio("pcm", Some(1), false)],
        );
        let off = AudioSettings {
            convert: false,
            bitrate_kbps: 640,
        };
        assert_eq!(plan(&original, off), [AudioPlan::Copy, AudioPlan::Copy]);
        let rate = AudioSettings {
            convert: true,
            bitrate_kbps: 1024,
        };
        assert_eq!(
            plan(&original, rate),
            [
                AudioPlan::Convert {
                    kbps: 1024,
                    downmix: false
                },
                AudioPlan::Convert {
                    kbps: 256,
                    downmix: false
                }
            ]
        );
    }

    #[test]
    fn an_hevc_video_is_copied_only_when_audio_is_converted() {
        // AC-06, AC-07.
        let hevc_flac = media("h265", vec![audio("flac", Some(6), false)]);
        assert!(copies_video(&hevc_flac, &plan(&hevc_flac, ON)));
        let hevc_eac3 = media("h265", vec![audio("eac3", Some(6), false)]);
        assert!(!copies_video(&hevc_eac3, &plan(&hevc_eac3, ON)));
        let avc_flac = media("h264", vec![audio("flac", Some(6), false)]);
        assert!(!copies_video(&avc_flac, &plan(&avc_flac, ON)));
        let off = AudioSettings {
            convert: false,
            bitrate_kbps: 640,
        };
        assert!(!copies_video(&hevc_flac, &plan(&hevc_flac, off)));
    }
}
