//! Which library files are too large for their resolution (spec reencode-rule, D31).
//!
//! Pure: the caller passes what it knows about a file, the rule answers with a verdict.

/// 1 MB in this feature: binary, 1,048,576 bytes.
pub const MB: u64 = 1024 * 1024;

/// The resolution classes with an own threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Resolution {
    /// Up to 1280×720, SD included.
    P720,
    /// Wider than 1280 or higher than 720.
    P1080,
    /// Wider than 1920 or higher than 1080.
    P2160,
}

impl Resolution {
    /// The class of a video track's pixel size: a 1920×800 movie is `1080p`.
    pub fn of(width: u32, height: u32) -> Self {
        if width > 1920 || height > 1080 {
            Self::P2160
        } else if width > 1280 || height > 720 {
            Self::P1080
        } else {
            Self::P720
        }
    }

    /// `720p`, `1080p` or `2160p`.
    pub fn name(self) -> &'static str {
        match self {
            Self::P720 => "720p",
            Self::P1080 => "1080p",
            Self::P2160 => "2160p",
        }
    }
}

/// The largest allowed size per minute (MB) of each resolution class; 0 switches a class off.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thresholds {
    pub p720: f64,
    pub p1080: f64,
    pub p2160: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            p720: 30.0,
            p1080: 60.0,
            p2160: 150.0,
        }
    }
}

impl Thresholds {
    pub fn of(&self, resolution: Resolution) -> f64 {
        match resolution {
            Resolution::P720 => self.p720,
            Resolution::P1080 => self.p1080,
            Resolution::P2160 => self.p2160,
        }
    }
}

/// The first video track as far as the rule needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoFacts {
    pub width: u32,
    pub height: u32,
    /// Dolby Vision or HDR10+: dynamic metadata a plain HEVC encode would lose.
    pub dynamic_metadata: bool,
}

/// What is known about one library file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileFacts {
    /// The file size in bytes.
    pub size: u64,
    /// `None` without media info or without a video width and height.
    pub video: Option<VideoFacts>,
    /// The duration the file's header names, in milliseconds.
    pub header_ms: Option<u64>,
    /// The runtime from the metadata for the whole file (a multi-episode file: all its
    /// episodes); 0 is unknown.
    pub metadata_minutes: u32,
}

/// Where a duration comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationSource {
    File,
    Metadata,
}

/// Why a file is left out without being fine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Exclusion {
    NoMediaInfo,
    ResolutionOff,
    UnknownDuration,
    DynamicMetadata,
}

/// The numbers behind a verdict.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measure {
    pub resolution: Resolution,
    pub duration_seconds: f64,
    pub source: DurationSource,
    pub mb_per_minute: f64,
    pub threshold: f64,
}

/// What the rule says about a library file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verdict {
    /// Too large and nothing speaks against encoding it.
    Candidate(Measure),
    /// At or below the threshold.
    Fine(Measure),
    Excluded(Exclusion),
}

/// Judges one library file: the first verdict that applies, in the order of the spec.
pub fn judge(file: &FileFacts, thresholds: &Thresholds) -> Verdict {
    let Some(video) = file.video else {
        return Verdict::Excluded(Exclusion::NoMediaInfo);
    };
    let resolution = Resolution::of(video.width, video.height);
    let threshold = thresholds.of(resolution);
    if threshold <= 0.0 {
        return Verdict::Excluded(Exclusion::ResolutionOff);
    }
    let (duration_seconds, source) = match file.header_ms {
        Some(ms) if ms > 0 => (ms as f64 / 1000.0, DurationSource::File),
        _ if file.metadata_minutes > 0 => (
            f64::from(file.metadata_minutes) * 60.0,
            DurationSource::Metadata,
        ),
        _ => return Verdict::Excluded(Exclusion::UnknownDuration),
    };
    let megabytes = file.size as f64 / MB as f64;
    let measure = Measure {
        resolution,
        duration_seconds,
        source,
        mb_per_minute: megabytes * 60.0 / duration_seconds,
        threshold,
    };
    // Compared without the division, so a file exactly at the threshold is never above it
    // by a rounding error.
    if megabytes * 60.0 <= threshold * duration_seconds {
        Verdict::Fine(measure)
    } else if video.dynamic_metadata {
        Verdict::Excluded(Exclusion::DynamicMetadata)
    } else {
        Verdict::Candidate(measure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HD: VideoFacts = VideoFacts {
        width: 1920,
        height: 1080,
        dynamic_metadata: false,
    };

    fn file(
        size_mb: u64,
        video: VideoFacts,
        header_minutes: Option<u64>,
        metadata: u32,
    ) -> FileFacts {
        FileFacts {
            size: size_mb * MB,
            video: Some(video),
            header_ms: header_minutes.map(|m| m * 60_000),
            metadata_minutes: metadata,
        }
    }

    fn judged(file: &FileFacts) -> Verdict {
        judge(file, &Thresholds::default())
    }

    #[test]
    fn resolution_classes_follow_width_or_height() {
        for (width, height, class) in [
            (720, 576, Resolution::P720),
            (1280, 720, Resolution::P720),
            (1281, 536, Resolution::P1080),
            (960, 721, Resolution::P1080),
            (1920, 800, Resolution::P1080),
            (1920, 1080, Resolution::P1080),
            (1921, 800, Resolution::P2160),
            (1440, 1081, Resolution::P2160),
            (3840, 1600, Resolution::P2160),
        ] {
            assert_eq!(Resolution::of(width, height), class, "{width}x{height}");
        }
        assert_eq!(Resolution::P2160.name(), "2160p");
    }

    #[test]
    fn exactly_the_threshold_is_fine_and_above_it_is_a_candidate() {
        // AC-03: 120 min, 7,200 MB = 60 MB per minute at a threshold of 60.
        let Verdict::Fine(measure) = judged(&file(7200, HD, Some(120), 0)) else {
            panic!("at the threshold");
        };
        assert_eq!(measure.resolution, Resolution::P1080);
        assert_eq!(measure.threshold, 60.0);
        assert_eq!(measure.mb_per_minute, 60.0);
        assert_eq!(measure.duration_seconds, 7200.0);
        let mut above = file(7200, HD, Some(120), 0);
        above.size += 1;
        assert!(matches!(judged(&above), Verdict::Candidate(_)));
        // A 1920×800 movie at 70 MB per minute and a 3840×1600 one at 140.
        let scope = VideoFacts { height: 800, ..HD };
        assert!(matches!(
            judged(&file(7000, scope, Some(100), 0)),
            Verdict::Candidate(_)
        ));
        let uhd = VideoFacts {
            width: 3840,
            height: 1600,
            ..HD
        };
        let Verdict::Fine(measure) = judged(&file(14_000, uhd, Some(100), 0)) else {
            panic!("4K below its threshold");
        };
        assert_eq!(
            (measure.resolution, measure.threshold),
            (Resolution::P2160, 150.0)
        );
    }

    #[test]
    fn the_header_duration_wins_and_metadata_fills_in() {
        // AC-05: header 5 min against metadata 120 min.
        let Verdict::Candidate(measure) = judged(&file(1000, HD, Some(5), 120)) else {
            panic!("200 MB per minute");
        };
        assert_eq!(measure.source, DurationSource::File);
        assert_eq!(measure.mb_per_minute, 200.0);
        // A double episode of 2 × 45 minutes without a header duration.
        let Verdict::Fine(measure) = judged(&file(5000, HD, None, 90)) else {
            panic!("55.6 MB per minute");
        };
        assert_eq!(measure.source, DurationSource::Metadata);
        assert_eq!(measure.duration_seconds, 5400.0);
        assert!((measure.mb_per_minute - 55.555).abs() < 0.01);
        // Seconds are not rounded to minutes.
        let mut odd = file(61, HD, None, 0);
        odd.header_ms = Some(61_000);
        let Verdict::Fine(measure) = judged(&odd) else {
            panic!("60 MB per minute");
        };
        assert!((measure.mb_per_minute - 60.0).abs() < 1e-9);
    }

    #[test]
    fn every_file_gets_the_first_verdict_that_applies() {
        let dolby = VideoFacts {
            width: 3840,
            height: 2160,
            dynamic_metadata: true,
        };
        let off = Thresholds {
            p2160: 0.0,
            ..Thresholds::default()
        };
        // 1. No media info beats everything.
        let mut blind = file(90_000, dolby, None, 0);
        blind.video = None;
        assert_eq!(
            judge(&blind, &off),
            Verdict::Excluded(Exclusion::NoMediaInfo)
        );
        // 2. A switched-off class, whatever else applies (AC-04).
        assert_eq!(
            judge(&file(90_000, dolby, None, 0), &off),
            Verdict::Excluded(Exclusion::ResolutionOff)
        );
        assert_eq!(
            judge(&file(1, dolby, Some(100), 0), &off),
            Verdict::Excluded(Exclusion::ResolutionOff)
        );
        // 3. No duration from header or metadata.
        assert_eq!(
            judged(&file(90_000, dolby, None, 0)),
            Verdict::Excluded(Exclusion::UnknownDuration)
        );
        let mut zero = file(90_000, HD, None, 0);
        zero.header_ms = Some(0);
        assert_eq!(judged(&zero), Verdict::Excluded(Exclusion::UnknownDuration));
        // 4. Fine comes before the Dolby Vision exclusion (AC-04) ...
        assert!(matches!(
            judged(&file(10_000, dolby, Some(100), 0)),
            Verdict::Fine(_)
        ));
        // 5. ... which only takes files that are too large.
        assert_eq!(
            judged(&file(40_000, dolby, Some(100), 0)),
            Verdict::Excluded(Exclusion::DynamicMetadata)
        );
        // 6. Everything else is a candidate.
        let plain = VideoFacts {
            dynamic_metadata: false,
            ..dolby
        };
        assert!(matches!(
            judged(&file(40_000, plain, Some(100), 0)),
            Verdict::Candidate(_)
        ));
    }
}
