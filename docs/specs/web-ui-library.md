---
status: todo
created: 2026-11-12
size: L
design: [sofarr.md §2, sofarr.md §4, sofarr.md §7, sofarr.md §9 D16, sofarr.md §9 D23, sofarr.md §9 D24]
media: both
oracle: GET /api/v3/rename (rename preview)
---

# Web UI: library

## Context

With spec `web-ui-shell` sofarr has a UI frame; this spec fills it with the pages for day-to-day
use after Radarr and Sonarr are switched off: what is downloading, what was grabbed or imported,
what is missing, the movie and series library with their files, a calendar, the blocklist (spec
`fail-handling`) and the backups (spec `backups`). The owner's library has about 2,400 movies
(about 50 monitored) and 150 series (`docs/parity.md`).

Terms used below:

- **Item** – a movie, a series, a season or an episode.
- **Queue** – the active grabs (spec `fail-handling`) with the download client's progress.
- **Activity** – grabs and imports, newest first, with their state and message.
- **Import problem** – a finished download whose import failed (the grab stays completed with
  a message), e.g. no matching file, not enough free space, a target that exists.
- **Missing** – a monitored movie or episode (of a monitored series) without a file that is
  available (movies: per `minimumAvailability`; episodes: aired). **Cutoff unmet** – a monitored
  item whose file is below its profile's cutoff. A series counts as missing or cutoff unmet when
  at least one of its episodes does, and as complete when none of its monitored, aired episodes
  is missing.
- **Release dates** – a movie's cinema, digital and disc dates; an episode's air date.
- **File action** – an action that moves, renames or deletes files: rename, move folder, remove
  with files.
- **Current folder name** – the folder name the movie or series would get today from the naming
  format (`movieFolderFormat`, `seriesFolderFormat`) and its current metadata.
- **Poster** – the image TMDB (movies) or TVDB (series) offers for an item, served through sofarr (D24).

## Scope

### In scope

- Pages: Übersicht, Filme, Film, Serien, Serie, Kalender, Fehlend, Blocklist, Backups
- List view with filters, search and sorting, switchable to a poster grid
- Actions: search, mark as failed, monitor on/off (also in bulk), change quality profile and minimum availability, remove (optionally with files and onto the exclusion list), rename files, move a folder to its current folder name
- Poster images through sofarr with a cache
- The API endpoints these pages need

### Out of scope

- Adding new movies or series — because that comes with spec `discover` (search, trending, people)
- Interactive release search (a list of releases to pick) — because grabs follow the decision engine; manual search grabs the best release (spec `fail-handling`)
- Editing quality profiles, custom formats or naming formats — because they are imported from Radarr/Sonarr and edited in `sofarr.toml`'s source of truth; a later spec may add it
- Subtitles and trailers on the item pages — because specs `subtitles` and `trailers` add them
- Bulk rename or move of the whole library at once — because a preview per item keeps file moves under the owner's eye; bulk can follow when trusted

## Acceptance criteria

- [ ] AC-01 (event): When the user opens the overview (the page at `/`), then it shows the queue with progress, size and remaining time per download ("–" where the client reports none, e.g. paused or waiting), the last 20 activities, every failed grab and import problem of the last 7 days, and the release dates of monitored items in the next 7 days; it refreshes (spec `web-ui-shell`, every 15 seconds).
- [ ] AC-02 (event): When the user opens the movie list, then it shows the movies of the library with poster, title, year, quality of the file (or "fehlt"), monitored state and labels for missing and cutoff unmet, as a list by default and as a poster grid after switching; further movies load while scrolling until every matching movie is shown; the chosen view is remembered in the browser.
- [ ] AC-03 (event): When the user filters (monitored, missing, cutoff unmet, with file — each as yes, no or any — quality profile, tag; all filters combined with AND), searches by title (also original, German and alternative titles) or sorts (title, year, date added, size), then the list shows exactly the matching movies; filter, search and sorting are part of the page address, so a reload or a shared link shows the same list.
- [ ] AC-04 (always): With 2,500 movies the list shows its first entries within 1 second of opening and updates within 200 ms after a filter or search change (Playwright measurement against the built binary with a generated library).
- [ ] AC-05 (event): When the user opens a movie, then it shows poster, titles, year, overview text, release dates, status (announced, in cinemas, released) and whether it counts as available by its minimum availability, quality profile, tags, folder, the file with quality, size, release group and media info (spec `mediainfo`, when built), the grabs of this movie with state and message, and its blocklist entries.
- [ ] AC-06 (event): When the user opens the series list or a series, then they get the same as AC-02, AC-03 and AC-05 for series, where a series shows the number of files instead of a file quality, labels with the number of missing and cutoff-unmet episodes, and the filters are monitored, missing, cutoff unmet and complete (each yes, no or any; no "with file"), following the series rule above; a series shows its seasons with episode counts (with file / monitored / total) and, per season, the episodes with number, title, air date, file and monitored state.
- [ ] AC-07 (event): When the user opens the calendar, then it shows the release dates of monitored movies and the air dates of all episodes of monitored series for the chosen week or month, each date as its own entry (a movie with cinema, digital and disc dates appears three times) marked as present (the item has a file), not yet released (the date lies ahead), missing (the item counts as missing) or neutral (otherwise, e.g. an unmonitored episode or a movie not yet available by its minimum availability); it starts at the current week and can step back and forth.
- [ ] AC-08 (event): When the user opens the missing page, then it lists missing and cutoff-unmet items in two tabs with item, profile and the reason ("fehlt", or the file's quality and the cutoff it misses), and offers a search for one item, several selected ones, or all items of the tab (every page).
- [ ] AC-09 (event): When the user starts a search or marks a grab as failed from any page, then the UI calls the actions of spec `fail-handling` and shows their German result (grabs made, errors) without leaving the page.
- [ ] AC-10 (event): When the user switches monitoring for one item, then it is saved at once; for several selected items the UI asks one confirmation naming their number and then saves all of them or, on an error, none; switching a series or season switches its episodes.
- [ ] AC-11 (event): When the user changes the quality profile of a movie or series, or the minimum availability of a movie, then, before saving, the page shows which items would become missing or cutoff unmet by it; after saving, the new value applies to the next decision.
- [ ] AC-12 (event): When the user removes a movie or series, then a dialog offers "Dateien löschen" and "auf die Ausschlussliste"; after confirming, the item leaves the library, its files go to the recycle bin (`recycleBin`) or are deleted when none is set, and with the exclusion option an import list never adds it again. If a file cannot be moved to the recycle bin or deleted, the item stays in the library and points only to the files still in place, files already moved stay in the recycle bin, and the UI shows a German message naming the file.
- [ ] AC-24 (error): If a file action is started for an item with an active grab or a running import, then sofarr refuses it with a German message; removing such an item without files also removes its active downloads from the download client.
- [ ] AC-13 (event): When the user opens the rename preview of a movie or series, then it shows every file whose name differs from the naming format with old and new name; after confirming, the files are renamed and the library points to the new names.
- [ ] AC-14 (always): The new file names of the rename preview are the same names the import would give the file today (same naming tokens, spec `mediainfo` when built).
- [ ] AC-23 (event, manual: needs running Radarr/Sonarr): When the owner runs `compare-rename` for movies (against Radarr) or, new, for series (against Sonarr's rename preview), then it reports sofarr's names against the oracle's per file; the command for series is tested against a fake Sonarr.
- [ ] AC-15 (event): When the folder of a movie or series differs from its current folder name (e.g. because its title changed after it was added), then its page shows this with old and new folder; after confirming the move, the folder and everything in it move to the new name under the same root folder and the library points to the new path.
- [ ] AC-16 (error): If a rename or move target already exists, the source is gone, or moving fails midway, then nothing is overwritten, files already moved are moved back where possible, the library points to where the files actually are, and the UI shows a German message naming the file.
- [ ] AC-17 (state): While sofarr runs in shadow mode, file actions are not offered — the UI says that they need live mode — and actions that only change the library (monitoring, profile, removing without files) work.
- [ ] AC-18 (event): When the user opens the blocklist page, then it lists the blocklist page by page (spec `fail-handling`) with item, release title, reason and date, and removes a selected entry after confirmation.
- [ ] AC-19 (event): When the user opens the backups page, then it lists the backups (spec `backups`) and lets the user create, download and delete one, choose one for restore with a confirmation that names its time, and cancel a pending restore; the last backup error is shown.
- [ ] AC-20 (always): Posters reach the browser only from sofarr: the first request loads an image from TMDB/TVDB and stores it, every later request is answered from the cache, also while the image provider cannot be reached; if storing fails (folder not writable, disk full), the image is still delivered, the failure is logged, and a later request tries to store it again.
- [ ] AC-21 (error): If an item has no poster or the provider cannot be reached for an uncached one, then the UI shows a neutral placeholder with the item's initial; no request leaves the browser for anyone but sofarr.
- [ ] AC-22 (always): Every list and page works at 360 px width without horizontal scrolling (spec `web-ui-shell`), with lists shown as cards on narrow screens.

## Technical notes

- New API endpoints (sofarr's own JSON, types generated for the UI): movie and series lists with
  filter/search/sort/paging, movie/series/season/episode details, queue, activity, calendar
  (`from`, `to`), missing and cutoff-unmet lists, monitor (bulk), profile/availability update,
  remove, rename preview + execute, folder move preview + execute, images
  (`GET /api/images/{kind}/{id}/{size}` with `size` = `list` or `detail`, mapped to the provider's
  sizes, e.g. TMDB `w342` / `w780`, TVDB's poster and its thumbnail).
- Image cache under `/config/cache/images/`, keyed by provider path and size (e.g. TMDB `w342`
  for lists, `w780` for detail pages); a changed provider path loads the new image.
- Rename and folder names reuse the naming code of the importer (`importer::movie_folder`,
  `sofarr_core::naming`); `compare-rename` (`/api/v3/rename`) already checks the names against Radarr.
- Removing reuses the recycle-bin handling of the import (old files on upgrade).
- Exclusions: the imported `exclusions` document of the import lists.
- Page addresses: `/filme`, `/filme/{id}`, `/serien`, `/serien/{id}`, `/kalender`, `/fehlend`,
  `/blocklist`, `/backups`.
- Lists page on the server (filters applied server-side) so 2,500 items stay fast (AC-04).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| The download client cannot be reached | The queue shows the last known state with a German notice (spec `web-ui-shell` AC-13) |
| Removing a series with files spread over several season folders | All its files go to the recycle bin; empty folders are removed like the disk scan does |
| A rename where two files would get the same name (e.g. duplicate episodes) | Refused before anything moves (AC-16) |
| Moving a folder whose new name differs only in letter case | Moved via a temporary name, so it works on case-insensitive file systems |
| A movie whose German title changed and the format uses `{Movie Title:DE}` | Its folder shows as differing (AC-15) |
| Bulk monitoring for 300 selected episodes | One request, one confirmation |
| A poster changes at TMDB | The new image is loaded once the metadata refresh sees the new path |
| The calendar for a month without releases | An empty calendar with a German note, no error |
| A search started for all 400 missing episodes | Runs as one search request; the page shows that it runs and the result when done |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The image cache has no size limit | Posters of ~2,600 titles at list size are about 150 MB | A very large library grows the cache; a limit can be added |
| Rename and folder moves are offered per item, not for the whole library | The owner sees every move before it happens | Renaming many items takes many clicks |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `overview_tests::queue_shows_progress_size_and_remaining_time` (`crates/sofarr-server/src/overview_tests.rs`); `overview_tests::overview_shows_activity_problems_and_the_next_days` (`crates/sofarr-server/src/overview_tests.rs`); `library::tests::activity_and_problems_newest_first` (`crates/sofarr-db/src/library/tests.rs`); `overview (AC-01), two tests` (`ui/e2e/library-pages.spec.ts`) | built |        |
| AC-02 | `library_api_tests::movie_list_filters_searches_sorts_and_pages` (`crates/sofarr-server/src/library_api_tests.rs`); `shows the movies as a list with poster, title, year, quality and labels; loads further movies while scrolling; the poster grid is remembered in the browser` (`ui/e2e/library-lists.spec.ts`); unit `rows.test.ts`, `paged.test.ts` | built |        |
| AC-03 | `library_api_tests::movie_list_filters_searches_sorts_and_pages` (`crates/sofarr-server/src/library_api_tests.rs`); `filters combine with AND, are part of the address and survive a reload; cutoff filter matches the API; searches original and alternative titles; sorts by year descending, kept in the address` (`ui/e2e/library-lists.spec.ts`); unit `list-query.test.ts` | built |        |
| AC-04 | `library_api_tests::a_large_library_answers_a_filtered_page_fast` (`crates/sofarr-server/src/library_api_tests.rs`); `library::tests::every_library_change_moves_the_version` (`crates/sofarr-db/src/library/tests.rs`); `first entries within 1 s, filter and search updates within 200 ms (Playwright project perf, bench server with 2,501 movies)` (`ui/e2e/library-perf.spec.ts`) | built |        |
| AC-05 | `library_api_tests::movie_page_shows_everything_about_one_movie` (`crates/sofarr-server/src/library_api_tests.rs`); `library_view::tests::movies_are_missing_only_when_available` (`crates/sofarr-server/src/library_view.rs`); `shows cinema, digital and disc dates and the tags` (`ui/e2e/library-detail.spec.ts`); `shows titles, dates, status, availability, profile, folder and the file; grabs with state and message and blocklist entries` (`ui/e2e/library-detail.spec.ts`); `library_api_tests::movie_page_shows_the_media_info_read_from_the_file` (`crates/sofarr-server/src/library_api_tests.rs`); `shows the media info read from the file` (`ui/e2e/library-detail.spec.ts`) | built |        |
| AC-06 | `library_api_tests::series_list_counts_and_filters_by_the_series_rule` (`crates/sofarr-server/src/library_api_tests.rs`); `library_api_tests::series_page_shows_seasons_and_episodes`, `library_api_tests::series_page_shows_what_the_movie_page_shows` (`crates/sofarr-server/src/library_api_tests.rs`); `series list (AC-06)` (`ui/e2e/library-lists.spec.ts`); `shows seasons with counts and episodes with number, title, air date, file, monitoring`, `shows what the movie page shows: overview, profile, tags, folder, grabs, blocklist and the file of an episode` (`ui/e2e/library-detail.spec.ts`) | built |        |
| AC-07 | `overview_tests::calendar_marks_every_date_of_a_range` (`crates/sofarr-server/src/overview_tests.rs`); `overview_tests::calendar_shows_all_episodes_of_monitored_series` (`crates/sofarr-server/src/overview_tests.rs`); `calendar tests: current week, three dates of a movie, stepping, empty month` (`ui/e2e/library-pages.spec.ts`); unit `calendar.test.ts` | built |        |
| AC-08 | `library_edit_tests::missing_page_lists_both_tabs_with_reasons` (`crates/sofarr-server/src/library_edit_tests.rs`); `library_edit_tests::a_tab_is_searched_as_one_request` (`crates/sofarr-server/src/library_edit_tests.rs`); `lists missing and cutoff-unmet items in two tabs; searches one item, the selected ones and all of the tab` (`ui/e2e/library-pages.spec.ts`) | built |        |
| AC-09 | `library_edit_tests::a_tab_is_searched_as_one_request` (`crates/sofarr-server/src/library_edit_tests.rs`); `a search shows its result without leaving the page; a search report with grabs and errors reads in German; marking without a reachable download client reports the German message` (`ui/e2e/library-detail.spec.ts`); unit `format.test.ts` | built |        |
| AC-10 | `library_edit_tests::monitoring_one_or_many_items_is_saved_at_once_or_not_at_all` (`crates/sofarr-server/src/library_edit_tests.rs`); `library_edit_tests::switching_a_series_or_season_switches_its_episodes` (`crates/sofarr-server/src/library_edit_tests.rs`); `one item is saved at once; several items after one confirmation naming their number; a failing bulk change saves none` (`ui/e2e/library-lists.spec.ts`); `switching a season switches its episodes` (`ui/e2e/library-detail.spec.ts`) | built |        |
| AC-11 | `library_edit_tests::a_profile_change_shows_what_it_does_and_applies_to_the_next_decision` (`crates/sofarr-server/src/library_edit_tests.rs`); `library_edit_tests::an_availability_change_shows_movies_that_become_missing` (`crates/sofarr-server/src/library_edit_tests.rs`); `the page shows what becomes missing before saving the availability; a profile change is previewed, saved, or dropped` (`ui/e2e/library-detail.spec.ts`) | built |        |
| AC-12 | `library_files_tests::removing_a_series_with_files_uses_the_recycle_bin` (`crates/sofarr-server/src/library_files_tests.rs`); `library_files_tests::removing_a_movie_without_recycle_bin_deletes_its_files` (`crates/sofarr-server/src/library_files_tests.rs`); `remove with and without files (live server)` (`ui/e2e/library-files.spec.ts`) | built |        |
| AC-13 | `library_files_tests::rename_gives_the_name_the_import_would_give` (`crates/sofarr-server/src/library_files_tests.rs`); `the preview shows old and new name; after confirming the library has the new name` (`ui/e2e/library-files.spec.ts`) | built |        |
| AC-14 | `library_files_tests::rename_gives_the_name_the_import_would_give` (`crates/sofarr-server/src/library_files_tests.rs`); `rename_media_tests::a_movie_rename_takes_the_media_info_tokens_from_the_file_like_the_import`, `rename_media_tests::an_episode_rename_takes_the_media_info_tokens_from_the_file` (`crates/sofarr-server/src/rename_media_tests.rs`) | built |        |
| AC-15 | `library_files_tests::a_changed_title_moves_the_folder_also_in_letter_case` (`crates/sofarr-server/src/library_files_tests.rs`); `a differing folder shows old and new; after confirming it moves` (`ui/e2e/library-files.spec.ts`) | built |        |
| AC-16 | `library_files_tests::an_existing_target_is_never_overwritten` (`crates/sofarr-server/src/library_files_tests.rs`); `library_files_tests::a_series_rename_that_fails_midway_moves_back` (`crates/sofarr-server/src/library_files_tests.rs`); `a rename or move whose target exists is refused with a German message` (`ui/e2e/library-files.spec.ts`) | built |        |
| AC-17 | `library_files_tests::shadow_mode_refuses_file_actions_but_removes_from_the_library` (`crates/sofarr-server/src/library_files_tests.rs`); `file actions are not offered; removing without files works; monitoring and profile still work in shadow mode` (`ui/e2e/library-detail.spec.ts`) | built |        |
| AC-18 | `api_tests::blocklist_is_listed_page_by_page`, `api_tests::removing_a_blocklist_entry_frees_the_release` (`crates/sofarr-server/src/api_tests.rs`); `lists the entries page by page and removes one after confirmation; an empty blocklist says so` (`ui/e2e/library-pages.spec.ts`) | built |        |
| AC-19 | `backup_api_tests::a_manual_backup_through_the_api`, `the_backup_list`, `download_and_delete_a_backup`, `choosing_and_cancelling_a_restore` (`crates/sofarr-server/src/backup_api_tests.rs`); `create, download, choose for restore with its time, cancel, delete; the last backup error is shown` (`ui/e2e/library-pages.spec.ts`) | built |        |
| AC-20 | `image_tests::posters_are_cached_and_served_while_the_provider_is_down` (`crates/sofarr-server/src/image_tests.rs`); `image_tests::a_cache_that_cannot_store_still_delivers_and_tries_again` (`crates/sofarr-server/src/image_tests.rs`); `metadata::tests::the_refresh_stores_poster_and_overview_and_keeps_them` (`crates/sofarr-server/src/metadata.rs`); movie page test in `ui/e2e/library-detail.spec.ts` (poster loaded from `/api/images/...`) | built |        |
| AC-21 | `image_tests::posters_are_cached_and_served_while_the_provider_is_down` (`crates/sofarr-server/src/image_tests.rs`); `an unavailable poster shows the initial; no request leaves for anyone but sofarr` (`ui/e2e/library-detail.spec.ts`); unit `rows.test.ts` | built |        |
| AC-22 | `narrow screens (AC-22), three tests` (`ui/e2e/library-lists.spec.ts`); `360 px (AC-22), all 9 pages` (`ui/e2e/library-a11y.spec.ts`) | built |        |
| AC-23 | `compare_rename::tests::reports_names_per_file_against_sonarr` (`crates/sofarr-server/src/compare_rename.rs`); manual: Daniel Rück checked `compare-rename` against Radarr (200 movies) and Sonarr (3,625 episode files) with `scripts/compare-acceptance.ps1`, which reported the names per file (deviations in `docs/parity.md`, "Stand Dateinamen"), 2026-11-12 | built |        |
| AC-24 | `library_files_tests::an_active_download_blocks_file_actions_and_goes_with_the_item` (`crates/sofarr-server/src/library_files_tests.rs`); `a file action refused for an active download shows the German message` (`ui/e2e/library-detail.spec.ts`) | built |        |

## Decisions

- 2026-11-12: List view by default, switchable to posters; calendar as its own page (owner).
- 2026-11-12: Actions: monitoring, profile, remove, rename, and moving a folder whose title changed since it was added (owner).
- 2026-11-12: Posters through sofarr with a cache (owner; D24).
- 2026-11-12: New endpoints live under `/api/library/...`; the existing `/api/movies` and `/api/wanted` keep their shape so existing clients and scripts keep working.
- 2026-11-12: List cache: a counter `library_version` (migration 0008, triggers on movies, series, episodes, files, titles and settings) keeps a computed list in memory until the library or the day changes; without it an answer took 120–200 ms in a debug build, with it under 10 ms (AC-04).
- 2026-11-12: "Missing" as the spec defines it: a movie counts as missing only once it is available by its minimum availability; the CLI `wanted` and `/api/wanted` stay unchanged.
- 2026-11-12: Switching a series on monitors its episodes except season 0, like the metadata refresh; switching it off affects all episodes.
- 2026-11-12: "Today" is UTC, as everywhere else in sofarr.
- 2026-11-12: The calendar answers at most 62 days per request (a month view with its weeks); the overview shows only effectively monitored dates, the calendar also unmonitored episodes of monitored series, shown as neutral.
- 2026-11-12: Remaining time is the remaining bytes divided by nzbget's `DownloadRate`, only while downloading, else none.
- 2026-11-12: While the download client cannot be reached, the last known queue stays in memory and is shown with "Download-Client nicht erreichbar – Stand von …".
- 2026-11-12: Import problems are grabs in state completed with a message (including "waiting for episode titles"), dated by their last change (`updated_at`).
- 2026-11-12: A bulk search runs synchronously in one request under the jobs lock; a season is searched as a whole once two or more of its episodes are wanted, else per episode; an unknown target is an error in the report instead of failing the request.
- 2026-11-12: A rename gives the name the import would give: the release title is the scene name, else the file name; a missing release group and revision come from the file record; only video files are renamed, companion files (.srt, .nfo) are not — own subtitles of spec `subtitles` follow their video there.
- 2026-11-12: No ring swaps: an existing target is always refused, even when it is a source of the same run; a change of letter case only goes through a temporary name; duplicate targets are detected ignoring case.
- 2026-11-12: Folder move: the root folder is the longest matching path of the `rootfolder` setting (series: `sonarr_rootfolder`), else the parent folder; paths keep their separator convention.
- 2026-11-12: Removing with files moves every file of the item's folder to `<recycleBin>/<folder name>/<relative path>` (an occupied name gets " (1)"), or deletes them without a recycle bin; then empty folders including the item's folder are removed; after a partial failure the item stays in the library but forgets the moved files; without a configured download client no downloads are removed.
- 2026-11-12: Exclusion list: movies go to Radarr's `exclusions` (`tmdbId`, `movieTitle`, `movieYear`); series to a new setting `sonarr_importlistexclusion` (`tvdbId`, `title`), because sofarr has no series import lists yet.
- 2026-11-12: Image cache under `<database folder>/cache/images` (`/config/cache/images` by default, no new config key), keyed by the SHA-256 of the provider path, stored by kind and size: TMDB w342 for lists and w780 for pages, TVDB's `…_t.<ext>` thumbnail for v4 paths with the poster as fallback; only paths of image.tmdb.org and artworks.thetvdb.com without `..` are taken; the browser may cache an image for a day (`private, max-age=86400`).
- 2026-11-12: Poster paths and overview texts come from the Radarr/Sonarr import (`images`, `overview`), the TMDB/TVDB refresh and the import lists; an empty value never overwrites a known one.
- 2026-11-12: Status codes of file actions: 409 when refused up front (shadow mode, active download, target exists, source gone, duplicate names, nothing to move), 422 when something fails midway, 503 when the download client cannot be reached while removing; 5xx answers stay generic.
- 2026-11-12: `compare-rename --app sonarr [--limit N]` compares series names with Sonarr's rename preview; the default stays `radarr` (AC-23).
- 2026-11-13: Green at once, pinning behavior built before the acceptance asked for it, each checked by a mutation that turns it red: `movie_page_shows_the_media_info_read_from_the_file` and the e2e test `shows the media info read from the file` (media info left out of the file detail / not shown), `series_page_shows_what_the_movie_page_shows` (grabs, blocklist, tags or overview of the series left out, media info of the episode file left out), and the two `rename_media_tests` (renames without the file's media info). The e2e test adds the media info to the real answer in the browser, because the seeded files are no readable videos.
- 2026-11-13: A series page shows per episode a "Datei" section that loads the episode on opening and shows its file like the movie page (quality, size, release group, media info), because AC-06 asks for what AC-05 shows and the API already answered it; the series page e2e test checks overview, profile, tags, folder, grabs with state and message, blocklist and that file, each checked by a mutation (no file section, no history box) that turns it red. `FileBox` names its heading with `$props.id()`, so several open episode files keep their ids unique.
- 2026-11-12: AC-04 is measured in its own Playwright project `perf` after all other tests, against a bench server with an unchanged library, because parallel tests changing the library made the timings meaningless; the browser is warmed on `/system` first, because a fresh headless Chromium holds its very first frame for about 1.3 s whatever the page.
- 2026-11-12: The e2e libraries are seeded the real way: `ui/e2e/seed.mjs` writes a Radarr export for `import-radarr --exports` and serves a fake Sonarr v3 API for `import-sonarr`; grabs, the queue, blocklist entries, the active-grab refusal and a backup error need an indexer or download client and are tested with simulated API answers.
- 2026-11-12: Filters in the page address use German names and values (`?ueberwacht=ja&fehlt=nein&...&sortierung=jahr&richtung=ab`), like the German page addresses; the list or poster view is remembered per list in the browser (`sofarr-ansicht-filme`, `sofarr-ansicht-serien`), not in the address.
- 2026-11-12: Lists load more while scrolling with a "Weitere laden" button as keyboard fallback and are cards on narrow screens; the missing and blocklist pages page with buttons; the calendar is a list of days per week or month instead of a month grid, so it works at 360 px.
- 2026-11-12: Monitoring is one toggle button "Überwachen „Titel“" shown pressed while monitored, because axe refused a control whose visible text differed from its accessible name.
- 2026-11-12: Results of actions appear as notices at the top of the page they started on and are cleared on the next page change; filter choices (profiles, tags) load after the first list entries so they do not delay them.
