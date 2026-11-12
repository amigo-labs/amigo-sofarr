//! External subtitle files next to a video by Jellyfin's naming rules:
//! `<video name>.<language>[.<flags>].<extension>`, e.g. `Film.de.srt`,
//! `Film.de.forced.srt`, `Film.en.sdh.srt`.

/// Extensions of subtitle files (lower case).
pub const SUBTITLE_EXTENSIONS: &[&str] = &[
    "srt", "ass", "ssa", "vtt", "sub", "idx", "sup", "smi", "ttml",
];

const FORCED_FLAGS: &[&str] = &["forced", "foreign"];
const HEARING_IMPAIRED_FLAGS: &[&str] = &["sdh", "cc", "hi"];
const OTHER_FLAGS: &[&str] = &["default"];

/// A subtitle file next to a video, split into its parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sidecar {
    /// The parts between the video name and the extension that are no flag, in order;
    /// the first that names a language is the file's language.
    pub parts: Vec<String>,
    pub forced: bool,
    pub hearing_impaired: bool,
}

/// `file_name` split as a subtitle of the video with the file stem `video_stem`;
/// `None` if it is no subtitle file of that video. Names compare case-insensitively.
pub fn parse_sidecar(video_stem: &str, file_name: &str) -> Option<Sidecar> {
    let (rest, extension) = file_name.rsplit_once('.')?;
    if !SUBTITLE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()) {
        return None;
    }
    let prefix = rest.get(..video_stem.len())?;
    if !prefix.eq_ignore_ascii_case(video_stem) {
        return None;
    }
    let tail = &rest[video_stem.len()..];
    let mut sidecar = Sidecar {
        parts: Vec::new(),
        forced: false,
        hearing_impaired: false,
    };
    if tail.is_empty() {
        return Some(sidecar);
    }
    // The video name must end where the parts begin (`Film.de`, not `Filmmusik.de`).
    let tail = tail.strip_prefix('.')?;
    for part in tail.split('.') {
        let lower = part.to_ascii_lowercase();
        if FORCED_FLAGS.contains(&lower.as_str()) {
            sidecar.forced = true;
        } else if HEARING_IMPAIRED_FLAGS.contains(&lower.as_str()) {
            sidecar.hearing_impaired = true;
        } else if !OTHER_FLAGS.contains(&lower.as_str()) && !part.is_empty() {
            sidecar.parts.push(part.to_owned());
        }
    }
    Some(sidecar)
}

/// The file name of a subtitle sofarr writes: `<video stem>.<language>[.forced][.sdh].srt`.
pub fn sidecar_name(
    video_stem: &str,
    language: &str,
    forced: bool,
    hearing_impaired: bool,
) -> String {
    let mut name = format!("{video_stem}.{language}");
    if forced {
        name.push_str(".forced");
    }
    if hearing_impaired {
        name.push_str(".sdh");
    }
    name.push_str(".srt");
    name
}

/// The name of a subtitle after its video was renamed from `old_stem` to `new_stem`;
/// `None` if the file does not start with the old video name.
pub fn renamed_sidecar(old_stem: &str, new_stem: &str, file_name: &str) -> Option<String> {
    let prefix = file_name.get(..old_stem.len())?;
    let rest = &file_name[old_stem.len()..];
    (prefix.eq_ignore_ascii_case(old_stem) && rest.starts_with('.'))
        .then(|| format!("{new_stem}{rest}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(sidecar: &Sidecar) -> Vec<&str> {
        sidecar.parts.iter().map(String::as_str).collect()
    }

    #[test]
    fn sidecars_of_a_video_are_split() {
        let video = "Film (2019) Bluray-1080p";
        let plain = parse_sidecar(video, "Film (2019) Bluray-1080p.de.srt").expect("de");
        assert_eq!(parts(&plain), ["de"]);
        assert!(!plain.forced && !plain.hearing_impaired);
        let forced = parse_sidecar(video, "Film (2019) Bluray-1080p.ger.forced.ass").expect("f");
        assert_eq!(parts(&forced), ["ger"]);
        assert!(forced.forced);
        let sdh =
            parse_sidecar(video, "film (2019) bluray-1080p.English.SDH.default.SRT").expect("sdh");
        assert_eq!(parts(&sdh), ["English"]);
        assert!(sdh.hearing_impaired && !sdh.forced);
        for flag in ["foreign", "cc", "hi"] {
            let name = format!("{video}.de.{flag}.srt");
            assert!(parse_sidecar(video, &name).is_some(), "{name}");
        }
        let bare = parse_sidecar(video, "Film (2019) Bluray-1080p.srt").expect("bare");
        assert!(bare.parts.is_empty());
    }

    #[test]
    fn other_files_are_no_sidecars() {
        let video = "Film";
        assert_eq!(parse_sidecar(video, "Film.mkv"), None);
        assert_eq!(parse_sidecar(video, "Film.de.nfo"), None);
        assert_eq!(parse_sidecar(video, "Filmmusik.de.srt"), None);
        assert_eq!(parse_sidecar(video, "Anderer.de.srt"), None);
        assert_eq!(parse_sidecar(video, "srt"), None);
    }

    #[test]
    fn names_follow_jellyfin() {
        assert_eq!(sidecar_name("Film", "de", false, false), "Film.de.srt");
        assert_eq!(
            sidecar_name("Film", "de", true, false),
            "Film.de.forced.srt"
        );
        assert_eq!(sidecar_name("Film", "en", false, true), "Film.en.sdh.srt");
        assert_eq!(
            sidecar_name("Film", "de", true, true),
            "Film.de.forced.sdh.srt"
        );
        assert_eq!(
            renamed_sidecar("Alt", "Neu (2020)", "Alt.de.forced.srt").as_deref(),
            Some("Neu (2020).de.forced.srt")
        );
        assert_eq!(renamed_sidecar("Alt", "Neu", "Altes.de.srt"), None);
    }
}
