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
  2. „Fehlt noch“: monitored movies that are missing, newest added first.
  3. „Unter Cutoff“: movies whose file is below the cutoff, newest added first.
  4. „Am besten bewertet“: best IMDb rating first (votes as tie-breaker), only while ratings are on.
- [ ] AC-02 (state): While the series list is plain, sofarr shows the same four rows for series: „Fehlt noch“ holds series with missing episodes, „Unter Cutoff“ series with episodes below the cutoff.
- [ ] AC-03 (always): Below the rows the full list stands under the heading „Alle Filme“ or „Alle Serien“, with the toolbar and everything it does today.
- [ ] AC-04 (state): While the list is not plain (a search text or a filter is set), sofarr shows no rows, so the results stand at the top; when the search and the filters are cleared, the rows show again.
- [ ] AC-05 (event): When the user activates „Alle anzeigen“ of a row, then the full list shows that row's titles: the sort or filter of the row is set in the toolbar and the address (as a reload keeps it), and the page scrolls to the full list.
- [ ] AC-06 (state): While a row has no title, it does not show; while none has, the page starts with the full list as today.
- [ ] AC-07 (always): A tile in a row is a poster tile of `ui-cinema-tiles` with its monitor toggle; it has no selection checkbox. A change of monitoring in a row shows in the full list and in the other rows without a reload.
- [ ] AC-08 (always): With the e2e library of 2,501 movies, the movie list shows its rows and the first page of the full list within 2 s after the page starts loading, and the time measurement of `web-ui-library` AC-04 still holds.
- [ ] AC-09 (always): Both lists with their rows meet WCAG 2.1 AA in the dark palette at 1440 px and 360 px (axe scans); each row is a list named by its heading, reachable by keyboard with a visible focus.
- [ ] AC-10 (always, manual: images): The website screenshot `movies.png` is regenerated and shows the rows above the full list; the owner checked the file.
- [ ] AC-11 (state, manual: visual design): While the dark palette is active, the movie and series lists look like a streaming service's library at 1440 px and 360 px.

## Technical notes

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
| Rows hide while searching or filtering | A search should show its results at once, not below four rows | Low |
| No selection in rows | Bulk actions on a row would duplicate the full list's selection bar | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |

## Decisions

- 2026-11-22: The movie and series lists show rows on top and the full list with search and filters below (owner).
