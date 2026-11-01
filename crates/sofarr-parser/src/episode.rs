//! Episode information in release titles of series.
//!
//! Own approach on top of the tokenizer: the first token that marks an episode
//! (`S01E02`, `1x02`, `S01`, `Season 1`/`Staffel 1`, an air date, or an absolute number
//! after ` - `) ends the series title; numbers are read from that token and its
//! neighbours. Clean room: see `CLAUDE.md`.

use crate::{
    ParsedRelease,
    token::{self, Sep, Token},
};

/// Episode part of a release title.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EpisodeInfo {
    pub series_title: String,
    /// `None` for daily and absolute-only releases.
    pub season: Option<u16>,
    /// Last season of a multi-season pack (`S01-S03`).
    pub last_season: Option<u16>,
    pub episodes: Vec<u16>,
    pub absolute_episodes: Vec<u16>,
    /// `(year, month, day)` of daily shows.
    pub air_date: Option<(u16, u8, u8)>,
    /// A whole season without episode numbers.
    pub full_season: bool,
    /// Quality, group, languages, … of the release (title fields refer to the movie
    /// layout and are not meaningful here).
    pub release: ParsedRelease,
}

impl EpisodeInfo {
    pub fn is_special(&self) -> bool {
        self.season == Some(0)
    }

    pub fn is_multi_season(&self) -> bool {
        self.last_season.is_some_and(|l| Some(l) != self.season)
    }

    pub fn is_daily(&self) -> bool {
        self.air_date.is_some()
    }
}

fn digits(s: &str) -> Option<u16> {
    (!s.is_empty() && s.len() <= 4 && s.bytes().all(|b| b.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
}

/// `s01e02e03`, `s01e02-03` (inside one token), `s01`, `s2026e10` → (season, episodes).
fn season_episode(key: &str) -> Option<(u16, Vec<u16>)> {
    let rest = key.strip_prefix('s')?;
    let season_len = rest.bytes().take_while(u8::is_ascii_digit).count();
    if season_len == 0 || season_len > 4 {
        return None;
    }
    let season = digits(&rest[..season_len])?;
    let mut rest = &rest[season_len..];
    let mut episodes = Vec::new();
    while !rest.is_empty() {
        rest = rest.strip_prefix('-').unwrap_or(rest);
        rest = rest.strip_prefix('e')?;
        let n = rest.bytes().take_while(u8::is_ascii_digit).count();
        if n == 0 || n > 4 {
            return None;
        }
        episodes.push(digits(&rest[..n])?);
        rest = &rest[n..];
    }
    Some((season, episodes))
}

/// `1x02`, `01x02x03` → (season, episodes).
fn cross(key: &str) -> Option<(u16, Vec<u16>)> {
    let mut parts = key.split('x');
    let season = parts.next().and_then(digits).filter(|s| *s < 100)?;
    let episodes: Option<Vec<u16>> = parts
        .map(|p| digits(p).filter(|_| (2..=3).contains(&p.len())))
        .collect();
    let episodes = episodes.filter(|e| !e.is_empty())?;
    Some((season, episodes))
}

/// Continues an episode list with following tokens: `-E03`, `E03`, `-03`.
fn more_episodes(tokens: &[Token<'_>], mut i: usize, episodes: &mut Vec<u16>) {
    while let Some(t) = tokens.get(i) {
        let next = t
            .key
            .strip_prefix('e')
            .and_then(digits)
            .or_else(|| (t.sep == Sep::Dash).then(|| digits(&t.key)).flatten());
        match next {
            Some(n)
                if episodes
                    .last()
                    .is_some_and(|&last| n > last && n - last < 50) =>
            {
                let last = *episodes.last().unwrap_or(&n);
                // A dash means a range: `E01-E03` covers E02 as well.
                if t.sep == Sep::Dash {
                    episodes.extend(last + 1..=n);
                } else {
                    episodes.push(n);
                }
                i += 1;
            }
            _ => break,
        }
    }
}

fn season_word(key: &str) -> bool {
    matches!(
        key,
        "season" | "staffel" | "saison" | "temporada" | "stagione"
    )
}

/// What marks the start of the episode part.
enum Marker {
    Episode { season: u16, episodes: Vec<u16> },
    Season { season: u16, last: Option<u16> },
    Daily { date: (u16, u8, u8) },
    Absolute { numbers: Vec<u16> },
}

fn marker_at(tokens: &[Token<'_>], i: usize, title_start: usize) -> Option<Marker> {
    let t = &tokens[i];
    let key = t.key.as_str();
    if let Some((season, mut episodes)) = season_episode(key).or_else(|| cross(key)) {
        if episodes.is_empty() {
            // Season pack, maybe multi-season `S01-S03` or `S01.S02`.
            let last = tokens
                .get(i + 1)
                .and_then(|n| season_episode(&n.key))
                .filter(|(s, e)| e.is_empty() && *s > season)
                .map(|(s, _)| s);
            return Some(Marker::Season { season, last });
        }
        more_episodes(tokens, i + 1, &mut episodes);
        return Some(Marker::Episode { season, episodes });
    }
    if season_word(key)
        && let Some(season) = tokens
            .get(i + 1)
            .and_then(|n| digits(&n.key))
            .filter(|s| *s < 100)
    {
        // `Season 1 Episode 2`
        if let (Some(word), Some(ep)) = (tokens.get(i + 2), tokens.get(i + 3))
            && matches!(word.key.as_str(), "episode" | "folge" | "ep")
            && let Some(ep) = digits(&ep.key)
        {
            return Some(Marker::Episode {
                season,
                episodes: vec![ep],
            });
        }
        let last = match (tokens.get(i + 2), tokens.get(i + 3)) {
            (Some(dash), _) if dash.sep == Sep::Dash => digits(&dash.key).filter(|l| *l > season),
            (Some(word), Some(n)) if word.key == "to" || word.key == "bis" => {
                digits(&n.key).filter(|l| *l > season)
            }
            _ => None,
        };
        return Some(Marker::Season { season, last });
    }
    // Air date: 2026.10.31 / 2026-10-31.
    if let (Some(year), Some(m), Some(d)) = (
        digits(key).filter(|y| (1900..2100).contains(y) && key.len() == 4),
        tokens
            .get(i + 1)
            .filter(|n| n.key.len() == 2)
            .and_then(|n| digits(&n.key)),
        tokens
            .get(i + 2)
            .filter(|n| n.key.len() == 2)
            .and_then(|n| digits(&n.key)),
    ) && (1..=12).contains(&m)
        && (1..=31).contains(&d)
        && i > title_start
    {
        return Some(Marker::Daily {
            date: (year, u8::try_from(m).ok()?, u8::try_from(d).ok()?),
        });
    }
    // Absolute numbering (anime): `Title - 012 [1080p]`, `Title - 12v2`, not a year. Only
    // before any year or resolution, and followed by a bracket, a dash or nothing.
    let clean_title = tokens[title_start..i].iter().all(|p| {
        let year = p.key.len() == 4 && digits(&p.key).is_some_and(|y| (1900..2100).contains(&y));
        let resolution = p.key.ends_with('p') && digits(&p.key[..p.key.len() - 1]).is_some();
        !year && !resolution
    });
    let next_ok = tokens
        .get(i + 1)
        .is_none_or(|n| matches!(n.sep, Sep::Bracket | Sep::Dash) || n.bracketed);
    if t.sep == Sep::Dash && i > title_start && clean_title && next_ok {
        let (num, _version) = key.split_once('v').unwrap_or((key, ""));
        if (2..=4).contains(&num.len())
            && let Some(n) = digits(num)
            && !(1900..2100).contains(&n)
        {
            let mut numbers = vec![n];
            more_episodes(tokens, i + 1, &mut numbers);
            return Some(Marker::Absolute { numbers });
        }
    }
    None
}

/// Parses the episode part of a release title; `None` if it names no episode or season.
pub fn parse_episode(title: &str) -> Option<EpisodeInfo> {
    let input = token::strip_extension(title);
    let tokens = token::tokenize(input);
    // A leading bracketed group (`[SubsPlease] Title - 01`) is not part of the title.
    let title_start = tokens.iter().take_while(|t| t.bracketed).count();
    // A marker needs a series title before it (`S86.2026.720p` is a movie).
    let (index, marker) = (title_start + 1..tokens.len())
        .find_map(|i| marker_at(&tokens, i, title_start).map(|m| (i, m)))?;
    let series_title =
        crate::title::title_text(input, &tokens, title_start..index).unwrap_or_default();
    let mut release = crate::parse(title);
    if release.release_group.is_none() && title_start > 0 {
        // Anime style: the leading bracket names the group.
        release.release_group = Some(tokens[0].text.to_owned());
    }
    let mut info = EpisodeInfo {
        series_title,
        release,
        ..EpisodeInfo::default()
    };
    match marker {
        Marker::Episode { season, episodes } => {
            info.season = Some(season);
            info.episodes = episodes;
        }
        Marker::Season { season, last } => {
            info.season = Some(season);
            info.last_season = last;
            info.full_season = true;
        }
        Marker::Daily { date } => info.air_date = Some(date),
        Marker::Absolute { numbers } => info.absolute_episodes = numbers,
    }
    Some(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(title: &str) -> EpisodeInfo {
        parse_episode(title).unwrap_or_else(|| panic!("no episode in {title}"))
    }

    #[test]
    fn standard_episode() {
        let e = ep("Kleine.Insel.S02E05.German.1080p.WEB.h264-GRUPPE");
        assert_eq!(e.series_title, "Kleine Insel");
        assert_eq!(e.season, Some(2));
        assert_eq!(e.episodes, [5]);
        assert_eq!(e.release.release_group.as_deref(), Some("GRUPPE"));
        assert_eq!(e.release.quality.resolution, crate::Resolution::R1080);
    }

    #[test]
    fn multi_episode_forms() {
        assert_eq!(ep("Show.S01E01E02.720p.HDTV-X").episodes, [1, 2]);
        assert_eq!(ep("Show.S01E01-E03.720p.HDTV-X").episodes, [1, 2, 3]);
        assert_eq!(ep("Show S01E01-03 720p").episodes, [1, 2, 3]);
        assert_eq!(ep("Show.S01E01.E02.720p").episodes, [1, 2]);
    }

    #[test]
    fn cross_notation() {
        let e = ep("Old Show 3x07 Title of Episode");
        assert_eq!((e.season, e.episodes.as_slice()), (Some(3), &[7][..]));
        assert_eq!(e.series_title, "Old Show");
        // 1920x1080 is no episode: the second part is too long.
        assert!(parse_episode("Movie.2020.1920x1080.x264").is_none());
    }

    #[test]
    fn season_packs() {
        let e = ep("Kleine.Insel.S03.German.DL.1080p.BluRay.x264-GRP");
        assert!(e.full_season);
        assert_eq!(e.season, Some(3));
        assert!(e.episodes.is_empty());
        let m = ep("Kleine.Insel.S01-S04.1080p.WEB-DL");
        assert!(m.is_multi_season());
        assert_eq!(m.last_season, Some(4));
        let s = ep("Kleine Insel Staffel 2 German 720p");
        assert_eq!((s.season, s.full_season), (Some(2), true));
        let w = ep("Kleine Insel Season 1 Episode 4 720p");
        assert_eq!((w.season, w.episodes.as_slice()), (Some(1), &[4][..]));
    }

    #[test]
    fn specials_and_years_in_titles() {
        let e = ep("Doctor.Show.2005.S00E12.Special.720p");
        assert!(e.is_special());
        assert_eq!(e.series_title, "Doctor Show 2005");
        let y = ep("1883.S01E02.1080p.WEB");
        assert_eq!(y.series_title, "1883");
        assert_eq!(y.episodes, [2]);
    }

    #[test]
    fn daily_shows() {
        let e = ep("The.Late.Talk.2026.10.31.Guest.Name.720p.WEB");
        assert_eq!(e.air_date, Some((2026, 10, 31)));
        assert_eq!(e.series_title, "The Late Talk");
        assert!(e.is_daily());
    }

    #[test]
    fn absolute_anime() {
        let e = ep("[SubGroup] Sky Pirates - 012 [1080p].mkv");
        assert_eq!(e.series_title, "Sky Pirates");
        assert_eq!(e.absolute_episodes, [12]);
        assert_eq!(e.release.release_group.as_deref(), Some("SubGroup"));
        assert_eq!(ep("[G] Sky Pirates - 12v2 (720p)").absolute_episodes, [12]);
    }

    #[test]
    fn movies_are_not_episodes() {
        for title in [
            "Kleiner.Garten.2019.1080p.WEB-DL-GRP",
            "Film (2004) 720p BluRay",
            "Some.Movie.1999.German.DL.1080p",
            "Wayward.2026.1080p.WEB.H264-13",
            "S86.2026.720p.WEB-DL.AAC2.0.H.264-X",
            "K-12.2019.1080p.AMZN.WEB-DL.DDP2.0.H.264-NTG",
            "Whitlam.Dismissal.-.50.Years.2025.1080p.WEB-DL",
            "Formula 1 - Grand Prix-Rennen-25 Oktober 2026-720p.WEB-DL",
        ] {
            assert_eq!(parse_episode(title), None, "{title}");
        }
    }
}
