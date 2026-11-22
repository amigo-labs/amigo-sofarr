---
status: todo
created: 2026-11-22
size: M
design: [sofarr.md §3, sofarr.md §9 D30, sofarr.md §9 D34]
media: both
oracle: none
---

# Cinema look 5: rows on the movie and series lists

## Context

The movie and series lists (`/filme`, `/serien`) are one grid or list with a toolbar: search, sort, view and filters (`ui-list-toolbar`, `ui-list-items`). The cinema look (D34) asks for rows to browse.

The owner decided on 2026-11-22: the lists show rows on top, below them the full grid or list with search and filters as today.

This spec builds on `ui-cinema-tiles` and on the row of `ui-cinema-home`.

## Terms

- **Row**: as in `ui-cinema-home`: a heading, one line of poster tiles that scrolls sideways, arrows and „Alle anzeigen“.
- **Plain list**: a movie or series list without a search text and without any filter (the chips of `ui-list-toolbar` show none).
- **Full list**: the grid or list of today below the rows.

## Scope

### In scope

- Four rows on each of the two lists while the list is plain.
- The heading of the full list.

### Out of scope

- Rows by genre — because the library stores no genres.
- Selecting titles in a row — because bulk actions belong to the full list (`web-ui-library` AC-10).
- Changing the toolbar or the full list — because `ui-list-toolbar` and `ui-list-items` settled them.

## Acceptance criteria

- [ ] AC-01 (state): While the movie list is plain, sofarr shows above its toolbar these rows in this order, each with at most 20 movies:
  1. „Zuletzt hinzugefügt“: newest added first.
  2. „Fehlt noch“: the movies of the toolbar's filter „Fehlt“ (monitored, available, without a file), newest added first.
  3. „Unter Cutoff“: the movies of the toolbar's filter „Cutoff nicht erreicht“, newest added first.
  4. „Am besten bewertet“: the movies of the toolbar's sort by rating (best first, votes as tie-breaker, `imdb-ratings` AC-06), without the movies that have no rating; only while ratings are on.
- [ ] AC-02 (state): While the series list is plain, sofarr shows the same four rows for series: „Fehlt noch“ holds series with missing episodes, „Unter Cutoff“ series with episodes below the cutoff.
- [ ] AC-03 (always): Below the rows the full list stands under the heading „Alle Filme“ or „Alle Serien“, with the toolbar and everything it does today.
- [ ] AC-04 (state): While the list is not plain (a search text or a filter is set), sofarr shows no rows, so the results stand at the top; when the search and the filters are cleared, the rows show again.
- [ ] AC-05 (event): When the user activates „Alle anzeigen“ of a row, then the full list shows that row's titles, set in the toolbar and the address (as a reload keeps it):
  - „Zuletzt hinzugefügt“: sort „Hinzugefügt“, newest first; the rows stay, and the page scrolls to the full list.
  - „Am besten bewertet“: sort „Bewertung“, best first; the rows stay, and the page scrolls to the full list.
  - „Fehlt noch“ and „Unter Cutoff“: their filter and the sort „Hinzugefügt“, newest first; the list is no longer plain, so the rows hide (AC-04) and the page starts with the full list.
- [ ] AC-06 (state): While a row has no title, it does not show; while none has, the page starts with the full list as today.
- [ ] AC-07 (always): A tile in a row is a poster tile of `ui-cinema-tiles` with its monitor toggle; it has no selection checkbox. A change of monitoring in a row or in the full list shows on every tile of that title on the page without a reload; a title that no longer matches its row stays in it until the rows load again (AC-12).
- [ ] AC-08 (always): With the e2e library of 2,501 movies, the movie list shows its rows and the first page of the full list within 2 s after the page starts loading, and the time measurement of `web-ui-library` AC-04 still holds.
- [ ] AC-09 (always): Both lists with their rows meet WCAG 2.1 AA in the dark palette at 1440 px and 360 px (axe scans); each row is a list named by its heading, reachable by keyboard with a visible focus.
- [ ] AC-10 (always, manual: images): The website screenshot `movies.png` is regenerated and shows the rows above the full list; the owner checked the file.
- [ ] AC-11 (state, manual: visual design, judged by the owner): While the dark palette is active, the movie and series lists look like a streaming service's library at 1440 px and 360 px: the rows carry the top of the page, the full list is easy to find below them, nothing overlaps.
- [ ] AC-12 (event): When the list page opens, or the list becomes plain again, then its rows load; if a row's data cannot be loaded, then that row does not show, the rest of the page shows as today, and the row loads again with the next of these events.

## Technical notes

The notes are suggestions for the build; the ACs are binding.


- **Data:** each row is one request to the list API of today (`sort`, `order`, `missing`, `cutoff_unmet`, `page_size=20`); no new endpoint.
- **UI:** `ui/src/lib/library/components/MediaListPage.svelte`, the row component of `ui-cinema-home`, the toolbar's address handling in `ui/src/lib/library/list-query.ts`.
- **Tests:** Playwright in a new `ui/e2e/cinema-library-rows.spec.ts` on the library and ratings servers; the time measurement of `library-perf.spec.ts` stays as it is.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Ratings switched off | No row „Am besten bewertet“ |
| A movie in three rows | It shows in each; a monitor change shows in all of them |
| The list opened with a filter in its address | No rows until the filter is cleared |
| The view switched to „Liste“ | The rows stay poster tiles; only the full list is a list |
| A window of 360 px | Each row scrolls sideways; the page does not |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| At most 20 tiles per row | The discover shelves show 20 (`discover` AC-01); „Alle anzeigen“ leads to all | Low |
| The rows stand above the toolbar | The toolbar belongs to the full list below; above it, it would seem to filter the rows | Low |
| The series list has „Zuletzt hinzugefügt“, not „Demnächst“ | „Demnächst“ is the overview's and the calendar's; both lists get the same four rows | Low |
| Rows hide while searching or filtering | A search should show its results at once, not below four rows | Low |
| No selection in rows | Bulk actions on a row would duplicate the full list's selection bar | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/cinema-library-rows.spec.ts`: „rows of the movie list (AC-01)“ (3 tests); `ui/src/lib/library/list-rows.test.ts`: „the rows“ | built |  |
| AC-02 | `ui/e2e/cinema-library-rows.spec.ts`: „rows of the series list (AC-02)“ (2 tests) | built |  |
| AC-03 | `ui/e2e/cinema-library-rows.spec.ts`: „the full list (AC-03)“ (movies, series) | built |  |
| AC-04 | `ui/e2e/cinema-library-rows.spec.ts`: „searching and filtering (AC-04)“ (3 tests); `list-rows.test.ts`: „plain lists“ | built |  |
| AC-05 | `ui/e2e/cinema-library-rows.spec.ts`: „„Alle anzeigen“ (AC-05)“ (3 tests); `list-rows.test.ts`: „„Alle anzeigen““ | built |  |
| AC-06 | `ui/e2e/cinema-library-rows.spec.ts`: „empty rows (AC-06)“ (2 tests) | built |  |
| AC-07 | `ui/e2e/cinema-library-rows.spec.ts`: „tiles and monitoring (AC-07)“ (2 tests) | built |  |
| AC-08 | `ui/e2e/cinema-library-rows.spec.ts`: „loading time (AC-08)“; `ui/e2e/library-perf.spec.ts` (project `perf`) | built |  |
| AC-09 | `ui/e2e/cinema-library-rows.spec.ts`: „accessibility in the dark palette (AC-09)“ (`/filme`, `/serien` at 1440 and 360 px) | built |  |
| AC-10 | `docs/site/screenshots/movies.png` regenerated (`cd ui && bun run screenshots`); owner's check pending | open |  |
| AC-11 |      | open   |        |
| AC-12 | `ui/e2e/cinema-library-rows.spec.ts`: „failures and reloads (AC-12)“ | built |  |

## Decisions

- 2026-11-22: The movie and series lists show rows on top and the full list with search and filters below (owner).
- 2026-11-22: The total („2501 Filme“) stays beside the page heading „Filme“ / „Serien“, because `ui-list-toolbar` AC-01 puts heading and count in one row; „Alle Filme“ / „Alle Serien“ is an `h2` of its own between the rows and the toolbar.
- 2026-11-22: `ui-list-toolbar` AC-01 („the first item of the list starts at most 200 px below the top of `main`“) holds only while no row stands above the list, which this spec's owner decision puts there on a plain list; its test now serves the rows empty (`noListRows` in `ui/e2e/library-helpers.ts`, AC-06: the page starts with the full list as today). Tests of other specs that changed the list API's answers or waited for its first answer now leave the rows' requests (page size 20) alone, and `cinema-tiles` waits until the rows have loaded before measuring the grid.
- 2026-11-22: The website screenshot `movies.png` is taken with the heading „Alle Filme“ a little above the middle of the window (`ui/e2e/site/screenshots.mjs`), so it shows the last rows above the full list; clicking „Poster“ for the shot used to scroll the toolbar into view anyway.
- 2026-11-22: „Alle anzeigen“ is a link that adds a history entry (Back returns to the plain list with its rows), unlike the toolbar's changes, which replace it; a click with a modifier key opens the link as the browser does.
- 2026-11-22: After „Alle anzeigen“ the focus moves to the heading „Alle Filme“ / „Alle Serien“ (`tabindex="-1"`), so a keyboard user continues at the full list.
- 2026-11-22: „Am besten bewertet“ asks the list API for `sort=rating&order=asc`: the API sorts by rating best first in ascending order (imdb-ratings AC-06), as the toolbar's „Bewertung“ does; titles without a rating, which the API puts last, are dropped in the UI.
- 2026-11-22: While ratings are on, a row tile shows the compact rating in its overlay and the rating with votes in its link's name, as the full list's poster tiles do (ui-cinema-tiles AC-05).
- 2026-11-22: The rows load only when the page opens and when the list becomes plain (AC-12); a change of sort or view keeps them. They do not refresh on their own, so `keepRowFocus` is not needed.
- 2026-11-22: `RowTile` takes an optional `monitor` (monitored, busy, ontoggle) for the toggle in the poster's top right corner, and `TileRow` an optional `onmore` click handler for „Alle anzeigen“; the overview passes neither.
- 2026-11-22: The unit tests of `list-rows.ts` were written with the module; the e2e tests of `cinema-library-rows.spec.ts` were red first (no rows, no heading „Alle Filme“).
