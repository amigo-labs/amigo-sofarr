---
status: implemented
created: 2026-11-19
size: M
design: [sofarr.md §2, sofarr.md §5, sofarr.md §7, sofarr.md §8, sofarr.md §9 D6, sofarr.md §9 D12]
media: both
oracle: GET /api/v3/manualimport?folder=… (assignment and quality of downloads, `compare-manual-import`); naming by GET /api/v3/rename (`compare-rename`)
---

# Download import

## Context

Once a release is handed to nzbget (spec `rss-sync`), sofarr follows the download and brings the
finished files into the library — Radarr's and Sonarr's "completed download handling", which the
owner's setup has on (`docs/parity.md`). The download client and the library share one `/data`
mount (§7), so a finished download is moved into the movie or series folder, renamed by the
naming settings imported from Radarr/Sonarr (spec `library-import`) and recorded as the library
file. Jellyfin picks the new file up itself (D12).

This spec was extracted from code built in phases 4 and 5, before the spec workflow; it
describes what the code does today. Failed and vanished downloads and an unreachable download
client are spec `fail-handling`.

## Scope

### In scope

- The scheduled download check and the CLI `amigo-sofarr check-downloads`
- Following an active grab through the client's queue and history
- Importing a finished movie download: choosing the video, naming, moving, replacing the old file
- Importing a finished series download: assigning video files to the grabbed episodes, naming
  them into season folders
- Waiting for missing episode titles (`episodeTitleRequired`)
- Cleaning up the download after the import ("remove completed")

### Out of scope

- Failed downloads, broken NZBs, vanished downloads, an unreachable client, marking a grab as failed — because spec `fail-handling` owns them (AC-01, AC-02, AC-10, AC-11, AC-15 there)
- Hardlinks or copies instead of moving (`copyUsingHardlinks`) — because Usenet downloads are not seeded, so moving loses nothing (`docs/parity.md`, deliberately different)
- Importing extra files (`importExtraFiles`) — because the owner's setup has it off; subtitles come from spec `subtitles`
- Reading the media info of the imported file — because spec `mediainfo` owns it (AC-01, AC-06 there)
- Subtitles and trailers after an import — because specs `subtitles` and `trailers` (AC-02 there) own them
- Manual import of a folder the user picks — because there is no such feature; `compare-manual-import` only compares the assignment with Radarr
- A refresh of Jellyfin after the import — because Jellyfin watches the library itself (D12)

## Acceptance criteria

- [x] AC-01 (option): Where `[download] check_interval_minutes` is set (default 2), the server checks the download client at that interval while sofarr runs in live mode and `[nzbget]` is configured; 0, shadow mode or no `[nzbget]` turns the scheduled check off.
- [x] AC-02 (event): When a check finds the download of an active grab queued or paused, then the grab stays as it is; when it finds it downloading, then the grab is marked as downloading.
- [x] AC-03 (event): When a check finds the download of an active movie grab finished successfully, then sofarr translates the client's folder through `[[download.path_map]]`, moves the largest video file of the download that is neither a sample nor a trailer into the movie's folder — named by Radarr's movie format where `renameMovies` is on —, records it as the movie's file and marks the grab as imported.
- [x] AC-04 (option): Where Radarr's `renameMovies` is off, the imported file keeps the name it had in the download, with its extension in lower case.
- [x] AC-05 (always): An import moves only the video: other files of the download (`.nfo`, subtitles, samples) stay where they are, and the video is no longer in the download folder afterwards.
- [x] AC-06 (always): The recorded file of an imported movie carries its size, the grab's quality, release group and revision from the release name, the release name as scene name, and the custom format score the release name gets under the movie's quality profile.
- [x] AC-07 (event): When an import replaces an existing file of the movie or episode, then the old file goes to the recycle bin set in Radarr's or Sonarr's media management (`recycleBin`), or is deleted where none is set.
- [x] AC-08 (event): When a check finds the download of an active series grab finished successfully, then sofarr assigns each video file of the download to the grabbed episodes its file name names (where the grab holds a single episode, a file whose name names no episode belongs to that episode), moves each into the season folder named by Sonarr's season folder format (where the series uses season folders), named by Sonarr's episode format for the series type (standard, daily, anime), records the file for its episodes with a `/`-separated path relative to the series folder and marks the grab as imported; a video that belongs to no grabbed episode stays in the download.
- [x] AC-09 (state): While Sonarr's `episodeTitleRequired` holds back an import — `always`, or `bulkSeasonReleases` for a season pack — because the episode format uses the episode title and an episode of the download has none yet (empty or `TBA`) and aired less than 48 hours ago, sofarr imports nothing of that download and keeps the grab completed with a German note that it waits for episode titles; a later check imports it once the title is known or the 48 hours are over.
- [x] AC-10 (error): If an import fails (no video in the download, too little free space, the movie or series no longer in the library or without a folder, the client reports no folder, a file that cannot be moved), then the library stays unchanged, the grab stays completed with a German reason, and every later check tries the import again.
- [x] AC-11 (option): Where Radarr's or Sonarr's `minimumFreeSpaceWhenImporting` is above 0 and `skipFreeSpaceCheckWhenImporting` is off, an import that would leave less free space (in MB) on the target's file system is refused by AC-10 before anything is moved.
- [x] AC-12 (option): Where `[download] remove_completed` is on (the default), an imported download is removed from the client's history and its folder is deleted unless a video file is still in it; where it is off, both stay.

## Technical notes

- The client's folder of a finished download is nzbget's `FinalDir`, else its `DestDir`;
  `[[download.path_map]]` replaces the first matching `remote` prefix (whole path segments only)
  by `local`.
- Video extensions: `mkv mp4 m4v avi m2ts ts wmv mov mpg mpeg webm` (case-insensitive). A sample
  has the word `sample` in its name (split at `.`, `-`, `_`, space); trailers follow spec
  `trailers` (AC-07 there).
- Moving uses a rename; across file systems the file is copied and the source removed.
- Free space is measured on the nearest existing ancestor of the target folder (`fs4`).
- The download check imports series grabs first, then movie grabs, in one job run; it runs under
  the same jobs lock as the RSS sync.
- CLI `amigo-sofarr check-downloads` runs one check (also in shadow mode) and prints
  `Importiert: <n>` (files imported).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| The download holds a sample larger than the movie | The sample is never chosen (AC-03) |
| A movie without a folder in the library | Import refused (AC-10) |
| A season pack whose download also holds an episode of another season | That file stays; because a video is left, the download folder is kept even with "remove completed" (AC-08, AC-12) |
| A season pack covering an episode that has not aired yet | Its file is imported with the others: the pack's grab holds the whole season |
| The new file has the same name as the old one | The old file is not recycled; the new one takes its place |
| An episode without a known air date and without a title | No wait: there is no 48-hour window to wait for (AC-09) |
| The download disappears from the client while the import is retried | Spec `fail-handling` AC-10 (vanished after 24 hours) |
| A grab is in shadow mode | Not followed: shadow grabs are not active |
| nzbget reports the download finished with a warning or failure status | Spec `fail-handling` AC-01, AC-02 |
| Removing the download from the client's history fails | Only logged; the import stands |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The largest non-sample video is the movie | Usenet movie releases hold one main video plus samples and extras (`docs/parity.md`, "Import") | A release with several parts imports only the largest |
| Episodes are assigned by file name, the release name only as a fallback | Season packs name each file; single-episode downloads may be obfuscated (`docs/parity.md`, "Serien: Episoden-Import") | An obfuscated multi-file pack imports nothing (AC-10) |
| Usenet downloads are moved, never hardlinked | Nothing seeds them, so a second copy only costs space (`docs/parity.md`, deliberately different) | None known |
| A folder with no video left after the import may be deleted | What remains are extras the setup does not import (`importExtraFiles` off) | Extras the user wanted are lost with the folder |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `rss_tests::rss_and_download_jobs_follow_the_config` (`crates/sofarr-server/src/rss_tests.rs`); `config::tests::maps_download_paths` (`crates/sofarr-server/src/config.rs`) | verified | 0db7cad |
| AC-02 | `download_import_tests::queued_paused_and_downloading_downloads` (`crates/sofarr-server/src/download_import_tests.rs`); `tests::maps_queue_and_history` (`crates/sofarr-download/src/lib.rs`) | verified | 0db7cad |
| AC-03 | `app::tests::live_pipeline_grabs_and_imports` (`crates/sofarr-server/src/app/tests.rs`); `download_import_tests::remove_completed_clears_the_history_and_the_leftover_folder` (`crates/sofarr-server/src/download_import_tests.rs`); `importer::tests::imports_and_replaces_old_file`, `importer::tests::trailers_are_never_the_video` (`crates/sofarr-server/src/importer.rs`); `config::tests::maps_download_paths` (`crates/sofarr-server/src/config.rs`) | verified | 0db7cad |
| AC-04 | `download_import_tests::without_renaming_the_video_keeps_its_name_and_the_extras_stay` (`crates/sofarr-server/src/download_import_tests.rs`) | verified | 0db7cad |
| AC-05 | `download_import_tests::without_renaming_the_video_keeps_its_name_and_the_extras_stay` (`crates/sofarr-server/src/download_import_tests.rs`); `importer::tests::imports_and_replaces_old_file` (`crates/sofarr-server/src/importer.rs`) | verified | 0db7cad |
| AC-06 | `download_import_tests::without_renaming_the_video_keeps_its_name_and_the_extras_stay`, `download_import_tests::a_proper_or_real_release_records_its_revision` (`crates/sofarr-server/src/download_import_tests.rs`); `app::tests::live_pipeline_grabs_and_imports` (`crates/sofarr-server/src/app/tests.rs`) | verified | 0db7cad, a7886fb |
| AC-07 | `download_import_tests::an_upgrade_recycles_or_deletes_the_old_file`, `download_import_tests::a_season_pack_imports_each_grabbed_episode`, `download_import_tests::an_episode_upgrade_recycles_the_old_file` (`crates/sofarr-server/src/download_import_tests.rs`); `importer::tests::imports_and_replaces_old_file` (`crates/sofarr-server/src/importer.rs`) | verified | 0db7cad, 8987674 |
| AC-08 | `download_import_tests::a_season_pack_imports_each_grabbed_episode`, `download_import_tests::an_import_waits_for_a_missing_episode_title` (`crates/sofarr-server/src/download_import_tests.rs`); `app::tests::live_series_pipeline` (`crates/sofarr-server/src/app/tests.rs`); `series::tests::names_episodes` (`crates/sofarr-server/src/series/tests.rs`); `importer::tests::relative_paths_use_slashes` (`crates/sofarr-server/src/importer.rs`) | verified | 0db7cad |
| AC-09 | `download_import_tests::an_import_waits_for_a_missing_episode_title` (`crates/sofarr-server/src/download_import_tests.rs`); `series::tests::waits_for_episode_titles` (`crates/sofarr-server/src/series/tests.rs`) | verified | 0db7cad |
| AC-10 | `download_import_tests::a_failed_import_stays_completed_and_is_tried_again` (`crates/sofarr-server/src/download_import_tests.rs`); `importer::tests::free_space_check_refuses` (`crates/sofarr-server/src/importer.rs`) | verified | 0db7cad |
| AC-11 | `importer::tests::free_space_check_refuses`, `importer::tests::reads_settings` (`crates/sofarr-server/src/importer.rs`); `download_import_tests::the_minimum_free_space_refuses_an_import_unless_the_check_is_skipped`, `download_import_tests::the_minimum_free_space_refuses_an_episode_import_unless_the_check_is_skipped` (`crates/sofarr-server/src/download_import_tests.rs`) | verified | a7886fb, 8987674 |
| AC-12 | `download_import_tests::remove_completed_clears_the_history_and_the_leftover_folder`, `download_import_tests::without_renaming_the_video_keeps_its_name_and_the_extras_stay`, `download_import_tests::a_season_pack_imports_each_grabbed_episode` (`crates/sofarr-server/src/download_import_tests.rs`); `importer::tests::removes_leftovers_without_videos` (`crates/sofarr-server/src/importer.rs`) | verified | 0db7cad |

## Summary

> Completed: 2026-11-19

- The download check follows active grabs through nzbget's queue and history at `[download] check_interval_minutes` in live mode.
- Finished movie downloads move their largest non-sample video into the movie folder, named by Radarr's format, and record it with quality, revision, release group, scene name and score.
- Series downloads assign videos to the grabbed episodes, name them into season folders by Sonarr's formats, and wait for missing episode titles.
- Replaced files go to the recycle bin; failed imports stay completed with a German reason and are retried; free space and "remove completed" follow the settings.

## Decisions

- 2026-11-19: Extracted from the existing code (phases 4 and 5) with `spec-extract`; tests for ACs without one were added in 0db7cad.
