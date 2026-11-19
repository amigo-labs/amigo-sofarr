---
status: todo
created: 2026-11-19
size: L
design: [sofarr.md §2 (v1 parser, v2 episode parsing), sofarr.md §3, sofarr.md §4 (ParsedRelease), sofarr.md §5, sofarr.md §6]
media: both
oracle: GET /api/v3/parse?title=… (Radarr, fixtures/public/radarr-parse.jsonl; Sonarr, fixtures/public/sonarr-parse.jsonl)
---

# Release-name parser

## Context

Every decision sofarr takes starts with a release name: an indexer title in the RSS feed or a
search, the name of a download, or a file name found on disk. The parser turns such a name into
structured data — for movies the title candidates, year, quality (source, resolution, modifier),
revision, edition, release group, languages and burnt-in subtitles (`ParsedRelease`, sofarr.md
§4); for series additionally the series title and the season, episode, absolute or air-date
numbers (`EpisodeInfo`).

The parser was built in phases 1 (movies) and 5 (series), before the spec workflow existed. This
spec records what it does today (reverse-engineered, `spec-extract`). The parser is pure (no IO,
no clock) and clean-room: its rules come from the Radarr/Sonarr parse oracle answers
(`GET /api/v3/parse`, recorded in `fixtures/public/`), from the public user documentation and
from general knowledge of release naming, never from Servarr code (sofarr.md §6).

Terms used below:

- **Movie oracle** — Radarr's parse answers in `fixtures/public/radarr-parse.jsonl` (4,952
  titles today), compared per field: title (any of the oracle's title candidates, compared
  lowercase with punctuation as spaces), year, source, resolution, modifier, revision, edition,
  release group, languages, hardcoded subs.
- **Episode oracle** — Sonarr's parse answers in `fixtures/public/sonarr-parse.jsonl`.
- **Ratchet baseline** — the pass rate per field in permille, recorded in a
  `fixtures/*-baseline.toml` file together with the fixture count and a fingerprint of the
  fixture titles.
- **Release group** — the team that made a release (`-NORDLICHT` at the end, `[SubGroup]` at
  the start).
- **Metadata** — the part of a name after the title: year, quality keywords, languages, group.

Related specs: `parser-episode-gaps` owns four anime/season-less episode forms, the limit of
movie titles read as episodes (AC-06 there) and the episode-oracle ratchet (AC-07 there).
`decision-engine` turns the parsed quality into a named quality; `naming` guesses codecs from a
release name for file names.

## Scope

### In scope

- Movie release names: title candidates, year, source, resolution, modifier, revision,
  edition, release group, languages, burnt-in subtitles.
- Series release names: series title, season, episodes, multi-season packs, specials, daily
  air dates, absolute (anime) numbers, plus the movie fields of the same name.
- The movie-oracle ratchet.

### Out of scope

- The four episode forms, the movie-as-episode limit and the episode-oracle ratchet — because
  `parser-episode-gaps` already owns them (AC-01 to AC-07 there).
- Video and audio codecs as parsed fields — because the parser has no codec field; codec words
  only steer source and modifier (an `x264` without source is a TV capture, `AVC` next to a
  Blu-ray is a remux), and the codec guess for file names lives in `naming`.
- Matching a parsed title to a movie or series of the library — because that is the caller's
  job (RSS, import, disk scan) and the decision engine's year rule (`decision-engine`).
- Languages from indexer metadata — because the parser sees only the name; the decision engine
  takes reported languages in place of parsed ones (`decision-engine`).

## Acceptance criteria

- [ ] AC-01 (always): sofarr reads the title as the words before the first metadata, with dots, spaces and underscores as word separators and keywords in any case, and the year as a four-digit number from the metadata — `The.Quiet.Lighthouse.2014.1080p.BluRay.x264-NORDLICHT` gives "The Quiet Lighthouse", 2014; `Paper Kites over Lisbon (2021) [2160p] [WEB-DL]` gives "Paper Kites over Lisbon", 2021; a year inside the title stays in the title (`Escape.From.Station.2088.2019…` gives "Escape From Station 2088", 2019); a title made only of a year-like number stays a title (`1922.2017…` gives "1922", 2017; `1999.2160p…` gives "1999" and no year); of a double feature the first year counts.
- [ ] AC-02 (always): sofarr gives several title candidates for `A.AKA.B` (`Night.Train.AKA.Nachtzug.2015…` gives "Night Train" and "Nachtzug"), keeps a bracketed alternative title and a leading parenthesis in the title (`Nebelwald.(The.Misty.Forest).1981…` gives "Nebelwald (The Misty Forest)", `(300) Letters to Nobody (2015)` gives "(300) Letters to Nobody"), and keeps a language or source word that stands inside the title (`The.German.Tailor.2008…` gives "The German Tailor" without a language, `Spinners.Web.2006…` gives "Spinners Web").
- [ ] AC-03 (always): sofarr reads the release group as the last dash segment after the metadata (`…x264-A-TEAM` gives "TEAM", `…-WATT-1` gives "WATT", a keyword such as `-ATMOS` included), from a leading square bracket (`[Kirschblüte] Moonlit Ferry 2012…` gives "Kirschblüte"), from the end of a trailing bracket (`(1080p.WEB-DL.H264.English.-.WATT)` gives "WATT") or as a trailing bracket after the year (`Low.Tide.(1977).[DVD9]` gives "DVD9", `Low Tide (1977) [1977]` gives none), not from a hyphen inside the title (`Spider-Moth.Rising…` keeps "Spider-Moth"), and no group where none is written; a trailing file extension (`.mkv`, `.mp4`, `.avi`, `.m4v`, `.wmv`, `.mov`, `.nzb`) is ignored.
- [ ] AC-04 (always): sofarr reads the source from keywords — `HDTV`/`PDTV` TV, `DVDRip`/`DVDR` DVD, `CAM`, `HDTS`, `TC`, `WORKPRINT`, `WEB`/`WEB-DL`/`AmazonHD` WEB-DL, `WEBRip`, `BluRay`/`Blu-Ray`/`BDRip` Blu-ray; a name with a resolution or an AVC encoder (`x264`) but no source is TV, one with neither (also one with only `x265`) is unknown; `WEB-DLRip`, a WEB split by spaced dashes (`WEB - DL`) and a `WEB` followed by a dash other than `-DL`/`-Rip` (`Film.2001.1080p.WEB-G`, `Copper Bell-1958-2160p-4K-WEB-5.1-GLOCKE`) are no source.
- [ ] AC-05 (always): sofarr reads the resolution as 360/480/540/576/720/1080/2160 lines from `<n>p` or from `UHD`/`4K` (2160), an explicit `<n>p` winning over `UHD`/`4K`; without a resolution a Blu-ray encode counts as 720p, a rip (`BDRip`, `WebRip`) and a TV capture as 480p, and DVD rips have no resolution even when one is named; WEB at 540p counts as 480p and a Blu-ray at 360p as 480p.
- [ ] AC-06 (always): sofarr marks a Blu-ray release as remux when it says `REMUX`, `BD Remux` or `MKVRemux` at 1080p or 2160p (without a resolution a `BD Remux` counts as 1080p), or names AVC video (HEVC only at 2160p) without an encoder and without disc audio; and as full disc (`brdisk`, counted as 1080p) when it says `COMPLETE.BLURAY`, `UNTOUCHED`, a disc size (`BD25`/`BD50`/`BD66`/`BD100`, also `BD-50`, not next to a 2160p resolution, only the source next to a remux), 3D `MVC`, or untouched video (AVC, HEVC only at 2160p) with disc audio (`DTS-HD`, `TrueHD`, `LPCM`, `FLAC`, `DD`, not `AC3`/`EAC3`); a 720p remux, a WEB remux, a rip, a 10-bit encode and a bare `REMUX`/`BDREMUX` with neither source nor resolution are no remux.
- [ ] AC-07 (always): sofarr reads a DVD disc (`DVD9`, `DVD5`, `DVDR`, also with `PAL`/`NTSC`) as DVD remux at 480p, a screener (`DVDSCR`) as DVD screener, and a region code (`R5`, `R6`, also as group suffix `-r2`) as regional DVD at 480p whatever else is named, except next to a cinema source (`R6 HDCAM` stays CAM).
- [ ] AC-08 (always): sofarr reads the revision: version 1 by default, `PROPER` or `REPACK` version 2, `REPACK2` version 3, `REAL` counted only next to such a fix (`Real.Ghosts.2011…` keeps "Real" in the title and counts no `REAL`).
- [ ] AC-09 (always): sofarr reads the edition as the first edition keyword of the metadata, joined only by a following `Remastered` or `Open Matte` (`EXTENDED.REMASTERED` gives "EXTENDED REMASTERED", `Extended.Directors.Cut` gives "Extended", `Remastered.Uncut` gives "Remastered"), with ordinal anniversaries (`30th Anniversary Edition`) and editions written before the year or in parentheses (`Nebelwald (Uncut) (1981)` gives "Nebelwald", edition "Uncut").
- [ ] AC-10 (always): sofarr reads the audio languages from language words in the metadata, in order and without duplicates; `German.DL` adds the original language, `German.ML` adds English and the original language, `DL` or `ML` without German adds nothing; `VF2` counts as French; a release without a language word has no language.
- [ ] AC-11 (always): sofarr marks burnt-in subtitles as generic for `SUBBED` and `HC` and as tagged with the written tag for a one-to-three-letter code glued to `sub`/`subs` (`NLsub`, `DKsubs`); soft subtitles (`SUBS`, `Sub.Ita`), words that start with `sub` (`Subspecies`) and a title word (`Subbed.2001…`) mark none.
- [ ] AC-12 (event): When a series release names a season and episodes (`S02E05`, `3x07`, `Season 1 Episode 4`, or more episodes as `S01E01E02`, `S01E01-E03`, `S01E01-03`, `S01E01.E02`, `S01E01.S01E02`), then sofarr reads the series title before that marker, the season and every episode, and the movie fields of the release (group, quality); a tight dash makes a range, a spaced ` - 23` does not, a second marker of another season is no continuation, a year in the series title stays there (`Doctor.Show.2005.S00E12` gives "Doctor Show 2005", season 0 as special), and `1920x1080` is no episode.
- [ ] AC-13 (event): When a series release names only a season (`S03`, `Staffel 2`) or a season range (`S01-S04`), then sofarr reads a full-season pack without episodes (with the last season of a range); when it names an air date (`2026.10.31`), then a daily episode with that date and no season; when it has a leading bracket group and ` - 012` (optionally `v2`), then the absolute episode 12 without a season and the bracket as group — also behind `S2` or `Season 2` in such an anime title, which is then no season pack (season words before ` - 06` and a season inside a bracketed title: spec `parser-season-in-title`).
- [ ] AC-14 (always): sofarr reads no episode from movie names (`Kleiner.Garten.2019.1080p.WEB-DL-GRP`, `S86.2026.720p…`, `K-12.2019…`, `Whitlam.Dismissal.-.50.Years.2025…`, `Formula 1 - Grand Prix-Rennen-25 Oktober 2026-720p…`), never panics on any input (empty, only separators, unbalanced brackets, emoji, control characters) and leaves what it does not recognise empty or unknown.
- [ ] AC-15 (always): Against the movie oracle, no field's pass rate falls below `fixtures/parser-baseline.toml` while the fixture set is unchanged; `SOFARR_UPDATE_BASELINE=1` raises the baseline (only upwards for the same set, to the current rates after the set changed), and private fixtures, when present, are reported but not ratcheted.

## Technical notes

- Entry points: `sofarr_parser::parse(&str) -> ParsedRelease` and
  `sofarr_parser::parse_episode(&str) -> Option<EpisodeInfo>` (`None` when no episode marker
  with a title before it is found). `EpisodeInfo.release` carries the movie fields of the same
  name; its title fields refer to the movie layout.
- Pipeline: strip extension → tokenize (separator kinds plain, dash, bracket) → classify each
  token by keyword tables (longest phrase wins, phrases never cross brackets) → layout (where
  the title ends, which token is the year, which the group) → fields.
- Vocabulary of `Source` and `Modifier` (`as_str`) follows the public Radarr API docs
  (`webdl`, `bluray`, `brdisk`, `remux`, …), so the oracle harness compares them directly.
- Pass rates today (‰, `fixtures/parser-baseline.toml`, 4,952 titles): edition 999,
  hardcoded_subs 1000, languages 993, modifier 999, release_group 994, resolution 999,
  revision 1000, source 998, title 997, year 999.
- The ratchet harness lives in `crates/sofarr-parser/tests/oracle.rs`; CI runs it with
  `cargo test --workspace`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Empty name, `-`, `()`, `[[2019]]`, `AKA`, `\u{0}`, `🎬.2020.1080p` | No panic; empty or unknown fields |
| Name without metadata (`Simple Title`) | The whole name is the title, no year, unknown quality |
| `Grey.Harbour.2017.480p.DVDRip.x264` | DVD without resolution (the named 480p is dropped) |
| `Copper.Bell.1958.UHD.BluRay.x265.2160p.TrueHD.BD50-GLOCKE` | Blu-ray 2160p encode: a disc size next to 2160p counts for nothing |
| `Copper.Bell.1958.Studio.UHD.BD66` | TV 2160p, no full disc |
| `GRP-Copper.Bell.1958.BDREMUX` | Bare remux without resolution: no modifier |
| `Copper.Bell.1958.ITA.UHD.BluRay…DL.Remux` | Italian only: `DL` adds the original language only next to German |
| `Some Film 2001 1080p WEB-DL X264 NL SUBS` | No burnt-in subtitles (`SUBS` alone is soft) |
| `Cartoon Town - 2017 - S16E06 - 23 Bunte Tage - DL` | Episode 6 only: ` - 23` starts the episode title |
| `Show.S01E01.S02E02.720p` | Episode 1 only: another season is no continuation |
| `Hollow.Pines.2012.720p.BluRay.x264-WALD.mkv` | Group "WALD": the extension is cut off |
| A private fixture set is present | Reported next to the public one, not part of the baseline |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| This spec describes the behavior of 2026-11-19 as found in code and tests, not a target | Reverse-engineered spec (`spec-extract`) for phases 1 and 5 | A rule that is wrong against the oracle is recorded as is; the ratchet, not this spec, drives improvements |
| The examples in the ACs are representatives; the full keyword tables are the code's | The tables have hundreds of entries and change with every oracle refresh | A keyword not named here may change without touching an AC |
| The movie ratchet compares only the public fixtures | Private fixtures (history, library) are not in CI (`AGENTS.md`, "Secrets & private Daten") | A regression visible only in private fixtures is reported but does not fail CI |
| No ratchet rate is named as an AC threshold | The baseline file is the threshold and may only rise; naming today's numbers would make the AC stale with every rise | none |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `tests::dotted_title_with_year`, `tests::spaced_title_with_bracket_year`, `tests::year_inside_title`, `tests::title_made_of_numbers`, `tests::first_year_of_a_double_feature`, `tests::separators_and_case_do_not_matter` (`crates/sofarr-parser/src/lib.rs`) | built | acaddf0 |
| AC-02 | `tests::aka_gives_two_titles`, `tests::bracketed_alternative_title_stays`, `tests::leading_parenthesis_is_part_of_title`, `tests::language_word_inside_title_stays`, `tests::ambiguous_keyword_inside_title` (`crates/sofarr-parser/src/lib.rs`) | built | |
| AC-03 | `tests::group_is_last_dash_segment`, `tests::bracketed_and_numeric_groups`, `tests::leading_bracket_group`, `tests::hyphenated_title_is_not_a_group`, `tests::missing_group`, `tests::group_rules`, `tests::file_extension_is_ignored` (`crates/sofarr-parser/src/lib.rs`) | built | |
| AC-04 | `tests::sources`, `tests::dashed_web_is_no_source`, `tests::only_avc_encoder_implies_tv` (`crates/sofarr-parser/src/lib.rs`) | built | |
| AC-05 | `tests::standard_definition_default`, `tests::explicit_resolution_beats_uhd_keyword`, `tests::sd_web_and_tv`, `tests::uhd_blu_ray_abbreviations` (`crates/sofarr-parser/src/lib.rs`); `classify::tests::years_and_resolutions` (`crates/sofarr-parser/src/classify.rs`) | built | |
| AC-06 | `tests::modifiers`, `tests::remux_needs_hd`, `tests::remux_without_resolution_is_hd`, `tests::bare_remux_without_resolution_is_unknown`, `tests::untouched_video_without_disc_audio_is_remux`, `tests::full_disc_is_1080p_and_rips_are_encodes`, `tests::remux_needs_a_bluray`, `tests::full_disc_versus_remux_and_encode`, `tests::disc_sizes`, `tests::disc_audio_and_untouched`, `tests::bare_disc_remux_and_mkv_remux` (`crates/sofarr-parser/src/lib.rs`) | built | |
| AC-07 | `tests::dvd_discs`, `tests::modifiers`, `tests::region_codes` (`crates/sofarr-parser/src/lib.rs`) | built | |
| AC-08 | `tests::revisions`, `tests::numbered_repack_and_multi_language`, `tests::real_in_title_is_not_a_revision` (`crates/sofarr-parser/src/lib.rs`); `classify::tests::real_only_next_to_a_fix` (`crates/sofarr-parser/src/classify.rs`) | built | |
| AC-09 | `tests::editions`, `tests::edition_joins_only_remastered_and_open_matte`, `tests::keywords_before_the_year`, `tests::bracketed_alternative_title_stays` (`crates/sofarr-parser/src/lib.rs`); `classify::tests::ordinal_anniversary_edition` (`crates/sofarr-parser/src/classify.rs`) | built | |
| AC-10 | `tests::languages`, `tests::dual_language_adds_original`, `tests::dual_language_original_only_with_german`, `tests::numbered_repack_and_multi_language`, `tests::french_version_tags`, `tests::keywords_before_the_year` (`crates/sofarr-parser/src/lib.rs`) | built | |
| AC-11 | `tests::hardcoded_subs` (`crates/sofarr-parser/src/lib.rs`) | built | |
| AC-12 | `episode::tests::standard_episode`, `episode::tests::multi_episode_forms`, `episode::tests::cross_notation`, `episode::tests::season_packs`, `episode::tests::specials_and_years_in_titles`, `episode::tests::spaced_dash_number_is_no_range` (`crates/sofarr-parser/src/episode.rs`) | built | |
| AC-13 | `episode::tests::season_packs`, `episode::tests::daily_shows`, `episode::tests::absolute_anime`, `episode::tests::anime_season_in_title_is_no_pack` (`crates/sofarr-parser/src/episode.rs`) | built | |
| AC-14 | `episode::tests::movies_are_not_episodes` (`crates/sofarr-parser/src/episode.rs`); `tests::degenerate_input_never_panics`, `tests::title_without_metadata` (`crates/sofarr-parser/src/lib.rs`) | built | |
| AC-15 | `oracle`, `harness_tests::same_set_only_ratchets_up`, `harness_tests::changed_set_may_go_down`, `harness_tests::expected_values_from_documented_fields`, `harness_tests::normalization_ignores_case_and_punctuation` (`crates/sofarr-parser/tests/oracle.rs`); baseline `fixtures/parser-baseline.toml` | built | |

## Decisions

- 2026-11-19 (extract): Written from the code of phases 1 and 5 with `spec-extract`. One test
  was added because no test covered underscores and keyword case as separators
  (`separators_and_case_do_not_matter`, AC-01); it passed on the unchanged parser.
- 2026-11-19 (check): AC-04 and AC-06 revised to what the code and `parity.md` say: a `WEB` followed by a dash (other than `-DL`/`-Rip`) is no source, and HEVC counts as disc video only at 2160p; the acceptance check found the wording broader than the behavior.
