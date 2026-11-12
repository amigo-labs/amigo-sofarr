---
status: implemented
created: 2026-11-12
size: M
design: [sofarr.md §2, sofarr.md §5, sofarr.md §7, sofarr.md §9 D13, sofarr.md §9 D21]
media: both
oracle: none
---

# Failure handling and search

## Summary

> Completed: 2026-11-12

- Per-item blocklist (GUID and normalized title) filled by release failures, broken NZBs and manual marking; listed page by page and edited through `GET /api/blocklist` and `DELETE /api/blocklist/{id}`.
- Download check classifies nzbget failures as release or local failures, treats downloads missing for 24 hours of successful checks as vanished, and leaves everything unchanged when nzbget is unreachable.
- Re-search after failures per media kind (`[download] redownload_failed_movies` / `redownload_failed_series`, else the imported `autoRedownloadFailed`), each movie, season or episode once per job run, with fallback to the next candidate after a broken NZB.
- Series search through the indexers by TVDB id or by title in the series' numbering (season/episode, air date, absolute number), driven by each indexer's caps.
- Manual search for a movie, series, season or episode (`POST /api/command/search`, `amigo-sofarr search [--series --season --episode]`) and marking an active grab as failed (`POST /api/grabs/{movie|series}/{id}/fail`).

## Context

Today a download that the client reports as failed only sets the grab to `failed`. Nothing
stops the same release from coming back through a second indexer (only its GUID is remembered),
nothing searches for a replacement, and a download that disappears from the client entirely keeps
its grab `queued` forever, which blocks every better release for that movie or episode. There is
also no search for series at all; only movies can be searched (CLI `amigo-sofarr search <movie-id>`).

The owner's setup has `autoRedownloadFailed` off in Radarr and on in Sonarr (`docs/parity.md`).
D13 decides failure handling per app with a blocklist by GUID and title; D21 lets `sofarr.toml`
override the imported setting.

Terms used below:

- **Grab** – a release sofarr decided to take for a movie or for one or more episodes of a series;
  each grab has its own id.
- **Shadow mode** – `[automation] mode = "shadow"`: decisions are recorded as shadow grabs,
  nothing is sent to the download client or moved.
- **Active grab** – a grab sent to the download client and not yet imported or failed
  (queued, downloading, or completed but not imported).
- **Download check** – the periodic job that reads the client's queue and history
  (`[download] check_interval_minutes`); it is successful when both could be read.
- **Release failure** – a final client state caused by the release itself: health or par check
  failed, unpacking failed, or deleted by hand in the client.
- **Local failure** – any other non-success final state (e.g. moving the files failed, not enough
  disk space, a post-processing script failed, rejected by the client as a duplicate of an earlier
  download).
- **Blocklist** – per movie or per series, releases sofarr never takes again for it. Per item,
  not global, because a failure only proves a release unusable for the item it was grabbed for,
  while the same release name can be a valid candidate for another item (e.g. a pack).
- **Normalized title** – the release title lower-cased, with every run of characters that are not
  letters or digits treated as one separator, leading and trailing separators ignored
  (`Film.2020.1080p`, `film 2020 1080p` and `Film_2020-1080p.` are equal).
- **Allowed indexers** – the indexers whose tags allow the movie or series (`[[indexer]] tags`;
  an indexer without tags serves everything).
- **Wanted item** – a monitored movie, or a monitored episode of a monitored series, that has no
  file or whose file is below the profile's cutoff.
- **Broken NZB** – the indexer answers the NZB download with HTTP 404 or 410 or with the Newznab
  error "no such item", or the download client rejects the NZB as invalid.
- **Outage** – the indexer or the download client cannot be reached, times out, or answers with
  any HTTP error other than those of a broken NZB (e.g. 401, 403, 429, 5xx).
- **Season pack** – a release whose name covers one or more whole seasons without episode numbers.
- **Re-search** – the automatic search right after a failure.

## Scope

### In scope

- Blocklist for movies and series, filled by release failures, broken NZBs and manual marking; listed and edited through the API
- Re-search after a failure, per media kind, configurable
- Grabs whose download vanished from the client
- Search for series (series, season, episode) through the indexers
- Manual search for a movie, series, season or episode through the API and the CLI
- Marking an active grab as failed by hand

### Out of scope

- Automatic expiry of blocklist entries — because a broken release stays broken; entries are removed by hand (owner)
- Importing the blocklists of Radarr/Sonarr — because they are not part of the config export, and sofarr already never re-takes the GUID of one of its own past grabs
- Marking an already imported grab as failed — because the file is in the library and an upgrade replaces it anyway (owner)
- Interactive search that lists releases for the user to pick from — because there is no UI yet (spec `web-ui`)
- Removing failed downloads from the client's history automatically — because nzbget, the only download client (D6), cleans up its history by its own settings
- Retrying a re-search whose indexers failed, and re-searching after an outage — because automatic retries would hammer an indexer that is down; a manual search (AC-12) repeats it on demand

## Acceptance criteria

- [x] AC-01 (event): When the download check finds that the download of an active grab ended in a release failure, then sofarr sets the grab to failed with the client's status text and puts the release on the blocklist of that movie or series.
- [x] AC-02 (event): When the download check finds that the download of an active grab ended in a local failure, then sofarr sets the grab to failed with the client's status text and does not put the release on the blocklist.
- [x] AC-03 (error): If grabbing a release fails with a broken NZB, then sofarr sets the grab to failed with the reason and puts the release on the blocklist.
- [x] AC-04 (error): If grabbing a release fails with an outage, then sofarr sets the grab to failed with the reason, does not put the release on the blocklist and starts no re-search.
- [x] AC-05 (always): sofarr never grabs a release that is on the blocklist of the movie or series it is considered for, matched by GUID or by normalized title — in the RSS sync, in a re-search and in a manual search; the same release stays allowed for any other movie or series.
- [x] AC-22 (always): sofarr does not take a release (by GUID) again for a movie or series it has any earlier grab of that release for, shadow grabs included — unless every such grab failed without a blocklist entry (local failure, outage, vanished download) or its blocklist entry was removed (AC-19).
- [x] AC-06 (event): When a movie grab fails by AC-01, AC-02, AC-03, AC-10 or AC-15 while re-search for movies is on and the movie is monitored, then sofarr searches the allowed indexers for the movie and grabs the best release by the same rules as the RSS sync.
- [x] AC-07 (event): When a series grab fails by AC-01, AC-02, AC-03, AC-10 or AC-15 while re-search for series is on and the series is monitored, then sofarr searches for its wanted episodes — each season of a season pack, otherwise each covered episode — and grabs the best releases by the same rules as the RSS sync; a season or episode covered by several failed grabs of the same job run is searched once.
- [x] AC-08 (state): While re-search for a media kind is off, a failed grab of that kind starts no search; the movie or episodes are no longer held by the grab, so the next RSS sync may take another release for them.
- [x] AC-09 (option): Where `sofarr.toml` sets re-search for movies or for series, that value applies; otherwise the `autoRedownloadFailed` imported from Radarr (movies) or Sonarr (series) applies; without an imported value re-search is off.
- [x] AC-10 (event): When the download of an active grab is absent from the client's queue and history at a download check, and has been absent at every successful download check since an absence first seen at least 24 hours earlier, then sofarr sets the grab to failed with a reason that names the vanished download, does not put the release on the blocklist, and re-searches as in AC-06 to AC-08; a download seen again in between restarts the 24 hours.
- [x] AC-11 (error): If the download client cannot be reached during a download check, then no grab changes its state and the check counts neither as an absence nor as a sighting for AC-10.
- [x] AC-12 (event): When the user starts a search for a movie, a series, a season or an episode through the API or the CLI, then sofarr searches the allowed indexers, grabs the best release for each item it covers that has no file or a file below the cutoff, by the same rules as the RSS sync (a series or season search covers only monitored episodes; a search for one movie or one episode also covers it when unmonitored), and answers with the number of distinct releases (by GUID) the indexers returned, the grabs made and the indexer errors. An item with an active grab only takes a release that the RSS sync would rate better.
- [x] AC-13 (error): If a manual search names a movie, series, season or episode that is not in the library, or the request names none or more than one target, then sofarr searches nothing and answers with a German error message (API: status 404 for an unknown target, 400 for a malformed request; CLI: exit code ≠ 0).
- [x] AC-14 (always): A series search asks every allowed indexer in a form that indexer supports and considers the releases of all of them: an indexer that announces TV search by TVDB id is asked by id; one that does not — and every indexer for a series without a TVDB id — is asked by title with the episode in the numbering the series uses (season and episode, the air date for daily series, the absolute number for anime).
- [x] AC-15 (event): When the user marks an active grab as failed, then sofarr removes the download and its files from the download client (if the client still has it), sets the grab to failed with a reason that names the manual marking, puts the release on the blocklist and re-searches as in AC-06 to AC-08.
- [x] AC-16 (error): If the user marks a grab as failed that is not active (shadow, imported or already failed), then sofarr changes nothing and answers with a German error message (API: status 409); an unknown grab id answers status 404.
- [x] AC-17 (error): If the download client cannot be reached while the user marks a grab as failed, then sofarr changes nothing and answers with a German error message (API: status 503).
- [x] AC-18 (event): When the user lists the blocklist through the API, then sofarr answers with the entries page by page (pages counted from 1, newest blocklisting first, 1 to 500 entries per page, default 50; a page past the end is empty), each with movie or series, release title, reason and the time it was blocklisted; a page below 1 or a page size outside 1 to 500 answers status 400.
- [x] AC-19 (event): When the user removes a blocklist entry through the API, then the release may be grabbed again for that movie or series; removing an unknown entry answers status 404.
- [x] AC-20 (always): A release is on the blocklist of a movie or series at most once; blocklisting it again (same GUID or same normalized title) adds no second entry; removing the movie or series from the library removes its entries.
- [x] AC-21 (state): While sofarr runs in shadow mode, a re-search or manual search only records shadow grabs and never contacts the download client.
- [x] AC-24 (event): When grabbing the release chosen by a search (re-search or manual) fails with a broken NZB, then sofarr takes the next best candidate for the same item from the same search results, without a new indexer search; an outage ends the search for that item.
- [x] AC-23 (error): If no indexer is allowed for the target of a search, then sofarr searches nothing and answers with 0 releases and a German note that no indexer serves the item.

## Technical notes

- Config: `[download] redownload_failed_movies` and `redownload_failed_series` (bool, optional; AC-09).
- API: `GET /api/blocklist?page=&page_size=` (default page size 50), `DELETE /api/blocklist/{id}`,
  `POST /api/grabs/movie/{grab-id}/fail`, `POST /api/grabs/series/{grab-id}/fail`,
  `POST /api/command/search` with a body naming exactly one of: `movie_id`; `series_id`;
  `series_id` + `season`; `series_id` + `season` + `episode`.
- CLI: `amigo-sofarr search <movie-id>` keeps working; series via
  `amigo-sofarr search --series <id> [--season <n> [--episode <n>]]`.
- The imported settings live in the `settings` table: `config_downloadclient` (Radarr) and
  `sonarr_config_downloadclient` (Sonarr), field `autoRedownloadFailed`.
- nzbget history states (prefix of `Status`): release failures are `FAILURE/HEALTH`, `FAILURE/PAR`,
  `FAILURE/UNPACK`, `DELETED/HEALTH`, `DELETED/MANUAL`; everything else that is not `SUCCESS`
  (e.g. `DELETED/DUPE`, `FAILURE/MOVE`, `WARNING/SPACE`, `WARNING/SCRIPT`) is a local failure.
- Broken NZB from Newznab: error code 300 ("No such item").
- Newznab TV search: `t=tvsearch` with `tvdbid`, `season`, `ep` where the indexer's caps announce
  them; otherwise `q=<title> S01E02` (or `S01` for a season).
- Jobs (RSS sync, download check, searches, marking) already run one at a time behind the job
  lock in `app.rs`; manual actions take the same lock, so they never race a check.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| The blocklisted release comes back from another indexer with a new GUID and the same title | Not grabbed (AC-05) |
| The title differs only in case or separators | Treated as the same release (AC-05) |
| A failed season pack where some episodes already have files at the cutoff | Season search; those episodes are not wanted, the engine only takes releases for the others |
| A failed upgrade grab for a movie that has a file | The file stays; the next RSS sync or the re-search may take another upgrade |
| The re-search finds nothing acceptable or every indexer fails | The grab stays failed, the item stays wanted; the errors are logged; the next RSS sync continues as usual |
| Two grabs of the same movie fail in the same check | One re-search for the movie |
| An episode grab and a season pack covering it fail in the same check | The season is searched once (AC-07) |
| A release blocklisted for one movie appears for another movie | Allowed for the other movie (AC-05) |
| nzbget restarts and its queue is briefly empty | Not vanished: only 24 hours of absence count (AC-10) |
| nzbget is down for more than 24 hours | Failed checks count for nothing, nothing vanishes (AC-11) |
| A download marked as vanished shows up again later and finishes | The grab stays failed and the download is not imported |
| The user marks a grab as failed while the download check is about to import it | The actions run one after the other; whichever runs second sees the new state (a grab already imported → AC-16) |
| Re-search is on, but the movie is no longer monitored | No re-search, the grab is failed — because sofarr only grabs for monitored items, as in the RSS sync |
| A manual search for a season where no episode is monitored | Nothing is grabbed; the answer reports 0 grabs (not an error) |
| The movie of a blocklist entry is removed from the library | The entry disappears with it (AC-20) |
| A release covering several seasons fails | Each of those seasons is searched (AC-07) |
| The movie is removed while its re-search is pending in the same job run | The re-search is skipped |
| A manual search for a movie with an active grab | Only a release the RSS sync would rate better is taken (AC-12) |
| A grab fails at grab time with an outage and the indexer is back minutes later | The next RSS sync may take the same release again (AC-04, AC-22) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Without an imported `autoRedownloadFailed`, re-search is off | The conservative choice: no unexpected indexer traffic or downloads | A fresh install without import never re-searches until configured |
| A vanished download triggers a re-search but no blocklist entry | Its release is not proven broken; the title is free again (owner, 2026-11-12) | A release that keeps vanishing for other reasons may be retried |
| 24 hours of absence before a download counts as vanished | Long enough to ride out restarts, maintenance and a cleared history being refilled; short enough that a missing download does not block a title for days | A download lost right after a grab blocks better releases for a day |
| A search for one movie or episode also covers it when unmonitored | The user named it explicitly, as with today's CLI `search <movie-id>`; series and season searches stay with monitored episodes | An accidental search grabs an unmonitored title |
| A download deleted by hand in the client is a release failure | Whoever deletes a download does not want that release (owner, 2026-11-12) | A deletion by mistake blocks a good release until removed from the blocklist |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `failure_tests::release_failure_fails_the_grab_and_blocklists_the_release`, `failure_tests::release_failure_of_a_series_grab_blocklists_for_the_series` (`crates/sofarr-server/src/failure_tests.rs`); `tests::classifies_failed_downloads` (`crates/sofarr-download/src/lib.rs`) | verified | 21fa06e |
| AC-02 | `failure_tests::local_failure_fails_without_blocklist_and_frees_the_release` (`crates/sofarr-server/src/failure_tests.rs`); `tests::classifies_failed_downloads` (`crates/sofarr-download/src/lib.rs`) | verified | 21fa06e |
| AC-03 | `failure_tests::broken_nzb_at_grab_time_blocklists`, `failure_tests::nzb_rejected_by_the_download_client_blocklists` (`crates/sofarr-server/src/failure_tests.rs`); `tests::classifies_nzb_downloads` (`crates/sofarr-indexer/src/lib.rs`); `tests::a_rejected_nzb_is_its_own_error` (`crates/sofarr-download/src/lib.rs`) | verified | 130254f |
| AC-04 | `failure_tests::outage_at_grab_time_fails_without_blocklist_or_search` (`crates/sofarr-server/src/failure_tests.rs`); `tests::classifies_nzb_downloads` (`crates/sofarr-indexer/src/lib.rs`) | verified | 130254f |
| AC-05 | `failure_tests::blocklisted_releases_are_skipped_by_title_per_movie`, `failure_tests::blocklisted_series_releases_are_skipped` (`crates/sofarr-server/src/failure_tests.rs`); `search_tests::manual_search_skips_blocklisted_releases`, `search_tests::failed_movie_grab_is_re_searched_once` (`crates/sofarr-server/src/search_tests.rs`); `tests::release_titles_differing_in_case_and_separators_are_equal` (`crates/sofarr-core/src/lib.rs`); `failure_tests::a_blocklisted_guid_is_skipped_under_another_title` (`crates/sofarr-server/src/failure_tests.rs`); `search_tests::manual_search_skips_a_blocklisted_guid_under_another_title` (`crates/sofarr-server/src/search_tests.rs`) | verified | 4e12aa0 |
| AC-06 | `search_tests::failed_movie_grab_is_re_searched_once`, `search_tests::unmonitored_movie_is_not_re_searched` (`crates/sofarr-server/src/search_tests.rs`); `api_tests::marking_an_active_grab_failed` (`crates/sofarr-server/src/api_tests.rs`); `search_tests::local_failure_of_a_movie_grab_is_re_searched`, `search_tests::broken_nzb_in_the_rss_sync_is_re_searched`, `search_tests::vanished_download_is_re_searched` (`crates/sofarr-server/src/search_tests.rs`) | verified | 4e12aa0 |
| AC-07 | `search_tests::failed_season_pack_and_episode_search_the_season_once`, `search_tests::failed_episode_grab_searches_its_episodes`, `search_tests::series_research_scope` (`crates/sofarr-server/src/search_tests.rs`); `search_tests::local_failure_of_a_series_grab_is_re_searched`, `search_tests::broken_season_pack_in_the_rss_sync_searches_the_season`, `search_tests::vanished_series_download_is_re_searched` (`crates/sofarr-server/src/search_tests.rs`); `api_tests::marking_a_series_grab_failed_searches_its_episodes` (`crates/sofarr-server/src/api_tests.rs`) | verified | 4e12aa0 |
| AC-08 | `search_tests::re_search_off_leaves_the_item_to_the_rss_sync` (`crates/sofarr-server/src/search_tests.rs`); `series_search_tests::re_search_off_frees_the_episodes_for_the_rss_sync` (`crates/sofarr-server/src/series_search_tests.rs`) | verified | 4e12aa0 |
| AC-09 | `search_tests::re_search_setting_comes_from_config_or_import` (`crates/sofarr-server/src/search_tests.rs`) | verified | 4e12aa0 |
| AC-10 | `failure_tests::a_download_absent_for_24_hours_has_vanished`, `failure_tests::a_download_seen_again_restarts_the_24_hours` (`crates/sofarr-server/src/failure_tests.rs`); `search_tests::vanished_download_is_re_searched` (`crates/sofarr-server/src/search_tests.rs`) | verified | 4e12aa0 |
| AC-11 | `failure_tests::an_unreachable_client_changes_nothing` (`crates/sofarr-server/src/failure_tests.rs`) | verified | 4e12aa0 |
| AC-12 | `search_tests::manual_movie_search_reports_releases_grabs_and_errors`, `search_tests::manual_search_replaces_an_active_grab_only_with_a_better_release`, `search_tests::a_search_for_one_movie_covers_it_when_unmonitored`, `search_tests::manual_series_season_and_episode_searches` (`crates/sofarr-server/src/search_tests.rs`); `api_tests::search_command_takes_exactly_one_target`, `api_tests::search_command_for_series_season_and_episode` (`crates/sofarr-server/src/api_tests.rs`); `search_tests::series_search_counts_every_release_returned` (`crates/sofarr-server/src/search_tests.rs`) | verified | 4e12aa0 |
| AC-13 | `search_tests::unknown_targets_are_not_found` (`crates/sofarr-server/src/search_tests.rs`); `api_tests::search_command_takes_exactly_one_target`, `api_tests::search_command_for_series_season_and_episode` (`crates/sofarr-server/src/api_tests.rs`); `search_rejects_unknown_and_malformed_targets` (`crates/sofarr-server/tests/cli_search.rs`) | verified | 4e12aa0 |
| AC-14 | `search_tests::series_search_asks_each_indexer_in_its_own_form`, `search_tests::title_search_results_must_match_the_series`, `search_tests::daily_and_anime_series_are_asked_in_their_numbering` (`crates/sofarr-server/src/search_tests.rs`); `tests::builds_tv_queries_from_caps`, `tests::reads_tv_search_caps` (`crates/sofarr-indexer/src/lib.rs`) | verified | 130254f |
| AC-15 | `api_tests::marking_an_active_grab_failed`, `api_tests::marking_a_series_grab_failed` (`crates/sofarr-server/src/api_tests.rs`); `downloads::tests::only_folders_named_after_the_download_are_deleted` (`crates/sofarr-server/src/downloads.rs`) | verified | 4e12aa0 |
| AC-16 | `api_tests::only_active_grabs_can_be_marked` (`crates/sofarr-server/src/api_tests.rs`) | verified | 4e12aa0 |
| AC-17 | `api_tests::marking_needs_the_download_client` (`crates/sofarr-server/src/api_tests.rs`) | verified | 4e12aa0 |
| AC-18 | `api_tests::blocklist_is_listed_page_by_page` (`crates/sofarr-server/src/api_tests.rs`); `blocklist::tests::blocklist_pages_newest_first` (`crates/sofarr-db/src/blocklist.rs`) | verified | 4e12aa0 |
| AC-19 | `api_tests::removing_a_blocklist_entry_frees_the_release` (`crates/sofarr-server/src/api_tests.rs`); `blocklist::tests::removing_entries_and_items` (`crates/sofarr-db/src/blocklist.rs`) | verified | 4e12aa0 |
| AC-20 | `blocklist::tests::blocklist_holds_a_release_once_per_item`, `blocklist::tests::removing_entries_and_items` (`crates/sofarr-db/src/blocklist.rs`) | verified | 9e7bc65 |
| AC-21 | `search_tests::shadow_mode_searches_only_record` (`crates/sofarr-server/src/search_tests.rs`); `series_search_tests::shadow_mode_series_searches_only_record` (`crates/sofarr-server/src/series_search_tests.rs`) | verified | 4e12aa0 |
| AC-22 | `failure_tests::earlier_grabs_hold_their_guid_unless_failed_without_entry`, `failure_tests::local_failure_fails_without_blocklist_and_frees_the_release`, `failure_tests::outage_at_grab_time_fails_without_blocklist_or_search` (`crates/sofarr-server/src/failure_tests.rs`); `search_tests::vanished_download_is_re_searched` (`crates/sofarr-server/src/search_tests.rs`) | verified | 4e12aa0 |
| AC-23 | `search_tests::no_allowed_indexer_searches_nothing` (`crates/sofarr-server/src/search_tests.rs`); `series_search_tests::no_allowed_indexer_searches_nothing_for_a_series` (`crates/sofarr-server/src/series_search_tests.rs`) | verified | 4e12aa0 |
| AC-24 | `search_tests::a_broken_nzb_falls_back_to_the_next_candidate`, `search_tests::an_outage_ends_the_search_for_the_item` (`crates/sofarr-server/src/search_tests.rs`) | verified | 4e12aa0 |

## Decisions

- 2026-11-12: Re-search is configurable per media kind in `sofarr.toml`, defaulting to the imported app setting (owner; D21).
- 2026-11-12: A download missing from the client for 24 hours counts as failed without a blocklist entry; a download deleted by hand counts as a release failure (owner).
- 2026-11-12: Only release failures and broken NZBs go on the blocklist; local failures and outages do not, so a good release is never blocked by an outage (owner).
- 2026-11-12: Blocklist entries never expire (owner).
- 2026-11-12: `DELETED/DUPE` is a local failure, not a release failure (corrects the list put to the owner): nzbget rejects by name against its own history, which says nothing about the release and would otherwise blocklist a good release that sofarr re-sends after an outage.
- 2026-11-12: Marking a grab as failed removes its download and files from the client — the owner chose "remove from nzbget" for manual marking, and the files of a failed release are of no use.
- 2026-11-12: Manual search (API + CLI) belongs to this spec because the series search it needs is built here anyway (owner).
- 2026-11-12: Tests and code of the server blocks were written together because the tests need the new APIs (`check_downloads_at`, `search`, `mark_failed`); the red run was the failing build. As a counter-check the core behavior was switched off one piece at a time (no blocklisting on failure, no re-search, never vanished, failed grabs holding their GUID, no fallback after a broken NZB, unreachable client read as empty): 16 of the new tests went red.
- 2026-11-12: AC-22 for shadow grabs held before this spec (every earlier grab blocked its GUID); the test for that part was green at once. New is that failed grabs no longer hold their GUID — the blocklist alone keeps a broken release out, so removing an entry (AC-19) frees it.
- 2026-11-12: A release found through a search by TVDB id belongs to the series whatever its title says; a release from a title search must match a title of the series (title, German title, alias, title with year), as in the RSS sync.
- 2026-11-12: An indexer's TV caps (`t=caps`) are asked once per process; an indexer whose caps cannot be read counts as an indexer error of that search and is skipped. An indexer without TV search gets `t=search` with the title query.
- 2026-11-12: Query forms: by id `tvdbid` with `season`/`ep` where the caps announce them, a daily episode as `season=YYYY&ep=MM/DD` (Newznab convention); by title `Title S01E02`, `Title S01`, `Title YYYY.MM.DD` (daily) and `Title 06` (anime, absolute number with two digits at least).
- 2026-11-12: A series search searches each season that has a wanted episode (monitored, aired, without file or below the cutoff); a season with nothing wanted is not asked.
- 2026-11-12: A release without a download link counts as a broken NZB, a missing `[nzbget]` section as an outage.
- 2026-11-12: Marking a grab as failed removes the download from nzbget and deletes its folder (mapped by `path_map`) only if the folder is named after the download — a client reporting a category or root folder never deletes more. Success answers `204 No Content`.
- 2026-11-12: The search body rejects unknown fields (`400`), so a misspelled key never turns into a different search.
- 2026-11-12: After the first verification round the re-search triggers AC-02, AC-03, AC-10 and AC-15 got their own tests for movies and series; they were green at once because the re-search already ran for every failure kind.
- 2026-11-12: The CLI checks the combination of `search` arguments itself instead of clap's `conflicts_with`/`requires`, so a malformed call also gets a German message (AC-13).
- 2026-11-12: API answers with status 500 carry a generic German message; the details go to the log (no internals in answers).
- 2026-11-12: A series search counts every distinct release the indexers returned, also those whose name holds no episode (AC-12); only the decision skips them.
- 2026-11-12: A download folder counts as the download's own when it is named exactly like the download or like nzbget's intermediate folder `<name>.#<id>`.
- 2026-11-12: The series cases of AC-08, AC-21 and AC-23 got their own tests after the third verification round; green at once, and red when failed series grabs keep holding their episodes or series grabs ignore shadow mode.
- 2026-11-12: The download check and the import moved from `app.rs` into `downloads.rs`, and the test modules of `app.rs` and `series.rs` into `app/tests.rs` and `series/tests.rs`, to keep the files below 800 lines.
