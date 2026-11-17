---
status: implemented
created: 2026-11-17
size: M
design: [sofarr.md §3, sofarr.md §9 D24, sofarr.md §9 D30]
media: both
oracle: none
---

# UI redesign 2: list header, toolbar and filter panel

## Context

On the movie and series lists, ten labelled dropdowns, a search field and the view buttons fill about a third of a 1440 px window before the first poster shows. The redesign series (D30, `ui-shell-redesign`) cleans the list pages up in two specs:

- **This one:** the page header, a one-row toolbar, the filters in a panel and the active filters as chips.
- **`ui-list-items`:** the cards and rows themselves.

Behavior stays as `web-ui-library` and `imdb-ratings` specify it:

- the filters, their URL parameters and their AND logic (web-ui-library AC-02, AC-03, AC-06)
- the remembered list or poster view
- the scroll loading and the bulk monitor actions (AC-10)
- the speed limit (AC-04, imdb-ratings AC-11)
- IMDb's attribution on every page with ratings (imdb-ratings AC-08)

Only the arrangement and the controls change.

Terms:

- **List page**: `/filme` or `/serien`.
- **Filter**: one of Überwacht, Fehlt, Cutoff nicht erreicht, Mit Datei (movies) or Komplett (series), Qualitätsprofil, Tag, Mindestbewertung and Mindeststimmen (the last two only while ratings are shown). Search and sort are **not** filters.
- **Active filter**: a filter whose value is not "Alle".
- **Toolbar**: the row of controls under the page header.
- **Filter panel**: the area that holds the filter dropdowns.
- **Filter chip**: a small control naming one active filter and its value, which removes that filter.

## Scope

### In scope

- The page header of both list pages: the heading and the count in one row.
- One toolbar row: search, sort field, sort direction, the list/poster view switch and a filter button with the number of active filters.
- The filter dropdowns in a panel that the filter button opens and closes.
- Active filters as removable chips, plus a reset action.
- IMDb's attribution moved from above the list to below it.

### Out of scope

- How a single movie or series shows in the list or grid — because that is `ui-list-items`.
- New filters, new sort fields or new URL parameters — because the redesign changes the look, not what can be filtered.
- Remembering whether the filter panel is open — because the owner chose a closed panel at every load, with the chips always visible.
- The discover filters (`/entdecken`) — because they belong to `ui-calendar`, `ui-overview-pages` and `ui-discover-polish`.

## Acceptance criteria

- [x] AC-01 (always): sofarr shows the list page's heading ("Filme" or "Serien") and the item count (the number of items matching search and filters, as today) in one row, and below it one toolbar.
  - The toolbar holds, in this order: the search field ("Suche"), the sort field ("Sortierung"), a sort direction button, the view switch ("Ansicht": Liste, Poster) and the filter button.
  - At 1280 px and wider the toolbar is one row, and the first item of the list starts at most 200 px below the top edge of the page's `main` element while the filter panel is closed and no chip shows.
- [x] AC-02 (event): When the user presses the sort direction button, then the list's direction flips between ascending and descending, exactly as the former "Reihenfolge" dropdown did.
  - It changes the same URL parameter in the same way.
  - The button's German name says what pressing it does ("Absteigend sortieren" or "Aufsteigend sortieren").
  - Its icon shows the current direction.
- [x] AC-03 (state): While no filter is active, the filter button reads "Filter"; while n filters are active, it reads "Filter (n)".
- [x] AC-04 (event): When the user presses the filter button, then the filter panel opens below the toolbar with every filter as a labelled dropdown (the labels and choices of today), and the button is marked expanded (`aria-expanded="true"`); pressing it again closes the panel.
  - Whenever the page loads, the panel is closed.
- [x] AC-05 (state): While a filter is active, a chip names it and its value (e.g. „Überwacht: Ja“, „Qualitätsprofil: HD-1080p“, „Mindestbewertung: ab 7,0“) below the toolbar, whether the panel is open or closed.
  - Chips appear in the panel's order.
  - Each chip's German name says that pressing it removes the filter („Filter Überwacht: Ja entfernen“).
- [x] AC-06 (event): When the user presses a chip, then that filter goes back to "Alle", the list and the URL update as if it had been set to "Alle" in its dropdown, and the focus moves to the next chip; when the removed chip was the last one, to the chip before it; when no chip is left, to the filter button.
- [x] AC-07 (event): When two or more filters are active and the user presses "Alle Filter zurücksetzen", then every filter goes back to "Alle" at once, in one list update, including rating minimums that are in the URL but hidden while ratings are off.
  - Search and sort stay as they were.
  - The focus moves to the filter button.
- [x] AC-08 (always): Opening a list address with filters, search and sort in it shows exactly the list of today (web-ui-library AC-03), with the chips of its active filters, the search text in the field, and the sort field and direction set; the URL parameters are unchanged.
- [x] AC-09 (always): While ratings are shown, IMDb's attribution with the date of the ratings data stands once on the list page, below the list and the "Weitere laden" control, not between the controls and the list.
- [x] AC-10 (state): While the window is narrow (below 768 px), the toolbar wraps without horizontal scrolling down to 360 px.
  - The search field takes the full width.
  - The filter panel shows its dropdowns in one or two columns.
  - The chips wrap.
- [x] AC-11 (always): The toolbar, the open filter panel and the chips meet WCAG 2.1 AA in both palettes (the axe scans of the list pages run with the panel closed and open and with chips shown), every control is reachable by keyboard with a visible focus, and every icon-only control has a German name.
- [x] AC-12 (always): The speed limits of web-ui-library AC-04 and imdb-ratings AC-11 still hold (the perf test with 2,500 movies), and opening a list page directly transfers at most 200 KB of JavaScript and CSS (web-ui-shell AC-16).
- [x] AC-13 (state, manual: visual design): While either palette is active, the movie and series lists look complete at 1440 px and 360 px with the filter panel closed and open and with chips shown: nothing overlapping, the toolbar on one row at 1440 px.

## Technical notes

- **Toolbar controls:**
  - The direction button is an icon button (Octicons `sort-asc-16` / `sort-desc-16`).
  - The view switch keeps its two `aria-pressed` buttons "Liste" and "Poster". The labels may become icons with the words as names, but the existing tests that read "Liste"/"Poster" on one line keep holding.
- **Filter button:** `aria-expanded` and `aria-controls` point at the panel. The panel is a plain region in the page (not a popover), so the list moves down while it is open.
- **Chips:** buttons with the shared chip look (`am-chip`) plus an `x-16` icon; "Alle Filter zurücksetzen" is a ghost button (`am-btn--ghost`).
- **Classes:** the controls of these components use the `am-*` classes directly (`am-input`, `am-btn`, `am-seg`, `am-chip`). This is the per-page rename D30 announced for specs 2–4.
- **Where the code lives:**
  - The chip texts come from the same labels and option texts as the dropdowns, in `list-query.ts` or next to it, as a pure, unit-tested function.
  - `ListControls.svelte` holds the toolbar, panel and chips; `MediaListPage.svelte` holds the header and moves `ImdbAttribution`.
- **Tests:**
  - Playwright: a new `ui/e2e/library-toolbar.spec.ts`. The existing tests that set "Reihenfolge" or other filters open the panel first, through a helper in `library-helpers.ts`.
  - Vitest: the chip texts and the count of active filters.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A list address with an unknown profile or tag id | Behaves as today (web-ui-library); the chip shows the id |
| A list address with an invalid filter value (`ueberwacht=vielleicht`) | Parsed as today: the filter counts as "Alle", no chip |
| Ratings are switched off while a rating filter is in the URL | The rating filter is not shown, counts as not active, and gets no chip (imdb-ratings AC-09) |
| Profiles and tags are still loading, or loading them failed | Their chips show the id until the names are known |
| Exactly one filter active | No "Alle Filter zurücksetzen" (the chip itself removes it) |
| The panel is open and the user changes a dropdown | The panel stays open; the chip appears at once |
| Search text only, no filter | The button reads "Filter", no chips |
| Keyboard user closes the panel with the button | The focus stays on the button |
| The list request fails after a chip was removed or after a reset | Filters, URL and chips keep the new state; the list shows the error notice with retry of today (web-ui-library) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 200 px from the top of `main` to the first item at 1280 px | Padding 24 px, header row ≈ 32 px, toolbar row 32 px and gaps of 16 px add up to about 120 px; the 1440 px screenshot of 2026-11-16 measures about 370 px today | Low |
| The toolbar fits one row at 1280 px | Beside the 240 px sidebar the main area is about 1000 px wide; sort field (≈ 160 px), direction (32 px), view switch (≈ 180 px) and filter button (≈ 110 px) leave about 450 px for the search | Low |
| The attribution may sit below the list | imdb-ratings AC-08 requires it on the page, not a place | Low |
| No "Alle Filter zurücksetzen" for one filter | The chip already does the same in one press | Low |

## Summary

> Completed: 2026-11-17

- The movie and series lists show heading and count in one row and one toolbar below: search, sort field, a sort direction icon button („Absteigend/Aufsteigend sortieren“), the Liste/Poster switch and a filter button reading „Filter“ or „Filter (n)“.
- The filters sit in a panel the filter button opens and closes (`aria-expanded`), closed at every load, two columns at most below 768 px.
- Active filters show as removable chips in the panel's order, built by the pure `filterChips` / `withoutFilters` (`ui/src/lib/library/filter-chips.ts`); the focus moves to the next chip, the one before it, or the filter button; „Alle Filter zurücksetzen“ clears every filter in one list update and keeps search and sort.
- IMDb's attribution moved below the list; URL parameters, filter logic, speed and transfer budget are unchanged.

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `header and toolbar (AC-01)` › `heading and count in one row, the toolbar in one row, the list starts high` (`ui/e2e/library-toolbar.spec.ts`) | verified | 30680b4 |
| AC-02 | `sort direction (AC-02)` › `the button flips the direction like the dropdown did, kept in the address` (`ui/e2e/library-toolbar.spec.ts`); `sorts by year descending, kept in the address` (`ui/e2e/library-lists.spec.ts`); `sorting by rating puts the best first …` (`ui/e2e/imdb-ratings.spec.ts`) | verified | 30680b4 |
| AC-03 | `filter button and panel (AC-03, AC-04)` › `reads Filter, then Filter (n); …`, `search text alone is no filter` (`ui/e2e/library-toolbar.spec.ts`); `filter chips` (`ui/src/lib/library/filter-chips.test.ts`) | verified | 30680b4 |
| AC-04 | `filter button and panel (AC-03, AC-04)` › `reads Filter, then Filter (n); opens and closes the panel; closed at every load` (`ui/e2e/library-toolbar.spec.ts`) | verified | 30680b4 |
| AC-05 | `chips (AC-05 .. AC-08)` › `name the active filters in order, also with the panel closed (AC-05, AC-08)`, `series have the Komplett filter and its chip` (`ui/e2e/library-toolbar.spec.ts`); `filter chips` › `names every active filter and its value in the panel's order (AC-05)`, `shows the id of an unknown profile or tag`, `leaves out the rating filters while ratings are off` (`ui/src/lib/library/filter-chips.test.ts`) | verified | 30680b4 |
| AC-06 | `chips (AC-05 .. AC-08)` › `a chip removes its filter; the focus moves on (AC-06)` (`ui/e2e/library-toolbar.spec.ts`); `removing a chip sets only that filter back to Alle (AC-06)` (`filter-chips.test.ts`) | verified | 30680b4 |
| AC-07 | `chips (AC-05 .. AC-08)` › `"Alle Filter zurücksetzen" clears every filter, keeps search and sort (AC-07)` (`ui/e2e/library-toolbar.spec.ts`); `resetting clears every filter and keeps search and sort (AC-07)` (`filter-chips.test.ts`) | verified | 30680b4 |
| AC-08 | `chips (AC-05 .. AC-08)` › `name the active filters in order, also with the panel closed (AC-05, AC-08)` (`ui/e2e/library-toolbar.spec.ts`); `filters combine with AND, are part of the address and survive a reload` (`ui/e2e/library-lists.spec.ts`); `minimum rating and votes combine, …`, `a minimum from the address that is not offered is kept and shown` (`ui/e2e/imdb-ratings.spec.ts`) | verified | 30680b4 |
| AC-09 | `attribution (AC-09)` › `IMDb's attribution stands once, below the list` (`ui/e2e/library-toolbar.spec.ts`) | verified | 30680b4 |
| AC-10 | `narrow window (AC-10)` › `the toolbar wraps, the search takes the full width, nothing scrolls sideways` (360 px); `narrow window between 600 and 767 px (AC-10)` › `the panel shows its dropdowns in at most two columns; the chips wrap` (`ui/e2e/library-toolbar.spec.ts`) | verified | 30680b4, 4dbc607 |
| AC-11 | `axe, dark theme` and `axe, light theme` › `open filter panel and chips (ui-list-toolbar AC-11)`, `Filme`, `Serien` (`ui/e2e/library-a11y.spec.ts`) | verified | 30680b4 |
| AC-12 | `large library (AC-04)` › `first entries within 1 s, filter and search updates within 200 ms` (`ui/e2e/library-perf.spec.ts`, the filter now set in the open panel); `transfer budget (shell AC-16)` › `Filme`, `Serien` (`ui/e2e/library-a11y.spec.ts`) | verified | 30680b4 |
| AC-13 | manual: the owner checked the movie and series lists with the filter panel closed and open and with chips, both palettes, 1440 px and 360 px („Sichtcheck passt“), 2026-11-17 | verified | d51afe1 |

## Decisions

- 2026-11-17: The list redesign of the plan's spec 2 is split into this spec and `ui-list-items`, because together they exceed one M block.
- 2026-11-17: The filter panel is closed at every load and its state is not remembered; active filters always show as chips (owner).
- 2026-11-17 (build): Red run against e07a792 (UI of `ui-shell-redesign`): `bunx playwright test e2e/library-toolbar.spec.ts` failed 10 of 10 (no toolbar role, no direction button, no filter button or chips, attribution above the list). The unit tests of `filter-chips.ts` were written before the module, but their red run against the missing module was not recorded.
- 2026-11-17 (build): The toolbar is `role="toolbar"` named „Liste“; the search and sort labels are visually hidden (the placeholder and the selected value show what they are), the filter dropdowns keep their visible labels in the panel.
- 2026-11-17 (build): The chips are filled `am-chip am-chip--accent` buttons with an `x` icon; „Alle Filter zurücksetzen“ is a small ghost button. Their names („Filter … entfernen“) contain the filter labels, so tests that look a dropdown up by `getByLabel('Mindestbewertung')` now pass `exact: true`.
- 2026-11-17 (build): Existing tests that set filters open the panel first (`openFilters` in `ui/e2e/library-helpers.ts`); the perf test opens it before the measured change, so the measurement still covers only the list update.
- 2026-11-17 (fix): spec-check found AC-10 broken: below 768 px the panel showed up to four columns (auto-fill). Red first: `narrow window between 600 and 767 px (AC-10)` at 700 px measured 4 columns; the panel is now two columns below 768 px.
