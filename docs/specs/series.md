---
status: todo
created: 2026-11-19
size: L
design: [sofarr.md §2, sofarr.md §4, sofarr.md §5, sofarr.md §9 D2, sofarr.md §9 D5, sofarr.md §9 D10, sofarr.md §9 D28]
media: series
oracle: GET /api/v3/series
---

# Series

## Context

Phase 5 extended the movie pipeline to series (D2: one service, D5: movies first): the library
comes over from Sonarr, TVDB keeps seasons and episodes current (D10: the same episode order as
Sonarr), and the RSS sync decides series releases with the movie rules plus the series-specific
part — which episodes a release covers and whether they are wanted. D28 settled how new episodes
are monitored at a refresh. This spec records that behavior, built before the spec workflow
existed (reverse-engineered on 2026-11-19).

The oracle is Sonarr's library (`GET /api/v3/series`, episodes and files per series), compared by
`amigo-sofarr compare-series-library`; series rules without oracle data are listed in
`docs/parity.md` („Annahmen ab Phase 3“, rows „Serien: …“).

Terms used below:

- **Series refresh** – reading a series and its episodes from TVDB into the library.
- **Known season** – a season of which the library already has at least one episode.
- **Monitored episode** – an episode that is monitored itself and belongs to a monitored series.
- **Aired** – the episode's air date is today or earlier (UTC).
- **Wanted episode** – a monitored, aired episode.
- **Covered episodes** – the library episodes a release names: by season and episode numbers, a
  whole season or a range of seasons (season pack), an air date (daily series) or absolute
  numbers (anime).
- **Season pack** – a release that covers one or more whole seasons.
- **Grab in flight** – a grab sent to the download client and not yet imported or failed.

## Scope

### In scope

- Taking over Sonarr's library (settings, series, episodes, episode files)
- The series refresh from TVDB: German title, aliases, poster, overview, episodes, monitoring of
  new episodes (D28)
- The refresh schedule
- Series decisions in the RSS sync: which series a release belongs to, which episodes it covers,
  whether they are wanted, which release wins (season packs)
- Importing a finished series download into the season folders

### Out of scope

- Adding a series with chosen seasons and the „neue Staffeln überwachen“ switch at add time —
  because spec `discover` (AC-08, AC-09) owns it
- Searching for series, seasons and episodes, failure handling and the blocklist — because spec
  `fail-handling` owns them
- The disk scan of series folders — because spec `disk-scan` owns it
- IMDb and TMDB ids of series — because spec `imdb-ratings` (AC-05, AC-12) owns them
- Monitoring, renaming and removing series in the UI — because spec `web-ui-library` owns them
- The quality, language, size and custom format rules themselves — because they are the movie
  decision engine's rules (`sofarr-core`), shared unchanged
- Episode file names in detail (tokens, multi-episode styles, colon replacement) — because the
  naming engine is shared with movies and checked against the oracle by `compare-rename`
- `compare-series-library` against a running Sonarr — because it is a manual check against a
  live instance (profile **Tests**: `manual:`)

## Acceptance criteria

- [ ] AC-01 (event): When the user imports from Sonarr, then sofarr takes over Sonarr's quality profiles, quality definitions, custom formats, media-management, indexer, naming and download-client settings, root folders and tags, every series with a TVDB id (title, year, runtime, original language, series type, monitored, season folders, quality profile, path, alternative titles, tags), its episodes (season, number, absolute number, title, air date, monitored) and its episode files (path, size, quality, revision, release group, custom format score); a series without TVDB id is left out.
- [ ] AC-02 (event): When a series is refreshed, then sofarr takes from TVDB the German title (TVDB's own title when there is no German one), adds TVDB's aliases and the German title to the series' alternative titles without duplicates, and takes the poster and the overview (the German one when TVDB has it); title, monitoring, quality profile and path stay as they are.
- [ ] AC-03 (event): When a series is refreshed, then every episode TVDB lists takes TVDB's title, German title, air date and absolute number; an episode the library knows keeps its monitoring and its file, an episode it does not know is added, and an episode TVDB no longer lists stays in the library unchanged.
- [ ] AC-04 (always): A new episode added by a refresh is monitored only when its series is monitored and it is not a special (season 0), and then: in a known season only when an episode of that season is monitored; in a new season only when the series' switch „neue Staffeln überwachen“ is on (default on).
- [ ] AC-05 (error): If TVDB cannot give a series during a refresh, then that series stays unchanged, the other series are refreshed, and the refresh ends without error.
- [ ] AC-06 (state): While no `[tvdb]` section is configured, a refresh changes no series and no episode.
- [ ] AC-07 (option): Where `[library] refresh_interval_hours` is set, the metadata refresh runs on its own every that many hours (default 24), in shadow mode too; 0 switches the schedule off.
- [ ] AC-08 (always): A release belongs to a monitored series whose title, German title, alternative title, or title followed by its year the release names, ignoring case and separators; an unmonitored series takes no release.
- [ ] AC-09 (always): sofarr covers by a release: the named episodes of the named season (several for a multi-episode release), every episode of the named seasons for a season pack (a range of seasons included), the episodes with the named air date for a daily release, the episodes with the named absolute numbers for an anime release; a release that covers no library episode is rejected as an unknown episode.
- [ ] AC-10 (always): sofarr takes a series release only if it covers at least one wanted episode; a release whose covered episodes are all unmonitored or not yet aired is rejected as not wanted.
- [ ] AC-11 (always): A release has to beat the weakest file among its wanted covered episodes by the quality profile's rules; when one of them has no file, the release counts as filling a missing episode.
- [ ] AC-12 (always): Of several approved releases that cover the same episodes of a series, sofarr takes the best one by the quality profile's ranking; a release that covers more episodes, including all episodes of another approved release, wins over it (a season pack over single episodes).
- [ ] AC-13 (state): While an episode has a grab in flight, it counts as having the grabbed release as its file: only a release that beats the grabbed one is taken for it.
- [ ] AC-14 (event): When a finished download of a series grab is imported, then each video is assigned to the grabbed episodes its name names, moved into the season folder (the series folder itself for a series without season folders) under a name made by the episode format of the series type (standard, daily, anime), and recorded as the file of these episodes.
- [ ] AC-15 (option): Where Sonarr's `episodeTitleRequired` is `always` — or `bulkSeasonReleases` and the download is a season pack — and the episode format uses an episode title, the import waits while a covered episode that aired less than 48 hours ago has no title yet (empty or `TBA`; for a German title token the German title, else the default one).

## Technical notes

- Import: `series::fetch_from_sonarr` (`/api/v3/series`, `/api/v3/episode?seriesId=…`,
  `/api/v3/episodefile?seriesId=…`) or an export folder (no episodes), then
  `series::import_documents`; settings are stored with the prefix `sonarr_` next to Radarr's.
  CLI `amigo-sofarr import-sonarr [--exports <dir>]`.
- Refresh: `series::refresh` per series (TVDB `series/{id}/extended?meta=translations`, all pages
  of `series/{id}/episodes/default`, German titles from `…/default/deu`), from
  `App::refresh(…, shows, manual)`: the scheduled job (`status::Job::Refresh`, first run one
  interval after the start) and `amigo-sofarr refresh-series` (with the disk scan of spec
  `disk-scan` AC-08). The refresh writes onto the series as it is when TVDB has answered, so
  changes other jobs made meanwhile are kept.
- `series::monitors_new_episode` implements AC-04 (D28); the switch is
  `series.monitor_new_seasons` (migration `0012_discover.sql`, default 1).
- Decisions: `series::decide` (RSS sync) and `series::decide_with` (searches, spec
  `fail-handling`) build a `SeriesTarget` per series and call
  `sofarr_core::series::evaluate_episodes`: the movie rules (`evaluate`) with the runtime of
  one episode times the number of covered episodes for the size check and without the year
  check; the file to beat is the weakest file (profile rank, then custom format score) of the
  wanted covered episodes. Rejection keys `unknown_episode`, `episode_not_wanted`.
- Title keys: `series::title_keys` (normalized title, German title, alternative titles,
  `Title YYYY`).
- Grabs in flight for AC-13 are the series grabs that are shadow or active (queued,
  downloading, completed but not imported).
- Import: `series::plan_episode_import` — a video whose name names no episode of the grab is
  skipped; when the grab covers a single episode, an unreadable (obfuscated) name falls back to
  the grab title. Without `renameEpisodes` the file keeps its name. A waiting import (AC-15) leaves the grab
  `completed` with „Import wartet auf Episodentitel (höchstens 48 Stunden nach Ausstrahlung)“.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| TVDB has no German title | The German title is TVDB's own title (AC-02) |
| TVDB drops an episode (e.g. merged into a double episode) | The library keeps it with its file and monitoring (AC-03) |
| A refresh finds a new special (season 0) | Added unmonitored (AC-04) |
| A refresh finds the first episode of a new season, switch off | Added unmonitored (AC-04) |
| TVDB is down for every series | Every series stays unchanged; the refresh ends without error (AC-05) |
| A season pack covers an episode with a Bluray file and one with a WEB-DL file (Bluray cutoff) | The WEB-DL decides: a Bluray pack is an upgrade (AC-11) |
| A season pack covers only unaired or unmonitored episodes | Rejected as not wanted (AC-10) |
| A release names a season the library does not know | Rejected as unknown episode (AC-09) |
| Two series share an alternative title | The release is decided for both; each takes it only if it is wanted there |
| A series in Sonarr's export without TVDB id | Left out of the import (AC-01) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Episodes TVDB no longer lists are kept | Deleting would drop files and monitoring on a TVDB edit | Orphaned episodes stay wanted when monitored and never aired |
| The weakest wanted file decides a season pack | A pack must not be rejected while one of its wanted episodes still needs an upgrade; recorded without oracle data in `docs/parity.md` („Serien: Staffelpakete“) | A pack may replace good files to upgrade one weak episode |
| „Aired“ uses the date in UTC | No time of day is stored for episodes | An episode can count as aired a few hours early or late |
| The refresh skips a series TVDB cannot give and goes on | One deleted series must not stop the others | A permanent TVDB error stays visible only in the log |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `series::tests::imports_sonarr_and_decides`, `series::tests::sonarr_series_without_tvdb_id_are_skipped` (`crates/sofarr-server/src/series/tests.rs`) | built | 7fadd63 |
| AC-02 | `series::tests::refresh_takes_titles_and_episodes_from_tvdb` (`crates/sofarr-server/src/series/tests.rs`); `tvdb::tests::reads_series_and_episodes` (`crates/sofarr-metadata/src/tvdb.rs`) | built | 7fadd63 |
| AC-03 | `series::tests::refresh_takes_titles_and_episodes_from_tvdb` (`crates/sofarr-server/src/series/tests.rs`); `tvdb::tests::logs_in_and_relogs_on_401` (`crates/sofarr-metadata/src/tvdb.rs`) | built | 7fadd63 |
| AC-04 | `series::tests::new_episodes_follow_their_season_and_new_seasons_the_switch`, `series::tests::refresh_takes_titles_and_episodes_from_tvdb` (`crates/sofarr-server/src/series/tests.rs`); `add_tests::an_added_series_monitors_the_chosen_seasons_and_new_ones_by_the_switch` (`crates/sofarr-server/src/add_tests.rs`) | built | 7fadd63 |
| AC-05 | `series::tests::refresh_takes_titles_and_episodes_from_tvdb` (`crates/sofarr-server/src/series/tests.rs`) | built | 7fadd63 |
| AC-06 | `series::tests::without_tvdb_series_are_not_refreshed` (`crates/sofarr-server/src/series/tests.rs`) | built | 7fadd63 |
| AC-07 | `series::tests::the_refresh_is_scheduled_by_its_interval` (`crates/sofarr-server/src/series/tests.rs`) | built | 7fadd63 |
| AC-08 | `series::tests::releases_match_series_by_title_german_title_alias_or_year` (`crates/sofarr-server/src/series/tests.rs`) | built | 7fadd63 |
| AC-09 | `series::tests::daily_and_absolute`, `series::tests::upgrades_and_season_packs`, `series::tests::unwanted_and_unknown`, `series::tests::multi_season_and_multi_episode_releases` (`crates/sofarr-core/src/series.rs`) | built | fd36e06 |
| AC-10 | `series::tests::missing_episode_is_wanted`, `series::tests::unwanted_and_unknown` (`crates/sofarr-core/src/series.rs`) | built |  |
| AC-11 | `series::tests::upgrades_and_season_packs`, `series::tests::multi_season_and_multi_episode_releases` (`crates/sofarr-core/src/series.rs`); `series::tests::imports_sonarr_and_decides` (`crates/sofarr-server/src/series/tests.rs`) | built | fd36e06 |
| AC-12 | `series::tests::imports_sonarr_and_decides`, `series::tests::the_best_release_wins_and_grabs_count_as_files` (`crates/sofarr-server/src/series/tests.rs`) | built | 7fadd63 |
| AC-13 | `series::tests::the_best_release_wins_and_grabs_count_as_files` (`crates/sofarr-server/src/series/tests.rs`) | built | 7fadd63 |
| AC-14 | `app::tests::live_series_pipeline` (`crates/sofarr-server/src/app/tests.rs`); `series::tests::names_episodes`, `series::tests::episode_formats_follow_the_series_type_and_season_folders` (`crates/sofarr-server/src/series/tests.rs`) | built | 7fadd63 |
| AC-15 | `series::tests::waits_for_episode_titles` (`crates/sofarr-server/src/series/tests.rs`) | built |  |

## Decisions

- 2026-11-19: Reverse-engineered from the phase-5 code (`spec-extract`); the ACs describe the
  behavior as built. Uncertain points are recorded under Assumptions instead of being asked.
