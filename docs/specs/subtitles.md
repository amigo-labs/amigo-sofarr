---
status: implemented
created: 2026-11-12
size: L
design: [sofarr.md §6, sofarr.md §8, sofarr.md §9 D12, sofarr.md §9 D17, sofarr.md §9 D25]
media: both
oracle: none
---

# Subtitles

## Context

Many releases in the owner's library are German with an original audio track ("German DL"), but
not all carry subtitles, and English-only releases have no German ones. Jellyfin shows external
subtitles that sit next to the video under its naming rules (`Film.de.srt`, `Film.de.forced.srt`,
`Film.en.sdh.srt`), but its OpenSubtitles plugin is unreliable (limits, no control). D17 decides
that sofarr fetches subtitles itself through the official OpenSubtitles.com REST API. Bazarr is not
used, and its code is off limits (AGENTS.md, Clean Room). Spec `mediainfo` tells which audio and
subtitle languages a file already contains.

Terms used below:

- **Wanted languages** – `[subtitles] languages`, default `["de", "en"]` (ISO 639-1).
- **Covered language** – a language the video file already has as an audio track, as an embedded
  non-forced subtitle track, or as an external non-forced subtitle file next to it. Languages are
  compared after mapping the file's ISO 639-2 codes (spec `mediainfo`) to ISO 639-1; `und` and
  codes without an ISO 639-1 equivalent cover nothing. An external file next to the video
  counts by Jellyfin's naming rules: `<video name>.<language>[.<flags>].<extension>`, where the
  language is an ISO 639-1 or 639-2 code or an English or German language name and the flags
  include `forced`/`foreign` and `sdh`/`cc`/`hi`; a file without a language part, and files in
  subfolders, cover nothing.
- **Needed subtitle** – for a monitored movie or episode with a file: a normal subtitle for each
  wanted language that is not covered; and a forced subtitle in the language of the first audio
  track, when that language is a wanted language and the file has no forced subtitle in it
  (embedded or external) — the audio track itself does not cover a forced subtitle. A file whose
  first audio language is unknown needs no forced subtitle.
- **Candidate** – a subtitle the provider offers for the file and language.
- **Match quality** – from best to worst: the provider's file hash matches the video; then the
  number of release attributes that match the video's release (group, source, resolution,
  edition; more is better); then only the movie or episode matches. A candidate whose release
  name, parsed by sofarr, names another season or episode than the video is discarded. A hearing-impaired (SDH)
  candidate is taken only when no normal candidate exists at all.
- **Own subtitle** – an external subtitle file that sofarr wrote; every other subtitle file is
  someone else's.
- **Quota** – the provider's download limit per day for the account.

## Scope

### In scope

- Finding needed subtitles for movies and episodes, after an import and on a schedule
- Searching, ranking and downloading from OpenSubtitles.com within the quota
- Storing them next to the video by Jellyfin's naming rules, in UTF-8
- Keeping own subtitles with their video when the video is renamed, moved, upgraded or removed
- API and UI: subtitles per file, a list of needed subtitles, a manual search per file

### Out of scope

- Providers other than OpenSubtitles.com — because one reliable provider covers German and English well (D17)
- Upgrading a subtitle to a better match later — because a found subtitle of the right movie or episode serves its purpose; the owner can delete it to search again
- Timing correction (sync) or translation of subtitles — because they need audio analysis or a language model
- Subtitles for unmonitored items — because the owner only cares about monitored ones in day-to-day use; a manual search covers others
- Extracting embedded subtitles into files — because Jellyfin reads embedded tracks itself

## Acceptance criteria

- [x] AC-01 (event): When sofarr imports a video file of a monitored movie or episode, then it determines the file's needed subtitles and searches for them.
- [x] AC-02 (event): When the subtitle job runs (every `interval_hours`), then sofarr searches the needed subtitles of the library that are due (AC-05), newest imported files first, as far as the quota allows.
- [x] AC-03 (always): sofarr never searches a normal subtitle for a covered language, and a file never has more than one own subtitle per language and kind (normal or forced) at a time.
- [x] AC-04 (event): When the provider offers candidates for a needed subtitle, then sofarr downloads the candidate of the best match quality as SRT, stores it next to the video as `<video name>.<language>[.forced][.sdh].srt` in UTF-8, and records it as an own subtitle.
- [x] AC-05 (state): While candidates exist but none was downloaded (quota used up, shadow mode), the needed subtitle stays due for the next run. While no candidate of a needed subtitle exists, it is due again one day after its last search during the first 7 days after the import, then one week after its last search; a forced subtitle the provider does not have keeps being searched on that schedule but is not listed as needed in AC-11.
- [x] AC-06 (always): sofarr never overwrites or deletes a subtitle file that is not its own; if the target name is taken by another subtitle, that language counts as covered.
- [x] AC-07 (error): If the quota is used up, then sofarr stops downloading until the provider's reset time and continues there, newest files first; searches that need no download continue.
- [x] AC-08 (error): If the provider cannot be reached, answers a server error, or refuses the login or the API key (a failing download of a single candidate is AC-14; a used-up quota is AC-07), then sofarr stops the run — subtitles already stored in it stay — logs the reason (without credentials), shows it on the system page (spec `web-ui-shell`), and tries again at the next run.
- [x] AC-09 (event): When a video is renamed or its folder moved (spec `web-ui-library`), then its own subtitles get the matching new names and place.
- [x] AC-10 (event): When a video is replaced by an upgrade or removed, then its own subtitles go with it (to the recycle bin or deleted like the video), and the new video gets its needed subtitles determined afresh.
- [x] AC-11 (event): When the user opens the subtitle overview, then it lists every needed subtitle with item, language, kind, last search and next search, and the subtitles found in the last 7 days; per file the user can start a search at once.
- [x] AC-12 (event): When the user opens a movie or episode file in the UI, then it shows the file's subtitles — embedded, external, own — with language and kind.
- [x] AC-13 (state): While sofarr runs in shadow mode, it searches but downloads and writes nothing, and the overview shows what it would have downloaded.
- [x] AC-14 (error): If downloading a single candidate fails (e.g. it was removed), or the downloaded subtitle is empty, not text, or cannot be converted to UTF-8, then sofarr discards it, keeps no file of it, does not record it as an own subtitle, and tries the next candidate.
- [x] AC-15 (option): Where `[subtitles] enabled = false` is set or `[opensubtitles]` is missing, sofarr searches and downloads no subtitles; the overview says why.

## Technical notes

- Config: `[subtitles] enabled` (default true when `[opensubtitles]` is set), `languages`
  (ISO 639-1), `interval_hours` (default 12); `[opensubtitles] api_key`, `username`, `password`
  (secrets, masked on the system page).
- OpenSubtitles.com REST API (public documentation): `POST /login` → token, `GET /subtitles`
  with `moviehash`, `imdb_id`/`tmdb_id` or `parent_imdb_id` + `season_number` + `episode_number`,
  `languages`, `foreign_parts_only`, `hearing_impaired`; `POST /download` → a link, `remaining`
  and `reset_time` (AC-07). The required `Api-Key` and a `User-Agent` naming sofarr go with
  every request.
- Movie hash: OpenSubtitles' published algorithm (file size plus checksums of the first and last
  64 KiB), tested against the example values in its documentation where it gives some, otherwise
  against synthetic files whose hash follows from the documented algorithm by hand.
- Release-name match uses `sofarr-parser` on the candidate's release name and on the video's
  scene name or file name. A video that cannot be read for the hash is searched without it.
- Episodes: `parent_imdb_id`, else `parent_tmdb_id` (the series' TMDB id, D26), else the series
  title with `season_number` and `episode_number`.
- Table `subtitles` (file, language, kind, path, provider id, match quality, created) and
  per needed subtitle the last and next search time.
- A new IO crate `sofarr-subtitles` for the provider client, used by `sofarr-server` (D25).
- Language codes in file names are ISO 639-1 (`de`, `en`), as in Jellyfin's examples.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A German DL release with German and English audio | de and en are covered; only a German forced subtitle may be needed |
| An English release with English subtitles embedded | en covered; a German subtitle is needed |
| A file with an external `Film.de.srt` from elsewhere | de covered; the file is never touched (AC-06) |
| A season pack of 10 episodes imported at once with 5 downloads left today | The newest 5 needed subtitles now, the rest after the reset (AC-07) |
| Only an SDH subtitle exists for German | It is downloaded as `.de.sdh.srt` |
| The provider returns a subtitle for the wrong episode (mislabelled) | Ranked by match quality; a hash match wins; a wrong episode without any match is not offered |
| The video's media info is unknown (unsupported container) | Only external files count as covered; audio and embedded tracks are unknown, so subtitles may be fetched although the file has them |
| The owner deletes an own subtitle | It is needed again and searched at the next run |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The language of the first audio track is the language for forced subtitles | Releases put the main language first; Jellyfin plays it by default | A file whose first track is the original language gets forced subtitles in that language |
| Retry daily for 7 days, then weekly | New releases get subtitles within days; older titles rarely gain new ones | A subtitle published weeks later arrives up to a week late |
| A free OpenSubtitles account is enough at first | The owner imports a few files a day | A large initial library backlog takes weeks within a small quota |

## Summary

> Completed: 2026-11-12

- sofarr determines the needed subtitles of every monitored movie and episode file (wanted languages not covered by audio, embedded tracks or external files; a forced subtitle in the first audio language) and searches them after each import and every `interval_hours`, newest files first.
- The new IO crate `sofarr-subtitles` talks to OpenSubtitles.com (login, search, download, quota), computes the movie hash, ranks candidates by hash, release attributes and title, and converts downloads to UTF-8 SRT named by Jellyfin's rules.
- Own subtitles are recorded, never more than one per language and kind; foreign subtitle files are never touched; own subtitles follow renames and folder moves and leave with upgraded or removed videos.
- Quota, provider failures, broken candidates, shadow mode and switched-off subtitles are handled as specified, with retries daily for 7 days, then weekly; failures show on the system page.
- API and UI: the "Untertitel" overview with needs, recent finds and a search per file, and the subtitles of each movie and episode file.

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `an_episode_import_replaces_own_subtitles_and_searches_the_new_file` (crates/sofarr-server/src/subtitles/api_tests.rs); `a_movie_upgrade_recycles_own_subtitles_and_queues_the_new_file`, `the_running_schedule_searches_an_imported_movie_at_once` (`crates/sofarr-server/src/subtitles/import_tests.rs`); `unmonitored_movies_are_skipped_by_the_schedule_and_after_an_import`, `episodes_of_unmonitored_series_or_unmonitored_episodes_are_skipped` (`crates/sofarr-server/src/subtitles/schedule_tests.rs`) | verified | 69f801e |
| AC-02 | `a_run_stores_the_best_candidate_in_utf8_next_to_the_video`, `a_used_up_quota_waits_for_the_reset_newest_first` (crates/sofarr-server/src/subtitles/tests.rs); `a_failing_provider_shows_on_the_system_page` (crates/sofarr-server/src/subtitles/api_tests.rs, schedule); `unmonitored_movies_are_skipped_by_the_schedule_and_after_an_import`, `episodes_of_unmonitored_series_or_unmonitored_episodes_are_skipped` (`crates/sofarr-server/src/subtitles/schedule_tests.rs`) | verified | 69f801e |
| AC-03 | `covered_languages_are_never_searched`, `a_deleted_own_subtitle_is_needed_again` (crates/sofarr-server/src/subtitles/tests.rs); `one_own_subtitle_per_file_language_and_kind` (crates/sofarr-db/src/subtitles/tests.rs); needs tests in crates/sofarr-server/src/subtitles/needs.rs | verified | 004cf07 |
| AC-04 | `a_run_stores_the_best_candidate_in_utf8_next_to_the_video` (crates/sofarr-server/src/subtitles/tests.rs); `rank::tests::*`, `naming::tests::names_follow_jellyfin`, `text::tests::*`, `hash::tests::*` (crates/sofarr-subtitles) | verified | 9447653 |
| AC-05 | `needs_without_candidates_are_retried_daily_then_weekly`, `a_used_up_quota_waits_for_the_reset_newest_first`, `shadow_mode_searches_but_writes_nothing` (crates/sofarr-server/src/subtitles/tests.rs); `overview_lists_needs_and_recent_subtitles_and_file_subtitles` (crates/sofarr-server/src/subtitles/api_tests.rs, forced not listed); `needs_keep_their_schedule_while_they_stay` (crates/sofarr-db/src/subtitles/tests.rs) | verified | 004cf07 |
| AC-06 | `foreign_subtitles_cover_their_language_and_stay_untouched` (crates/sofarr-server/src/subtitles/tests.rs); `writing_never_replaces_a_file` (crates/sofarr-server/src/subtitles/files.rs); foreign file kept in `an_episode_import_replaces_own_subtitles_and_searches_the_new_file` (crates/sofarr-server/src/subtitles/api_tests.rs) | verified | 69f801e |
| AC-07 | `a_used_up_quota_waits_for_the_reset_newest_first`, `a_quota_refusal_stops_downloads_until_the_reset` (crates/sofarr-server/src/subtitles/tests.rs); `quota_refused_login_and_broken_candidates_are_told_apart` (crates/sofarr-subtitles/src/client_tests.rs); `the_schedule_waits_for_the_quota_reset_also_without_an_interval`, `the_running_schedule_continues_at_the_quota_reset` (`crates/sofarr-server/src/subtitles/schedule_tests.rs`) | verified | 9447653 |
| AC-08 | `provider_failures_stop_the_run_and_keep_what_was_stored` (crates/sofarr-server/src/subtitles/tests.rs); `a_failing_provider_shows_on_the_system_page` (crates/sofarr-server/src/subtitles/api_tests.rs); `search_failures_end_the_run` (crates/sofarr-subtitles/src/client_tests.rs) | verified | 9447653 |
| AC-09 | `own_subtitles_follow_a_renamed_and_moved_video` (`crates/sofarr-subtitles/src/files.rs`) with `Db::rename_own_subtitle`; `library_files_tests::own_subtitles_follow_a_rename_and_a_folder_move` (`crates/sofarr-server/src/library_files_tests.rs`); `own_subtitles_follow_a_series_rename`, `a_failing_subtitle_follow_does_not_stop_a_rename` (`crates/sofarr-server/src/subtitles/rename_tests.rs`) | verified | c4a9c69 |
| AC-10 | `an_episode_import_replaces_own_subtitles_and_searches_the_new_file` (`crates/sofarr-server/src/subtitles/api_tests.rs`); `own_subtitles_leave_with_their_video` (`crates/sofarr-server/src/subtitles/files.rs`); `library_files_tests::own_subtitles_leave_with_a_removed_movie` (`crates/sofarr-server/src/library_files_tests.rs`); `a_movie_upgrade_recycles_own_subtitles_and_queues_the_new_file` (`crates/sofarr-server/src/subtitles/import_tests.rs`) | verified | 69f801e |
| AC-11 | `overview_lists_needs_and_recent_subtitles_and_file_subtitles`, `overview_shows_shadow_candidates_and_needs_with_schedule` (`crates/sofarr-server/src/subtitles/api_tests.rs`); `lists needed subtitles with item, language, kind, last and next search`; `shows the subtitles found in the last 7 days`; `a search for one file at once loads what the provider has now` (`ui/e2e/subtitles.spec.ts`); axe and 360 px in `ui/e2e/extras-a11y.spec.ts`; unit `src/lib/subtitles/subtitles.test.ts`; `the_overview_lists_needs_of_monitored_items_only` (`crates/sofarr-server/src/subtitles/api_tests.rs`) | verified | 69f801e |
| AC-12 | `overview_lists_needs_and_recent_subtitles_and_file_subtitles` (`crates/sofarr-server/src/subtitles/api_tests.rs`); `a movie file shows its own and foreign subtitle files and what is still needed`; `embedded tracks show language, kind and format`; `an episode file shows its subtitles when opened` (`ui/e2e/subtitles.spec.ts`); unit `src/lib/subtitles/subtitles.test.ts` | verified | 69f801e |
| AC-13 | `shadow_mode_searches_but_writes_nothing` (crates/sofarr-server/src/subtitles/tests.rs); `overview_shows_shadow_candidates_and_needs_with_schedule` (crates/sofarr-server/src/subtitles/api_tests.rs) | verified | 69f801e |
| AC-14 | `broken_candidates_are_discarded_and_the_next_one_tried` (crates/sofarr-server/src/subtitles/tests.rs); `text::tests::unusable_downloads_are_refused` (crates/sofarr-subtitles/src/text.rs) | verified | 9447653 |
| AC-15 | `switched_off_subtitles_search_nothing` (crates/sofarr-server/src/subtitles/tests.rs); `switched_off_subtitles_say_why` (crates/sofarr-server/src/subtitles/api_tests.rs); `subtitle_settings_have_defaults_and_are_checked` (crates/sofarr-server/src/config.rs) | verified | 69f801e |

## Decisions

- 2026-11-12: Wanted languages de and en, fetched only when neither an audio nor a subtitle track nor an external file covers them (owner).
- 2026-11-12: A forced subtitle in addition when the provider has one; SDH only when nothing else fits (owner).
- 2026-11-12: The login answer's `base_url` is not followed; every request goes to `[opensubtitles] url` (default `https://api.opensubtitles.com/api/v1`), which tests point at a fake.
- 2026-11-12: Requests are spaced by `[opensubtitles] pause_ms` (default 250) for the API's request limit; an HTTP 429 stops the run like a server error (AC-08).
- 2026-11-12: A used-up quota is a `/download` answer with HTTP 406, or a failed one with `remaining` ≤ 0; it lasts until `reset_time_utc` (24 hours without one) and is kept in memory only: after a restart the next refused download sets it again.
- 2026-11-12: Only the first result page of a search is ranked; ties of match quality go to the more downloaded candidate.
- 2026-11-12: A candidate whose release names another episode is discarded even when its hash matches (AC-04 wording).
- 2026-11-12: When every candidate of a need fails to download or convert (AC-14), the need is searched again like one without candidates (a day or a week later), not at the next run.
- 2026-11-12: A downloaded subtitle must be text with at least one SRT timing line (`-->`); text that is not UTF-8 or UTF-16 with byte-order mark is read as Windows-1252.
- 2026-11-12: Language parts of external file names: ISO 639-1/639-2 codes, English names (the table of `sofarr-mediainfo`) and common German names; `hi` counts as the hearing-impaired flag, not as Hindi.
- 2026-11-12: Own subtitles are recorded by file name in the video's folder (a folder move changes nothing in the database); their records belong to the file record and go with it. A disk scan that drops a vanished file record leaves the subtitle files on disk alone (sofarr did not remove the video).
- 2026-11-12: The search after an import runs in the subtitle schedule's task, woken by the import, so a slow provider never holds up the download check; it covers the files imported since its last run.
- 2026-11-12: A multi-episode file is searched as its first episode; it counts as monitored when its series and one of its episodes are.
- 2026-11-12: An upgrade sends the own subtitles of the replaced video to the recycle bin (or deletes them) even when the new video gets the same name.
- 2026-11-12: `[subtitles] interval_hours = 0` leaves only the search after imports; `[opensubtitles] username` is masked on the system page like the password and API key.
- 2026-11-12: API: `GET /api/subtitles`, `GET /api/movies/{id}/subtitles`, `GET /api/series/{id}/episodes/{episode_id}/subtitles`, `POST …/subtitles/search` for both; a search while subtitles are off answers 409 with the reason, a provider failure 502 with the provider's German message.
- 2026-11-12: A file whose video is not there or whose folder cannot be read (e.g. an unmounted share) is skipped by the run: no search, no change to its own subtitles or needs.
- 2026-11-12: A rename of the library pages moves the own subtitles of every renamed video (`subtitles::follow_rename`); a folder move needs nothing, because own subtitles are stored by file name next to the video and move with the folder; removing with files moves the whole folder, own subtitles included, and the subtitle records go with the file record (`ON DELETE CASCADE`).
- 2026-11-12: The subtitle API reuses the library's `ItemKind` (movie, episode) instead of a second type of the same name.
- 2026-11-12: The migration is `0010_subtitles.sql` (0007–0009 belong to web-ui-library and imdb-ratings).
- 2026-11-12: There is no episode page: each episode with a file gets an "Untertitel" disclosure in its season, loaded when opened (AC-12).
- 2026-11-12: The overview groups the needed subtitles per file, with "Jetzt suchen" per file; the file search button on item pages reads "Untertitelsuche starten" (AC-11).
- 2026-11-12: Language names come from the browser (`Intl.DisplayNames`, German), "Sprache unbekannt" for `und` or no code.
- 2026-11-12: The overview lists only the needs of monitored items, as the Terms define a needed subtitle (AC-11). Needs that a manual search stored for an unmonitored file, and those of an item unmonitored later, stay stored with their schedule but are neither listed nor searched by the schedule; monitored again, they are listed and searched when due. The file page (AC-12) keeps showing what its own file lacks.
- 2026-11-12: When the own subtitles of a renamed video cannot follow it (e.g. a database error), sofarr logs a warning and goes on: every moved video gets its new path recorded, and the rename answers as done (AC-09).
- 2026-11-12: The tests added after the acceptance check for the import side of AC-01, the monitored filter of AC-01/AC-02, the wake at the quota reset (AC-07), the series rename (AC-09) and the movie upgrade (AC-10) cover behavior that already existed, so they were green at once; each was proven by breaking its code path temporarily (red). Only the overview filter (AC-11) and the rename that goes on after a failed subtitle follow were red first.
