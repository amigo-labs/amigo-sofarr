---
status: implemented
created: 2026-11-19
size: M
design: [sofarr.md §2, sofarr.md §4, sofarr.md §5, sofarr.md §9 D2, sofarr.md §9 D6, sofarr.md §9 D7, sofarr.md §9 D9]
media: both
oracle: GET /api/v3/history (actual grabs, `compare-history`); the decisions themselves follow GET /api/v3/release (release oracle, `sofarr-parity`)
---

# RSS sync

## Context

New releases reach sofarr through the RSS feeds of the owner's Newznab indexers (D9, Usenet
only, D7). At a fixed interval sofarr fetches the latest releases, assigns each to a movie or
series of the library, lets the decision engine of `sofarr-core` (quality profiles, custom
formats, sizes, upgrades; measured against the release oracle, `docs/parity.md`) rate them and
takes the best release per movie or episode set. In shadow mode (§5, tool 3) it only records what
it would take, so its choices can be compared with Radarr's history while Radarr still runs; in
live mode it hands the release to nzbget (D6).

This spec was extracted from code built in phases 3 and 5, before the spec workflow; it
describes what the code does today. What happens when grabbing fails is spec `fail-handling`.

## Scope

### In scope

- The scheduled RSS sync and the CLI `amigo-sofarr rss-sync`
- Assigning releases to library movies and series
- Choosing the best approved release per movie and per covered episode set
- Grabbing: shadow grab or hand-over to nzbget
- Indexer tags

### Out of scope

- The rules of the decision engine itself (qualities, custom formats, sizes, languages, upgrades) — because they live in `sofarr-core` and are measured by the release oracle ratchet (`fixtures/decision-baseline.toml`)
- Broken NZBs, outages at grab time, the blocklist, never taking a GUID twice and re-search after failures — because spec `fail-handling` owns them (AC-03 to AC-05, AC-22 there)
- Manual and automatic searches through the indexers — because spec `fail-handling` (AC-12, AC-14) owns them; the RSS sync only reads the feeds
- Delay profiles and indexer flags — because the owner's setup configures none (`docs/parity.md`)
- Torrents and Prowlarr — because sofarr is Usenet only and talks to Newznab directly (D7, D9)
- What happens once the download finishes — because that is spec `download-import`

## Acceptance criteria

- [x] AC-01 (event): When an RSS sync runs, then sofarr asks every configured indexer once for its latest releases in the indexer's movie and series categories together, decides them against the library and reports the number of movie releases (those not read as an episode or season), of movie releases matched to a monitored, available movie, of approved movie decisions and of series releases taken.
- [x] AC-02 (error): If an indexer cannot be reached or answers with an error, then the sync goes on with the releases of the other indexers and reports that indexer's error as a German message that starts with the indexer's name.
- [x] AC-03 (always): A release whose name reads as an episode or season of a series is decided for series only, every other release for movies only.
- [x] AC-04 (always): A release belongs to a library movie when the indexer gives that movie's IMDb id; otherwise when its title equals, after normalization, the movie's title, original title, German title or one of its alternative titles and — where both are known — its year differs from the movie's by at most one.
- [x] AC-05 (state): While a movie is unmonitored or not yet available (its minimum availability plus the availability delay not reached today), or a series is unmonitored, the sync takes no release for it.
- [x] AC-06 (always): sofarr takes a release for a movie only when the decision engine approves it under the movie's quality profile — a movie whose file meets the cutoff gets none, a movie with a file below the cutoff gets only an upgrade.
- [x] AC-07 (always): A sync takes at most one release per movie: among the approved ones the best by quality rank in the profile, then custom format score, then revision (unless propers are not preferred), then size.
- [x] AC-08 (always): A series release belongs to a monitored series whose title, German title, alias or title with year equals the release's series title after normalization; per series and covered set of episodes the best approved release by the order of AC-07 is taken, and a release covering more episodes (a season pack) wins over the releases of single episodes it contains.
- [x] AC-09 (state): While a movie or episode has an open grab (shadow, or not yet imported), that grab counts as its file: only a release the decision engine rates as an upgrade over it is taken.
- [x] AC-10 (always): sofarr does not take a release whose name equals (ignoring case) the name of a release already imported for the same movie or series.
- [x] AC-11 (option): Where an indexer has `tags`, it serves only movies and series that carry one of these tags (labels compared without case); an indexer without tags serves all.
- [x] AC-12 (state): While sofarr runs in shadow mode (the default), a chosen release is only recorded as a shadow grab and logged; no NZB is fetched and the download client is not contacted.
- [x] AC-13 (state): While sofarr runs in live mode, a chosen release's NZB is fetched and handed to nzbget, named after the release, in the category `[nzbget] category` (default `movies`) for movies or `tv_category` (default `tv`) for series, and the grab is recorded as queued with nzbget's download id.
- [x] AC-14 (option): Where `[rss] interval_minutes` is set, the server runs the sync at that interval (default 30 minutes); 0, or no indexer configured, turns the scheduled sync off.

## Technical notes

- Indexer requests: `t=search` with `cat=<categories ∪ tv_categories>`, `limit`, `extended=1`,
  `apikey` (`sofarr-indexer`, `Newznab::rss`); a release without publication date counts as
  published now (age 0).
- Normalized titles: `sofarr_core::normalize_title` (case, separators, punctuation).
- Grabs: movie grabs in `grabs`, series grabs in `series_grabs` with the covered episode ids;
  status `shadow` or `queued`, later `downloading`, `completed`, `imported`, `failed`.
- All jobs (RSS sync, download check, searches, import lists) run one after another (one jobs
  lock), so a sync never overlaps a download check.
- The scheduled sync runs once right at the server start, then every interval; a run longer
  than the interval is followed by the next at once.
- CLI `amigo-sofarr rss-sync` prints `RSS: <n> Releases, <m> einem Film zugeordnet, <a> Filme und
  <s> Serien-Releases angenommen` plus one `Fehler: …` line per indexer error.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| The library has no movies (series only) | The movie part is skipped without needing Radarr's settings; series are decided as usual |
| The same release appears at two indexers | Decided twice; at most one is taken per movie (AC-07), and a second sync never takes the GUID again (spec `fail-handling` AC-22) |
| A release title matches two library movies (remakes with the same title and close years) | Decided for each; each movie may take it |
| A release without year matches a title | Matched (AC-04); the decision engine decides by its own year rule |
| Every indexer fails | No release; the report lists one error per indexer; nothing changes in the library |
| The indexer gives an IMDb id that no library movie has | The release is matched by its title as if it had no IMDb id (AC-04) |
| A grab for the movie is open in shadow mode and a better release comes | Taken as a second shadow grab (AC-09) |
| An indexer answers with a Newznab error document | Reported like an unreachable indexer (AC-02) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Matching by title tolerates a year difference of one | Release years and TMDB years differ by one around New Year and for festival releases (`docs/parity.md`, "RSS: Film-Zuordnung") | Two films with the same title one year apart both match; the engine's year rule decides |
| Best release: quality rank, then custom format score, then revision, then size | No oracle shows Radarr's order in the RSS sync (`docs/parity.md`, "RSS: bestes Release") | sofarr picks another of several approved releases than Radarr would |
| An open grab counts as the file | Avoids grabbing every equal release while a download is running (`docs/parity.md`, "Pending Grab") | A stuck download blocks equal releases until it fails or vanishes (spec `fail-handling`) |
| Shadow mode is the default | sofarr must never act next to a running Radarr unless told so (§5) | A forgotten `mode = "live"` leaves sofarr idle after the switch |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `rss_tests::asks_every_indexer_and_goes_on_after_a_failing_one`, `rss_tests::the_report_counts_movie_releases_and_matches_to_monitored_available_movies` (`crates/sofarr-server/src/rss_tests.rs`); `rss::tests::picks_best_release_per_movie` (`crates/sofarr-server/src/rss.rs`); `tests::queries_rss_and_movie_search` (`crates/sofarr-indexer/src/lib.rs`) | verified | 0db7cad, a7886fb |
| AC-02 | `rss_tests::asks_every_indexer_and_goes_on_after_a_failing_one` (`crates/sofarr-server/src/rss_tests.rs`); `tests::reports_api_errors` (`crates/sofarr-indexer/src/lib.rs`) | verified | 0db7cad |
| AC-03 | `rss_tests::shadow_mode_records_without_touching_the_client`, `rss_tests::live_grabs_go_to_the_client_in_the_category_of_their_kind`, `rss_tests::an_episode_release_is_decided_for_series_only`, `rss_tests::the_report_counts_movie_releases_and_matches_to_monitored_available_movies` (`crates/sofarr-server/src/rss_tests.rs`) | verified | 0db7cad, a7886fb |
| AC-04 | `rss_tests::releases_match_by_imdb_id_or_title_and_year` (`crates/sofarr-server/src/rss_tests.rs`); `rss::tests::picks_best_release_per_movie` (`crates/sofarr-server/src/rss.rs`) | verified | 0db7cad |
| AC-05 | `rss::tests::waits_for_availability_and_respects_skip`, `rss::tests::picks_best_release_per_movie` (`crates/sofarr-server/src/rss.rs`); `rss_tests::series_releases_go_to_monitored_series_by_title_alias_or_year` (`crates/sofarr-server/src/rss_tests.rs`) | verified | 0db7cad |
| AC-06 | `rss::tests::picks_best_release_per_movie` (`crates/sofarr-server/src/rss.rs`) | verified | before a8f635d |
| AC-07 | `rss_tests::the_best_release_wins_by_quality_score_revision_and_size` (`crates/sofarr-server/src/rss_tests.rs`); `rss::tests::picks_best_release_per_movie` (`crates/sofarr-server/src/rss.rs`) | verified | 0db7cad |
| AC-08 | `rss_tests::series_releases_go_to_monitored_series_by_title_alias_or_year` (`crates/sofarr-server/src/rss_tests.rs`); `series::tests::imports_sonarr_and_decides`, `series::tests::releases_match_series_by_title_german_title_alias_or_year`, `series::tests::the_best_release_wins_and_grabs_count_as_files` (`crates/sofarr-server/src/series/tests.rs`) | verified | 0db7cad, f4b5412 |
| AC-09 | `rss_tests::an_open_grab_counts_as_the_file`, `rss_tests::an_open_series_grab_counts_as_the_episode_file` (`crates/sofarr-server/src/rss_tests.rs`) | verified | fc4baeb |
| AC-10 | `rss_tests::a_release_named_like_an_imported_one_is_not_taken_again`, `rss_tests::a_series_release_named_like_an_imported_one_is_not_taken_again` (`crates/sofarr-server/src/rss_tests.rs`) | verified | 0db7cad, 8987674 |
| AC-11 | `rss_tests::a_tagged_indexer_serves_only_movies_with_its_tag`, `rss_tests::a_tagged_indexer_serves_only_series_with_its_tag` (`crates/sofarr-server/src/rss_tests.rs`); `app::tests::indexer_tags_restrict_indexers` (`crates/sofarr-server/src/app/tests.rs`) | verified | 0db7cad, a7886fb |
| AC-12 | `rss_tests::shadow_mode_records_without_touching_the_client` (`crates/sofarr-server/src/rss_tests.rs`) | verified | 0db7cad |
| AC-13 | `rss_tests::live_grabs_go_to_the_client_in_the_category_of_their_kind` (`crates/sofarr-server/src/rss_tests.rs`); `app::tests::live_pipeline_grabs_and_imports` (`crates/sofarr-server/src/app/tests.rs`) | verified | 0db7cad |
| AC-14 | `rss_tests::rss_and_download_jobs_follow_the_config` (`crates/sofarr-server/src/rss_tests.rs`); `config::tests::full_config_parses` (`crates/sofarr-server/src/config.rs`) | verified | 0db7cad |

## Summary

> Completed: 2026-11-19

- The RSS sync asks every indexer once, splits releases into series and movie releases and decides them against the library.
- Movies match by IMDb id or normalized title with year ±1; series by title, German title, alias or title with year.
- At most one release per movie (and per covered episode set) is taken by quality, score, revision, size; season packs win, open grabs count as files, imported names and indexer tags are respected.
- Shadow mode only records; live mode hands the NZB to nzbget in the category of its kind; the interval follows `[rss] interval_minutes`.

## Decisions

- 2026-11-19: Extracted from the existing code (phases 3 and 5) with `spec-extract`; tests for ACs without one were added in 0db7cad and fc4baeb.
- 2026-11-19 (check): AC-01 revised to what the report counts: releases read as episodes are not in the release count, and a match counts only for monitored, available movies (the matcher only knows those); the acceptance check found the wording broader.
