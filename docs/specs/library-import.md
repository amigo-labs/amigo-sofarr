---
status: todo
created: 2026-11-19
size: M
design: [sofarr.md §2, sofarr.md §4, sofarr.md §5, sofarr.md §9 D2, sofarr.md §9 D5, sofarr.md §9 D10]
media: both
oracle: GET /api/v3/movie, GET /api/v3/series, GET /api/v3/episode, GET /api/v3/episodefile (source of the import; `compare-library` and `compare-series-library` check the result)
---

# Library import from Radarr and Sonarr

## Context

sofarr replaces Radarr first, then Sonarr (§1, D5). It does not start from an empty library:
the owner's settings (quality profiles, custom formats, naming, media management, import lists,
…) and the libraries (2,420 movies, 150 series, `docs/parity.md`) are taken over from the running
instances. Everything else — RSS sync, decisions, imports, naming, the disk scan — works on what
this import stored. The parity target is the owner's setup (§2), so sofarr keeps Radarr's and
Sonarr's documents as they are and builds its rules from them.

This spec was extracted from code built in phases 3 and 5, before the spec workflow; it
describes what the code does today.

## Scope

### In scope

- CLI `amigo-sofarr import-radarr [--exports <folder>]`: settings and movies (with their files)
  from the Radarr API or from a config export folder
- CLI `amigo-sofarr import-sonarr [--exports <folder>]`: settings, series, episodes and episode
  files from the Sonarr API, or settings and series from a config export folder
- Re-running an import to bring the library up to date with Radarr/Sonarr
- The German summary the CLI prints, and the errors it stops with

### Out of scope

- Comparing the result with Radarr/Sonarr (`compare-library`, `compare-series-library`) — because those are parity tools, not part of the takeover
- German titles, new episodes and metadata from TMDB/TVDB (`refresh-metadata`, `refresh-series`) — because they come from the metadata providers after the import (D10, D26)
- Matching the library to the files on disk (`rescan`) — because the disk scan is its own job
- Importing Radarr's or Sonarr's history and blocklist — because history is only an oracle (`compare-history`) and the blocklist is not part of the export (spec `fail-handling`)
- Removing movies or series that Radarr/Sonarr no longer list — because the import only adds and updates; removing is done in sofarr (spec `web-ui-library`)
- Adding movies and series without Radarr/Sonarr — because that is spec `discover`

## Acceptance criteria

- [ ] AC-01 (event): When the user runs `amigo-sofarr import-radarr` without `--exports`, then sofarr reads the settings (quality profiles, quality definitions, custom formats, media management, indexer settings, naming, delay profiles, root folders, download client settings, import lists, list exclusions, tags) and the movie list from the Radarr configured in `[radarr]` and stores them.
- [ ] AC-02 (option): Where `--exports <folder>` is given, sofarr reads the same documents from the `<name>.json` files of a config export folder instead of the API, leaves out every document whose file is missing, and — for `import-sonarr` — reads the settings and the series list only, without episodes and episode files.
- [ ] AC-03 (event): When an import finishes, then the CLI prints a German summary: for Radarr the number of settings, movies, movies with a file and skipped entries, followed by the number of quality profiles and custom formats of the resulting rules, or a German hint that the rules are still incomplete; for Sonarr the number of settings, series, episodes and files.
- [ ] AC-04 (always): sofarr keeps one movie per TMDB id: importing again updates the movie's fields from Radarr in the same record, leaves movies Radarr no longer lists unchanged, and removes the file record of a movie Radarr lists without a file; an entry without a TMDB id or without a title is skipped and counted as skipped.
- [ ] AC-05 (always): An imported movie carries Radarr's title, original title, IMDb id, year, runtime, original language, monitored flag, quality profile, folder, Radarr id, alternative titles, tags, minimum availability (`released` when Radarr gives none), release status and the dates of cinema, digital and physical release (date only), poster and overview.
- [ ] AC-06 (always): The file of an imported movie carries Radarr's relative path, size, quality, revision, release group and scene name; its custom format score is Radarr's `customFormatScore`, and where Radarr gives none, the score the imported custom formats give the scene name under the movie's profile.
- [ ] AC-07 (event): When the user runs `amigo-sofarr import-sonarr` without `--exports`, then sofarr reads the settings (quality profiles, quality definitions, custom formats, media management, indexer settings, naming, download client settings, root folders, tags), the series list and, for every series, its episodes and episode files from the Sonarr configured in `[sonarr]`, and stores them: one series per TVDB id, one episode per season and number, a file that holds several episodes linked to each of them; importing again adds no second record, and a series without a TVDB id or title is left out.
- [ ] AC-08 (always): Sonarr's settings are kept apart from Radarr's: movies are decided, named and imported by Radarr's profiles, custom formats and settings, series by Sonarr's; a series import leaves the movies unchanged.
- [ ] AC-09 (always): A setting that an import does not bring along keeps its stored value.
- [ ] AC-10 (error): If Radarr or Sonarr refuses the API key or cannot be reached, then the import stops with a German error message that names the app and the reason (the refused key with its HTTP status, or the endpoint that could not be fetched).
- [ ] AC-11 (error): If the export folder holds none of the known documents, a document is no valid JSON, or no `--exports` is given while `[radarr]` (`import-radarr`) or `[sonarr]` (`import-sonarr`) is missing in the configuration, then the import stops with a German error message (naming the file for invalid JSON, the missing section otherwise) and the CLI exits with a code ≠ 0.

## Technical notes

- Settings are stored in the `settings` table under their export names (`qualityprofile`,
  `config_naming`, …); Sonarr's carry the prefix `sonarr_`. The API path of a key is the key with
  `config_` turned into `config/` (`config_mediamanagement` → `api/v3/config/mediamanagement`).
- Export folders are the layout of `scripts/export-arr-config.sh` (`…/config/radarr`,
  `…/config/sonarr`); `movie.json` and `series.json` hold the library lists.
- The API is called with the `X-Api-Key` header; errors never contain the URL (the key could be
  in it).
- Where the scene name is missing, the custom format score is computed from the relative path.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| The settings are imported after the movies, or not at all | Movies are stored; scores that Radarr does not give are 0; the CLI hints that the rules are incomplete (AC-03) |
| An import runs again after the user changed a movie in sofarr (monitored, profile) | Radarr's values overwrite sofarr's (AC-04): Radarr stays the source until it is switched off |
| An import runs again after a metadata refresh | Radarr's fields overwrite the movie's; the German title from TMDB is cleared for movies (series keep theirs) until the next refresh |
| Radarr lists a movie with TMDB id 0 | Skipped and counted (AC-04) |
| A Sonarr export folder is used | Series without episodes; the next `import-sonarr` against the API or `refresh-series` brings the episodes |
| Sonarr lists an episode without season or episode number | Left out |
| The export folder holds no known file at all | Error; the message speaks of a Radarr export for Sonarr folders too (AC-11) |
| Radarr refuses the key half-way through the documents | Nothing is stored: every document is fetched before the first one is written |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| An import never deletes movies or series | A partial export or a filtered API answer must not wipe the library | A movie removed in Radarr stays in sofarr until removed there too |
| Radarr's own custom format score wins over one computed by sofarr | Radarr scored the file with its full knowledge (media info, languages); the computed score is only a fallback (`docs/parity.md`, "CF-Score vorhandener Dateien") | A stale score in Radarr is taken over |
| Sonarr export folders carry no episodes | The export script stores only settings and the series list; episodes need the API | A takeover from an export alone has no episodes until a refresh |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `library_import_tests::fetches_settings_and_movies_from_radarr` (`crates/sofarr-server/src/library_import_tests.rs`) | built | a93a7ca |
| AC-02 | `import_radarr_reads_an_export_folder`, `import_sonarr_reads_series_without_episodes_from_an_export_folder` (`crates/sofarr-server/tests/cli_import.rs`) | built | a93a7ca |
| AC-03 | `import_radarr_reads_an_export_folder`, `import_sonarr_reads_series_without_episodes_from_an_export_folder` (`crates/sofarr-server/tests/cli_import.rs`) | built | a93a7ca |
| AC-04 | `library_import_tests::reimport_updates_movies_by_tmdb_id_and_keeps_the_rest` (`crates/sofarr-server/src/library_import_tests.rs`); `library::tests::imports_library_and_computes_wanted` (`crates/sofarr-server/src/library.rs`) | built | a93a7ca |
| AC-05 | `library_import_tests::movies_keep_radarrs_fields` (`crates/sofarr-server/src/library_import_tests.rs`); `image_tests::imports_and_refreshes_store_poster_paths` (`crates/sofarr-server/src/image_tests.rs`) | built | a93a7ca |
| AC-06 | `library_import_tests::movies_keep_radarrs_fields` (`crates/sofarr-server/src/library_import_tests.rs`); `library::tests::imports_library_and_computes_wanted` (`crates/sofarr-server/src/library.rs`) | built | a93a7ca |
| AC-07 | `library_import_tests::fetches_series_episodes_and_files_from_sonarr` (`crates/sofarr-server/src/library_import_tests.rs`); `series::tests::imports_sonarr_and_decides` (`crates/sofarr-server/src/series/tests.rs`) | built | a93a7ca |
| AC-08 | `library_import_tests::movie_and_series_settings_stay_apart` (`crates/sofarr-server/src/library_import_tests.rs`) | built | a93a7ca |
| AC-09 | `library_import_tests::reimport_updates_movies_by_tmdb_id_and_keeps_the_rest` (`crates/sofarr-server/src/library_import_tests.rs`) | built | a93a7ca |
| AC-10 | `library_import_tests::a_rejected_key_or_an_unreachable_app_stops_the_import` (`crates/sofarr-server/src/library_import_tests.rs`) | built | a93a7ca |
| AC-11 | `imports_fail_on_bad_exports_and_missing_connections` (`crates/sofarr-server/tests/cli_import.rs`) | built | a93a7ca |

## Decisions

- 2026-11-19: Extracted from the existing code (phases 3 and 5) with `spec-extract`; tests for ACs without one were added in a93a7ca.
