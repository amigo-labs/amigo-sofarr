---
status: todo
created: 2026-11-12
size: L
design: [sofarr.md §3, sofarr.md §8, sofarr.md §9 D11, sofarr.md §9 D19, sofarr.md §9 D24, sofarr.md §9 D26]
media: both
oracle: none
---

# Discover and people

## Context

After Radarr and Sonarr are gone, new titles reach the library only through import lists
(TMDb persons, D11) — there is no way to browse what is new or popular, to look up an actor's
films, or to add a single title. D19 decides discover and people pages on top of TMDB, without
requests or approvals (one user). Adding a movie exists today only inside the import list sync
(metadata, folder by `movieFolderFormat`, search when available); adding a series does not exist.

Terms used below:

- **Title** – a movie or a series as TMDB knows it.
- **Shelf** – one row of titles on the discover page (e.g. "Trending diese Woche").
- **Title preview** – the page of a title that is not in the library: poster, overview, year,
  genres, runtime or seasons, IMDb rating (spec `imdb-ratings`), cast and crew, trailer link,
  and the add action. A title in the library links to its library page instead.
- **Person page** – a person's photo, biography, birth and death dates, and all their movies and
  series, as cast and per crew department.
- **Add** – putting a title into the library with quality profile, root folder, monitored,
  minimum availability (movies) or monitored seasons (series), tags, and whether to search at once.
- **Add defaults** – the add choices last used, per media kind.
- **Library state** – per title: in the library, excluded (on the import list exclusion list), or neither.

## Scope

### In scope

- Discover page with shelves: trending today and this week, popular movies, upcoming in cinemas (Germany), now in cinemas, new as stream or disc (Germany, one shelf), popular series
- Filters across the discover page: media kind, genre, year range, original language, minimum IMDb rating
- Search for movies, series and people
- Title preview and person page; the cast on the library pages of movies and series links to person pages
- Adding movies and series from shelves, search, previews and person pages, with remembered defaults
- Creating a TMDb person import list from a person page
- A cache for TMDB answers

### Out of scope

- Requests, approvals and several users — because one user adds directly (D19)
- Recommendations ("because you watched …") — because sofarr knows no watch history; Jellyfin has it
- Streaming-provider availability ("on Netflix") — because the owner downloads instead
- Import list types other than TMDb persons — because the owner's setup uses only those (D11)
- Music — because sofarr has no music (sofarr.md §2)

## Acceptance criteria

- [ ] AC-01 (event): When the user opens the discover page, then it shows the shelves in this order, each with up to 20 titles and a way to see more: trending today, trending this week, popular movies, upcoming in cinemas, now in cinemas, new as stream or disc, popular series; each title with poster, title (German where TMDB has it), year, IMDb rating and votes (a placeholder while its IMDb id is still loading, "–" when there is none) and its library state.
- [ ] AC-02 (event): When the user sets at least one filter (media kind, genre, year range, original language, minimum IMDb rating), then the shelves give way to one result list of matching titles, most popular first (movies and series merged by TMDB's popularity value when no media kind is set), loading more while scrolling; the filters are part of the page address; clearing them brings the shelves back. The rating filter keeps only titles whose rating is known and reaches it — a title whose rating is still loading stays visible with the placeholder until it is known — and the list keeps loading until at least 20 titles are visible or TMDB has no more; while `[imdb] enabled = false`, the rating filter is not offered.
- [ ] AC-03 (event): When the user searches, then sofarr shows matching movies, series and people in three groups, each title with its library state.
- [ ] AC-04 (event): When the user opens a title that is not in the library, then the title preview is shown; for a title in the library, its library page opens.
- [ ] AC-05 (event): When the user opens a person, then the person page shows every movie and series of the person (ratings appear as they become known, titles without one sort last), cast and crew departments separately, sortable by year and (while `[imdb] enabled`) by IMDb rating, filterable to "nur fehlende" (titles neither in the library nor excluded), to movies or series, and with each title's library state.
- [ ] AC-06 (always): The library pages of movies and series (spec `web-ui-library`) show the first 10 billed cast members from TMDB with links to their person pages; a series without a TMDB id (spec `imdb-ratings`, AC-12) shows no cast.
- [ ] AC-07 (event): When the user adds a movie, then sofarr puts it into the library exactly as the import lists do today — metadata, folder by the naming format under the chosen root folder, monitored state, profile, minimum availability, tags — and, when chosen, searches for it once it counts as available. If the folder already exists on disk, it is used, and a disk scan of that folder right after adding takes up the files in it.
- [ ] AC-08 (event): When the user adds a series, then sofarr finds its TVDB id through TMDB, puts the series with its seasons and episodes into the library like `refresh-series` (folder by `seriesFolderFormat` under the chosen root folder; an existing folder is used and scanned as for movies), monitors the chosen seasons — and seasons that appear later, unless "neue Staffeln überwachen" is switched off — and, when chosen, searches for its missing monitored episodes.
- [ ] AC-09 (error): If a series has no TVDB id at TMDB, TVDB does not know it, or TVDB cannot be reached, then sofarr does not add it and shows a German message saying why.
- [ ] AC-10 (state): While the add dialog is open, it is filled with the add defaults of that media kind; adding stores the choices as the new defaults; a stored default that no longer exists (profile, root folder, tag) falls back to its first-time value; without stored defaults the dialog offers the first quality profile by name, the only root folder (or none chosen when there are several), monitored on, minimum availability "released", no tags, search on.
- [ ] AC-11 (error): If the user adds a title that is already in the library (e.g. from a second tab) or the chosen root folder no longer exists, then sofarr adds nothing and shows a German message; an excluded title is added only after the user confirms that it is on the exclusion list, and adding it removes it from that list.
- [ ] AC-12 (event): When the user creates an import list from a person page, then sofarr creates a TMDb person import list for that person with the chosen departments (cast and crew departments), profile, root folder, monitored state, minimum availability, tags and search on add, and the next import list sync adds the person's titles like any other list; if a list for that person exists already, sofarr creates none and says so in German.
- [ ] AC-13 (always): TMDB answers are cached: a shelf, a filtered result list or a search answers from the cache for 6 hours, a title or person for 24 hours; a cached answer is shown with the time it was fetched.
- [ ] AC-14 (error): If TMDB cannot be reached, then pages show the cached data with a German notice and its age, or, without cached data, a German message; no page breaks.
- [ ] AC-15 (state): While no TMDB API key is configured, the discover page explains in German that it needs `[tmdb]` in `sofarr.toml`; search, previews, person pages, cast links and add actions are not offered.
- [ ] AC-16 (always): Titles use German titles and texts where TMDB has them (language German, region Germany for release dates) and fall back to the original.
- [ ] AC-17 (always): Every image of these pages comes through sofarr's image cache (D24).

## Technical notes

- TMDB endpoints: `/trending/{movie|tv|all}/{day|week}`, `/movie/popular`, `/movie/upcoming`
  and `/movie/now_playing` with `region=DE`, `/discover/movie` with `region=DE`,
  `with_release_type=4|5` and a date range for new digital/disc releases, `/tv/popular`,
  `/discover/{movie|tv}` for filters, `/search/multi`, `/movie/{id}` and `/tv/{id}` with
  `append_to_response=credits,external_ids,videos`, `/person/{id}` with `combined_credits`,
  `/genre/{movie|tv}/list`; all with `language=de-DE`.
- The IMDb rating of a TMDB title needs its IMDb id: from the detail answer (cached 24 hours);
  entries without a cached detail show the rating once the detail is loaded in the background.
  Background detail loads stay at most at 40 TMDB requests per second (TMDB's limit is about 50),
  so a person with 300 credits is complete within seconds and then cached for 24 hours.
- The add flow is extracted from the import list sync (`app.rs`, `sync_import_lists`) into one
  function both use; a refactoring without behavior change for the lists.
- Add defaults are stored in the `settings` table, per media kind.
- TMDB data of a series (cast, videos) uses its TMDB id (D26, spec `imdb-ratings` AC-12).
- New API: discover shelves, search, title preview, person, add movie, add series, create import
  list; all in sofarr's own JSON with generated UI types.
- The minimum IMDb rating filter is applied by sofarr on the TMDB results.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| The same title appears in two shelves | Shown in both with the same library state |
| A title added while the discover page is open | Its library state changes to "in der Bibliothek" without a reload |
| A person with 300 credits (e.g. a prolific voice actor) | The page stays responsive; credits load in pages |
| A person known for crew only | The cast section is empty with a note; crew departments are shown |
| An upcoming movie without a German release date | Not on the German cinema shelf |
| A TMDB series whose TVDB id points to a merged series | TVDB's answer decides; a refused id gives AC-09 |
| The user adds a series with specials | Season 0 is not monitored unless chosen |
| Two root folders and no stored defaults | The dialog requires choosing one (AC-10) |
| A media kind filter "Serien" | Only series in the result list (AC-02) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Six hours of cache for shelves | Trending lists change slowly; keeps TMDB traffic low | A brand-new trending title shows up to 6 hours late |
| Season 0 is not monitored by default when adding | Specials are often unwanted extras | A wanted special must be monitored by hand |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `shelves_come_in_order_with_library_state_and_ratings_that_load` (`crates/sofarr-server/src/discover/tests.rs`); UI pending | open |        |
| AC-02 | `the_filtered_list_merges_movies_and_series_by_popularity` (`crates/sofarr-server/src/discover/tests.rs`); `the_rating_filter_reads_on_until_twenty_titles_are_visible` (`crates/sofarr-server/src/discover/tests.rs`); `genres_of_movies_and_series_are_merged` (`crates/sofarr-server/src/discover/tests.rs`); UI pending | open |        |
| AC-03 | `search_groups_movies_series_and_people_with_library_state` (`crates/sofarr-server/src/discover/tests.rs`); UI pending | open |        |
| AC-04 | `previews_show_the_title_and_say_whether_it_is_in_the_library` (`crates/sofarr-server/src/discover/title_tests.rs`); UI pending | open |        |
| AC-05 | `person_pages_list_cast_and_crew_departments_with_library_state` (`crates/sofarr-server/src/discover/title_tests.rs`); UI pending | open |        |
| AC-06 | `library_titles_get_their_first_ten_cast_members` (`crates/sofarr-server/src/discover/title_tests.rs`); UI pending | open |        |
| AC-07 | `an_added_movie_is_put_like_the_import_lists_and_its_folder_scanned` (`crates/sofarr-server/src/add_tests.rs`); `the_import_list_sync_adds_selected_movies_with_the_lists_settings` (`crates/sofarr-server/src/import_list_tests.rs`); UI pending | open |        |
| AC-08 | `an_added_series_monitors_the_chosen_seasons_and_new_ones_by_the_switch` (`crates/sofarr-server/src/add_tests.rs`); `series::tests::new_episodes_follow_their_season_and_new_seasons_the_switch` (`crates/sofarr-server/src/series/tests.rs`); UI pending | open |        |
| AC-09 | `a_series_tvdb_cannot_give_is_not_added` (`crates/sofarr-server/src/add_tests.rs`); UI pending | open |        |
| AC-10 | `add_defaults_are_remembered_per_kind_and_fall_back` (`crates/sofarr-server/src/add_tests.rs`); UI pending | open |        |
| AC-11 | `duplicates_missing_root_folders_and_excluded_titles_are_refused` (`crates/sofarr-server/src/add_tests.rs`); UI pending | open |        |
| AC-12 | `a_person_import_list_is_created_once_and_synced_like_any_other` (`crates/sofarr-server/src/add_tests.rs`); UI pending | open |        |
| AC-13 | `answers_come_from_the_cache_for_six_and_twenty_four_hours` (`crates/sofarr-server/src/discover/tests.rs`); `discover::cache::tests::*` (`crates/sofarr-server/src/discover/cache.rs`); UI pending | open |        |
| AC-14 | `tmdb_down_shows_cached_data_with_a_notice_or_a_german_message` (`crates/sofarr-server/src/discover/tests.rs`); UI pending | open |        |
| AC-15 | `without_a_tmdb_key_the_pages_say_what_is_missing` (`crates/sofarr-server/src/discover/tests.rs`); UI pending | open |        |
| AC-16 | `requests_ask_for_german_texts_and_german_releases` (`crates/sofarr-metadata/src/browse/tests.rs`); `details_fall_back_to_english_texts_and_find_ids_and_the_trailer` (`crates/sofarr-metadata/src/browse/tests.rs`); `list_entries_of_movies_series_and_people` (`crates/sofarr-metadata/src/browse/tests.rs`) | built |        |
| AC-17 | `tmdb_images_are_loaded_once_and_served_from_the_cache` (`crates/sofarr-server/src/discover/title_tests.rs`); UI pending | open |        |

## Decisions

- 2026-11-12: The add dialog remembers the last choices per media kind (owner).
- 2026-11-12: TMDB answers are cached in memory, bounded per kind, and kept up to 7 days as a stale fallback (AC-13, AC-14); one process needs no table, and the cache starts empty after a restart.
- 2026-11-12: Without a media kind, page n of the result list reads TMDB page n of movies and of series and merges them by popularity; one request reads at most 10 TMDB pages and then returns `next_page`, so a strict rating filter cannot run away (AC-02).
- 2026-11-12: The person page gets every credit with library state and rating, newest first, undated last; sorting and the "nur fehlende", kind and paging choices are made in the browser, because one TMDB request delivers all credits anyway (AC-05).
- 2026-11-12: `POST /api/discover/states` gives the current library state and rating of up to 500 titles, so pages refresh ratings still loading and titles added meanwhile without a reload.
- 2026-11-12: A title TMDB answers 404 for shows no rating instead of staying "pending".
- 2026-11-12: The new-season switch is stored per series (`series.monitor_new_seasons`, migration 0012, default on so existing series behave as before); Sonarr's own setting is not imported (AC-08).
- 2026-11-12: The series refresh monitors a new episode of a known season only if an episode of that season is monitored, and a new season by the series' switch; before, every new episode of a monitored series was monitored, which would monitor seasons the user left out when adding (AC-08). Series imported from Sonarr with a fully unmonitored season now keep that season unmonitored.
- 2026-11-12: A stored default tag that no longer exists is dropped on its own; a profile or root folder that no longer exists falls back to its first-time value (AC-10).
- 2026-11-12: New series get series type "standard" and season folders; root folders come from `rootfolder` (movies) and `sonarr_rootfolder` (series).
- 2026-11-12: The search on add runs after the answer: movies once they count as available (the import list rule), series for their missing monitored episodes; import lists keep searching inline.
- 2026-11-12: A person import list offers cast and the crew departments Directing, Production, Sound and Writing, the ones the existing list filter supports, and is stored as a `TMDbPersonImport` entry of the `importlist` setting (AC-12).
- 2026-11-12: An excluded title answers 428 until the request carries `confirm_excluded: true` (AC-11).
- 2026-11-12: The add-movie flow of the import list sync and discover is one function; the import list tests stayed green across the refactoring.
- 2026-11-12: `App` has a `tvdb_api` field next to `tmdb_api`, so tests point TVDB at a fake (`tmdb_fake.rs`); the series refresh now also builds its TMDB client through `metadata::tmdb`.
- 2026-11-12: `/api/images/tmdb/{size}/{file}` loads from TMDB only images sofarr handed out in an answer (remembered in memory per process), so the route is no open proxy to TMDB; cached images are served also after a restart, others come back with the next answer that hands them out (AC-17; code scanning `rust/request-forgery`).
