//! The container-independent track model: what a container parser collects per track, the
//! frame analysis shared by both containers, and the assembly of [`MediaInfo`].

use crate::source::Stop;
use crate::video::{self, DvConfig, Findings};
use crate::{Audio, Container, DynamicRange, MediaInfo, ReadStatus, Subtitle, Video, audio};

/// The most bytes read of one track's first frame.
pub(crate) const FRAME_CAP: u64 = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Video,
    Audio,
    Subtitle,
}

/// How the first frame of a track is analyzed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Probe {
    None,
    /// Length-prefixed H.264 (`hevc: false`) or H.265 NAL units.
    Nal {
        hevc: bool,
        length_size: usize,
    },
    Mpeg4Part2,
    /// AC-3 without a configuration box: the channels come from the first syncframe.
    Ac3,
    Eac3,
    TrueHd,
    Dts,
}

/// One track as a container parser collects it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Draft {
    pub(crate) kind: Kind,
    pub(crate) codec: String,
    /// ISO 639-2/B.
    pub(crate) language: String,
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
    /// Transfer characteristics (ITU-T H.273): 16 is PQ, 18 is HLG.
    pub(crate) transfer: Option<u64>,
    /// Mastering-display metadata in the container.
    pub(crate) mastering: bool,
    pub(crate) dv: Option<DvConfig>,
    pub(crate) findings: Findings,
    pub(crate) channels: Option<u8>,
    pub(crate) object_audio: bool,
    pub(crate) forced: bool,
    pub(crate) default: bool,
    pub(crate) hearing_impaired: bool,
    pub(crate) probe: Probe,
}

impl Draft {
    pub(crate) fn new(kind: Kind) -> Self {
        Self {
            kind,
            codec: String::new(),
            language: "und".to_string(),
            width: None,
            height: None,
            transfer: None,
            mastering: false,
            dv: None,
            findings: Findings::default(),
            channels: None,
            object_audio: false,
            forced: false,
            default: false,
            hearing_impaired: false,
            probe: Probe::None,
        }
    }

    /// Applies what the first frame of the track shows.
    pub(crate) fn analyze_frame(&mut self, frame: &[u8]) {
        match self.probe {
            Probe::None => {}
            Probe::Nal { hevc, length_size } => {
                video::scan_length_prefixed(frame, length_size, hevc, &mut self.findings);
            }
            Probe::Mpeg4Part2 => {
                if let Some(vendor) = video::mpeg4_vendor(frame) {
                    self.codec = vendor.to_string();
                }
            }
            // AC-3 has no extensions that add channels: the syncframe is the stream's layout.
            Probe::Ac3 => {
                if let Some(channels) = audio::ac3_channels(frame) {
                    self.channels = Some(channels);
                }
            }
            Probe::Eac3 => {
                if audio::eac3_joc(frame) == Some(true) {
                    self.object_audio = true;
                }
            }
            Probe::TrueHd => {
                if audio::truehd_atmos(frame) == Some(true) {
                    self.object_audio = true;
                }
            }
            Probe::Dts => {
                if let Some(profile) = audio::dts_profile(frame) {
                    self.codec = profile.to_string();
                }
                // Some muxers leave the LFE out of the header's count; extensions (ES, HD)
                // only add channels to the core, so the larger count is the stream's.
                if let Some(core) = audio::dts_core_channels(frame) {
                    self.channels = Some(self.channels.map_or(core, |header| header.max(core)));
                }
            }
        }
    }

    fn dynamic_range(&self) -> DynamicRange {
        let mut range = DynamicRange::default();
        match self.transfer {
            Some(16) => range.hdr10 = true,
            Some(18) => range.hlg = true,
            // Unknown or unspecified transfer: mastering metadata only comes with PQ in practice.
            None | Some(2) if self.mastering || self.findings.mastering => range.hdr10 = true,
            _ => {}
        }
        if let Some(dv) = self.dv {
            range.dolby_vision = true;
            if matches!(dv.compatibility, 1 | 6) || dv.profile == 7 {
                range.hdr10 = true;
            }
            if dv.compatibility == 4 {
                range.hlg = true;
            }
        } else if self.findings.dv_rpu {
            range.dolby_vision = true;
        }
        range.hdr10_plus = self.findings.hdr10_plus;
        range
    }

    fn into_video(self, tag_encoder: Option<&str>) -> Video {
        let dynamic_range = self.dynamic_range();
        Video {
            encoder: self.findings.encoder.or(tag_encoder).map(str::to_string),
            codec: self.codec,
            width: self.width,
            height: self.height,
            dynamic_range,
        }
    }
}

/// The read status for the reason reading stopped before every track header was read.
pub(crate) fn status_of(stop: Stop) -> ReadStatus {
    match stop {
        Stop::Truncated | Stop::Limit => ReadStatus::Partial,
        Stop::Damaged => ReadStatus::Damaged,
        Stop::Io => ReadStatus::Unreadable,
    }
}

/// Media info without tracks.
pub(crate) fn empty(status: ReadStatus, container: Option<Container>) -> MediaInfo {
    MediaInfo {
        status,
        container,
        video: None,
        audio: Vec::new(),
        subtitles: Vec::new(),
    }
}

/// Builds the media info from the tracks in file order.
pub(crate) fn finish(
    status: ReadStatus,
    container: Option<Container>,
    drafts: Vec<Draft>,
    tag_encoder: Option<&str>,
) -> MediaInfo {
    let mut info = empty(status, container);
    for draft in drafts {
        match draft.kind {
            Kind::Video => {
                if info.video.is_none() {
                    info.video = Some(draft.into_video(tag_encoder));
                }
            }
            Kind::Audio => info.audio.push(Audio {
                codec: draft.codec,
                channels: draft.channels,
                object_audio: draft.object_audio,
                language: draft.language,
            }),
            Kind::Subtitle => info.subtitles.push(Subtitle {
                language: draft.language,
                format: draft.codec,
                forced: draft.forced,
                default: draft.default,
                hearing_impaired: draft.hearing_impaired,
            }),
        }
    }
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(transfer: Option<u64>, dv: Option<(u8, u8)>) -> DynamicRange {
        let mut draft = Draft::new(Kind::Video);
        draft.transfer = transfer;
        draft.dv = dv.map(|(profile, compatibility)| DvConfig {
            profile,
            compatibility,
        });
        draft.dynamic_range()
    }

    #[test]
    fn dynamic_range_from_transfer_and_dolby_vision() {
        assert_eq!(video(Some(1), None), DynamicRange::default());
        assert!(video(Some(16), None).hdr10);
        assert!(video(Some(18), None).hlg);
        let p81 = video(None, Some((8, 1)));
        assert!(p81.dolby_vision && p81.hdr10 && !p81.hlg);
        let p84 = video(None, Some((8, 4)));
        assert!(p84.dolby_vision && p84.hlg && !p84.hdr10);
        let p5 = video(Some(2), Some((5, 0)));
        assert!(p5.dolby_vision && !p5.hdr10);
        assert!(video(None, Some((7, 6))).hdr10);
    }

    #[test]
    fn mastering_metadata_without_transfer_counts_as_hdr10() {
        let mut draft = Draft::new(Kind::Video);
        draft.mastering = true;
        assert!(draft.dynamic_range().hdr10);
        draft.transfer = Some(1);
        assert!(!draft.dynamic_range().hdr10);
    }
}
