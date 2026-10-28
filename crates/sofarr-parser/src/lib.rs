//! Release-name parser: turns raw release titles into structured data.
//!
//! Rules: pure logic only — no IO, no async, no network, no clock.
//! Clean room: see `CLAUDE.md`.
//!
//! Approach: the title is split into tokens ([`token`]), each token is classified via
//! keyword tables ([`classify`]), and the boundary between title and metadata is derived
//! from the classified sequence ([`title`]).

mod classify;
mod model;
mod title;
mod token;

pub use model::{
    HardcodedSubs, Language, Modifier, ParsedRelease, Quality, Resolution, Revision, Source,
};

use classify::Tag;

/// Parses a release title. Never fails: anything unrecognised stays empty or `Unknown`.
pub fn parse(title: &str) -> ParsedRelease {
    let input = token::strip_extension(title);
    let tokens = token::tokenize(input);
    let tags = classify::classify(&tokens);
    let layout = title::layout(input, &tokens, &tags);

    let mut release = ParsedRelease {
        titles: layout
            .titles
            .iter()
            .filter_map(|range| title::title_text(input, &tokens, range.clone()))
            .collect(),
        year: layout.year.map(|(_, year)| year),
        release_group: layout.group.clone().map(|r| input[r].to_owned()),
        ..ParsedRelease::default()
    };

    // A group made of a keyword (`-ATMOS`, `[DVD9]`) still counts as metadata.
    let in_group = |i: usize| {
        tags[i] == Tag::Plain && layout.group_tokens.as_ref().is_some_and(|r| r.contains(&i))
    };
    let metadata: Vec<usize> = (layout.metadata_start..tokens.len())
        .filter(|&i| !in_group(i))
        .collect();

    // Source implied by a modifier, and whether it came from a bare `REMUX` (`BD.Remux`
    // names the disc).
    let mut implied_source: Option<(Source, bool)> = None;
    let mut resolution_from_keyword = false;
    let mut edition: Vec<&str> = Vec::new();
    let mut edition_done = false;
    for &i in &metadata {
        let tag = tags[i];
        match tag {
            Tag::Quality {
                source,
                resolution,
                modifier,
            } => {
                let quality = &mut release.quality;
                match (source, modifier) {
                    // A modifier only implies a source; an explicit source takes precedence.
                    (Some(s), Some(_)) => {
                        let bare =
                            tokens[i].key == "remux" && (i == 0 || tokens[i - 1].key != "bd");
                        implied_source.get_or_insert((s, bare));
                    }
                    (Some(s), None) if quality.source == Source::Unknown => quality.source = s,
                    _ => {}
                }
                if let Some(m) = modifier
                    && quality.modifier == Modifier::None
                {
                    quality.modifier = m;
                }
                // An explicit `1080p` beats a `UHD`/`4K` keyword (`UHD.BluRay.1080p`).
                if let Some(r) = resolution {
                    let keyword = matches!(tokens[i].key.as_str(), "uhd" | "4k");
                    if quality.resolution == Resolution::Unknown
                        || (!keyword && resolution_from_keyword)
                    {
                        quality.resolution = r;
                        resolution_from_keyword = keyword;
                    }
                }
            }
            Tag::Proper => {
                release.revision.proper = true;
                release.revision.version = release.revision.version.max(2);
            }
            Tag::Repack => {
                release.revision.repack = true;
                release.revision.version = release.revision.version.max(2);
            }
            Tag::NumberedFix { repack, version } => {
                if repack {
                    release.revision.repack = true;
                } else {
                    release.revision.proper = true;
                }
                release.revision.version = release.revision.version.max(version);
            }
            Tag::Version(v) => release.revision.version = release.revision.version.max(v),
            Tag::Real => release.revision.real = release.revision.real.saturating_add(1),
            Tag::Language(language) if !release.languages.contains(&language) => {
                release.languages.push(language);
            }
            _ => {}
        }

        // The first edition keyword counts, joined only by a following `Remastered` or
        // `Open Matte` (`Extended.Remastered`); `Remastered.Uncut` is just `Remastered`.
        if !edition_done {
            let joins = matches!(tokens[i].key.as_str(), "remastered" | "open");
            let starts = tag == Tag::Edition && (edition.is_empty() || joins);
            let continues = tag == Tag::Continued && !edition.is_empty();
            if starts || continues {
                edition.push(tokens[i].text);
            } else if !edition.is_empty() {
                edition_done = true;
            }
        }
    }
    if release.quality.source == Source::Unknown
        && let Some((source, bare_remux)) = implied_source
    {
        // A bare `REMUX` with neither source nor resolution says too little.
        if bare_remux && release.quality.resolution == Resolution::Unknown {
            release.quality.modifier = Modifier::None;
        } else {
            release.quality.source = source;
        }
    }
    apply_quality_defaults(&mut release.quality, &tokens, &metadata);
    // `German.DL` is German plus the original track; `DL` alone or with other
    // languages adds nothing.
    if metadata.iter().any(|&i| tags[i] == Tag::DualLanguage)
        && release.languages.contains(&Language::German)
        && !release.languages.contains(&Language::Original)
    {
        release.languages.push(Language::Original);
    }
    // `German.ML` is German, English and the original track.
    if metadata.iter().any(|&i| tags[i] == Tag::MultiLanguage)
        && release.languages.contains(&Language::German)
    {
        for language in [Language::English, Language::Original] {
            if !release.languages.contains(&language) {
                release.languages.push(language);
            }
        }
    }
    if !edition.is_empty() {
        release.edition = Some(edition.join(" "));
    }
    release.hardcoded_subs = tokens[layout.metadata_start..]
        .iter()
        .find_map(hardcoded_subs);
    release
}

/// `SUBBED` and `HC` mark burnt-in subtitles; a short language code glued to `sub(s)`
/// (`NLsub`, `DKsubs`, `Esub`) names them. `SUBS` alone means soft subtitles.
fn hardcoded_subs(token: &token::Token<'_>) -> Option<HardcodedSubs> {
    let key = token.key.as_str();
    if matches!(key, "subbed" | "hc") {
        return Some(HardcodedSubs::Generic);
    }
    let code = key
        .strip_suffix("subs")
        .or_else(|| key.strip_suffix("sub"))?;
    ((1..=3).contains(&code.len()) && code.bytes().all(|b| b.is_ascii_alphabetic()))
        .then(|| HardcodedSubs::Tagged(token.text.to_owned()))
}

/// Audio formats found on Blu-ray discs (prefixes, so `dtshd` or `truehd7` match too).
/// `DDP` is a streaming format and does not count.
const DISC_AUDIO: &[&str] = &[
    "lpcm", "pcm", "truehd", "dts", "flac", "ac3", "eac3", "dd", "atmos",
];

/// Rules that depend on the quality as a whole rather than on a single token.
fn apply_quality_defaults(quality: &mut Quality, tokens: &[token::Token<'_>], metadata: &[usize]) {
    // Does a keyword occur in the metadata, as one token or split over two?
    let has = |glued: &[&str], split: &[(&str, &str)]| {
        metadata.iter().any(|&i| {
            let key = tokens[i].key.as_str();
            glued.contains(&key)
                || tokens
                    .get(i + 1)
                    .is_some_and(|next| split.contains(&(key, next.key.as_str())))
        })
    };

    // DVDs: an untouched disc (`DVD5`, `DVD9`, `DVD-R`) is Radarr's "DVD-R" at 480p,
    // anything else (`DVDRip`, `DVD.REMUX`) is plain DVD without a resolution.
    if quality.source == Source::Dvd {
        // `DVD-9` counts, `DVD 9` does not.
        let dashed_disc = metadata.iter().any(|&i| {
            tokens[i].key == "dvd"
                && tokens.get(i + 1).is_some_and(|next| {
                    next.sep == token::Sep::Dash && matches!(next.key.as_str(), "5" | "9")
                })
        });
        let disc = dashed_disc
            || has(&["dvdr", "dvd5", "dvd9"], &[("dvd", "r")])
            || metadata.iter().any(|&i| {
                let key = tokens[i].key.as_str();
                key.ends_with("xdvd5") || key.ends_with("xdvd9")
            });
        if disc {
            quality.modifier = Modifier::Remux;
            quality.resolution = Resolution::R480;
        } else {
            if quality.modifier == Modifier::Remux {
                quality.modifier = Modifier::None;
            }
            quality.resolution = Resolution::Unknown;
        }
        return;
    }

    // A remux only exists of a Blu-ray; with another explicit source the word is noise.
    if quality.modifier == Modifier::Remux && quality.source != Source::BluRay {
        quality.modifier = Modifier::None;
    }

    // Untouched Blu-ray video names the disc's codec, encodes name the encoder. MPEG-2
    // video is always a full disc; AVC or VC-1 (HEVC only for UHD: plenty of HD encodes
    // are labelled HEVC) is a full disc with the disc's audio listed, otherwise a remux.
    // `COMPLETE` marks a full disc as well.
    let uhd = quality.resolution == Resolution::R2160;
    let mpeg2 = has(&["mpeg2"], &[("mpeg", "2")]);
    let disc_codec = mpeg2 || has(&["avc", "vc1"], &[("vc", "1")]) || (uhd && has(&["hevc"], &[]));
    // A rip is an encode by definition, even when it names the codec (`BDRip.HEVC`).
    let encoder = has(
        &[
            "x264", "x265", "h264", "h265", "bdrip", "brrip", "uhdbdrip", "uhd2bd",
        ],
        &[
            ("x", "264"),
            ("x", "265"),
            ("h", "264"),
            ("h", "265"),
            ("bd", "rip"),
            ("br", "rip"),
        ],
    );
    let disc_audio = metadata.iter().any(|&i| {
        let key = tokens[i].key.as_str();
        !key.starts_with("ddp") && DISC_AUDIO.iter().any(|prefix| key.starts_with(prefix))
    });
    // Streaming audio formats only occur in encodes (`UHD.BluRay.HEVC.DDP`).
    let encode_audio = metadata.iter().any(|&i| {
        let key = tokens[i].key.as_str();
        key.starts_with("ddp") || key.starts_with("aac") || key.starts_with("opus")
    });
    if quality.source == Source::BluRay
        && quality.modifier == Modifier::None
        && !encoder
        && !encode_audio
    {
        if has(&["complete"], &[]) || mpeg2 || (disc_codec && disc_audio) {
            quality.modifier = Modifier::BrDisk;
        } else if disc_codec {
            quality.modifier = Modifier::Remux;
        }
    }

    // A full disc counts as 1080p, whatever its actual resolution.
    if quality.modifier == Modifier::BrDisk {
        quality.resolution = Resolution::R1080;
    }

    // Web and TV releases below 720p are SD.
    if matches!(quality.source, Source::WebDl | Source::WebRip | Source::Tv)
        && matches!(
            quality.resolution,
            Resolution::R360 | Resolution::R540 | Resolution::R576
        )
    {
        quality.resolution = Resolution::R480;
    }

    // A resolution or an encoder without any source is treated as a TV capture.
    if quality.source == Source::Unknown
        && quality.modifier == Modifier::None
        && (quality.resolution != Resolution::Unknown || encoder)
    {
        quality.source = Source::Tv;
        if quality.resolution < Resolution::R720 {
            quality.resolution = Resolution::R480;
        }
    }

    // A Blu-ray without a resolution is 720p, unless it is a rip (`BDRip`), which is SD.
    let rip = has(&["bdrip", "brrip"], &[("bd", "rip"), ("br", "rip")]);
    if quality.source == Source::BluRay
        && quality.modifier == Modifier::None
        && quality.resolution == Resolution::Unknown
        && !rip
    {
        quality.resolution = Resolution::R720;
    }

    // Remuxes without a resolution are HD; there are no remuxes below 1080p.
    if quality.modifier == Modifier::Remux {
        if quality.resolution == Resolution::Unknown {
            quality.resolution = Resolution::R1080;
        } else if quality.resolution < Resolution::R1080 {
            quality.modifier = Modifier::None;
        }
    }

    // Blu-ray, web and TV releases without a resolution are standard definition.
    if quality.resolution == Resolution::Unknown
        && quality.modifier == Modifier::None
        && matches!(
            quality.source,
            Source::BluRay | Source::WebDl | Source::WebRip | Source::Tv
        )
    {
        quality.resolution = Resolution::R480;
    }
}

#[cfg(test)]
mod tests {
    //! All example titles below are invented for these tests.

    use super::*;

    fn title(input: &str) -> Option<String> {
        parse(input).title().map(str::to_owned)
    }

    #[test]
    fn dotted_title_with_year() {
        let r = parse("The.Quiet.Lighthouse.2014.1080p.BluRay.x264-NORDLICHT");
        assert_eq!(r.title(), Some("The Quiet Lighthouse"));
        assert_eq!(r.year, Some(2014));
        assert_eq!(r.quality.source, Source::BluRay);
        assert_eq!(r.quality.resolution, Resolution::R1080);
        assert_eq!(r.quality.modifier, Modifier::None);
        assert_eq!(r.release_group.as_deref(), Some("NORDLICHT"));
    }

    #[test]
    fn spaced_title_with_bracket_year() {
        let r = parse("Paper Kites over Lisbon (2021) [2160p] [WEB-DL]");
        assert_eq!(r.title(), Some("Paper Kites over Lisbon"));
        assert_eq!(r.year, Some(2021));
        assert_eq!(r.quality.resolution, Resolution::R2160);
        assert_eq!(r.quality.source, Source::WebDl);
        assert_eq!(r.release_group, None);
    }

    #[test]
    fn year_inside_title() {
        let r = parse("Escape.From.Station.2088.2019.720p.WEBRip.x264-KOMET");
        assert_eq!(r.title(), Some("Escape From Station 2088"));
        assert_eq!(r.year, Some(2019));
        assert_eq!(r.quality.source, Source::WebRip);
    }

    #[test]
    fn title_made_of_numbers() {
        let r = parse("1922.2017.1080p.WEB-DL.DDP5.1.H.264-ZAHL");
        assert_eq!(r.title(), Some("1922"));
        assert_eq!(r.year, Some(2017));
        assert_eq!(r.quality.source, Source::WebDl);
        assert_eq!(r.release_group.as_deref(), Some("ZAHL"));

        let r = parse("1999.2160p.UHD.BluRay.x265");
        assert_eq!(r.title(), Some("1999"));
        assert_eq!(r.year, None);
    }

    #[test]
    fn missing_group() {
        let r = parse("Glass.Orchard.2010.1080p.BluRay.x264");
        assert_eq!(r.release_group, None);
        assert_eq!(r.title(), Some("Glass Orchard"));
    }

    #[test]
    fn hyphenated_title_is_not_a_group() {
        let r = parse("Saint-Ex");
        assert_eq!(r.title(), Some("Saint-Ex"));
        assert_eq!(r.release_group, None);

        let r = parse("Spider-Moth.Rising.2016.1080p.WEB-DL-FALTER");
        assert_eq!(r.title(), Some("Spider-Moth Rising"));
        assert_eq!(r.release_group.as_deref(), Some("FALTER"));
    }

    #[test]
    fn group_is_last_dash_segment() {
        // Only the last dash segment counts.
        let r = parse("Tin.Harbour.2003.1080p.BluRay.x264-A-TEAM");
        assert_eq!(r.release_group.as_deref(), Some("TEAM"));
    }

    #[test]
    fn leading_bracket_group() {
        let r = parse("[Kirschblüte] Moonlit Ferry 2012 1080p BluRay");
        assert_eq!(r.release_group.as_deref(), Some("Kirschblüte"));
        assert_eq!(r.title(), Some("Moonlit Ferry"));
        assert_eq!(r.year, Some(2012));
    }

    #[test]
    fn leading_parenthesis_is_part_of_title() {
        assert_eq!(
            title("(300) Letters to Nobody (2015) 720p"),
            Some("(300) Letters to Nobody".to_owned())
        );
    }

    #[test]
    fn sources() {
        let cases = [
            ("A.Film.2001.HDTV.x264-G", Source::Tv),
            ("A.Film.2001.DVDRip.XviD-G", Source::Dvd),
            ("A.Film.2001.CAM-G", Source::Cam),
            ("A.Film.2001.HDTS.x264-G", Source::Telesync),
            ("A.Film.2001.TC.x264-G", Source::Telecine),
            ("A.Film.2001.WORKPRINT-G", Source::Workprint),
            ("A.Film.2001.1080p.WEB.h264-G", Source::WebDl),
            ("A.Film.2001.1080p.Blu-Ray.x264-G", Source::BluRay),
            ("A.Film.2001.1080p.BDRip.x264-G", Source::BluRay),
            ("A.Film.2001.1080p-G", Source::Tv),
            ("A.Film.2001-G", Source::Unknown),
        ];
        for (input, expected) in cases {
            assert_eq!(parse(input).quality.source, expected, "{input}");
        }
    }

    #[test]
    fn modifiers() {
        let r = parse("Velvet.Canyon.2018.2160p.UHD.BluRay.REMUX.HDR.HEVC.Atmos-KLANG");
        assert_eq!(r.quality.modifier, Modifier::Remux);
        assert_eq!(r.quality.source, Source::BluRay);
        assert_eq!(r.quality.resolution, Resolution::R2160);

        let r = parse("Velvet.Canyon.2018.1080p.REMUX.AVC-KLANG");
        assert_eq!(r.quality.modifier, Modifier::Remux);
        assert_eq!(r.quality.source, Source::BluRay);

        let r = parse("Velvet.Canyon.2018.COMPLETE.BLURAY-KLANG");
        assert_eq!(r.quality.modifier, Modifier::BrDisk);

        let r = parse("Velvet.Canyon.2018.DVDSCR.x264-KLANG");
        assert_eq!(r.quality.modifier, Modifier::Screener);
        assert_eq!(r.quality.source, Source::Dvd);
    }

    #[test]
    fn revisions() {
        let r = parse("Cold.Meadow.2009.PROPER.1080p.BluRay.x264-G");
        assert!(r.revision.proper);
        assert_eq!(r.revision.version, 2);

        let r = parse("Cold.Meadow.2009.REPACK.720p.WEB-DL-G");
        assert!(r.revision.repack);
        assert_eq!(r.revision.version, 2);

        let r = parse("Cold.Meadow.2009.REAL.PROPER.720p.WEB-DL-G");
        assert_eq!(r.revision.real, 1);
        assert!(r.revision.proper);

        let r = parse("Cold.Meadow.2009.720p.WEB-DL-G");
        assert_eq!(r.revision, Revision::default());
    }

    #[test]
    fn real_in_title_is_not_a_revision() {
        let r = parse("Real.Ghosts.2011.1080p.WEB-DL-G");
        assert_eq!(r.title(), Some("Real Ghosts"));
        assert_eq!(r.revision.real, 0);
    }

    #[test]
    fn editions() {
        let r = parse("Iron.Lantern.1994.Directors.Cut.1080p.BluRay.x264-G");
        assert_eq!(r.edition.as_deref(), Some("Directors Cut"));
        assert_eq!(r.title(), Some("Iron Lantern"));

        // Only the first edition keyword counts.
        let r = parse("Iron.Lantern.1994.Extended.Directors.Cut.1080p.BluRay-G");
        assert_eq!(r.edition.as_deref(), Some("Extended"));

        let r = parse("Iron Lantern (1994) 30th Anniversary Edition 1080p");
        assert_eq!(r.edition.as_deref(), Some("30th Anniversary Edition"));

        let r = parse("Iron.Lantern.Uncut.1080p.BluRay-G");
        assert_eq!(r.title(), Some("Iron Lantern"));
        assert_eq!(r.edition.as_deref(), Some("Uncut"));
    }

    #[test]
    fn languages() {
        let r = parse("Der.Stille.Hafen.2016.German.DL.1080p.BluRay.x264-HAFEN");
        assert_eq!(r.title(), Some("Der Stille Hafen"));
        assert_eq!(r.languages, [Language::German, Language::Original]);

        let r = parse("Le.Petit.Jardin.2013.FRENCH.720p.BluRay-G");
        assert_eq!(r.languages, [Language::French]);

        let r = parse("Quiet.River.2020.1080p.WEB-DL-G");
        assert!(r.languages.is_empty());

        let r = parse("Ein.Kleines.Haus.German.1080p.WEB-DL-G");
        assert_eq!(r.title(), Some("Ein Kleines Haus"));
        assert_eq!(r.languages, [Language::German]);
    }

    #[test]
    fn dual_language_adds_original() {
        let r = parse("Leiser.Sturm.2019.German.DL.1080p.WEB.h264-WIND");
        assert_eq!(r.languages, [Language::German, Language::Original]);
        assert_eq!(r.title(), Some("Leiser Sturm"));

        let r = parse("Leiser.Sturm.2019.German.1080p.WEB.h264-WIND");
        assert_eq!(r.languages, [Language::German]);
    }

    #[test]
    fn french_version_tags() {
        let r = parse("Vent.Calme.2019.MULTi.VF2.1080p.WEB.H265-BRISE");
        assert_eq!(r.languages, [Language::French]);
    }

    #[test]
    fn standard_definition_default() {
        let r = parse("Grey.Harbour.2017.BDRip.x264-NEBEL");
        assert_eq!(r.quality.resolution, Resolution::R480);
        let r = parse("Grey.Harbour.2017.German.DL.WebRip.x264-NEBEL");
        assert_eq!(r.quality.resolution, Resolution::R480);
        // DVD rips have no resolution, even when one is named.
        let r = parse("Grey.Harbour.2017.DVDRip.x264-NEBEL");
        assert_eq!(r.quality.resolution, Resolution::Unknown);
        let r = parse("Grey.Harbour.2017.480p.DVDRip.x264-NEBEL");
        assert_eq!(r.quality.resolution, Resolution::Unknown);
        // An encoder without a source counts as an SD TV capture.
        let r = parse("Grey.Harbour.2017.x264-NEBEL");
        assert_eq!(r.quality.source, Source::Tv);
        assert_eq!(r.quality.resolution, Resolution::R480);
        // A Blu-ray encode without a resolution is 720p, a rip is SD.
        let r = parse("Grey.Harbour.2017.BluRay.x264-NEBEL");
        assert_eq!(r.quality.resolution, Resolution::R720);
        let r = parse("Grey.Harbour.2017.4K.Remastered.BDRip.x264-NEBEL");
        assert_eq!(r.quality.resolution, Resolution::R480);
        assert_eq!(r.edition.as_deref(), Some("Remastered"));
    }

    #[test]
    fn dvd_discs() {
        for input in [
            "Low.Tide.1977.PAL.DVD9-WATT",
            "Low Tide 1977 NTSC DVD-9",
            "Low.Tide.1977.PAL.DVDR-WATT",
            "Low.Tide.(1977).[2xDVD5]",
        ] {
            let r = parse(input);
            assert_eq!(r.quality.source, Source::Dvd, "{input}");
            assert_eq!(r.quality.modifier, Modifier::Remux, "{input}");
            assert_eq!(r.quality.resolution, Resolution::R480, "{input}");
        }
        let r = parse("Low.Tide.1977.480i.DVD.REMUX.MPEG-2-WATT");
        assert_eq!(r.quality.modifier, Modifier::None);
        assert_eq!(r.quality.resolution, Resolution::Unknown);
    }

    #[test]
    fn remux_needs_hd() {
        let r = parse("Low.Tide.1977.720p.BluRay.AVC.Remux-WATT");
        assert_eq!(r.quality.modifier, Modifier::None);
        assert_eq!(r.quality.resolution, Resolution::R720);
    }

    #[test]
    fn keywords_before_the_year() {
        let r = parse("Leiser.Sturm.GERMAN.2019.DL.PAL.DVDR-WIND");
        assert_eq!(r.title(), Some("Leiser Sturm"));
        assert_eq!(r.languages, [Language::German, Language::Original]);
        let r = parse("Leiser.Sturm.Directors.Cut.2019.1080p.WEB-DL.H.264-WIND");
        assert_eq!(r.title(), Some("Leiser Sturm"));
        assert_eq!(r.edition.as_deref(), Some("Directors Cut"));
        // Other languages stay part of the title.
        let r = parse("Johnny.English.2003.1080p.BluRay.x264-WIND");
        assert_eq!(r.title(), Some("Johnny English"));
    }

    #[test]
    fn first_year_of_a_double_feature() {
        let r = parse("Dark.Road.1968.-.Bright.Road.1970.1080p.Blu-ray.MPEG-2.LPCM.2.0-WIND");
        assert_eq!(r.year, Some(1968));
        assert_eq!(r.title(), Some("Dark Road"));
    }

    #[test]
    fn edition_joins_only_remastered_and_open_matte() {
        let r = parse("Low.Tide.1977.EXTENDED.REMASTERED.1080p.BluRay.x265-WATT");
        assert_eq!(r.edition.as_deref(), Some("EXTENDED REMASTERED"));
        let r = parse("Low.Tide.1977.Theatrical.Cut.Open.Matte.1080p.WEB-DL.H.264-WATT");
        assert_eq!(r.edition.as_deref(), Some("Theatrical Cut Open Matte"));
        let r = parse("Low.Tide.1977.Remastered.Uncut.1080p.BluRay.x264-WATT");
        assert_eq!(r.edition.as_deref(), Some("Remastered"));
    }

    #[test]
    fn bracketed_and_numeric_groups() {
        let r = parse("Low.Tide.(1977).[DVD9]");
        assert_eq!(r.release_group.as_deref(), Some("DVD9"));
        let r = parse("Low.Tide.(1977).(1080p.WEB-DL.H264.English.-.WATT)");
        assert_eq!(r.release_group.as_deref(), Some("WATT"));
        let r = parse("Low.Tide.1977.German.DL.WEB.x264-WATT-1");
        assert_eq!(r.release_group.as_deref(), Some("WATT"));
        let r = parse("Low Tide (1977) [1977]");
        assert_eq!(r.release_group, None);
    }

    #[test]
    fn numbered_repack_and_multi_language() {
        let r = parse("Low.Tide.1977.REPACK2.1080p.BluRay.x264-WATT");
        assert!(r.revision.repack);
        assert_eq!(r.revision.version, 3);
        let r = parse("Low.Tide.1977.German.ML.1080p.WEB.H264-WATT");
        let mut languages = r.languages.clone();
        languages.sort();
        assert_eq!(
            languages,
            [Language::English, Language::German, Language::Original]
        );
    }

    #[test]
    fn bracketed_alternative_title_stays() {
        let r = parse("Nebelwald.(The.Misty.Forest).1981.720p.BluRay.x264-WALD");
        assert_eq!(r.title(), Some("Nebelwald (The Misty Forest)"));
        let r = parse("Nebelwald (Uncut) (1981) 720p BluRay");
        assert_eq!(r.title(), Some("Nebelwald"));
        assert_eq!(r.edition.as_deref(), Some("Uncut"));
    }

    #[test]
    fn remux_without_resolution_is_hd() {
        let r = parse("Slow Lantern 1971 BD Remux FLAC");
        assert_eq!(r.quality.modifier, Modifier::Remux);
        assert_eq!(r.quality.resolution, Resolution::R1080);
    }

    #[test]
    fn bare_remux_without_resolution_is_unknown() {
        let r = parse("Slow Lantern-1971-Collection br remux avc-GRP");
        assert_eq!(r.quality, Quality::default());
        let r = parse("Slow.Lantern.1971.1080p.REMUX.AVC-GRP");
        assert_eq!(r.quality.modifier, Modifier::Remux);
        assert_eq!(r.quality.source, Source::BluRay);
    }

    #[test]
    fn untouched_video_without_disc_audio_is_remux() {
        let r = parse("Copper.Bell.1958.1080p.BluRay.AVC-GLOCKE");
        assert_eq!(r.quality.modifier, Modifier::Remux);
        let r = parse("Copper.Bell.1958.2160p.UHD.BluRay.HEVC-GLOCKE");
        assert_eq!(r.quality.modifier, Modifier::Remux);
        assert_eq!(r.quality.resolution, Resolution::R2160);
    }

    #[test]
    fn full_disc_is_1080p_and_rips_are_encodes() {
        let r = parse("Copper.Bell.1958.2160p.UHD.Blu-ray.HEVC.LPCM.2.0-GLOCKE");
        assert_eq!(r.quality.modifier, Modifier::BrDisk);
        assert_eq!(r.quality.resolution, Resolution::R1080);
        let r = parse("Copper.Bell.1958.COMPLETE.UHD.BLURAY-GLOCKE");
        assert_eq!(r.quality.resolution, Resolution::R1080);
        let r = parse("Copper.Bell.1958.2160p.UHD.BDRip.HEVC.10bit.TrueHD.7.1-GLOCKE");
        assert_eq!(r.quality.modifier, Modifier::None);
        assert_eq!(r.quality.resolution, Resolution::R2160);
    }

    #[test]
    fn remux_needs_a_bluray() {
        let r = parse("Copper.Bell.1958.UHD.WEB-DL.2160p.HEVC.DL.Remux-GLOCKE");
        assert_eq!(r.quality.source, Source::WebDl);
        assert_eq!(r.quality.modifier, Modifier::None);
    }

    #[test]
    fn explicit_resolution_beats_uhd_keyword() {
        let r = parse("Copper.Bell.1958.UHD.BluRay.1080p.DDP.5.1.x265-GLOCKE");
        assert_eq!(r.quality.resolution, Resolution::R1080);
        let r = parse("Copper.Bell.1958.UHD.BluRay.x265-GLOCKE");
        assert_eq!(r.quality.resolution, Resolution::R2160);
    }

    #[test]
    fn sd_web_and_tv() {
        let r = parse("Copper.Bell.1958.540p.WEB-DL.AAC2.0.H.264-GLOCKE");
        assert_eq!(r.quality.resolution, Resolution::R480);
        let r = parse("Copper.Bell.1958.576p.BluRay.x264-GLOCKE");
        assert_eq!(r.quality.resolution, Resolution::R576);
        let r = parse("Copper.Bell.1958.PDTV.x264-GLOCKE");
        assert_eq!(r.quality.source, Source::Tv);
        assert_eq!(r.quality.resolution, Resolution::R480);
    }

    #[test]
    fn dual_language_original_only_with_german() {
        let r = parse("Copper.Bell.1958.ITA.UHD.BluRay.2160p.HEVC.DTSMA.DL.Remux-GLOCKE");
        assert_eq!(r.languages, [Language::Italian]);
        let r = parse("Copper.Bell.1958.UHD.BluRay.2160p.HEVC.DTSMA.DL.Remux-GLOCKE");
        assert!(r.languages.is_empty());
    }

    #[test]
    fn group_rules() {
        let r = parse("Copper.Bell.1958.1080p.NF.WEB-DL.AAC2.0.H.264-ATMOS");
        assert_eq!(r.release_group.as_deref(), Some("ATMOS"));
        let r = parse("Copper.Bell.1958.2160p.UHD.BluRay.REMUX.HDR-Jo.The.Smith");
        assert_eq!(r.release_group.as_deref(), Some("Jo"));
        let r = parse("Copper.Bell.1958.1080p.WEB.H.264-AB-CD-Retail-Multi");
        assert_eq!(r.release_group.as_deref(), Some("Multi"));
        let r = parse("Copper.Bell.1958.1080p.WEB-DL");
        assert_eq!(r.release_group, None);
    }

    #[test]
    fn full_disc_versus_remux_and_encode() {
        let r = parse("Copper.Bell.1958.1080p.EUR.Blu-ray.AVC.DTS-HD.MA.2.0-GLOCKE");
        assert_eq!(r.quality.modifier, Modifier::BrDisk);
        assert_eq!(r.quality.source, Source::BluRay);
        assert_eq!(r.quality.resolution, Resolution::R1080);

        let r = parse("Copper.Bell.1958.1080p.Blu-ray.VC-1.LPCM.2.0-GLOCKE");
        assert_eq!(r.quality.modifier, Modifier::BrDisk);

        let r = parse("Copper.Bell.1958.1080p.Blu-ray.Remux.AVC.FLAC.2.0-GLOCKE");
        assert_eq!(r.quality.modifier, Modifier::Remux);

        let r = parse("Copper.Bell.1958.1080p.BluRay.AVC.x264-GLOCKE");
        assert_eq!(r.quality.modifier, Modifier::None);

        let r = parse("Copper.Bell.1958.1080p.WEB-DL.AVC-GLOCKE");
        assert_eq!(r.quality.modifier, Modifier::None);
    }

    #[test]
    fn language_word_inside_title_stays() {
        let r = parse("The.German.Tailor.2008.720p.BluRay-G");
        assert_eq!(r.title(), Some("The German Tailor"));
        assert!(r.languages.is_empty());
    }

    #[test]
    fn ambiguous_keyword_inside_title() {
        let r = parse("Spinners.Web.2006.1080p.BluRay-G");
        assert_eq!(r.title(), Some("Spinners Web"));
        assert_eq!(r.quality.source, Source::BluRay);
    }

    #[test]
    fn aka_gives_two_titles() {
        let r = parse("Night.Train.AKA.Nachtzug.2015.1080p.BluRay-G");
        assert_eq!(r.titles, ["Night Train", "Nachtzug"]);
    }

    #[test]
    fn file_extension_is_ignored() {
        let r = parse("Hollow.Pines.2012.720p.BluRay.x264-WALD.mkv");
        assert_eq!(r.release_group.as_deref(), Some("WALD"));
    }

    #[test]
    fn title_without_metadata() {
        let r = parse("Simple Title");
        assert_eq!(r.title(), Some("Simple Title"));
        assert_eq!(r.year, None);
        assert_eq!(r.quality, Quality::default());
    }

    #[test]
    fn hardcoded_subs() {
        let subs = |input: &str| parse(input).hardcoded_subs;
        assert_eq!(
            subs("Some.Film.2001.German.SUBBED.1080p.BluRay.x264-GRP"),
            Some(HardcodedSubs::Generic)
        );
        assert_eq!(
            subs("Some.Film.2001.1080p.HC.WEB-DL.x264-GRP"),
            Some(HardcodedSubs::Generic)
        );
        assert_eq!(
            subs("Some Film (2001) - 1080p - H265 BRRip - NLsub"),
            Some(HardcodedSubs::Tagged("NLsub".into()))
        );
        assert_eq!(
            subs("Some.Film.2001.Retail.DKsubs.1080p.WEB-DL.H.264-GRP"),
            Some(HardcodedSubs::Tagged("DKsubs".into()))
        );
        // Soft subtitles and words that merely start with `sub`.
        assert_eq!(subs("Some Film 2001 1080p WEB-DL X264 NL SUBS"), None);
        assert_eq!(subs("Some.Film.2001.720p.H.264.Ita.Eng.Sub.Ita-GRP"), None);
        assert_eq!(subs("Subspecies.1991.1080p.BluRay.REMUX.AVC-GRP"), None);
        // A title word is not a tag.
        assert_eq!(subs("Subbed.2001.1080p.WEB-DL.x264-GRP"), None);
    }

    #[test]
    fn degenerate_input_never_panics() {
        for input in [
            "",
            " ",
            "-",
            "...",
            "()",
            "[]",
            "[x]",
            "((((",
            "-G",
            "2019",
            "1080p",
            ".mkv",
            "ü",
            "Ä.Ö.Ü-ß",
            "[[2019]]",
            "AKA",
            "a AKA",
            "x-",
            "v2",
            "REAL",
            "\u{0}",
            "🎬.2020.1080p",
        ] {
            let _ = parse(input);
        }
    }
}
