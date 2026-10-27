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

pub use model::{Language, Modifier, ParsedRelease, Quality, Resolution, Revision, Source};

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

    let in_group = |i: usize| layout.group_tokens.as_ref().is_some_and(|r| r.contains(&i));
    let metadata: Vec<usize> = (layout.metadata_start..tokens.len())
        .filter(|&i| !in_group(i))
        .collect();

    let mut implied_source = None;
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
                        implied_source.get_or_insert(s);
                    }
                    (Some(s), None) if quality.source == Source::Unknown => quality.source = s,
                    _ => {}
                }
                if let Some(m) = modifier
                    && quality.modifier == Modifier::None
                {
                    quality.modifier = m;
                }
                if let Some(r) = resolution
                    && quality.resolution == Resolution::Unknown
                {
                    quality.resolution = r;
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
            Tag::Version(v) => release.revision.version = release.revision.version.max(v),
            Tag::Real => release.revision.real = release.revision.real.saturating_add(1),
            Tag::Language(language) if !release.languages.contains(&language) => {
                release.languages.push(language);
            }
            _ => {}
        }

        // The first run of edition keywords, e.g. `Extended.Directors.Cut`.
        let edition_part = tag == Tag::Edition || (tag == Tag::Continued && !edition.is_empty());
        if !edition_done && edition_part {
            edition.push(tokens[i].text);
        } else if !edition.is_empty() && tag != Tag::Continued {
            edition_done = true;
        }
    }
    if release.quality.source == Source::Unknown
        && let Some(source) = implied_source
    {
        release.quality.source = source;
    }
    if !edition.is_empty() {
        release.edition = Some(edition.join(" "));
    }
    release
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
    fn group_with_dash_inside() {
        let r = parse("Tin.Harbour.2003.1080p.BluRay.x264-A-TEAM");
        assert_eq!(r.release_group.as_deref(), Some("A-TEAM"));
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
            ("A.Film.2001.1080p-G", Source::Unknown),
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

        let r = parse("Iron.Lantern.1994.Extended.Directors.Cut.1080p.BluRay-G");
        assert_eq!(r.edition.as_deref(), Some("Extended Directors Cut"));

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
        assert_eq!(r.languages, [Language::German]);

        let r = parse("Le.Petit.Jardin.2013.FRENCH.720p.BluRay-G");
        assert_eq!(r.languages, [Language::French]);

        let r = parse("Quiet.River.2020.1080p.WEB-DL-G");
        assert!(r.languages.is_empty());

        let r = parse("Ein.Kleines.Haus.German.1080p.WEB-DL-G");
        assert_eq!(r.title(), Some("Ein Kleines Haus"));
        assert_eq!(r.languages, [Language::German]);
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
