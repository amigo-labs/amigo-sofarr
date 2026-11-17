---
status: todo
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
- The discover filters (`/entdecken`) — because they belong to `ui-pages-redesign`.

## Acceptance criteria

- [ ] AC-01 (always): sofarr shows the list page's heading ("Filme" or "Serien") and the item count (the number of items matching search and filters, as today) in one row, and below it one toolbar.
  - The toolbar holds, in this order: the search field ("Suche"), the sort field ("Sortierung"), a sort direction button, the view switch ("Ansicht": Liste, Poster) and the filter button.
  - At 1280 px and wider the toolbar is one row, and the first item of the list starts at most 200 px below the top edge of the page's `main` element while the filter panel is closed and no chip shows.
- [ ] AC-02 (event): When the user presses the sort direction button, then the list's direction flips between ascending and descending, exactly as the former "Reihenfolge" dropdown did.
  - It changes the same URL parameter in the same way.
  - The button's German name says what pressing it does ("Absteigend sortieren" or "Aufsteigend sortieren").
  - Its icon shows the current direction.
- [ ] AC-03 (state): While no filter is active, the filter button reads "Filter"; while n filters are active, it reads "Filter (n)".
- [ ] AC-04 (event): When the user presses the filter button, then the filter panel opens below the toolbar with every filter as a labelled dropdown (the labels and choices of today), and the button is marked expanded (`aria-expanded="true"`); pressing it again closes the panel.
  - Whenever the page loads, the panel is closed.
- [ ] AC-05 (state): While a filter is active, a chip names it and its value (e.g. „Überwacht: Ja“, „Qualitätsprofil: HD-1080p“, „Mindestbewertung: ab 7,0“) below the toolbar, whether the panel is open or closed.
  - Chips appear in the panel's order.
  - Each chip's German name says that pressing it removes the filter („Filter Überwacht: Ja entfernen“).
- [ ] AC-06 (event): When the user presses a chip, then that filter goes back to "Alle", the list and the URL update as if it had been set to "Alle" in its dropdown, and the focus moves to the next chip; when the removed chip was the last one, to the chip before it; when no chip is left, to the filter button.
- [ ] AC-07 (event): When two or more filters are active and the user presses "Alle Filter zurücksetzen", then every filter goes back to "Alle" at once, in one list update, including rating minimums that are in the URL but hidden while ratings are off.
  - Search and sort stay as they were.
  - The focus moves to the filter button.
- [ ] AC-08 (always): Opening a list address with filters, search and sort in it shows exactly the list of today (web-ui-library AC-03), with the chips of its active filters, the search text in the field, and the sort field and direction set; the URL parameters are unchanged.
- [ ] AC-09 (always): While ratings are shown, IMDb's attribution with the date of the ratings data stands once on the list page, below the list and the "Weitere laden" control, not between the controls and the list.
- [ ] AC-10 (state): While the window is narrow (below 768 px), the toolbar wraps without horizontal scrolling down to 360 px.
  - The search field takes the full width.
  - The filter panel shows its dropdowns in one or two columns.
  - The chips wrap.
- [ ] AC-11 (always): The toolbar, the open filter panel and the chips meet WCAG 2.1 AA in both palettes (the axe scans of the list pages run with the panel closed and open and with chips shown), every control is reachable by keyboard with a visible focus, and every icon-only control has a German name.
- [ ] AC-12 (always): The speed limits of web-ui-library AC-04 and imdb-ratings AC-11 still hold (the perf test with 2,500 movies), and opening a list page directly transfers at most 200 KB of JavaScript and CSS (web-ui-shell AC-16).
- [ ] AC-13 (state, manual: visual design): While either palette is active, the movie and series lists look complete at 1440 px and 360 px with the filter panel closed and open and with chips shown: nothing overlapping, the toolbar on one row at 1440 px.

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
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |

## Decisions

- 2026-11-17: The list redesign of the plan's spec 2 is split into this spec and `ui-list-items`, because together they exceed one M block.
- 2026-11-17: The filter panel is closed at every load and its state is not remembered; active filters always show as chips (owner).
