---
status: todo
created: 2026-11-12
size: M
design: [sofarr.md §7, sofarr.md §8, sofarr.md §9 D12, sofarr.md §9 D18, sofarr.md §9 D26]
media: both
oracle: none
---

# Trailers

## Context

Jellyfin plays local trailers on every client when they sit in a `trailers` folder next to the
movie or series (or carry a `-trailer` suffix); the YouTube links it gets from TMDB often do not
play on TV clients. D18 decides that sofarr downloads trailers with yt-dlp into `trailers/` and
that the runtime image becomes Alpine with yt-dlp and ffmpeg, because the `scratch` image cannot
run them. The owner's library has about 2,400 movies and 150 series; downloading everything at
once would be hundreds of gigabytes and hammer YouTube, so the backlog is filled slowly.

Terms used below:

- **Trailer candidates** – the videos TMDB lists for a movie or series on YouTube of type Trailer
  or Teaser (no clips or featurettes) in a wanted language (`[trailers] languages`, default
  `["de", "en"]`) or without a language. Ranked strictly in this order: language (the wanted
  languages in their order, then no language), type Trailer before Teaser, official before
  unofficial, then newest — so a German teaser beats an English trailer.
- **Language name** – the German name of the trailer's language in file names and the UI
  (`de` → Deutsch, `en` → Englisch, others by their German name), `Original` without a language.
- **Has a trailer** – the item's folder has a `trailers` folder with a video in it, or a video with a
  `-trailer`, `.trailer` or `_trailer` suffix (Jellyfin's rules), wherever it came from.
- **Eligible item** – a movie or series that has at least one video file and no trailer; a series
  needs its TMDB id (D26, spec `imdb-ratings` AC-12).
- **Newest added** – by the date the item was added to the library.
- **Daily budget** – `[trailers] per_day` downloads per day (default 20); an item counts once when
  a download was attempted for it, an item without candidates does not count.

## Scope

### In scope

- Downloading one trailer per eligible movie and series, newest added first, within the daily budget
- Picking the best candidate and falling back to the next when one fails
- Storing it in the item's `trailers` folder for Jellyfin
- The Alpine runtime image with yt-dlp and ffmpeg
- Keeping trailers out of the import and the disk scan
- API and UI: trailer state per item, a manual download

### Out of scope

- Playing trailers in sofarr's UI — because Jellyfin plays them; the UI links to the YouTube page
- More than one trailer per item, season trailers — because one trailer is what Jellyfin shows first
- Trailers for items without a file — because the owner watches what is in the library; a trailer for a wanted movie can be fetched by hand
- Sources other than YouTube via TMDB — because TMDB lists YouTube almost exclusively
- Updating yt-dlp inside the running container — because the image is rebuilt with a pinned newer version; the container stays read-only

## Acceptance criteria

- [ ] AC-01 (event): When the trailer job runs (daily), then sofarr downloads the trailer of eligible items, newest added first, until the daily budget is used.
- [ ] AC-02 (event): When a movie or series becomes eligible by an import, then it is first in line for the next trailer job.
- [ ] AC-03 (always): sofarr picks the best trailer candidate (AC terms), downloads it at most at `max_height` (default 1080) and stores it as one video file in the item's `trailers` folder, named `<item folder name> - Trailer (<language name>).mkv`, e.g. `Heat (1995) - Trailer (Deutsch).mkv`.
- [ ] AC-15 (error): If the free space on the item's file system is below the import's minimum (`minimumFreeSpaceWhenImporting`), then sofarr downloads no trailer there, counts neither an attempt (AC-05) nor the budget, and the item shows "zu wenig Speicherplatz" (AC-10).
- [ ] AC-04 (error): If a candidate cannot be downloaded (removed, private, blocked in the country, yt-dlp fails, takes longer than 10 minutes, or would exceed 500 MB), then sofarr removes any partial file, tries the next candidate, and counts one download against the budget per item, not per attempt.
- [ ] AC-05 (state): While an item has no candidate left or all failed, it is tried again 7 days later, at most four attempts in total; after that it is left alone until the user starts a manual download.
- [ ] AC-06 (always): sofarr never overwrites, renames or deletes a trailer it did not write — except when the user removes the item with its files, which removes its whole folder; the trailer job never downloads for an item that has a trailer.
- [ ] AC-07 (always): The import and the disk scan never treat a trailer as the movie or episode video: files in a `trailers` folder or with a trailer suffix are neither imported, nor added as library files, nor counted as the largest video.
- [ ] AC-08 (event): When an item is removed with its files, or its folder moved (spec `web-ui-library`), then its trailers go with it.
- [ ] AC-09 (event): When the user starts a trailer download for an item (also one without a file) through the API or the UI, then sofarr downloads it at once, outside the daily budget, following AC-03 and AC-04; if every candidate fails, the item's attempts start over at one and it is retried per AC-05; an own trailer is replaced by the new one; a trailer it did not write makes it refuse with a German message; an item without a folder on disk is refused with a German message.
- [ ] AC-10 (event): When the user opens a movie or series in the UI, then it shows whether it has a trailer, its language and source, or why there is none (no candidate, failed with the reason, waiting for the budget).
- [ ] AC-11 (state): While sofarr runs in shadow mode, it downloads no trailers and writes nothing.
- [ ] AC-12 (option): Where `[trailers] enabled = false` is set, or yt-dlp or ffmpeg cannot be found, sofarr downloads no trailers; the system page (spec `web-ui-shell`) says which applies.
- [ ] AC-13 (always): sofarr starts yt-dlp without a shell, with fixed arguments and a validated YouTube video id; nothing from TMDB or YouTube reaches the command line unchecked.
- [ ] AC-14 (event, CI: docker): When the Docker image is built, then it runs sofarr as UID/GID 1000, `amigo-sofarr healthcheck` reports healthy, and `yt-dlp --version` and `ffmpeg -version` work in it.

## Technical notes

- TMDB: `/movie/{id}/videos` and `/tv/{id}/videos` with `language=de-DE` and
  `include_video_language=de,en,null`; fields `site`, `type`, `official`, `iso_639_1`,
  `published_at`, `key`.
- yt-dlp: format selection `bv*[height<=1080]+ba/b[height<=1080]`, merge to MKV with ffmpeg,
  into a temporary folder under `/config/cache/trailers/`, then moved into `trailers/` (so a disk
  scan never sees a partial download). Timeout and size limit per AC-04.
- Config: `[trailers] enabled` (default true), `languages`, `max_height`, `per_day`, `yt_dlp`
  (path, default `yt-dlp`).
- Dockerfile: runtime `alpine` (pinned) with `yt-dlp`, `ffmpeg`, `ca-certificates`; the musl
  binary stays the same. `compose.example.yml` and `sofarr.md` §7 name the change.
- Table or columns per item: trailer state, language, video key, attempts, next attempt.
- Video id check: YouTube ids are 11 characters of `A-Z a-z 0-9 _ -`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A movie has only an English trailer | The English trailer is downloaded |
| A series' TMDB entry lists no videos | No candidate: retried after 7 days, at most three times (AC-05) |
| A trailer folder exists but is empty | No trailer: the item is eligible |
| The owner put an own `Film-trailer.mkv` next to the movie | The item has a trailer; sofarr does nothing (AC-06) |
| yt-dlp breaks because YouTube changed something | Every attempt fails with the reason; items wait per AC-05 until an updated image ships |
| The disk is nearly full | No trailer downloads on that file system (AC-15) |
| The movie folder is renamed while its trailer downloads | The download finishes in the temporary folder; it is moved into the current folder or discarded if the folder is gone |
| A series without a TMDB id | Not eligible until the metadata refresh finds one |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 20 trailers a day | Fills a 2,500-item library in about four months without drawing attention | The backlog takes long; the owner can raise `per_day` |
| Downloading trailers for private use in the own library is acceptable | The owner's personal media setup; the trailers are promotional material | YouTube's terms restrict downloading; YouTube may block it |
| 10 minutes and 500 MB per trailer | A 1080p trailer is 2–3 minutes and 50–200 MB | Very long trailers are skipped |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `trailers::tests::the_daily_run_downloads_newest_first_within_the_budget` (`crates/sofarr-server/src/trailers/tests.rs`); `trailers::tests::rules::the_queue_takes_imports_first_then_the_newest_added`, `trailers::tests::rules::the_daily_run_comes_a_day_after_the_last` (`crates/sofarr-server/src/trailers/tests/rules.rs`); `trailers::tests::items_carry_video_tmdb_id_and_trailer_state` (`crates/sofarr-db/src/trailers.rs`) | built | 174389d, 6699567 |
| AC-02 | `trailers::tests::an_import_puts_an_item_first_in_line` (`crates/sofarr-server/src/trailers/tests.rs`); `trailers::tests::rules::the_queue_takes_imports_first_then_the_newest_added` (`crates/sofarr-server/src/trailers/tests/rules.rs`); `app::tests::live_pipeline_grabs_and_imports`, `app::tests::live_series_pipeline` (`crates/sofarr-server/src/app/tests.rs`); `trailers::tests::an_import_puts_an_item_first_in_line_and_keeps_its_state` (`crates/sofarr-db/src/trailers.rs`) | built | 174389d, 6699567 |
| AC-03 | `videos::tests::a_german_teaser_beats_an_english_trailer`, `videos::tests::candidates_are_ranked_by_language_type_official_and_date`, `videos::tests::other_types_sites_and_languages_are_no_candidates`, `videos::tests::asks_for_the_videos_of_movies_and_series`, `videos::tests::reads_the_video_list` (`crates/sofarr-metadata/src/videos.rs`); `trailers::tests::rules::file_names_carry_the_folder_and_the_language` (`crates/sofarr-server/src/trailers/tests/rules.rs`); `trailers::tests::the_daily_run_downloads_newest_first_within_the_budget` (`crates/sofarr-server/src/trailers/tests.rs`); `trailers::ytdlp::tests::the_command_line_is_fixed`, `trailers::ytdlp::tests::results_are_one_mkv_within_the_limit` (`crates/sofarr-server/src/trailers/ytdlp.rs`) | built | 0bc23da, 6699567 |
| AC-04 | `trailers::tests::a_failed_candidate_falls_back_and_leaves_nothing_behind` (`crates/sofarr-server/src/trailers/tests.rs`); `trailers::ytdlp::tests::a_download_that_takes_too_long_is_stopped`, `trailers::ytdlp::tests::a_download_that_grows_too_large_is_stopped`, `trailers::ytdlp::tests::a_finished_program_reports_its_exit`, `trailers::ytdlp::tests::results_are_one_mkv_within_the_limit` (`crates/sofarr-server/src/trailers/ytdlp.rs`) | built | 6699567 |
| AC-05 | `trailers::tests::rules::attempts_count_up_and_wait_seven_days`, `trailers::tests::rules::the_queue_takes_imports_first_then_the_newest_added` (`crates/sofarr-server/src/trailers/tests/rules.rs`); `trailers::tests::a_failed_candidate_falls_back_and_leaves_nothing_behind` (`crates/sofarr-server/src/trailers/tests.rs`) | built | 6699567 |
| AC-06 | `trailers::tests::rules::placing_never_overwrites_a_file_sofarr_did_not_write`, `trailers::tests::rules::a_trailer_is_a_video_in_trailers_or_with_a_trailer_suffix` (`crates/sofarr-server/src/trailers/tests/rules.rs`); `trailers::tests::items_that_are_not_eligible_are_skipped`, `trailers::tests::the_daily_run_downloads_newest_first_within_the_budget` (`crates/sofarr-server/src/trailers/tests.rs`); `trailers::tests::api_tests::manual_downloads_are_refused_with_a_german_message` (`crates/sofarr-server/src/trailers/tests/api_tests.rs`). Removing an item with its whole folder comes with spec `web-ui-library` (AC-12), not built yet | built | 6699567 |
| AC-07 | `importer::tests::trailers_are_never_the_video` (`crates/sofarr-server/src/importer.rs`); `trailers::tests::the_disk_scan_never_records_a_trailer` (`crates/sofarr-server/src/trailers/tests.rs`) | built | 6699567 |
| AC-08 | `trailers::tests::removing_an_item_removes_its_trailer_state` (`crates/sofarr-db/src/trailers.rs`); trailers live inside the item folder, so a move or removal of the whole folder carries them. Removing with files and moving a folder are spec `web-ui-library` (AC-12, AC-15), not built yet; `library_files_tests::trailers_go_with_a_moved_folder_and_a_removed_movie` (`crates/sofarr-server/src/library_files_tests.rs`) | built |        |
| AC-09 | `trailers::tests::api_tests::a_manual_download_works_at_once_outside_the_budget`, `a_failed_manual_download_starts_the_attempts_over`, `manual_downloads_are_refused_with_a_german_message`, `shadow_mode_refuses_a_manual_download` (`crates/sofarr-server/src/trailers/tests/api_tests.rs`); `a trailer someone else put there, and why sofarr downloads none (AC-09)`; `downloads at once and shows the trailer with language and source`; `a series trailer that fails reports the reason`; `a refusal shows the German message` (`ui/e2e/trailers.spec.ts`) | built |        |
| AC-10 | `trailers::tests::api_tests::the_item_shows_its_trailer_or_why_there_is_none` (`crates/sofarr-server/src/trailers/tests/api_tests.rs`); `a movie without a file and a series without TMDB id say why there is none`; `every other reason reads in German, with attempts and the next one` (`ui/e2e/trailers.spec.ts`); unit `src/lib/trailers/trailers.test.ts` | built |        |
| AC-11 | `trailers::tests::shadow_mode_and_switched_off_download_nothing` (`crates/sofarr-server/src/trailers/tests.rs`); `trailers::tests::api_tests::shadow_mode_refuses_a_manual_download` (`crates/sofarr-server/src/trailers/tests/api_tests.rs`) | built | 6699567 |
| AC-12 | `trailers::tests::shadow_mode_and_switched_off_download_nothing`, `trailers::tests::missing_tools_download_nothing` (`crates/sofarr-server/src/trailers/tests.rs`); `trailers::ytdlp::tests::missing_tools_are_reported` (`crates/sofarr-server/src/trailers/ytdlp.rs`); `config::tests::trailer_settings_have_defaults_and_are_checked` (`crates/sofarr-server/src/config.rs`); `says why trailers are not downloaded, or that they are` (`ui/e2e/trailers.spec.ts`) | built |        |
| AC-13 | `trailers::ytdlp::tests::video_ids_are_checked`, `trailers::ytdlp::tests::the_command_line_is_fixed` (`crates/sofarr-server/src/trailers/ytdlp.rs`); `trailers::tests::a_failed_candidate_falls_back_and_leaves_nothing_behind` (`crates/sofarr-server/src/trailers/tests.rs`) | built | 6699567 |
| AC-14 | CI: docker – job `docker` (`.github/workflows/ci.yml`) checks UID/GID 1000:1000, `amigo-sofarr healthcheck`, `yt-dlp --version`, `ffmpeg -version`; local `docker build` and the same checks passed 2026-11-12 | built | f00ba74, a446b5c |
| AC-15 | `trailers::tests::too_little_free_space_downloads_nothing_and_counts_nothing` (`crates/sofarr-server/src/trailers/tests.rs`); `trailers::tests::api_tests::the_item_shows_its_trailer_or_why_there_is_none` (`crates/sofarr-server/src/trailers/tests/api_tests.rs`) | built | 6699567 |

## Decisions

- 2026-11-12: Trailers for items with a file, newest added first, within a daily budget; series get one trailer too (owner).
- 2026-11-12: The runtime image becomes Alpine with yt-dlp and ffmpeg (owner; D18).
- 2026-11-12: Candidates are ranked in `sofarr-metadata` (pure function next to the TMDB video parsing); TMDB writes "no language" as null, "" or "xx"; the request sends `language=<first configured language>` and `include_video_language=<configured languages>,null`. Unknown language codes show as the upper-case code.
- 2026-11-12: yt-dlp runs via `std::process::Command` in a blocking task that polls every 200 ms (no tokio `process` feature, no new dependency); fixed arguments: `--ignore-config --no-playlist --no-progress --no-cache-dir --no-mtime --socket-timeout 30 --format bv*[height<=H]+ba/b[height<=H] --merge-output-format mkv --remux-video mkv --max-filesize 500M --output trailer.%(ext)s --paths <work> -- <URL>`; the size limit (500 MiB) also watches the output folder while yt-dlp runs. On a timeout yt-dlp is killed; an ffmpeg child it started is not (no process groups without `unsafe` or a new crate), the work folder is removed anyway.
- 2026-11-12: Timeout and size limit are tested with the test binary re-invoking itself as the fake program (`trailers::ytdlp::tests::fake_program`, selected with `--exact` and an environment variable): runs on Windows and Linux without extra tools. The real command line is tested by its argument list; the trailer logic uses an in-process fake `Downloader`.
- 2026-11-12: Work folder `<database folder>/cache/trailers/<kind>-<id>-<ms>/` (`/config/cache/trailers` in the container), removed after every candidate and cleaned at start. Across file systems the trailer is copied under a hidden `.<name>.partial` next to the target and renamed. Before the move the item folder is read again from the library (moved folder); a folder that is gone counts as a failed candidate.
- 2026-11-12: The daily run comes 24 h after the last one (setting `trailers_ran_at`, so a restart does not run it again; the first run is right after the start); `per_day = 0` switches the daily run off; a TMDB outage stops the run without counting anything, a TMDB 404 is "no candidate". The system page gets the job `trailers` ("Trailer") and `trailers: {active, message}` (switched off, shadow mode, no `[tmdb]`, tools missing or not yet checked).
- 2026-11-12: "Becomes eligible by an import" = the import records the first file of a movie or series; stored as `first_in_line` in the trailer state, cleared by the next try. Low free space sets state `low_space` without an attempt; `skipFreeSpaceCheckWhenImporting` is honoured like at the import. An item whose sofarr trailer was deleted by the user is eligible again.
- 2026-11-12: The manual download answers when it is finished with the new `TrailerInfo` (200 also when every candidate failed); refusals are 409 with a German message, TMDB unreachable 502, a running download of the same item 409. ffmpeg is looked up on `PATH` (no config key); `yt-dlp --version` and `ffmpeg -version` are checked at start, before each run and before the first manual download.
- 2026-11-12: Runtime image `alpine:3.24` pinned by digest with `ca-certificates`, `ffmpeg`, `yt-dlp` from that branch (package versions are not pinned: Alpine drops old package versions, which would break the build); the CI `docker` job also checks UID/GID, the healthcheck, `yt-dlp --version` and `ffmpeg -version`.
- 2026-11-12: Trailers need nothing of their own when an item is removed or its folder moved: both actions of spec `web-ui-library` take the whole item folder, `trailers/` included, and the trailer state goes with the item (`ON DELETE CASCADE`); the test was green at once (AC-08).
- 2026-11-12: The trailer button is disabled while the API names a reason (`blocked`), which it points to as its description; it reads "Trailer neu laden" when sofarr's own trailer exists; a loaded trailer, a failure and a refusal become notices (AC-09).
- 2026-11-12: The system page shows "Trailer: werden heruntergeladen" or "keine Downloads – {reason}" in its overview box (AC-12).
- 2026-11-12: Trailer states that need TMDB or yt-dlp are tested in the browser with simulated API answers; the server behavior behind them is covered by the Rust tests.
