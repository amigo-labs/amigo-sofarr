---
status: implemented
created: 2026-11-19
size: M
design: [sofarr.md §2, sofarr.md §5, sofarr.md §9 D11, sofarr.md §9 D19]
media: movies
oracle: GET /api/v3/movie
---

# Import lists

## Summary

> Completed: 2026-11-19

- The import list sync reads TMDB movie credits for every usable `TMDbPersonImport` list (enabled, synced automatically, with person and quality profile) and adds the selected cast and crew-department movies that are neither in the library nor excluded.
- Added movies carry TMDB metadata and the list's quality profile, minimum availability, tags, root folder and monitoring; search on add only for monitored, available movies; a movie several lists select is added once with the first list's settings.
- Broken lists are skipped with German warnings, a movie without TMDB details is retried at the next sync, unreadable credits end the sync with an error.
- Schedule `[import_lists] interval_hours` (default 24, live mode with `[tmdb]` only) and the CLI `amigo-sofarr sync-import-lists` (also in shadow mode, prints `Hinzugefügt: <n>` and the titles).
- Fix: credits are kept once per movie and role, so a crew-only list no longer misses movies the person also acted in (AC-11).

## Context

Without Radarr, new movies reach the library only through import lists. D11 put TMDb person
lists before the switch-off of Radarr (phase 4): the owner's setup has five active
`TMDbPersonImport` lists, and without them no new movies would arrive. This spec records the
behavior built in phase 4, before the spec workflow existed (reverse-engineered on 2026-11-19).

The lists are the `importlist` setting taken over from Radarr (`import-radarr`), plus the person
lists created from a person page (spec `discover`, AC-12). The exclusion list is Radarr's
`exclusions` setting. The add flow (metadata, folder, monitoring, search when available) is the
one the discover pages use for a single movie (spec `discover`, AC-07); this spec owns the sync
that drives it.

Terms used below:

- **Import list sync** – one pass over all usable lists: read each person's movie credits from
  TMDB, select the movies, add the new ones.
- **Usable list** – an entry of the `importlist` setting of type TMDb person
  (`implementation: TMDbPersonImport`) that is enabled (`enabled` not `false`), synced
  automatically (`enableAuto` not `false`) and names a person (`personId`) and a quality profile
  (`qualityProfileId`).
- **Selected movie** – a movie in the person's TMDB movie credits that the list takes: a cast
  credit when the list includes cast, a crew credit when its department is one the list chose.
- **Available** – the movie's minimum availability is reached (the rule of the RSS sync and the
  wanted list).

## Scope

### In scope

- The import list sync over the usable TMDb person lists, on a schedule and by command
- Selection by cast and crew departments (Directing, Production, Sound, Writing), the exclusion
  list, movies already in the library
- Adding with the list's settings and the search on add
- Error handling for TMDB failures

### Out of scope

- Import list types other than TMDb persons (Trakt, IMDb lists, …) — because the owner's setup
  uses only TMDb person lists (D11)
- Series import lists — because the parity matrix (`docs/parity.md`) lists import lists only for Radarr
- Creating person lists from a person page — because spec `discover` (AC-12) owns it
- The add flow's details for a single movie (folder by naming format, scan of an existing folder)
  — because spec `discover` (AC-07) owns it
- Editing lists or the exclusion list in sofarr — because they come from Radarr's export and
  from the person pages; nothing else changes them today
- Removing movies a person list no longer selects — because the sync only adds; nothing in
  the owner's setup asks for a clean-up
- `compare-import-lists` against a running Radarr — because it is a manual check against a live
  instance (profile **Tests**: `manual:`), not a behavior of the sync

## Acceptance criteria

- [x] AC-01 (event): When the import list sync runs, then sofarr reads the movie credits of each usable list's person from TMDB and adds every selected movie that is neither in the library nor on the exclusion list; a list includes cast unless it switches cast off, and crew credits only of the departments it chose (Directing, Production, Sound, Writing).
- [x] AC-02 (event): When the sync adds a movie, then the movie gets its TMDB metadata (title, year, IMDb id, poster) and the list's quality profile, minimum availability and tags, a folder under the list's root folder named by the movie folder format, and is monitored unless the list's monitoring is `none`.
- [x] AC-03 (option): Where a list has search on add, sofarr searches the indexers for an added movie right away if it is monitored and available; a movie that is unmonitored or not yet available is not searched, and a list without search on add searches nothing.
- [x] AC-04 (always): sofarr adds a movie at most once: a movie already in the library is skipped, so a repeated sync adds nothing new; a movie that several lists select is added once, with the settings of the first of these lists in the `importlist` setting.
- [x] AC-05 (state): While no list is usable (none stored, or only lists that are disabled, not synced automatically or of another type), the sync adds nothing and sends no request to TMDB.
- [x] AC-06 (error): If a TMDb person list names no person or no quality profile, then sofarr skips that list with a German warning in the log naming the list (`Import List <Name>: personId fehlt` / `qualityProfileId fehlt`), and syncs the other lists.
- [x] AC-07 (error): If TMDB cannot give the details of one selected movie, then sofarr skips that movie, adds the list's other movies, and tries the skipped one again at the next sync.
- [x] AC-08 (error): If TMDB cannot give a list's credits (unknown person, TMDB unreachable), then the sync ends with an error; movies that earlier lists added in the same run stay in the library.
- [x] AC-09 (option): Where `[import_lists] interval_hours` is set, the sync runs on its own every that many hours (default 24); 0 switches the schedule off, and the schedule runs only in live mode with `[tmdb]` configured.
- [x] AC-10 (event): When the user runs `amigo-sofarr sync-import-lists`, then sofarr runs the sync once, in shadow mode too, and prints the number of added movies (`Hinzugefügt: <n>`).
- [x] AC-11 (event): When the person both acted in and worked in a chosen crew department on a movie, then a list selects it through either credit, and adds it once.

## Technical notes

- Lists: `import_lists::person_lists` reads the `importlist` setting (Radarr's
  `/api/v3/importlist` shape: `name`, `implementation`, `enabled`, `enableAuto`, `monitor`,
  `qualityProfileId`, `rootFolderPath`, `searchOnAdd`, `minimumAvailability`, `tags`, `fields`
  with `personId` (number or numeric string), `personCast`, `personCastDirector`,
  `personCastProducer`, `personCastSound`, `personCastWriting`). Defaults of missing keys: cast
  on, no crew department, monitored, no search, `minimumAvailability` `released`, no tags, empty
  root folder.
- Exclusions: the `exclusions` setting, entries with `tmdbId`.
- Credits: `Tmdb::person_movies` (`person/{id}/movie_credits`); cast entries get the role `cast`,
  crew entries their `department`; a movie appears once per role (cast, and once per department).
- The sync runs under the jobs lock (RSS sync, download check and disk scans wait) and returns
  the titles it added; the CLI prints them one per line after the count.
- Scheduler: `status::Job::ImportLists` (`import-lists`, label „Import-Listen“), first run right
  at start (`app::every`, not delayed); its state shows in the system status (spec
  `web-ui-shell`).
- The search on add is `App::search_movie` (spec `fail-handling` AC-12 rules); in shadow mode it
  records shadow grabs only.
- Oracle: `amigo-sofarr compare-import-lists` lists, per list, the selected movies that are
  neither in Radarr's library (`GET /api/v3/movie`) nor excluded there (`/api/v3/exclusions`);
  Radarr adds list movies itself, so the set should be empty.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `personId` given as a string (`"1234"`) | Read as the number 1234 |
| A movie the person both acted in and directed, list with Directing only | Selected (AC-11) |
| The same movie, list with cast and Directing | Selected and added once (AC-11) |
| A selected movie is announced (not available yet) on a list with search on add | Added, not searched; the RSS sync takes it once available |
| A selected movie is on the exclusion list | Never added, also not by a later sync |
| A movie in the library that the list selects | Left unchanged (monitoring, profile and tags are not touched) |
| TMDB answers for the credits but not for one movie | That movie is skipped (warning in the log), the rest is added (AC-07) |
| `[tmdb]` missing while usable lists exist | The sync ends with the error `[tmdb] fehlt in der Konfiguration`; the schedule is off anyway (AC-09) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| A movie already in the library is never changed by a list | Radarr's lists do not re-apply their settings to existing movies in the owner's setup | A list's profile change does not reach movies it added earlier |
| The sync stops at the first list whose credits fail | Silent partial syncs would hide a broken list | One broken list delays the lists after it until it is fixed |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `import_list_tests::the_import_list_sync_adds_selected_movies_with_the_lists_settings` (`crates/sofarr-server/src/import_list_tests.rs`); `import_lists::tests::reads_person_lists`, `import_lists::tests::missing_list_keys_have_defaults` (`crates/sofarr-server/src/import_lists.rs`) | verified | 83e52b8 |
| AC-02 | `import_list_tests::the_import_list_sync_adds_selected_movies_with_the_lists_settings` (quality profile 3, `inCinemas`), `import_list_tests::unmonitored_or_silent_lists_add_without_searching` (`crates/sofarr-server/src/import_list_tests.rs`) | verified | 83e52b8, 9ba1551 |
| AC-03 | `import_list_tests::the_import_list_sync_adds_selected_movies_with_the_lists_settings`, `import_list_tests::unmonitored_or_silent_lists_add_without_searching` (`crates/sofarr-server/src/import_list_tests.rs`) | verified | 83e52b8 |
| AC-04 | `import_list_tests::the_import_list_sync_adds_selected_movies_with_the_lists_settings`, `import_list_tests::unmonitored_or_silent_lists_add_without_searching` (`crates/sofarr-server/src/import_list_tests.rs`) | verified | 83e52b8 |
| AC-05 | `import_list_tests::without_a_usable_list_tmdb_is_not_asked` (`crates/sofarr-server/src/import_list_tests.rs`); `import_lists::tests::reads_person_lists` (`crates/sofarr-server/src/import_lists.rs`) | verified | 83e52b8 |
| AC-06 | `import_lists::tests::reads_person_lists`, `import_lists::tests::missing_list_keys_have_defaults` (`crates/sofarr-server/src/import_lists.rs`); `import_list_tests::without_a_usable_list_tmdb_is_not_asked`, `import_list_tests::a_broken_list_is_skipped_with_a_warning_and_the_others_sync` (warnings in the log, the usable list still adds) (`crates/sofarr-server/src/import_list_tests.rs`) | verified | 83e52b8, 9ba1551 |
| AC-07 | `import_list_tests::a_movie_without_details_is_skipped_and_the_rest_added` (`crates/sofarr-server/src/import_list_tests.rs`) | verified | 83e52b8 |
| AC-08 | `import_list_tests::unreadable_credits_end_the_sync_with_an_error` (`crates/sofarr-server/src/import_list_tests.rs`) | verified | 83e52b8 |
| AC-09 | `import_list_tests::the_sync_is_scheduled_only_live_with_tmdb` (`crates/sofarr-server/src/import_list_tests.rs`) | verified | 83e52b8 |
| AC-10 | `sync_import_lists_prints_the_added_count`, `sync_import_lists_adds_and_prints_a_lists_movie_in_shadow_mode` (fake TMDB, shadow mode, `Hinzugefügt: 1` and the title) (`crates/sofarr-server/tests/cli_import_lists.rs`) | verified | 83e52b8, 9ba1551 |
| AC-11 | `tests::reads_credits_once_per_movie_and_role` (`crates/sofarr-metadata/src/lib.rs`); `import_lists::tests::reads_person_lists` (crew-only list and cast+crew list select the movie once) (`crates/sofarr-server/src/import_lists.rs`) | verified | 4f2b784 |

## Decisions

- 2026-11-19: Reverse-engineered from the phase-4 code (`spec-extract`); the ACs describe the
  behavior as built. Uncertain points are recorded under Assumptions instead of being asked.
- 2026-11-19 (spec-fix): TMDB credits are kept once per movie and role, and a list selects each movie once, because merging per movie (cast first) made a crew-only list miss movies the person also acted in (found while extracting).
