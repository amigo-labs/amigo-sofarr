//! File and folder naming from a token pattern such as
//! `{Movie CleanTitle:DE} ({Release Year}) {Quality.Full}{.MediaInfo.Simple}{-Release Group}`.
//!
//! Token syntax follows the public user documentation: a token is written in braces;
//! characters before or after the token name inside the braces are a prefix/suffix
//! that only appears when the value is not empty; the separator between the words of
//! the token name (space, `.`, `_`, `-`) is used between the words of the value;
//! a `:XX` suffix selects a translation.

use sofarr_parser::Revision;

/// How `:` in titles is replaced.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ColonReplacement {
    Delete,
    Dash,
    #[default]
    SpaceDash,
    SpaceDashSpace,
    /// `A: B` → `A - B`, `A:B` → `A-B`.
    Smart,
}

impl ColonReplacement {
    pub fn from_setting(value: &serde_json::Value) -> Self {
        match value.as_str() {
            Some("delete") => Self::Delete,
            Some("dash") => Self::Dash,
            Some("spaceDash") => Self::SpaceDash,
            Some("spaceDashSpace") => Self::SpaceDashSpace,
            Some("smart") => Self::Smart,
            _ => match value.as_u64() {
                // Numeric form (Sonarr): order as listed in the user interface.
                Some(0) => Self::Delete,
                Some(1) => Self::Dash,
                Some(2) => Self::SpaceDash,
                Some(3) => Self::SpaceDashSpace,
                Some(4) => Self::Smart,
                _ => Self::default(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamingConfig {
    pub rename_movies: bool,
    pub replace_illegal_characters: bool,
    pub colon_replacement: ColonReplacement,
    pub standard_movie_format: String,
    pub movie_folder_format: String,
    pub rename_episodes: bool,
    pub standard_episode_format: String,
    pub daily_episode_format: String,
    pub anime_episode_format: String,
    pub series_folder_format: String,
    pub season_folder_format: String,
    pub multi_episode_style: MultiEpisodeStyle,
}

/// How several episodes in one file are written.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MultiEpisodeStyle {
    /// `S01E01-02-03`
    Extend,
    /// `S01E01.S01E02`
    Duplicate,
    /// `S01E01E02E03`
    Repeat,
    /// `S01E01-E02-E03`
    Scene,
    /// `S01E01-03`
    Range,
    /// `S01E01-E03`
    #[default]
    PrefixedRange,
}

impl MultiEpisodeStyle {
    /// Numeric setting (order as listed in the user interface); unknown values fall
    /// back to the prefixed range.
    pub fn from_setting(value: &serde_json::Value) -> Self {
        match value.as_u64() {
            Some(0) => Self::Extend,
            Some(1) => Self::Duplicate,
            Some(2) => Self::Repeat,
            Some(3) => Self::Scene,
            Some(4) => Self::Range,
            _ => Self::PrefixedRange,
        }
    }
}

impl Default for NamingConfig {
    fn default() -> Self {
        Self {
            rename_movies: true,
            replace_illegal_characters: true,
            colon_replacement: ColonReplacement::default(),
            standard_movie_format: "{Movie Title} ({Release Year}) {Quality Full}".to_owned(),
            movie_folder_format: "{Movie Title} ({Release Year})".to_owned(),
            rename_episodes: true,
            standard_episode_format:
                "{Series Title} - S{season:00}E{episode:00} - {Episode Title} {Quality Full}"
                    .to_owned(),
            daily_episode_format: "{Series Title} - {Air-Date} - {Episode Title} {Quality Full}"
                .to_owned(),
            anime_episode_format:
                "{Series Title} - S{season:00}E{episode:00} - {Episode Title} {Quality Full}"
                    .to_owned(),
            series_folder_format: "{Series Title}".to_owned(),
            season_folder_format: "Season {season}".to_owned(),
            multi_episode_style: MultiEpisodeStyle::default(),
        }
    }
}

impl NamingConfig {
    /// Reads `config_naming.json`.
    pub fn from_json(value: &serde_json::Value) -> Self {
        let default = Self::default();
        let text = |key: &str, fallback: String| {
            value[key]
                .as_str()
                .filter(|s| !s.is_empty())
                .map_or(fallback, str::to_owned)
        };
        Self {
            rename_movies: value["renameMovies"]
                .as_bool()
                .unwrap_or(default.rename_movies),
            replace_illegal_characters: value["replaceIllegalCharacters"]
                .as_bool()
                .unwrap_or(default.replace_illegal_characters),
            colon_replacement: ColonReplacement::from_setting(&value["colonReplacementFormat"]),
            standard_movie_format: text("standardMovieFormat", default.standard_movie_format),
            movie_folder_format: text("movieFolderFormat", default.movie_folder_format),
            rename_episodes: value["renameEpisodes"]
                .as_bool()
                .unwrap_or(default.rename_episodes),
            standard_episode_format: text("standardEpisodeFormat", default.standard_episode_format),
            daily_episode_format: text("dailyEpisodeFormat", default.daily_episode_format),
            anime_episode_format: text("animeEpisodeFormat", default.anime_episode_format),
            series_folder_format: text("seriesFolderFormat", default.series_folder_format),
            season_folder_format: text("seasonFolderFormat", default.season_folder_format),
            multi_episode_style: MultiEpisodeStyle::from_setting(&value["multiEpisodeStyle"]),
        }
    }
}

/// Everything a pattern may refer to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamingInput {
    pub title: String,
    /// German title, used for `:DE`.
    pub title_de: Option<String>,
    pub original_title: Option<String>,
    pub year: Option<u16>,
    /// Quality name, e.g. `Bluray-1080p`.
    pub quality: Option<String>,
    pub revision: Revision,
    pub release_group: Option<String>,
    pub edition: Option<String>,
    /// Video and audio codec, e.g. `x264 DTS`.
    pub media_info_simple: Option<String>,
    pub imdb_id: Option<String>,
    pub tmdb_id: Option<i64>,
    /// Series: season and episode numbers of the file.
    pub season: Option<u16>,
    pub episodes: Vec<u16>,
    pub episode_title: Option<String>,
    /// German episode title, used for `:DE`.
    pub episode_title_de: Option<String>,
    /// `YYYY-MM-DD`.
    pub air_date: Option<String>,
    /// `HDR`, `DV HDR10`, … or nothing for SDR.
    pub dynamic_range: Option<String>,
}

/// Formats a pattern. Slashes written in the pattern itself (folder levels) are kept;
/// slashes inside values are replaced.
pub fn format(pattern: &str, input: &NamingInput, config: &NamingConfig) -> String {
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let lead = out.chars().last();
        out.push_str(&render_token(&after[..end], input, config, lead));
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    tidy(&out)
}

/// Collapses doubled separators left behind by empty tokens and trims the ends.
fn tidy(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let last = out.chars().last();
        if c == ' ' && matches!(last, Some(' ') | None) {
            continue;
        }
        out.push(c);
    }
    out.trim_matches(|c: char| c == ' ' || c == '.' || c == '-' || c == '_')
        .to_owned()
}

const SEPARATORS: [char; 4] = [' ', '.', '_', '-'];

/// The known token names (normalised to lower case, single spaces).
const TOKENS: &[&str] = &[
    "movie title",
    "movie cleantitle",
    "movie titlethe",
    "movie cleantitlethe",
    "movie titlefirstcharacter",
    "movie originaltitle",
    "movie cleanoriginaltitle",
    "release year",
    "quality full",
    "quality title",
    "mediainfo simple",
    "release group",
    "edition tags",
    "imdbid",
    "tmdbid",
    "series title",
    "series titleyear",
    "series cleantitle",
    "series cleantitleyear",
    "episode title",
    "episode cleantitle",
    "air date",
    "mediainfo videodynamicrange",
    "season",
    "episode",
];

/// What follows the `:` of a token: a translation (`:DE`) or a number width (`:00`).
#[derive(Debug, Clone, Copy, Default)]
struct Spec<'a> {
    language: Option<&'a str>,
    width: usize,
    /// The character written right before the token in the pattern (`E` of `E{episode}`).
    lead: Option<char>,
}

fn render_token(
    raw: &str,
    input: &NamingInput,
    config: &NamingConfig,
    lead: Option<char>,
) -> String {
    let mut spec = Spec {
        lead,
        ..Spec::default()
    };
    let language;
    let body = match raw.rsplit_once(':') {
        Some((body, lang)) if lang.len() == 2 && lang.chars().all(|c| c.is_ascii_alphabetic()) => {
            language = lang.to_ascii_uppercase();
            spec.language = Some(language.as_str());
            body
        }
        Some((body, width)) if !width.is_empty() && width.chars().all(|c| c == '0') => {
            spec.width = width.len();
            body
        }
        _ => raw,
    };
    // Find the token name inside the body; the rest is prefix and suffix.
    let lower = body.to_ascii_lowercase();
    let mut found = None;
    for token in TOKENS {
        for sep in SEPARATORS {
            let candidate = token.replace(' ', &sep.to_string());
            if let Some(pos) = lower.find(&candidate) {
                let better = found
                    .is_none_or(|(_, len, _, _): (usize, usize, &str, char)| candidate.len() > len);
                if better {
                    found = Some((pos, candidate.len(), *token, sep));
                }
            }
        }
    }
    let Some((pos, len, token, sep)) = found else {
        return format!("{{{raw}}}");
    };
    let prefix = &body[..pos];
    let suffix = &body[pos + len..];
    let value = token_value(token, input, config, spec);
    if value.is_empty() {
        return String::new();
    }
    let joined = if sep == ' ' {
        value
    } else {
        value
            .split(' ')
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join(&sep.to_string())
    };
    // Title case of the token decides the case of the value (`{MOVIE TITLE}`).
    let name = &body[pos..pos + len];
    let numeric = matches!(token, "season" | "episode");
    let joined = if numeric {
        joined
    } else if name
        .chars()
        .filter(char::is_ascii_alphabetic)
        .all(|c| c.is_ascii_uppercase())
    {
        joined.to_uppercase()
    } else if name
        .chars()
        .filter(char::is_ascii_alphabetic)
        .all(|c| c.is_ascii_lowercase())
    {
        joined.to_lowercase()
    } else {
        joined
    };
    format!("{prefix}{joined}{suffix}")
}

fn token_value(token: &str, input: &NamingInput, config: &NamingConfig, spec: Spec<'_>) -> String {
    let language = spec.language;
    let pad = |n: u16| format!("{n:0width$}", width = spec.width.max(1));
    let title_year = |t: String| match input.year {
        Some(y) => format!("{t} ({y})"),
        None => t,
    };
    let episode_title = match language {
        Some("DE") => input
            .episode_title_de
            .as_ref()
            .or(input.episode_title.as_ref()),
        _ => input.episode_title.as_ref(),
    };
    let title = match language {
        Some("DE") => input.title_de.as_deref().unwrap_or(&input.title),
        _ => &input.title,
    };
    let original = input.original_title.as_deref().unwrap_or(&input.title);
    match token {
        "movie title" => title_text(title, config),
        "movie cleantitle" => clean_title(title),
        "movie titlethe" => title_text(&move_article(title), config),
        "movie cleantitlethe" => clean_title(&move_article(title)),
        "movie titlefirstcharacter" => first_character(title),
        "movie originaltitle" => title_text(original, config),
        "movie cleanoriginaltitle" => clean_title(original),
        "release year" => input.year.map(|y| y.to_string()).unwrap_or_default(),
        "quality full" => quality_full(input),
        "quality title" => input.quality.clone().unwrap_or_default(),
        "mediainfo simple" => input.media_info_simple.clone().unwrap_or_default(),
        "release group" => input.release_group.clone().unwrap_or_default(),
        "edition tags" => input.edition.clone().unwrap_or_default(),
        "imdbid" => input.imdb_id.clone().unwrap_or_default(),
        "tmdbid" => input.tmdb_id.map(|i| i.to_string()).unwrap_or_default(),
        "series title" => title_text(title, config),
        "series titleyear" => title_year(title_text(title, config)),
        "series cleantitle" => clean_title(title),
        "series cleantitleyear" => title_year(clean_title(title)),
        "episode title" => episode_title
            .map(|t| title_text(t, config))
            .unwrap_or_default(),
        "episode cleantitle" => episode_title.map(|t| clean_title(t)).unwrap_or_default(),
        "air date" => input.air_date.clone().unwrap_or_default(),
        "mediainfo videodynamicrange" => input.dynamic_range.clone().unwrap_or_default(),
        "season" => input.season.map(pad).unwrap_or_default(),
        "episode" => episode_numbers(
            &input.episodes,
            input.season,
            config.multi_episode_style,
            spec.lead,
            &pad,
        ),
        _ => String::new(),
    }
}

/// A title made safe for file systems according to the naming settings.
fn title_text(title: &str, config: &NamingConfig) -> String {
    let colon = match config.colon_replacement {
        ColonReplacement::Delete => "",
        ColonReplacement::Dash => "-",
        ColonReplacement::SpaceDash => " -",
        ColonReplacement::SpaceDashSpace | ColonReplacement::Smart => " - ",
    };
    let mut out = String::new();
    let mut chars = title.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ':' if config.colon_replacement == ColonReplacement::Smart => {
                if chars.peek() == Some(&' ') {
                    out.push_str(" -");
                } else {
                    out.push('-');
                }
            }
            ':' => {
                out.push_str(colon);
                // `A: B` with space-dash-space must not become `A -  B`.
                if colon.ends_with(' ') && chars.peek() == Some(&' ') {
                    chars.next();
                }
            }
            '\\' | '/' if config.replace_illegal_characters => out.push('+'),
            '?' | '*' | '"' | '<' | '>' | '|' if config.replace_illegal_characters => {}
            '\\' | '/' | '?' | '*' | '"' | '<' | '>' | '|' => {}
            _ => out.push(c),
        }
    }
    out
}

/// Title without punctuation: letters, digits and spaces remain; `&` becomes `and`.
fn clean_title(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if c == '&' {
            out.push_str(" and ");
        } else if c.is_whitespace() || matches!(c, '-' | ':' | '/' | '\\' | '.') {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `The Matrix` → `Matrix, The`.
fn move_article(title: &str) -> String {
    for article in ["The ", "A ", "An ", "Der ", "Die ", "Das "] {
        if let Some(rest) = title.strip_prefix(article)
            && !rest.is_empty()
        {
            return format!("{rest}, {}", article.trim_end());
        }
    }
    title.to_owned()
}

/// First letter or digit of the title without leading article, upper case.
fn first_character(title: &str) -> String {
    let moved = move_article(title);
    moved
        .chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default()
}

fn episode_numbers(
    episodes: &[u16],
    season: Option<u16>,
    style: MultiEpisodeStyle,
    lead: Option<char>,
    pad: &dyn Fn(u16) -> String,
) -> String {
    let prefix = lead
        .filter(char::is_ascii_alphabetic)
        .map(String::from)
        .unwrap_or_default();
    let joined = |sep: &str| {
        episodes
            .iter()
            .map(|&e| pad(e))
            .collect::<Vec<_>>()
            .join(sep)
    };
    let (Some(&first), Some(&last)) = (episodes.first(), episodes.last()) else {
        return String::new();
    };
    if episodes.len() == 1 {
        return pad(first);
    }
    match style {
        MultiEpisodeStyle::Extend => joined("-"),
        // The season is written with the episode's padding: `{season:00}` is the norm.
        MultiEpisodeStyle::Duplicate => {
            let season = season.map(&pad).unwrap_or_default();
            joined(&format!(".S{season}{prefix}"))
        }
        MultiEpisodeStyle::Repeat => joined(&prefix),
        MultiEpisodeStyle::Scene => joined(&format!("-{prefix}")),
        MultiEpisodeStyle::Range => format!("{}-{}", pad(first), pad(last)),
        MultiEpisodeStyle::PrefixedRange => format!("{}-{prefix}{}", pad(first), pad(last)),
    }
}

/// `HDR` information guessed from a release name.
pub fn dynamic_range_from_name(name: &str) -> Option<String> {
    let tokens: Vec<String> = name
        .split(['.', ' ', '_', '-', '[', ']', '(', ')'])
        .map(str::to_ascii_lowercase)
        .collect();
    let has = |t: &str| tokens.iter().any(|x| x == t);
    let dv = has("dv") || has("dovi") || name.to_ascii_lowercase().contains("dolby.vision");
    let hdr = has("hdr") || has("hdr10") || has("hdr10+") || has("hdr10plus");
    match (dv, hdr) {
        (true, true) => Some("DV HDR10".to_owned()),
        (true, false) => Some("DV".to_owned()),
        (false, true) => Some("HDR".to_owned()),
        (false, false) => None,
    }
}

fn quality_full(input: &NamingInput) -> String {
    let Some(quality) = &input.quality else {
        return String::new();
    };
    let mut out = quality.clone();
    let r = input.revision;
    if r.version > 1 {
        if r.repack {
            out.push_str(" Repack");
        } else {
            out.push_str(" Proper");
        }
        if r.version > 2 {
            out.push_str(&format!("{}", r.version));
        }
    }
    if r.real > 0 {
        out.push_str(" REAL");
    }
    out
}

/// Video and audio codec guessed from a release name, e.g. `x265 DTS-HD MA`.
///
/// sofarr reads no media info from files yet; the release name is the best source.
pub fn media_info_from_name(name: &str) -> Option<String> {
    let tokens: Vec<String> = name
        .split(['.', ' ', '_', '-', '[', ']', '(', ')'])
        .map(str::to_ascii_lowercase)
        .collect();
    let has = |t: &str| tokens.iter().any(|x| x == t);
    let video =
        if has("x265") || has("hevc") || has("h265") || name.to_ascii_lowercase().contains("h.265")
        {
            Some("x265")
        } else if has("x264")
            || has("avc")
            || has("h264")
            || name.to_ascii_lowercase().contains("h.264")
        {
            Some("x264")
        } else if has("av1") {
            Some("AV1")
        } else if has("xvid") {
            Some("XviD")
        } else {
            None
        };
    let lower = name.to_ascii_lowercase();
    let audio = if lower.contains("truehd") {
        Some("TrueHD")
    } else if lower.contains("dts-hd") || lower.contains("dtshd") {
        Some("DTS-HD MA")
    } else if lower.contains("dts") {
        Some("DTS")
    } else if lower.contains("eac3") || lower.contains("ddp") || lower.contains("dd+") {
        Some("EAC3")
    } else if lower.contains("ac3")
        || has("dd5")
        || lower.contains("dd5.1")
        || lower.contains("dd2.0")
    {
        Some("AC3")
    } else if lower.contains("flac") {
        Some("FLAC")
    } else if lower.contains("aac") {
        Some("AAC")
    } else {
        None
    };
    match (video, audio) {
        (None, None) => None,
        (v, a) => Some([v, a].into_iter().flatten().collect::<Vec<_>>().join(" ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> NamingInput {
        NamingInput {
            title: "Mission: Impossible - Fallout".to_owned(),
            title_de: Some("Mission: Impossible – Fallout".to_owned()),
            original_title: None,
            year: Some(2018),
            quality: Some("Bluray-1080p".to_owned()),
            revision: Revision::default(),
            release_group: Some("GRP".to_owned()),
            edition: None,
            media_info_simple: Some("x264 DTS".to_owned()),
            imdb_id: Some("tt4912910".to_owned()),
            tmdb_id: Some(353_081),
            ..NamingInput::default()
        }
    }

    fn config() -> NamingConfig {
        NamingConfig {
            colon_replacement: ColonReplacement::SpaceDashSpace,
            ..NamingConfig::default()
        }
    }

    #[test]
    fn formats_file_names_with_prefixes() {
        let name = format(
            "{Movie CleanTitle:DE} ({Release Year}) {Quality.Full}{.MediaInfo.Simple}{-Release Group}",
            &input(),
            &config(),
        );
        assert_eq!(
            name,
            "Mission Impossible Fallout (2018) Bluray-1080p.x264.DTS-GRP"
        );
    }

    #[test]
    fn empty_tokens_drop_their_prefix() {
        let mut i = input();
        i.release_group = None;
        i.media_info_simple = None;
        let name = format(
            "{Movie Title} {Quality Full}{.MediaInfo.Simple}{-Release Group}",
            &i,
            &config(),
        );
        assert_eq!(name, "Mission - Impossible - Fallout Bluray-1080p");
    }

    #[test]
    fn folders_and_first_character() {
        let mut i = input();
        i.title_de = Some("Der Pate".to_owned());
        assert_eq!(
            format(
                "{Movie TitleFirstCharacter:DE}/{Movie Title:DE} ({Release Year})",
                &i,
                &config()
            ),
            "P/Der Pate (2018)"
        );
        assert_eq!(
            format(
                "{Movie TitleThe}",
                &NamingInput {
                    title: "The Thing".to_owned(),
                    ..NamingInput::default()
                },
                &config()
            ),
            "Thing, The"
        );
    }

    #[test]
    fn colon_modes_and_illegal_characters() {
        let i = NamingInput {
            title: "Who? What: Where/When".to_owned(),
            ..NamingInput::default()
        };
        let mut c = config();
        assert_eq!(format("{Movie Title}", &i, &c), "Who What - Where+When");
        c.colon_replacement = ColonReplacement::Delete;
        assert_eq!(format("{Movie Title}", &i, &c), "Who What Where+When");
        c.colon_replacement = ColonReplacement::Dash;
        assert_eq!(format("{Movie Title}", &i, &c), "Who What- Where+When");
    }

    #[test]
    fn revisions_in_quality_full() {
        let mut i = input();
        i.revision = Revision {
            version: 2,
            proper: true,
            repack: false,
            real: 1,
        };
        assert_eq!(
            format("{Quality Full}", &i, &config()),
            "Bluray-1080p Proper REAL"
        );
        i.revision.repack = true;
        assert_eq!(
            format("{Quality.Full}", &i, &config()),
            "Bluray-1080p.Repack.REAL"
        );
    }

    fn episode_input() -> NamingInput {
        NamingInput {
            title: "Small Island".to_owned(),
            title_de: Some("Kleine Insel".to_owned()),
            year: Some(2019),
            quality: Some("WEBDL-1080p".to_owned()),
            season: Some(2),
            episodes: vec![5],
            episode_title: Some("The Storm: Part 1".to_owned()),
            episode_title_de: Some("Der Sturm".to_owned()),
            dynamic_range: Some("HDR".to_owned()),
            imdb_id: Some("tt7777777".to_owned()),
            ..NamingInput::default()
        }
    }

    #[test]
    fn formats_episodes() {
        let c = NamingConfig {
            colon_replacement: ColonReplacement::Smart,
            ..config()
        };
        let pattern = "{Series Title} - S{season:00}E{episode:00} - {Episode CleanTitle} {Quality Full}{.MediaInfo.VideoDynamicRange}";
        assert_eq!(
            format(pattern, &episode_input(), &c),
            "Small Island - S02E05 - The Storm Part 1 WEBDL-1080p.HDR"
        );
        let mut multi = episode_input();
        multi.episodes = vec![5, 6, 7];
        multi.dynamic_range = None;
        assert_eq!(
            format(pattern, &multi, &c),
            "Small Island - S02E05-E07 - The Storm Part 1 WEBDL-1080p"
        );
        for (style, expected) in [
            (MultiEpisodeStyle::Extend, "S02E05-06-07"),
            (MultiEpisodeStyle::Duplicate, "S02E05.S02E06.S02E07"),
            (MultiEpisodeStyle::Repeat, "S02E05E06E07"),
            (MultiEpisodeStyle::Scene, "S02E05-E06-E07"),
        ] {
            let styled = NamingConfig {
                multi_episode_style: style,
                ..c.clone()
            };
            assert_eq!(
                format("S{season:00}E{episode:00}", &multi, &styled),
                expected
            );
        }
        let range = NamingConfig {
            multi_episode_style: MultiEpisodeStyle::Range,
            ..c.clone()
        };
        assert_eq!(
            format("S{season:00}E{episode:00}", &multi, &range),
            "S02E05-07"
        );
        assert_eq!(
            format("{Episode Title}", &episode_input(), &c),
            "The Storm - Part 1"
        );
        assert_eq!(
            format("{Episode Title:DE}", &episode_input(), &c),
            "Der Sturm"
        );
    }

    #[test]
    fn formats_series_folders_and_daily() {
        let c = config();
        let mut i = episode_input();
        assert_eq!(
            format("{Series Title} {ImdbId}", &i, &c),
            "Small Island tt7777777"
        );
        assert_eq!(format("{Series TitleYear}", &i, &c), "Small Island (2019)");
        assert_eq!(format("Staffel {season:00}", &i, &c), "Staffel 02");
        assert_eq!(format("Season {season}", &i, &c), "Season 2");
        i.air_date = Some("2026-10-31".to_owned());
        assert_eq!(
            format("{Series Title} - {Air-Date}", &i, &c),
            "Small Island - 2026-10-31"
        );
        assert_eq!(
            dynamic_range_from_name("Show.S01E01.2160p.DV.HDR10.WEB").as_deref(),
            Some("DV HDR10")
        );
        assert_eq!(dynamic_range_from_name("Show.S01E01.1080p.WEB"), None);
    }

    #[test]
    fn numeric_settings() {
        assert_eq!(
            ColonReplacement::from_setting(&serde_json::json!(4)),
            ColonReplacement::Smart
        );
        assert_eq!(
            MultiEpisodeStyle::from_setting(&serde_json::json!(5)),
            MultiEpisodeStyle::PrefixedRange
        );
    }

    #[test]
    fn unknown_tokens_stay_visible() {
        assert_eq!(
            format("{Unbekannt} x", &input(), &config()),
            "{Unbekannt} x"
        );
    }

    #[test]
    fn media_info_guess() {
        assert_eq!(
            media_info_from_name("Film.2020.1080p.BluRay.DTS-HD.MA.5.1.x265-GRP").as_deref(),
            Some("x265 DTS-HD MA")
        );
        assert_eq!(
            media_info_from_name("Film.2020.German.DL.1080p.WEB.h264-GRP").as_deref(),
            Some("x264")
        );
        assert_eq!(media_info_from_name("Film 2020"), None);
    }

    #[test]
    fn reads_naming_config() {
        let c = NamingConfig::from_json(&serde_json::json!({
            "renameMovies": true, "replaceIllegalCharacters": true,
            "colonReplacementFormat": "spaceDashSpace",
            "standardMovieFormat": "{Movie Title}", "movieFolderFormat": ""
        }));
        assert_eq!(c.colon_replacement, ColonReplacement::SpaceDashSpace);
        assert_eq!(c.standard_movie_format, "{Movie Title}");
        assert_eq!(
            c.movie_folder_format,
            NamingConfig::default().movie_folder_format
        );
    }
}
