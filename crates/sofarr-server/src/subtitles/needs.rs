//! Needed subtitles of a file (spec subtitles, "Covered language", "Needed subtitle"):
//! pure, from the file's media info and the subtitle files next to it.

use std::{collections::HashMap, sync::OnceLock};

use sofarr_db::SubtitleKind;
use sofarr_mediainfo::{MediaInfo, language_code, language_from_name};

/// German language names (lower case) with their ISO 639-2/B code.
const GERMAN_NAMES: &[(&str, &str)] = &[
    ("deutsch", "ger"),
    ("englisch", "eng"),
    ("französisch", "fre"),
    ("spanisch", "spa"),
    ("italienisch", "ita"),
    ("niederländisch", "dut"),
    ("dänisch", "dan"),
    ("schwedisch", "swe"),
    ("norwegisch", "nor"),
    ("finnisch", "fin"),
    ("polnisch", "pol"),
    ("tschechisch", "cze"),
    ("ungarisch", "hun"),
    ("russisch", "rus"),
    ("türkisch", "tur"),
    ("griechisch", "gre"),
    ("portugiesisch", "por"),
    ("japanisch", "jpn"),
    ("chinesisch", "chi"),
    ("koreanisch", "kor"),
    ("arabisch", "ara"),
    ("hebräisch", "heb"),
];

/// The ISO 639-2/B code a part of a subtitle file name names: an ISO 639-1 or 639-2
/// code, or an English or German language name; `None` for anything else.
pub fn part_language(part: &str) -> Option<String> {
    let code = if part.len() <= 3 || part.contains(['-', '_']) {
        Some(language_code(part))
    } else {
        language_from_name(part).or_else(|| {
            let lower = part.to_lowercase();
            GERMAN_NAMES
                .iter()
                .find(|(name, _)| *name == lower)
                .map(|(_, code)| (*code).to_owned())
        })
    };
    code.filter(|c| c != "und")
}

/// The ISO 639-1 code of an ISO 639-2/B code, if it has one.
pub fn alpha2(code: &str) -> Option<String> {
    static TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut table = HashMap::new();
        for a in b'a'..=b'z' {
            for b in b'a'..=b'z' {
                let two = String::from_utf8(vec![a, b]).unwrap_or_default();
                // Withdrawn codes that map to languages with a current code.
                if ["in", "iw", "ji"].contains(&two.as_str()) {
                    continue;
                }
                let long = language_code(&two);
                if long != "und" {
                    table.entry(long).or_insert(two);
                }
            }
        }
        table
    });
    table.get(code).cloned()
}

/// A subtitle file next to a video, as far as coverage goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct External {
    pub file_name: String,
    /// ISO 639-2/B; `None` without a recognised language part.
    pub language: Option<String>,
    pub forced: bool,
    pub hearing_impaired: bool,
}

/// The needed subtitles of a file: a normal one per wanted language (ISO 639-1) that no
/// audio track, embedded non-forced subtitle or external non-forced subtitle covers, and
/// a forced one in the language of the first audio track when that is wanted and no
/// forced subtitle in it exists. Unknown media info covers nothing.
pub fn needs(
    wanted: &[String],
    media: Option<&MediaInfo>,
    external: &[External],
) -> Vec<(String, SubtitleKind)> {
    let audio: Vec<&str> = media
        .map(|m| m.audio.iter().map(|a| a.language.as_str()).collect())
        .unwrap_or_default();
    let embedded: Vec<(&str, bool)> = media
        .map(|m| {
            m.subtitles
                .iter()
                .map(|s| (s.language.as_str(), s.forced))
                .collect()
        })
        .unwrap_or_default();
    let covered = |code: &str, forced: bool| {
        embedded.iter().any(|(l, f)| *l == code && *f == forced)
            || external
                .iter()
                .any(|e| e.language.as_deref() == Some(code) && e.forced == forced)
    };
    let mut out = Vec::new();
    for language in wanted {
        let code = language_code(language);
        if code == "und" || audio.contains(&code.as_str()) || covered(&code, false) {
            continue;
        }
        out.push((language.clone(), SubtitleKind::Normal));
    }
    if let Some(first) = audio.first()
        && *first != "und"
        && let Some(language) = wanted.iter().find(|w| language_code(w) == *first)
        && !covered(first, true)
    {
        out.push((language.clone(), SubtitleKind::Forced));
    }
    out
}

#[cfg(test)]
mod tests {
    use sofarr_mediainfo::{Audio, ReadStatus, Subtitle};

    use super::*;

    fn media(audio: &[&str], subtitles: &[(&str, bool)]) -> MediaInfo {
        MediaInfo {
            status: ReadStatus::Complete,
            container: None,
            video: None,
            audio: audio
                .iter()
                .map(|l| Audio {
                    codec: "AC3".to_owned(),
                    channels: Some(6),
                    object_audio: false,
                    language: (*l).to_owned(),
                })
                .collect(),
            subtitles: subtitles
                .iter()
                .map(|(l, forced)| Subtitle {
                    language: (*l).to_owned(),
                    format: "SRT".to_owned(),
                    forced: *forced,
                    default: false,
                    hearing_impaired: false,
                })
                .collect(),
        }
    }

    fn external(language: &str, forced: bool) -> External {
        External {
            file_name: format!("Film.{language}.srt"),
            language: part_language(language),
            forced,
            hearing_impaired: false,
        }
    }

    fn wanted() -> Vec<String> {
        vec!["de".to_owned(), "en".to_owned()]
    }

    fn text(needs: &[(String, SubtitleKind)]) -> Vec<String> {
        needs
            .iter()
            .map(|(l, k)| format!("{l}:{}", k.as_str()))
            .collect()
    }

    #[test]
    fn german_dl_needs_only_a_german_forced_subtitle() {
        let dl = media(&["ger", "eng"], &[]);
        assert_eq!(text(&needs(&wanted(), Some(&dl), &[])), ["de:forced"]);
        let with_forced = media(&["ger", "eng"], &[("ger", true)]);
        assert!(needs(&wanted(), Some(&with_forced), &[]).is_empty());
        assert!(needs(&wanted(), Some(&dl), &[external("de", true)]).is_empty());
    }

    #[test]
    fn english_release_with_english_subtitles_needs_german() {
        let file = media(&["eng"], &[("eng", false)]);
        assert_eq!(
            text(&needs(&wanted(), Some(&file), &[])),
            ["de:normal", "en:forced"]
        );
        // A forced English track covers the forced need, not the German one.
        let forced = media(&["eng"], &[("eng", true)]);
        assert_eq!(text(&needs(&wanted(), Some(&forced), &[])), ["de:normal"]);
    }

    #[test]
    fn external_files_cover_their_language() {
        let file = media(&["eng"], &[]);
        assert_eq!(
            text(&needs(&wanted(), Some(&file), &[external("de", false)])),
            ["en:forced"]
        );
        // A forced file covers no normal need, a file without language nothing.
        let unnamed = External {
            file_name: "Film.srt".to_owned(),
            language: None,
            forced: false,
            hearing_impaired: false,
        };
        assert_eq!(
            text(&needs(
                &wanted(),
                Some(&file),
                &[external("de", true), unnamed]
            )),
            ["de:normal", "en:forced"]
        );
    }

    #[test]
    fn unknown_media_info_covers_nothing_and_needs_no_forced() {
        assert_eq!(
            text(&needs(&wanted(), None, &[])),
            ["de:normal", "en:normal"]
        );
        let unknown_audio = media(&["und"], &[("und", false)]);
        assert_eq!(
            text(&needs(&wanted(), Some(&unknown_audio), &[])),
            ["de:normal", "en:normal"]
        );
        // A first audio language that is not wanted needs no forced subtitle.
        let french = media(&["fre"], &[]);
        assert_eq!(
            text(&needs(&wanted(), Some(&french), &[])),
            ["de:normal", "en:normal"]
        );
    }

    #[test]
    fn languages_of_file_names() {
        for (part, code) in [
            ("de", Some("ger")),
            ("ger", Some("ger")),
            ("deu", Some("ger")),
            ("German", Some("ger")),
            ("Deutsch", Some("ger")),
            ("englisch", Some("eng")),
            ("pt-BR", Some("por")),
            ("und", None),
            ("Kommentar", None),
            ("1", None),
        ] {
            assert_eq!(part_language(part).as_deref(), code, "{part}");
        }
        assert_eq!(alpha2("ger").as_deref(), Some("de"));
        assert_eq!(alpha2("heb").as_deref(), Some("he"));
        assert_eq!(alpha2("yid").as_deref(), Some("yi"));
        assert_eq!(alpha2("fil"), None);
    }
}
