//! Episode information in release titles of series.
//!
//! Own approach on top of the tokenizer: the first token that marks an episode
//! (`S01E02`, `1x02`, `S01`, `Season 1`/`Staffel 1`, `E02` without a season, an air
//! date, or – behind a leading group – an absolute number after ` - `, in `[03]` or as a
//! batch `(23-26)`) ends the series title; numbers are read from that token and its
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

fn is_year(key: &str) -> bool {
    key.len() == 4 && digits(key).is_some_and(|y| (1900..2100).contains(&y))
}

fn is_resolution(key: &str) -> bool {
    key.ends_with('p') && digits(&key[..key.len() - 1]).is_some()
}

/// `e06`, `e06e07` without a season → episodes (one to three digits each).
fn bare_episodes(key: &str) -> Option<Vec<u16>> {
    let mut rest = key.strip_prefix('e')?;
    let mut episodes = Vec::new();
    loop {
        let n = rest.bytes().take_while(u8::is_ascii_digit).count();
        if n == 0 || n > 3 {
            return None;
        }
        episodes.push(digits(&rest[..n])?);
        rest = &rest[n..];
        if rest.is_empty() {
            return Some(episodes);
        }
        rest = rest.strip_prefix('e')?;
    }
}

/// An anime episode number in square brackets: `[06]`, `[052]`, `[06v2]` – two or three
/// digits, not a bare vertical resolution.
fn bracket_number(token: &Token<'_>) -> Option<u16> {
    if token.opener != Some('[') {
        return None;
    }
    let (num, _version) = token.key.split_once('v').unwrap_or((&token.key, ""));
    digits(num)
        .filter(|_| (2..=3).contains(&num.len()))
        .filter(|n| !matches!(n, 480 | 576 | 720))
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

/// Whether token `i` is joined to its predecessor by a single `-` without spaces
/// (`E01-03`, unlike `E01 - 03 Episode Title`).
fn tight_dash(tokens: &[Token<'_>], i: usize) -> bool {
    tokens[i].sep == Sep::Dash && i > 0 && tokens[i - 1].end + 1 == tokens[i].start
}

/// Continues an episode list with following tokens: `-E03`, `E03`, `-03`, and – with a
/// known season – a repeated `S01E03` of the same season.
fn more_episodes(tokens: &[Token<'_>], mut i: usize, season: Option<u16>, episodes: &mut Vec<u16>) {
    while let Some(t) = tokens.get(i) {
        // A bare number continues a standard episode list only after a tight dash;
        // absolute numbers (`season == None`) keep the looser anime form `01 - 03`.
        let bare_ok = t.sep == Sep::Dash && (season.is_none() || tight_dash(tokens, i));
        let next = t
            .key
            .strip_prefix('e')
            .and_then(digits)
            .or_else(|| bare_ok.then(|| digits(&t.key)).flatten())
            .or_else(|| {
                let (s, eps) = season_episode(&t.key)?;
                (Some(s) == season && eps.len() == 1).then(|| eps[0])
            });
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
            if anime_season_in_title(tokens, i + 1, title_start) {
                return None;
            }
            // Season pack, maybe multi-season `S01-S03` or `S01.S02`.
            let last = tokens
                .get(i + 1)
                .and_then(|n| season_episode(&n.key))
                .filter(|(s, e)| e.is_empty() && *s > season)
                .map(|(s, _)| s);
            return Some(Marker::Season { season, last });
        }
        more_episodes(tokens, i + 1, Some(season), &mut episodes);
        return Some(Marker::Episode { season, episodes });
    }
    if season_word(key)
        && let Some(season) = tokens
            .get(i + 1)
            .and_then(|n| digits(&n.key))
            .filter(|s| *s < 100)
    {
        if anime_season_in_title(tokens, i + 2, title_start)
            || dash_after_season_word(tokens, i, title_start)
        {
            return None;
        }
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
    // `Title.E06` names no season; it is the first one.
    if !t.bracketed
        && let Some(mut episodes) = bare_episodes(key)
    {
        more_episodes(tokens, i + 1, Some(1), &mut episodes);
        return Some(Marker::Episode {
            season: 1,
            episodes,
        });
    }
    absolute_at(tokens, i, title_start)
        .or_else(|| batch_at(tokens, i, title_start))
        .or_else(|| bracket_number_at(tokens, i, title_start))
}

/// Behind a leading group, a bracket that directly follows the title.
fn directly_after_title(tokens: &[Token<'_>], i: usize, title_start: usize) -> bool {
    title_start > 0 && i > title_start && tokens[i].sep == Sep::Bracket && !tokens[i - 1].bracketed
}

/// Anime batch `[Grp] Title (23-26)`: a rising range of fewer than 50 numbers.
fn batch_at(tokens: &[Token<'_>], i: usize, title_start: usize) -> Option<Marker> {
    let (first, last) = (&tokens[i], tokens.get(i + 1)?);
    let closed = tokens.get(i + 2).is_none_or(|n| n.sep == Sep::Bracket);
    if !directly_after_title(tokens, i, title_start)
        || first.opener != Some('(')
        || last.opener != Some('(')
        || !tight_dash(tokens, i + 1)
        || !closed
        || is_year(&first.key)
        || is_year(&last.key)
    {
        return None;
    }
    let (from, to) = (digits(&first.key)?, digits(&last.key)?);
    (from < to && to - from < 50).then(|| Marker::Absolute {
        numbers: (from..=to).collect(),
    })
}

/// `[Grp].Title.[06][1080p]`: an episode number alone in square brackets after the title.
fn bracket_number_at(tokens: &[Token<'_>], i: usize, title_start: usize) -> Option<Marker> {
    let alone = tokens.get(i + 1).is_none_or(|n| n.sep == Sep::Bracket);
    if !directly_after_title(tokens, i, title_start) || !alone {
        return None;
    }
    bracket_number(&tokens[i]).map(|n| Marker::Absolute { numbers: vec![n] })
}

/// `[Grp][Title][06][1080p]`: group, title and number each in their own brackets.
/// Returns the title's token range and the number.
fn bracketed_title(tokens: &[Token<'_>]) -> Option<(std::ops::Range<usize>, Marker)> {
    let starts: Vec<usize> = (0..tokens.len())
        .filter(|&k| matches!(tokens[k].sep, Sep::Start | Sep::Bracket) && tokens[k].bracketed)
        .collect();
    let (&title, &number) = (starts.get(1)?, starts.get(2)?);
    let alone = tokens.get(number + 1).is_none_or(|n| n.sep == Sep::Bracket);
    let wordy = tokens[title..number]
        .iter()
        .any(|t| digits(&t.key).is_none());
    // `[Grp][Title.S2][05]`: a season as the title's last word makes the number an
    // episode of that season (parser-season-in-title AC-02); elsewhere it is no such form.
    let seasons: Vec<(usize, u16)> = (title..number)
        .filter_map(|k| {
            season_episode(&tokens[k].key)
                .filter(|(_, e)| e.is_empty())
                .map(|(s, _)| (k, s))
        })
        .collect();
    let season = match seasons.as_slice() {
        [] => None,
        [(k, s)] if *k == number - 1 && *k > title => Some(*s),
        _ => return None,
    };
    if starts[0] != 0
        || !alone
        || !wordy
        || tokens[title..number]
            .iter()
            .any(|t| season_episode(&t.key).is_some_and(|(_, e)| !e.is_empty()))
        || !tokens[..=number].iter().all(|t| t.bracketed)
    {
        return None;
    }
    let n = bracket_number(&tokens[number])?;
    Some(match season {
        Some(season) => (
            title..number - 1,
            Marker::Episode {
                season,
                episodes: vec![n],
            },
        ),
        None => (title..number, Marker::Absolute { numbers: vec![n] }),
    })
}

/// `Title.2nd.Season.-.06`: a season word followed by a spaced dash and an absolute
/// number belongs to the title (parser-season-in-title AC-01).
fn dash_after_season_word(tokens: &[Token<'_>], i: usize, title_start: usize) -> bool {
    tokens.get(i + 1).is_some_and(|n| n.sep == Sep::Dash)
        && !tight_dash(tokens, i + 1)
        && absolute_at(tokens, i + 1, title_start).is_some()
}

/// Behind a leading group (anime style), a season followed by ` - 07` is part of the
/// title (`[Grp] Show S2 - 07`): the number is absolute, the release is no season pack.
fn anime_season_in_title(tokens: &[Token<'_>], after: usize, title_start: usize) -> bool {
    title_start > 0 && after < tokens.len() && absolute_at(tokens, after, title_start).is_some()
}

/// Absolute numbering (anime): `Title - 012 [1080p]`, `Title - 12v2`, not a year. Only
/// before any year or resolution, and followed by a bracket, a dash or nothing.
fn absolute_at(tokens: &[Token<'_>], i: usize, title_start: usize) -> Option<Marker> {
    let t = &tokens[i];
    let key = t.key.as_str();
    // `[Grp] Title (2026) - 06`: a year in parentheses may end the title.
    let year_before = title_start > 0
        && i > title_start + 1
        && t.dashed
        && tokens[i - 1].opener == Some('(')
        && is_year(&tokens[i - 1].key);
    let title_end = if year_before { i - 1 } else { i };
    let clean_title = tokens[title_start..title_end]
        .iter()
        .all(|p| !is_year(&p.key) && !is_resolution(&p.key));
    let next_ok = tokens
        .get(i + 1)
        .is_none_or(|n| matches!(n.sep, Sep::Bracket | Sep::Dash) || n.bracketed);
    let dash = t.sep == Sep::Dash || year_before;
    if dash && !t.bracketed && i > title_start && clean_title && next_ok {
        let (num, _version) = key.split_once('v').unwrap_or((key, ""));
        if (2..=4).contains(&num.len())
            && let Some(n) = digits(num)
            && !(1900..2100).contains(&n)
        {
            let mut numbers = vec![n];
            more_episodes(tokens, i + 1, None, &mut numbers);
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
    let (title_range, marker) = (title_start + 1..tokens.len())
        .find_map(|i| marker_at(&tokens, i, title_start).map(|m| (title_start..i, m)))
        .or_else(|| bracketed_title(&tokens))?;
    let series_title = crate::title::title_text(input, &tokens, title_range).unwrap_or_default();
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
        assert_eq!(ep("Show.S01E01.S01E02.720p").episodes, [1, 2]);
        assert_eq!(ep("Show S01E01-E02-E03 720p").episodes, [1, 2, 3]);
        // A different season is no continuation.
        assert_eq!(ep("Show.S01E01.S02E02.720p").episodes, [1]);
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
    fn anime_season_in_title_is_no_pack() {
        // Behind a leading group, `S2 - 07` names the second cour, not a season pack.
        for title in [
            "[Grp] Sky Pirates S2 - 07 (1080p) [ABCD1234]",
            "[Grp].Sky.Pirates.Season.2.-.07.[720p.WEB-DL]",
        ] {
            let e = ep(title);
            assert!(!e.full_season, "{title}");
            assert_eq!(e.season, None, "{title}");
            assert_eq!(e.absolute_episodes, [7], "{title}");
        }
        assert_eq!(
            ep("[Grp] Sky Pirates S2 - 07 (1080p)").series_title,
            "Sky Pirates S2"
        );
        // Without a leading group a season pack stays a season pack.
        assert!(ep("Kleine.Insel.S02.German.720p").full_season);
    }

    #[test]
    fn year_in_parentheses_before_absolute_number() {
        // AC-01
        let e = ep("[SubsPlease].Magic.Knight.Rayearth.(2026).-.06.(1080p).[96B7C6E9]");
        assert_eq!(e.series_title, "Magic Knight Rayearth (2026)");
        assert_eq!(e.absolute_episodes, [6]);
        assert_eq!((e.season, e.full_season), (None, false));
        assert_eq!(
            ep("[Grp] Sky Pirates (2026) - 12v2 [1080p]").absolute_episodes,
            [12]
        );
    }

    #[test]
    fn season_less_episode_marker() {
        // AC-02
        let e = ep("Jyuou.Mujin.Dandivine.E06.SUBFRENCH.1080p.WEB.H264-SLEEPINGFOREST");
        assert_eq!(e.series_title, "Jyuou Mujin Dandivine");
        assert_eq!((e.season, e.episodes.as_slice()), (Some(1), &[6][..]));
        let y = ep("Magic.Knight.Rayearth.2026.E06.SUBFRENCH.1080p.WEB.H264-SLEEPINGFOREST");
        assert_eq!(y.series_title, "Magic Knight Rayearth 2026");
        assert_eq!((y.season, y.episodes.as_slice()), (Some(1), &[6][..]));
        assert_eq!(ep("Show.E06-E08.1080p.WEB-G").episodes, [6, 7, 8]);
        assert_eq!(ep("Show.E06E07.1080p.WEB-G").episodes, [6, 7]);
        // A season marker still wins.
        assert_eq!(ep("Show.S02E06.1080p").season, Some(2));
    }

    #[test]
    fn anime_batch_in_parentheses() {
        // AC-03
        let e = ep("[SubsPlease].Yamato.yo.Towa.ni.-.Rebel.3199.(23-26).(1080p).[Batch]");
        assert_eq!(e.series_title, "Yamato yo Towa ni - Rebel 3199");
        assert_eq!(e.absolute_episodes, [23, 24, 25, 26]);
        assert_eq!((e.season, e.full_season), (None, false));
    }

    #[test]
    fn bracket_number_after_title() {
        // AC-04
        let e =
            ep("[Studio.GreenTea].Kusuriya.no.Hitorigoto.[52][WebRip][HEVC-10bit.1080p.AAC.ASSx2]");
        assert_eq!(e.series_title, "Kusuriya no Hitorigoto");
        assert_eq!(
            (e.season, e.absolute_episodes.as_slice()),
            (None, &[52][..])
        );
        assert_eq!(ep("[Grp].Some.Title.[06v2][WebRip]").absolute_episodes, [6]);
        // A bracketed title behind the group.
        let b = ep("[Nekomoe.kissaten][Chii.Fuyo][06][1080p][JPSC]");
        assert_eq!(b.series_title, "Chii Fuyo");
        assert_eq!(b.absolute_episodes, [6]);
    }

    #[test]
    fn new_forms_reject_non_episodes() {
        // AC-05
        for title in [
            "The Dam Busters (1955) - 1080p HDR - H265 BRRip - NLsub",
            "[Grp].Some.Title.[1080p][WebRip]",
            "[Grp].Some.Title.[2026][WebRip]",
            "[Grp].Some.Title.[720][WebRip]",
            "[Grp].Some.Title.[96B7C6E9]",
            "[Grp].Some.Title.(26-23).[Batch]",
            "[Grp].Some.Title.(01-120).[Batch]",
            "The.Godfather.Trilogy.(1-3).1080p.BluRay.x264-G",
            "Unforgettable (2011-2015) S 1-4",
        ] {
            let absolute = parse_episode(title).map(|e| e.absolute_episodes);
            assert!(
                absolute.as_ref().is_none_or(Vec::is_empty),
                "{title}: {absolute:?}"
            );
        }
        for title in [
            "Show.2026.-.06.1080p",
            "E06.1080p.WEB-G",
            "[Grp].[52][WebRip]",
        ] {
            assert_eq!(parse_episode(title), None, "{title}");
        }
    }

    #[test]
    fn spaced_dash_number_is_no_range() {
        // ` - 23 …` starts the episode title; only a tight `-` makes a range.
        let e = ep("Cartoon Town - 2017 - S16E06 - 23 Bunte Tage - DL");
        assert_eq!(e.episodes, [6]);
        assert_eq!(ep("Show.S03E29-30.720p.WEB-DL").episodes, [29, 30]);
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

    #[test]
    fn season_word_before_spaced_dash_number() {
        // parser-season-in-title AC-01
        let e = ep(
            "[Erai-raws].Kikansha.no.Mahou.wa.Tokubetsu.desu.2nd.Season.-.06.[1080p.CR.WEBRip.HEVC.AAC][MultiSub][BFA1EA3B]",
        );
        assert_eq!(
            e.series_title,
            "Kikansha no Mahou wa Tokubetsu desu 2nd Season"
        );
        assert_eq!((e.season, e.full_season), (None, false));
        assert_eq!(e.absolute_episodes, [6]);
        let s = ep("[Grp] Show 2nd Season - 06 (1080p)");
        assert_eq!(
            (s.series_title.as_str(), s.absolute_episodes.as_slice()),
            ("Show 2nd Season", &[6][..])
        );
        assert_eq!(
            ep("[Grp].Show.Season.-.06v2.[1080p]").absolute_episodes,
            [6]
        );
        assert_eq!(
            ep("[Grp].Show.Season.-.06-07.[1080p]").absolute_episodes,
            [6, 7]
        );
    }

    #[test]
    fn bracketed_title_with_season() {
        // parser-season-in-title AC-02
        for title in [
            "[Nekomoe.kissaten][Sasaki.to.Pii-chan.S2][05][1080p][JPTC]",
            "[Nekomoe.kissaten][Sasaki.to.Pii-chan.S2][05v2][1080p][JPSC]",
        ] {
            let e = ep(title);
            assert_eq!(e.series_title, "Sasaki to Pii-chan", "{title}");
            assert_eq!(
                (e.season, e.episodes.as_slice(), e.full_season),
                (Some(2), &[5][..], false),
                "{title}"
            );
            assert!(e.absolute_episodes.is_empty(), "{title}");
        }
    }

    #[test]
    fn season_forms_stay_unchanged() {
        // parser-season-in-title AC-03
        let pack = ep("Show.Name.Season.2.1080p.WEB-GRP");
        assert_eq!(
            (pack.series_title.as_str(), pack.season, pack.full_season),
            ("Show Name", Some(2), true)
        );
        let multi = ep("Show.Season.1-3.1080p");
        assert_eq!((multi.season, multi.last_season), (Some(1), Some(3)));
        let tight = ep("Show.Season-2.1080p.WEB-GRP");
        assert_eq!((tight.season, tight.full_season), (Some(2), true));
        let no_group = ep("Show.Season.2.-.07.[720p]");
        assert_eq!((no_group.season, no_group.full_season), (Some(2), true));
        let word = ep("Kleine Insel Season 1 Episode 4 720p");
        assert_eq!((word.season, word.episodes.as_slice()), (Some(1), &[4][..]));
        let cour = ep("[Grp].Sky.Pirates.Season.2.-.07.[720p.WEB-DL]");
        assert_eq!(
            (
                cour.series_title.as_str(),
                cour.absolute_episodes.as_slice()
            ),
            ("Sky Pirates Season 2", &[7][..])
        );
        assert_eq!(
            ep("[Nekomoe.kissaten][Chii.Fuyo][06][1080p][JPSC]").absolute_episodes,
            [6]
        );
    }

    #[test]
    fn season_in_title_rejects_non_episodes() {
        // parser-season-in-title AC-04
        for title in ["[Grp][Title.S2][1080p][WEB]", "[Grp][Title.S2][2026]"] {
            assert!(
                parse_episode(title).is_none_or(|e| e.episodes.is_empty()),
                "{title}"
            );
        }
        for title in [
            "[Grp].Title.Season.-.2026.[1080p]",
            "[Grp].Title.Season.-.1080p",
        ] {
            assert!(
                parse_episode(title).is_none_or(|e| e.absolute_episodes.is_empty()),
                "{title}"
            );
        }
    }
}
