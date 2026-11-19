---
status: todo
created: 2026-11-19
size: M
design: [sofarr.md §2, sofarr.md §5, sofarr.md §7]
media: both
oracle: none
---

# Disk scan

## Context

Files change on disk without sofarr: the owner deletes a movie, renames an episode, copies a
file in by hand, or a volume is not mounted. The disk scan brings the library in line with the
files on disk, following the media-management settings taken over from Radarr and Sonarr
(`autoUnmonitorPreviouslyDownloaded…`, `deleteEmptyFolders`, `createEmpty…Folders`,
`rescanAfterRefresh`). It was built in phases 4 and 5, before the spec workflow existed; this
spec records that behavior (reverse-engineered on 2026-11-19).

There is no oracle: the behavior is an assumption documented in `docs/parity.md`
(„Annahmen ab Phase 3“, row „Festplatten-Scan“).

Terms used below:

- **Disk scan** – one pass over all movies or all series of the library.
- **Recorded file** – the file the library holds for a movie or an episode.
- **Root folder** – the parent folder of a movie's or series' folder (e.g. `/data/media/movies`).
- **Usable root folder** – a root folder that exists and holds at least one entry; an unmounted
  volume usually leaves an empty mount point behind.
- **Video** – a file the import counts as a video: a video extension, not a sample, not a
  trailer (spec `trailers`).
- **Media-management settings** – Radarr's `config/mediamanagement` for movies, Sonarr's for
  series, as imported.

## Scope

### In scope

- Forgetting recorded files that are gone, with or without ending the monitoring
- Taking up videos found in movie and series folders
- Removing empty folders
- Protection against unmounted volumes; shadow mode
- When a scan runs: by command, after a metadata refresh, and its report

### Out of scope

- Reading media info of the scanned files — because spec `mediainfo` owns it
- The scan of a single folder right after adding a title — because spec `discover` (AC-07,
  AC-08) owns it
- Renaming found files to the naming format — because the scan records files as they are; a
  rename is spec `web-ui-library`'s action
- Files outside the movie or series folders (unknown folders in a root folder) — because
  sofarr only looks at titles it knows; a library import is `import-radarr` / `import-sonarr`
- The metadata refresh itself — because spec `series` owns the series refresh, and the movie
  refresh from TMDB is its own area (no spec yet)

## Acceptance criteria

- [ ] AC-01 (event): When a disk scan finds that the recorded file of a movie or an episode no longer exists, then sofarr forgets that file.
- [ ] AC-02 (option): Where the media-management setting `autoUnmonitorPreviouslyDownloadedMovies` (movies) or `autoUnmonitorPreviouslyDownloadedEpisodes` (series) is on, a monitored movie or episode whose recorded file is gone and not replaced in the same scan by a found file is no longer monitored; with the setting off, monitoring stays unchanged.
- [ ] AC-03 (event): When a disk scan finds a video in the folder of a movie without a recorded file, then sofarr records it with quality, revision and release group read from the file name and the custom format score of the movie's quality profile.
- [ ] AC-04 (event): When a disk scan finds a video in a series folder whose name names episodes of that series without a recorded file, then sofarr records the video for each of these episodes; a video that names no such episode is left alone.
- [ ] AC-05 (state): While the root folder of a movie or series is not usable (missing or empty), the scan skips that title and changes nothing for it.
- [ ] AC-06 (option): Where `deleteEmptyFolders` is on, the scan removes a movie folder that holds no file (with its empty sub folders) unless `createEmptyMovieFolders` is on, and removes the empty sub folders of a series folder, the series folder itself too when it ends up empty unless `createEmptySeriesFolders` is on; a folder that holds a file or a symbolic link stays, and links are never followed.
- [ ] AC-07 (state): While sofarr runs in shadow mode, the scan removes no folder, whatever `deleteEmptyFolders` says.
- [ ] AC-08 (option): Where `rescanAfterRefresh` is `always` (also when missing), a metadata refresh is followed by a disk scan of the same media kind; `afterManual` scans only after a refresh started by command, `never` not at all — set per media kind by its media-management settings.
- [ ] AC-09 (event): When a disk scan runs (command `amigo-sofarr rescan`, or after a refresh), then it reports per media kind how many titles it checked and skipped, how many files were missing, how many movies or episodes it stopped monitoring, how many files it found and how many folders it removed; a media kind without any title in the library is not scanned and reports nothing.
- [ ] AC-10 (event): When a scan removes the last empty folder of a usable root folder, then it still checks the titles of that root folder that come after it in the same pass.

## Technical notes

- `rescan::movies` / `rescan::all_series` with `ScanSettings::movies` / `ScanSettings::series`
  (`auto_unmonitor`, `delete_empty_folders`, `keep_empty_root`); `App::scan_settings` switches
  `delete_empty_folders` off in shadow mode. Report: `ScanReport { checked, skipped, missing,
  unmonitored, found, removed_folders, media_read }`; the CLI prints it as
  „Scan Filme: … geprüft, … übersprungen (Stammordner fehlt), … Dateien fehlen, … nicht mehr
  überwacht, … Dateien gefunden, … leere Ordner entfernt“.
- Settings keys: `config_mediamanagement` (Radarr) and `sonarr_config_mediamanagement`
  (Sonarr), fields `autoUnmonitorPreviouslyDownloadedMovies` / `…Episodes`,
  `deleteEmptyFolders`, `createEmptyMovieFolders` / `createEmptySeriesFolders`,
  `rescanAfterRefresh` (`always`, `afterManual`, `never`). A missing flag counts as off.
- Movies: the largest video below the movie folder (`importer::find_videos`) is recorded; relative paths
  are `/`-separated (AGENTS.md Gotchas).
- Series: videos anywhere below the series folder are parsed with `parse_episode` and matched
  like releases (`sofarr_core::series::covered`): season and episode, full season, air date for
  daily series, absolute number for anime. Only episodes without a recorded file (or whose file
  was just found gone) take a found video.
- The scan runs under the jobs lock (RSS sync, imports and the import list sync wait).
- Triggers: `amigo-sofarr rescan`; `refresh-metadata` (movies) and `refresh-series` (series)
  count as started by command; the scheduled refresh (`[library] refresh_interval_hours`, spec
  `series` AC-07) does not.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A recorded file was renamed in its folder | The old record is forgotten, the renamed file recorded; the movie or episode stays monitored (AC-02) |
| The volume is unmounted (empty mount point) | All its titles are skipped; nothing is forgotten or unmonitored (AC-05) |
| The file system cannot tell whether a recorded file exists (permissions, I/O error) | That movie or series is left unchanged; a warning is logged |
| A movie or series folder cannot be read | That title is left unchanged; a warning is logged |
| A series folder with a symbolic link to a folder elsewhere | The link counts as content; nothing behind it is touched (AC-06) |
| A multi-episode file (`S01E02E03`) | Recorded once, for each of its episodes |
| A video in a series folder for an episode that already has a file | Left alone (no replacement of a present file) |
| A movie folder with several videos and no recorded file | The largest one is recorded |
| The scan removes the last folder of a root folder | The root folder was judged usable when the pass reached it first; the titles after it in the same pass are still checked |
| A title without a folder path | Not scanned, not counted |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| An empty root folder means an unmounted volume | Docker bind mounts leave an empty mount point; forgetting a whole library would be far worse than missing a few deletions | A root folder whose titles were all deleted by hand is never cleaned up |
| A replaced file keeps the monitoring | A rename or manual replacement is not "previously downloaded and deleted" | A deleted file replaced by an unrelated video keeps the movie monitored |
| Shadow mode may still forget and record files (database only) | Shadow mode protects `/data`, not sofarr's own database | The shadow library drifts from Radarr/Sonarr's view when they disagree |
| Without the setting, `rescanAfterRefresh` is `always` | The owner's Radarr and Sonarr both use `always` (`docs/parity.md`) | A refresh scans even where nobody asked for it |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `rescan::tests::scans_movie_folders`, `rescan::tests::scans_series_folders`, `rescan::tests::without_auto_unmonitor_missing_files_keep_monitoring` (`crates/sofarr-server/src/rescan.rs`) | built | bbe7c1a |
| AC-02 | `rescan::tests::scans_movie_folders`, `rescan::tests::scans_series_folders`, `rescan::tests::keeps_monitoring_on_rename_and_skips_empty_roots`, `rescan::tests::without_auto_unmonitor_missing_files_keep_monitoring`, `rescan::tests::replaced_and_multi_episode_files_are_taken_up`, `rescan::tests::reads_settings` (`crates/sofarr-server/src/rescan.rs`) | built | bbe7c1a |
| AC-03 | `rescan::tests::scans_movie_folders`, `rescan::tests::keeps_monitoring_on_rename_and_skips_empty_roots` (`crates/sofarr-server/src/rescan.rs`) | built |  |
| AC-04 | `rescan::tests::scans_series_folders`, `rescan::tests::replaced_and_multi_episode_files_are_taken_up` (`crates/sofarr-server/src/rescan.rs`) | built | bbe7c1a |
| AC-05 | `rescan::tests::scans_movie_folders`, `rescan::tests::keeps_monitoring_on_rename_and_skips_empty_roots`, `rescan::tests::an_empty_library_is_not_scanned` (`crates/sofarr-server/src/rescan.rs`) | built | ff9f4f9 |
| AC-06 | `rescan::tests::scans_movie_folders`, `rescan::tests::scans_series_folders`, `rescan::tests::empty_folders_go_unless_empty_folders_are_kept`, `rescan::tests::removes_only_empty_folders`, `rescan::tests::never_follows_symlinks` (Unix only, runs in CI) (`crates/sofarr-server/src/rescan.rs`) | built | bbe7c1a |
| AC-07 | `rescan::tests::shadow_mode_deletes_no_folder` (`crates/sofarr-server/src/rescan.rs`) | built | bbe7c1a |
| AC-08 | `rescan::tests::refresh_scans_as_rescan_after_refresh_says`, `rescan::tests::reads_settings` (`crates/sofarr-server/src/rescan.rs`) | built | bbe7c1a |
| AC-09 | `rescan::tests::scans_movie_folders`, `rescan::tests::scans_series_folders`, `rescan::tests::an_empty_library_is_not_scanned` (`crates/sofarr-server/src/rescan.rs`) | built | ff9f4f9 |
| AC-10 |      | open   |        |

## Decisions

- 2026-11-19: Reverse-engineered from the phase-4/5 code (`spec-extract`); the ACs describe the
  behavior as built. Uncertain points are recorded under Assumptions instead of being asked.
- 2026-11-19 (spec-fix): A root folder is judged once per pass, when the pass first reaches it, because removing the last empty folder of a root folder made the titles after it count as unmounted (found while extracting).
