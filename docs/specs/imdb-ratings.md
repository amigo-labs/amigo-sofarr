---
status: todo
created: 2026-11-12
size: M
design: [sofarr.md §8, sofarr.md §9 D10, sofarr.md §9 D20, sofarr.md §9 D26]
media: both
oracle: none
---

# IMDb ratings

## Context

The owner wants to see the IMDb rating of every title. TMDB has its own user score but no IMDb
rating; per-title APIs (e.g. OMDb) have daily limits. D20 decides the official IMDb datasets: IMDb
publishes `title.ratings.tsv.gz` daily (about 1.5 million titles with average rating and number of
votes) for personal and non-commercial use, which fits a home setup. sofarr loads it once a day and
looks ratings up locally by IMDb id, so lists can be sorted and filtered by rating without any
request per title.

Terms used below:

- **Rating** – IMDb's average rating (0.0–10.0, one decimal) and number of votes of a title.
- **IMDb id** – the `tt…` id; movies carry it from TMDB, series from TVDB or TMDB's external ids.
- **TMDB id of a series** – the series' id at TMDB, found by looking up its TVDB id at TMDB (D26).
- **Ratings data** – the ratings of all titles from the last successfully loaded dataset, with the
  date it was loaded.

## Scope

### In scope

- Daily download and import of the IMDb ratings dataset
- IMDb ids for every movie and series of the library
- Showing rating and votes in the movie and series lists and pages (spec `web-ui-library`) and in the API answers that spec `discover` uses
- Sorting by rating and filtering by minimum rating and minimum votes
- The attribution IMDb's terms ask for

### Out of scope

- Episode ratings — because the owner decides on series as a whole; the dataset would allow them later
- Other IMDb data (cast, genres, titles) — because TMDB and TVDB provide them already
- Ratings from other sources (TMDB score, Rotten Tomatoes, Metacritic) — because the owner asked for IMDb
- Using ratings in grab decisions or import lists — because decisions follow the quality rules (sofarr.md §2)

## Acceptance criteria

- [ ] AC-01 (event): When a day has passed since the ratings data was loaded (or none was loaded yet), then sofarr downloads the IMDb ratings dataset and replaces the ratings data with it as a whole, while API requests keep being answered.
- [ ] AC-02 (error): If the download fails, the file is damaged or incomplete, a required column is missing, or it holds clearly too few titles (fewer than half of the previous load), then sofarr keeps the previous ratings data, logs the reason, and tries again every hour until a load succeeds; the daily schedule of AC-01 counts from that success. A first load (no previous data) needs at least 100,000 titles.
- [ ] AC-03 (always): Loading the ratings data never leaves a mixture of old and new ratings visible; a crash during the load keeps the previous data.
- [ ] AC-04 (always): Every title sofarr shows — in the library lists and pages, and in the discover, search and person pages of spec `discover` — that has an IMDb id shows its rating and votes; a title without an IMDb id or without a rating in the data shows "–".
- [ ] AC-05 (event): When a movie or series has no IMDb id, or its IMDb id changed at TMDB/TVDB, then the next metadata refresh sets the current one — movies from TMDB, series from TVDB or, if TVDB has none, from TMDB's external ids; if no source reports an IMDb id any more, the previous one is kept.
- [ ] AC-12 (event): When the metadata refresh meets a series without a TMDB id, then it looks the series up at TMDB by its TVDB id and stores the TMDB id; a series TMDB does not know keeps none and is looked up again at the next refresh.
- [ ] AC-06 (event): When the user sorts a library list (movies, series) by rating, then titles are ordered by rating (higher first, votes as a tie-breaker), titles without a rating last.
- [ ] AC-07 (event): When the user sets a minimum rating and a minimum number of votes in a library list, then it shows only titles that reach both; titles without a rating are hidden while either filter is set.
- [ ] AC-08 (always): Every page that shows IMDb ratings shows IMDb's attribution (the source, linked to imdb.com) and the date of the ratings data.
- [ ] AC-09 (option): Where `[imdb] enabled = false` is set, sofarr downloads nothing from IMDb and shows no ratings, sort or filter by rating.
- [ ] AC-10 (always): Loading a full dataset of 1.5 million titles takes at most 60 seconds and at most 200 MB more peak resident memory of the test process than before the load, on the CI runner (GitHub `ubuntu-latest`, the stand-in for the owner's mini PC): `cargo test -p sofarr-server --release imdb_load_full` with a generated dataset fails above either limit.
- [ ] AC-11 (always): Showing ratings sends no request to IMDb per title; the movie list of spec `web-ui-library` with 2,500 movies and their ratings stays within that spec's speed limit (its AC-04).

## Technical notes

- Source: `https://datasets.imdbws.com/title.ratings.tsv.gz`, tab-separated with a header line:
  `tconst`, `averageRating`, `numVotes`. Streamed through `flate2` (new workspace dependency).
- Table `imdb_ratings (imdb_id PRIMARY KEY, rating, votes)` plus the load date in `settings`;
  loaded into a new table and swapped in one transaction (AC-03).
- Attribution text (IMDb's dataset terms): "Information courtesy of IMDb (https://www.imdb.com).
  Used with permission."
- Config: `[imdb] enabled` (default true).
- The series IMDb id comes from TVDB's remote ids; TMDB `/tv/{id}/external_ids` as fallback.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| IMDb's server is down for two days | The old ratings stay with their date; hourly retries until a load succeeds (AC-02) |
| A title's rating changes | Shown after the next daily load |
| A title with a rating but very few votes (e.g. 9.8 from 12 votes) | Shown; a minimum votes filter hides it (AC-07) |
| A movie whose IMDb id is merged or redirected at IMDb | No rating found, "–" until TMDB reports the new id |
| The dataset has an extra column or a changed header | A missing expected column fails the load (AC-02); extra columns are ignored |
| sofarr starts without internet | No ratings until the first successful load; the UI shows "–" and no date |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The personal, non-commercial use terms of the IMDb datasets fit sofarr | Each owner's own instance downloads the data for themselves; sofarr does not redistribute it | A commercial use of sofarr would need its own license from IMDb |
| A daily load is fresh enough | IMDb updates the dataset daily | Ratings lag up to one day |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `imdb_tests::a_valid_dataset_replaces_the_ratings_and_bad_ones_keep_them`, `imdb_tests::the_next_attempt_is_a_day_after_success_or_an_hour_after_failure` (`crates/sofarr-server/src/imdb_tests.rs`) | built |        |
| AC-02 | `imdb_tests::a_valid_dataset_replaces_the_ratings_and_bad_ones_keep_them`, `imdb_tests::the_next_attempt_is_a_day_after_success_or_an_hour_after_failure` (`crates/sofarr-server/src/imdb_tests.rs`); `imdb::tests::rows_are_checked` (`crates/sofarr-server/src/imdb.rs`) | built |        |
| AC-03 | `imdb_tests::readers_see_the_old_ratings_until_the_swap` (`crates/sofarr-server/src/imdb_tests.rs`); `imdb::tests::a_load_is_swapped_in_as_a_whole` (`crates/sofarr-db/src/imdb.rs`) | built |        |
| AC-04 | server: `imdb_list_tests::lists_and_pages_show_rating_and_votes_or_nothing` (`crates/sofarr-server/src/imdb_list_tests.rs`); UI and discover pages pending | open |        |
| AC-05 | `imdb_ids_tests::series_ids_follow_tvdb_then_tmdb_and_are_kept_when_gone` (`crates/sofarr-server/src/imdb_ids_tests.rs`); `metadata::tests::the_refresh_takes_tmdbs_imdb_id_and_keeps_the_old_one_without` (`crates/sofarr-server/src/metadata.rs`); `tests::series_ids_from_find_and_external_ids` (`crates/sofarr-metadata/src/lib.rs`) | built |        |
| AC-06 | `imdb_list_tests::sorting_by_rating_puts_unrated_titles_last` (`crates/sofarr-server/src/imdb_list_tests.rs`); UI control pending | open |        |
| AC-07 | `imdb_list_tests::minimum_rating_and_votes_hide_unrated_titles` (`crates/sofarr-server/src/imdb_list_tests.rs`); UI control pending | open |        |
| AC-08 | server: `imdb_tests::a_valid_dataset_replaces_the_ratings_and_bad_ones_keep_them` (`GET /api/imdb` date and attribution, `crates/sofarr-server/src/imdb_tests.rs`); UI pending | open |        |
| AC-09 | `imdb_tests::switched_off_nothing_is_loaded` (`crates/sofarr-server/src/imdb_tests.rs`); `imdb_list_tests::switched_off_there_are_no_ratings_to_show_sort_or_filter` (`crates/sofarr-server/src/imdb_list_tests.rs`) | built |        |
| AC-10 | `imdb_tests::imdb_load_full` (`crates/sofarr-server/src/imdb_tests.rs`; release run in CI job `check`) | built |        |
| AC-11 | `imdb_list_tests::rated_lists_of_2500_movies_stay_fast` (`crates/sofarr-server/src/imdb_list_tests.rs`) | built |        |
| AC-12 | `imdb_ids_tests::series_ids_follow_tvdb_then_tmdb_and_are_kept_when_gone` (`crates/sofarr-server/src/imdb_ids_tests.rs`) | built |        |

## Decisions

- 2026-11-12: IMDb ratings are their own spec before `discover`, because they also change the library lists (owner).
- 2026-11-12: Ratings are loaded into `imdb_ratings_next` and swapped in by renaming in one transaction; readers keep the previous table until the commit (AC-03).
- 2026-11-12: The dataset is streamed: a blocking task decompresses and parses it and hands batches of rows over a bounded channel to the database, so the whole file never sits in memory (AC-10).
- 2026-11-12: The next attempt is computed from the date of the last success and the time of the last failure (`imdb::next_attempt`), so a restart neither skips nor repeats a load (AC-01, AC-02).
- 2026-11-12: Movies already took TMDB's IMDb id at the refresh and kept the old one when TMDB has none; the test for it was green at once (AC-05).
- 2026-11-12: The library list snapshot is keyed by the date of the ratings data too; this is belt and braces, because a finished load also changes `library_version`.
- 2026-11-12: Sorting or filtering by rating while `[imdb] enabled = false` answers 400 with a German message instead of ignoring the parameter, so a stale UI state is visible (AC-09).
- 2026-11-12: `GET /api/imdb` gives the UI whether ratings are on, the date of the data and the attribution text (AC-08).
