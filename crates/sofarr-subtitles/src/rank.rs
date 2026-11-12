//! Match quality of subtitle candidates (spec subtitles, "Match quality"): a file-hash
//! match first, then the number of release attributes shared with the video's release
//! (group, source, resolution, edition), then a match of the movie or episode only. A
//! candidate whose release name names another season or episode than the video is
//! discarded; a hearing-impaired candidate is taken only when no other exists.

use std::cmp::Reverse;

use sofarr_parser::{EpisodeInfo, ParsedRelease, Resolution, Source};

/// A subtitle the provider offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The provider's subtitle id.
    pub subtitle_id: String,
    /// The file to download.
    pub file_id: i64,
    /// ISO 639-1 as the provider names it.
    pub language: String,
    /// The release name the subtitle was made for (may be empty).
    pub release: String,
    pub hearing_impaired: bool,
    /// A forced subtitle (only the foreign-language parts).
    pub foreign_parts_only: bool,
    /// The provider matched it by the video's file hash.
    pub moviehash_match: bool,
    pub download_count: u64,
}

/// What is needed: a language and normal or forced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wanted<'a> {
    /// ISO 639-1.
    pub language: &'a str,
    pub forced: bool,
}

/// The video a subtitle is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Video<'a> {
    /// Its release (scene) name, or its file name.
    pub release: &'a str,
    /// Season and episode of an episode file; `None` for a movie.
    pub episode: Option<(u16, u16)>,
}

/// How well a candidate matches the video; greater is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchQuality {
    /// Only the movie or episode matches.
    Title,
    /// So many release attributes (group, source, resolution, edition) match.
    Release(u8),
    /// The provider's file hash matches the video.
    Hash,
}

impl MatchQuality {
    /// The stored and shown form: `hash`, `release:<n>`, `title`.
    pub fn label(self) -> String {
        match self {
            Self::Hash => "hash".to_owned(),
            Self::Release(n) => format!("release:{n}"),
            Self::Title => "title".to_owned(),
        }
    }
}

fn parse(release: &str) -> (ParsedRelease, Option<EpisodeInfo>) {
    match sofarr_parser::parse_episode(release) {
        Some(info) => (info.release.clone(), Some(info)),
        None => (sofarr_parser::parse(release), None),
    }
}

/// Whether a parsed release names another season or episode than `(season, episode)`.
fn other_episode(info: Option<&EpisodeInfo>, (season, episode): (u16, u16)) -> bool {
    let Some(info) = info else {
        return false;
    };
    match info.season {
        Some(s) if s != season => !info.last_season.is_some_and(|l| (s..=l).contains(&season)),
        Some(_) => {
            !info.full_season && !info.episodes.is_empty() && !info.episodes.contains(&episode)
        }
        None => false,
    }
}

fn shared_attributes(video: &ParsedRelease, candidate: &ParsedRelease) -> u8 {
    let same_text = |a: &Option<String>, b: &Option<String>| match (a, b) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    };
    let group = same_text(&video.release_group, &candidate.release_group);
    let source =
        video.quality.source != Source::Unknown && video.quality.source == candidate.quality.source;
    let resolution = video.quality.resolution != Resolution::Unknown
        && video.quality.resolution == candidate.quality.resolution;
    let edition = same_text(&video.edition, &candidate.edition);
    [group, source, resolution, edition]
        .into_iter()
        .map(u8::from)
        .sum()
}

/// The usable candidates for `wanted`, best first, with their match quality. Ties keep
/// the more downloaded candidate first, then the provider's order.
pub fn rank<'a>(
    candidates: &'a [Candidate],
    wanted: Wanted<'_>,
    video: Video<'_>,
) -> Vec<(MatchQuality, &'a Candidate)> {
    let (video_release, _) = parse(video.release);
    let mut usable: Vec<(MatchQuality, &Candidate)> = candidates
        .iter()
        .filter(|c| c.language.eq_ignore_ascii_case(wanted.language))
        .filter(|c| c.foreign_parts_only == wanted.forced)
        .filter_map(|c| {
            let (release, info) = parse(&c.release);
            if let Some(episode) = video.episode
                && other_episode(info.as_ref(), episode)
            {
                return None;
            }
            let quality = if c.moviehash_match {
                MatchQuality::Hash
            } else {
                match shared_attributes(&video_release, &release) {
                    0 => MatchQuality::Title,
                    n => MatchQuality::Release(n),
                }
            };
            Some((quality, c))
        })
        .collect();
    if usable.iter().any(|(_, c)| !c.hearing_impaired) {
        usable.retain(|(_, c)| !c.hearing_impaired);
    }
    usable.sort_by_key(|(quality, c)| (Reverse(*quality), Reverse(c.download_count)));
    usable
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, release: &str) -> Candidate {
        Candidate {
            subtitle_id: id.to_owned(),
            file_id: 1,
            language: "de".to_owned(),
            release: release.to_owned(),
            hearing_impaired: false,
            foreign_parts_only: false,
            moviehash_match: false,
            download_count: 0,
        }
    }

    fn ids(ranked: &[(MatchQuality, &Candidate)]) -> Vec<String> {
        ranked
            .iter()
            .map(|(q, c)| format!("{}:{}", c.subtitle_id, q.label()))
            .collect()
    }

    const DE: Wanted<'static> = Wanted {
        language: "de",
        forced: false,
    };

    #[test]
    fn hash_beats_release_attributes_beats_title() {
        let video = Video {
            release: "Der.Film.2019.German.DL.1080p.BluRay.x264-GRP",
            episode: None,
        };
        let mut hashed = candidate("h", "Der.Film.2019.720p.WEB.h264-OTHER");
        hashed.moviehash_match = true;
        let candidates = [
            candidate("t", "Der Film"),
            candidate("r1", "Der.Film.2019.720p.BluRay.x264-OTHER"),
            candidate("r3", "Der.Film.2019.1080p.BluRay.x264-GRP"),
            hashed,
        ];
        assert_eq!(
            ids(&rank(&candidates, DE, video)),
            ["h:hash", "r3:release:3", "r1:release:1", "t:title"]
        );
    }

    #[test]
    fn language_kind_and_episode_must_fit() {
        let video = Video {
            release: "Kleine.Insel.S01E02.German.1080p.WEB.h264-GRP",
            episode: Some((1, 2)),
        };
        let mut english = candidate("en", "Kleine.Insel.S01E02.1080p.WEB.h264-GRP");
        english.language = "en".to_owned();
        let mut forced = candidate("forced", "Kleine.Insel.S01E02.1080p.WEB.h264-GRP");
        forced.foreign_parts_only = true;
        let candidates = [
            english,
            forced,
            candidate("wrong-episode", "Kleine.Insel.S01E03.1080p.WEB.h264-GRP"),
            candidate("wrong-season", "Kleine.Insel.S02E02.1080p.WEB.h264-GRP"),
            candidate("pack", "Kleine.Insel.S01.1080p.WEB.h264-GRP"),
            candidate("double", "Kleine.Insel.S01E01E02.1080p.WEB.h264-GRP"),
            candidate("right", "Kleine.Insel.S01E02.720p.WEB.h264-GRP"),
            candidate("unnamed", ""),
        ];
        assert_eq!(
            ids(&rank(&candidates, DE, video)),
            [
                "pack:release:3",
                "double:release:3",
                "right:release:2",
                "unnamed:title"
            ]
        );
        let forced_only = rank(
            &candidates,
            Wanted {
                language: "de",
                forced: true,
            },
            video,
        );
        assert_eq!(ids(&forced_only), ["forced:release:3"]);
    }

    #[test]
    fn hearing_impaired_only_without_another_candidate() {
        let video = Video {
            release: "Der.Film.2019.1080p.BluRay.x264-GRP",
            episode: None,
        };
        let mut sdh = candidate("sdh", "Der.Film.2019.1080p.BluRay.x264-GRP");
        sdh.hearing_impaired = true;
        let plain = candidate("plain", "Der Film");
        assert_eq!(
            ids(&rank(&[sdh.clone(), plain], DE, video)),
            ["plain:title"]
        );
        assert_eq!(ids(&rank(&[sdh], DE, video)), ["sdh:release:3"]);
    }

    #[test]
    fn ties_go_to_the_more_downloaded() {
        let video = Video {
            release: "Der.Film.2019.1080p.BluRay.x264-GRP",
            episode: None,
        };
        let mut popular = candidate("popular", "");
        popular.download_count = 900;
        let rare = candidate("rare", "");
        assert_eq!(
            ids(&rank(&[rare, popular], DE, video)),
            ["popular:title", "rare:title"]
        );
    }
}
