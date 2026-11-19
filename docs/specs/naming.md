---
status: implemented
created: 2026-11-19
size: M
design: [sofarr.md §2 (v1 import/rename, v2 series), sofarr.md §3, sofarr.md §5 (naming oracle), sofarr.md §6]
media: both
oracle: GET /api/v3/rename?movieId=…, GET /api/v3/config/naming (Radarr and Sonarr; compared by `compare-rename`, spec web-ui-library AC-23)
---

# File and folder naming

## Context

When sofarr imports a download, it names the file and, for a new movie or a season, the folder
from the naming formats imported from Radarr/Sonarr (`config_naming`), e.g.
`{Movie CleanTitle:DE} ({Release Year}) {Quality.Full}{.MediaInfo.Simple}{-Release Group}`.
Files must end up with the same names Radarr/Sonarr would give them, so Jellyfin and the owner's
habits keep working after the switch.

Naming was built in phases 4 (movies) and 5 (series), before the spec workflow existed. This
spec records what it does today (reverse-engineered, `spec-extract`). The token engine is pure
(`sofarr-core`); the server fills it from the movie, series, episode, release name and media
info. The token syntax follows the public Servarr user documentation; the finer rules (clean
titles, illegal characters, `{Quality Full}`) were derived from comparing sofarr's names with
Radarr's and Sonarr's rename previews (`docs/parity.md`, "Stand Dateinamen" and "Annahmen ab
Phase 3"), never from Servarr code (sofarr.md §6).

Terms used below:

- **Format** — a naming pattern: literal text plus tokens in braces.
- **Token** — `{<prefix><name><suffix>[:<spec>]}`. The name is one of the known token names
  (Technical notes), written with its words separated by space, `.`, `_` or `-`, in any case.
  Text inside the braces before or after the name is the prefix or suffix. `:<spec>` is either
  two letters (a translation, `:DE`) or zeros (a number width, `:00`).
- **Value** — what a token stands for; **empty** when the input has nothing for it.
- **Folder level** — a part of a format between `/`.
- **Naming settings** — rename movies/episodes on or off, replace illegal characters, colon
  replacement, the formats, and the multi-episode style.

Related specs: `mediainfo` owns how `{MediaInfo Simple}` and `{MediaInfo VideoDynamicRange}`
take each part from the file and fall back to the release name (AC-06 there); this spec owns
the release-name guess itself. `web-ui-library` owns the rename preview, renaming existing files
(AC-13, AC-14, AC-16 there) and `compare-rename` (AC-23 there).

## Scope

### In scope

- The token syntax and every known token for movies, series and episodes.
- Colon replacement, illegal characters and tidying of the result.
- Clean titles (`{… CleanTitle}`).
- `{Quality Full}` with revision, the multi-episode styles.
- Reading the naming settings from the imported `config_naming`.
- The codec and dynamic-range guess from a release name.
- Where the import uses the formats: movie file and new movie folder, episode file and season
  folder.

### Out of scope

- Renaming existing files and moving folders — because spec `web-ui-library` owns them (AC-13,
  AC-14, AC-16 there) and reuses this naming.
- Filling the media-info tokens from the file — because spec `mediainfo` owns it (AC-06 there).
- Waiting for missing episode titles before an import (`episodeTitleRequired`) — because it is an
  import rule, not naming; it only uses the detection of episode-title tokens.
- Tokens the owner's formats do not use (e.g. `{Movie Collection}`, `{Custom Formats}`,
  `{MediaInfo AudioLanguages}`) — because the parity target is the owner's setup (sofarr.md §2).
- Remaining title differences from Radarr's rename (apostrophes in some titles, `11:14`,
  `Thunderbolts*`) — because no pattern is visible from outside; they stay listed in
  `docs/parity.md`.

## Acceptance criteria

- [x] AC-01 (always): sofarr replaces each token of a format by its value, recognising the token name in any of its separator forms (`{Movie Title}`, `{Movie.Title}`, `{Movie_Title}`, `{Movie-Title}`) and joining the words of the value with that separator; prefix and suffix inside the braces appear only with a non-empty value, and an empty token leaves nothing — `{Movie CleanTitle:DE} ({Release Year}) {Quality.Full}{.MediaInfo.Simple}{-Release Group}` gives `Mission Impossible – Fallout (2018) Bluray-1080p.x264.DTS-GRP`, and without media info and group `{Movie Title} {Quality Full}{.MediaInfo.Simple}{-Release Group}` ends after the quality.
- [x] AC-02 (always): sofarr writes a value in upper case when the letters of the token name are all upper case (`{MOVIE TITLE}`), in lower case when they are all lower case (`{movie title}`), and as it is otherwise; numbers (`{SEASON:00}`) are not affected.
- [x] AC-03 (error): If a token name is unknown (`{Unbekannt}`) or a brace is not closed, then sofarr keeps that text unchanged in the name.
- [x] AC-04 (always): sofarr keeps the `/` written in a format as folder levels, collapses runs of spaces left by empty tokens, and trims spaces, dots, dashes and underscores from both ends of every folder level, so no level ends with a dot or a space — `{Movie CleanTitle}/{Movie CleanTitle} ({Release Year})` for "Mr. Smith Jr." gives `Mr. Smith Jr/Mr. Smith Jr. (2018)`.
- [x] AC-05 (option): Where the colon replacement is set, sofarr writes a `:` in a title as nothing (delete), `-` (dash), ` -` (space dash, the default), ` - ` (space dash space, without doubling a following space) or, with "smart", ` -` before a space and `-` otherwise — "What: Where" gives "What Where", "What- Where", "What - Where", "What - Where" and "What - Where", and "A:B" with space dash gives "A -B".
- [x] AC-06 (option): Where illegal characters are replaced, sofarr writes `/` and `\` in a title as `+` and drops `? * " < > |`; where they are not, it drops all of them.
- [x] AC-07 (always): sofarr writes a clean title (`{… CleanTitle}`) with diacritics removed (`ö` → `o`, `ß` → `ss`, `æ` → `ae`), `&` as `and`, letters, digits and the marks `-`, `.`, `–`, `!`, `…`, `#`, `·`, `‘` kept (a run of dots as one dot), `:`, `/` and `\` as a space, a `-` standing alone between spaces and every other mark (apostrophes, commas, parentheses, `?`) dropped, and single spaces — "Der Herr der Ringe: Die Rückkehr des Königs" gives "Der Herr der Ringe Die Ruckkehr des Konigs", "Ocean's Eleven" gives "Oceans Eleven", "96 Hours - Taken 2" gives "96 Hours Taken 2".
- [x] AC-08 (always): sofarr fills the movie tokens: `{Movie Title}` and `{Movie CleanTitle}` (with `:DE` the German title, falling back to the title), `{Movie TitleThe}`/`{Movie CleanTitleThe}` with a leading article (The, A, An, Der, Die, Das) moved to the end (`The Thing` → `Thing, The`), `{Movie TitleFirstCharacter}` as the first letter or digit after such an article in upper case (`Der Pate` → `P`), `{Movie OriginalTitle}`/`{Movie CleanOriginalTitle}` falling back to the title, `{Release Year}`, `{Quality Title}`, `{Release Group}`, `{Edition Tags}`, `{ImdbId}` and `{TmdbId}`.
- [x] AC-09 (always): sofarr writes `{Quality Full}` as the quality name, then ` Proper` or ` Repack` from version 2 on (with the version number from 3 on, `Proper3`), then ` REAL` when the release was marked real; without a quality it is empty.
- [x] AC-10 (always): sofarr fills the series and episode tokens: `{Series Title}`, `{Series TitleYear}` (`Small Island (2019)`), `{Series CleanTitle}`, `{Series CleanTitleYear}`, `{Episode Title}` and `{Episode CleanTitle}` of the first episode (with `:DE` the German episode title, falling back to the episode title), `{Air-Date}` as `YYYY-MM-DD`, `{season}` and `{episode}` padded to the number of zeros after the colon (`Staffel {season:00}` → `Staffel 02`, `Season {season}` → `Season 2`).
- [x] AC-11 (option): Where a file holds several episodes, sofarr writes `{episode}` in the multi-episode style — extend `S02E05-06-07`, duplicate `S02E05.S02E06.S02E07`, repeat `S02E05E06E07`, scene `S02E05-E06-E07`, range `S02E05-07`, prefixed range `S02E05-E07` (the default and the fallback for unknown values) — repeating the letter written right before `{episode}`; sofarr's episode parser reads every one of these names back as episodes 5, 6, 7.
- [x] AC-12 (event): When sofarr reads the naming settings of Radarr or Sonarr, then it takes the colon replacement by name (`delete`, `dash`, `spaceDash`, `spaceDashSpace`, `smart`) or by number (0–4 in that order), the multi-episode style by number (0 extend to 5 prefixed range), and every format, switch and missing or empty value falls back to its default (movie `{Movie Title} ({Release Year}) {Quality Full}`, movie folder `{Movie Title} ({Release Year})`, season folder `Season {season}`, renaming on, replacing illegal characters on).
- [x] AC-13 (always): sofarr guesses from a release name the video codec as `x265` (`x265`, `HEVC`, `H265`, `H.265`), `x264` (`x264`, `AVC`, `H264`, `H.264`), `AV1` or `XviD`, the audio as the first of `TrueHD`, `DTS-HD MA`, `DTS`, `EAC3` (`EAC3`, `DDP`, `DD+`), `AC3` (`AC3`, `DD5`, `DD5.1`, `DD2.0`), `FLAC`, `AAC`, and the dynamic range as `DV HDR10`, `DV` (`DV`, `DoVi`, `Dolby.Vision`) or `HDR` (`HDR`, `HDR10`, `HDR10+`), nothing for SDR; `{MediaInfo Simple}` from the name is video and audio joined by a space, empty when both are unknown.
- [x] AC-14 (event): When a movie download is imported, then sofarr names the file by the movie format in the movie's folder with the video's extension in lower case, or keeps the video's own file name where renaming movies is off, records the movie's current file as the one being replaced, and refuses the import when the movie has no folder; a movie without a folder yet gets the root folder joined with the folder format (`{Movie TitleFirstCharacter:DE}/{Movie Title:DE} ({Release Year})` → `/data/media/movies/K/Der kleine Garten (2019)`).
- [x] AC-15 (event): When an episode download is imported, then sofarr names the file by the episode format of the series type (daily, anime, else standard), one name for all episodes the file holds, and puts it in the season folder named by the season folder format inside the series folder, or directly in the series folder when the series has no season folders.

## Technical notes

- Entry points (`sofarr_core::naming`): `format(pattern, &NamingInput, &NamingConfig) -> String`,
  `NamingConfig::from_json(config_naming)`, `episode_title_tokens(pattern)`,
  `media_info_from_name`, `media_info_simple`, `video_dynamic_range`, `dynamic_range_from_name`.
  Server: `importer::naming_input`, `importer::plan`, `importer::movie_folder`,
  `series::episode_input`, `series::episode_name`, `series::episode_pattern`,
  `series::season_folder`.
- Known token names (lower case, words split by any separator): `movie title`,
  `movie cleantitle`, `movie titlethe`, `movie cleantitlethe`, `movie titlefirstcharacter`,
  `movie originaltitle`, `movie cleanoriginaltitle`, `release year`, `quality full`,
  `quality title`, `mediainfo simple`, `release group`, `edition tags`, `imdbid`, `tmdbid`,
  `series title`, `series titleyear`, `series cleantitle`, `series cleantitleyear`,
  `episode title`, `episode cleantitle`, `air date`, `mediainfo videodynamicrange`, `season`,
  `episode`. The longest name found in a token body wins.
- Colon replacement and illegal characters apply to the plain title tokens (`Title`,
  `TitleThe`, `OriginalTitle`, series and episode titles); clean titles follow AC-07 instead.
- Release group, revision and edition of a file come from the release name (spec `parser`).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `{Release Year} {Movie Title` (unclosed) | `2018 {Movie Title` |
| `{-Release Group}` without a group | Nothing, not a lone `-` |
| Title ending in a dot as the last part of a folder level | The dot is trimmed (Windows refuses it) |
| `Who? What: Where/When`, replace on, space dash space | `Who What - Where+When` |
| `{Movie CleanTitleThe}` for "Das Boot: Teil 1" | `Boot Teil 1 Das` (the clean title drops the comma) |
| `{Movie CleanTitle:DE}` without a German title | The clean title |
| `multiEpisodeStyle` 9 | Prefixed range |
| `movieFolderFormat` empty in `config_naming` | Default `{Movie Title} ({Release Year})` |
| Video `grp-garten.MKV`, renaming off | `grp-garten.mkv` |
| Series type `other` | Standard episode format |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| This spec describes the behavior of 2026-11-19 as found in code and tests, not a target | Reverse-engineered spec (`spec-extract`) for phases 4 and 5 | A rule that deviates from Radarr/Sonarr is recorded as is; `compare-rename` and `docs/parity.md` drive improvements |
| `:DE` is the only translation that changes a value; any other two letters give the default title | The owner's formats use only `:DE` | Another translation would silently give the default title |
| `{Movie CleanTitleThe}` drops the comma of the moved article | It is the clean title of the moved title; no oracle answer for this token in the owner's setup | Radarr may keep the comma |
| The comparison with Radarr/Sonarr is manual | It needs running instances and the owner's library (web-ui-library AC-23) | A regression shows only at the next manual comparison |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `naming::tests::formats_file_names_with_prefixes`, `naming::tests::empty_tokens_drop_their_prefix`, `naming::tests::token_name_case_and_separator_shape_the_value` (`crates/sofarr-core/src/naming.rs`) | verified | 8a646d0 |
| AC-02 | `naming::tests::token_name_case_and_separator_shape_the_value` (`crates/sofarr-core/src/naming.rs`) | verified | 8a646d0 |
| AC-03 | `naming::tests::unknown_tokens_stay_visible` (`crates/sofarr-core/src/naming.rs`) | verified | 8a646d0 |
| AC-04 | `naming::tests::levels_collapse_spaces_and_trim_their_ends` (`crates/sofarr-core/src/naming.rs`), `naming::tests::every_folder_level_is_a_valid_windows_name`, `naming::tests::empty_tokens_drop_their_prefix`, `naming::tests::folders_and_first_character` (`crates/sofarr-core/src/naming.rs`) | verified | 3eeeab3 |
| AC-05 | `naming::tests::smart_colon_without_a_space` (`crates/sofarr-core/src/naming.rs`), `naming::tests::colon_modes_and_illegal_characters`, `naming::tests::colon_default_and_kept_illegal_characters`, `naming::tests::formats_episodes` (`crates/sofarr-core/src/naming.rs`) | verified | 8a646d0 |
| AC-06 | `naming::tests::colon_modes_and_illegal_characters`, `naming::tests::colon_default_and_kept_illegal_characters` (`crates/sofarr-core/src/naming.rs`) | verified | 8a646d0 |
| AC-07 | `naming::tests::clean_titles_spell_out_ligatures_and_drop_backslashes` (`crates/sofarr-core/src/naming.rs`), `naming::clean::tests::diacritics_are_removed_and_sharp_s_becomes_ss`, `naming::clean::tests::hyphens_dots_and_dashes_remain_other_marks_go`, `naming::clean::tests::a_hyphen_standing_alone_is_dropped_one_inside_a_word_stays`, `naming::clean::tests::exclamation_marks_and_typographic_marks_stay_dot_runs_become_one_dot` (`crates/sofarr-core/src/naming/clean.rs`) | verified | 3eeeab3 |
| AC-08 | `naming::tests::formats_file_names_with_prefixes`, `naming::tests::folders_and_first_character`, `naming::tests::other_movie_tokens` (`crates/sofarr-core/src/naming.rs`) | verified | 8a646d0 |
| AC-09 | `naming::tests::revisions_in_quality_full`, `naming::tests::numbered_revision_in_quality_full` (`crates/sofarr-core/src/naming.rs`) | verified | 8a646d0 |
| AC-10 | `naming::tests::series_clean_titles_and_german_episode_title_fallback` (`crates/sofarr-core/src/naming.rs`), `naming::tests::formats_episodes`, `naming::tests::formats_series_folders_and_daily` (`crates/sofarr-core/src/naming.rs`) | verified | 3eeeab3 |
| AC-11 | `naming::tests::formats_episodes`, `naming::tests::numeric_settings` (`crates/sofarr-core/src/naming.rs`); `episode::tests::multi_episode_forms` (`crates/sofarr-parser/src/episode.rs`) | verified | — (tests predate the branch) |
| AC-12 | `naming::tests::reads_naming_config`, `naming::tests::numeric_settings`, `naming::tests::missing_naming_settings_take_their_defaults` (`crates/sofarr-core/src/naming.rs`); `importer::tests::rename_setting_and_extension_case` (default movie format, `crates/sofarr-server/src/importer.rs`) | verified | 8a646d0 |
| AC-13 | `naming::tests::media_info_guess`, `naming::tests::release_name_guess_vocabulary`, `naming::tests::formats_series_folders_and_daily`, `naming::tests::media_info_parts_come_from_the_file_where_known` (`crates/sofarr-core/src/naming.rs`) | verified | 8a646d0 |
| AC-14 | `importer::tests::imports_and_replaces_old_file`, `importer::tests::rename_setting_and_extension_case`, `importer::tests::folder_for_new_movies` (`crates/sofarr-server/src/importer.rs`) | verified | 7e19452 |
| AC-15 | `series::tests::names_episodes`, `series::tests::episode_format_by_series_type_and_season_folders` (`crates/sofarr-server/src/series/tests.rs`) | verified | 7e19452 |

## Summary

> Completed: 2026-11-19

- The pure token engine in `sofarr_core::naming` fills every movie, series and episode token with prefix/suffix, separator and case rules, keeps unknown tokens and unclosed braces, and tidies each folder level.
- Colon replacement, illegal characters, clean titles, `{Quality Full}` with revisions and all six multi-episode styles (read back by sofarr's episode parser) follow the imported naming settings, with defaults for missing values.
- Codec, audio and dynamic range are guessed from the release name where the file gives nothing.
- The import names movie files and new movie folders, and episode files by series type into season folders.

## Decisions

- 2026-11-19 (extract): Written from the code of phases 4 and 5 with `spec-extract`. New tests
  cover what no test covered: token case and separators (AC-01, AC-02), an unclosed brace
  (AC-03), the default colon replacement and illegal characters kept off (AC-05, AC-06), the
  original title, edition, ids and `CleanTitleThe` (AC-08), `Proper3` (AC-09), defaults of the naming settings (AC-12), the codec guess
  vocabulary (AC-13), renaming off and the lower-case extension (AC-14), and the episode format
  by series type and season folders off (AC-15). All passed on the unchanged code.
