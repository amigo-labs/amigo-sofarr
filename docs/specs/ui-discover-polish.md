---
status: todo
created: 2026-11-17
size: M
design: [sofarr.md §3, sofarr.md §9 D19, sofarr.md §9 D30]
media: both
oracle: none
---

# UI redesign 8: discover and the new screenshots

## Context

Discover (`/entdecken`, spec `discover`) still opens with six labelled filter fields between the search and the first shelf. Its shelves can only be scrolled with a trackpad or the scrollbar. Its preview page has none of the head the library pages got in `ui-detail-head`. This last spec of the redesign series (D30):

- brings the list toolbar's pattern to discover: filters in a panel, active ones as chips
- gives the shelves arrow buttons
- gives the preview page the detail head's backdrop and compact facts, here with the title's real TMDB backdrop, which the preview answer already carries
- regenerates the website's screenshots, which still show the old look

What discover shows and does stays as `discover` AC-01 to AC-05 and AC-14, AC-15 specify it.

Terms:

- **Discover filters**: Art, Genre, Jahr von, Jahr bis, Originalsprache, IMDb-Bewertung (the last only while ratings are shown). The search text is not a filter.
- **Shelf**: one titled row of titles on the discover start page.
- **Preview page**: the page of a title that is not (or not yet) in the library (`/entdecken/filme/{id}`, `/entdecken/serien/{id}`).
- **Screenshots**: the images in `docs/site/screenshots/` that amigo-labs.dev shows.

## Scope

### In scope

- The discover start page: search row, filter panel, chips, shelf arrows.
- The preview page's head.
- New screenshots.

### Out of scope

- The search results, shelf and person pages — because their lists already use the title tiles and the people list; only the start page's controls change.
- New filters or shelves — because the redesign changes the look only.

## Acceptance criteria

- [ ] AC-01 (always): The discover start page shows the search field, its „Suchen“ button and a filter button in one row.
  - The filter button reads „Filter“, or „Filter (n)“ while n discover filters are active.
  - It opens and closes a panel with the discover filters of today (labels, choices, „Filter zurücksetzen“), marked `aria-expanded`.
  - The panel is closed whenever the page loads.
- [ ] AC-02 (state): While a discover filter is active, a chip names it and its value (e.g. „Art: Filme“, „Jahr von: 1990“, „Originalsprache: Dänisch“, „IMDb-Bewertung: ab 7,0“), with the panel open or closed.
  - Pressing a chip removes that filter as its field's empty choice would.
  - The address parameters stay as today (discover AC-02).
- [ ] AC-03 (always): Every shelf has two arrow buttons beside its heading, named „Zurück in „{Regal}““ and „Weiter in „{Regal}““.
  - Each scrolls the shelf by about its visible width.
  - The back button is disabled at the shelf's start and the forward button at its end.
  - The page itself never scrolls sideways.
- [ ] AC-04 (state): While a preview's title has a backdrop image, the preview page's head shows it blurred behind the head under the page ground at an opacity of at least 70 %, as the library head does (ui-detail-head AC-01); without one, the head has the plain page ground. The facts of the preview head are the compact panel of ui-detail-head AC-03.
- [ ] AC-05 (state): While the window is narrow (below 768 px), the search row wraps, the filter panel shows its fields in at most two columns, the arrow buttons stay reachable, and nothing scrolls sideways at 360 px.
- [ ] AC-06 (always): The discover start page with the panel open and chips shown, and a preview page with a backdrop, meet WCAG 2.1 AA in both palettes at 1440 px and 360 px (new axe scans), and every control is reachable by keyboard with a visible focus.
- [ ] AC-07 (always): The screenshots in `docs/site/screenshots/` show the redesigned UI: they are regenerated with `bun run screenshots` from this spec's build. The set:
  - discover
  - movie list
  - movie page
  - series page
  - calendar month
  - person page
  - overview (new)
- [ ] AC-08 (state, manual: visual design): While either palette is active, the discover start page and a preview page look complete at 1440 px and 360 px, and the new screenshots read well on amigo-labs.dev.

## Technical notes

- **Chips:** a pure, unit-tested `discoverChips(filters, genres, ratings)` next to `parseFilters` in `ui/src/lib/discover/filters.ts`, the same shape as `filterChips` of the library lists (`ui/src/lib/library/filter-chips.ts`).
- **Shelf arrows:** `scrollBy({ left: ±clientWidth * 0.9 })` on the strip; the disabled state follows the strip's `scroll` event.
- **Preview backdrop:** `TitlePreview.backdrop` is a cached image address of sofarr (w1280; D24 serves it), so no request leaves sofarr.
- **Screenshots:** `ui/e2e/site/screenshots.mjs` gains the overview.
- **Tests:** Playwright in a new `ui/e2e/discover-polish.spec.ts` against the discover server; the existing discover tests keep their selectors (`shelf`, `title-card`, the labels, „Filter zurücksetzen“).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A shelf that fits the window | Both arrows disabled |
| A genre id from the address that is not offered | The chip shows the id |
| Ratings off with `bewertung` in the address | No rating chip, not counted |
| The preview backdrop fails to load | No backdrop, plain ground |
| Discover without a TMDB key | The notice of discover AC-15 as today, no filter button |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The overview is a worthwhile new screenshot | It shows the key figures and is the first page after login | Low |

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

## Decisions

- 2026-11-17: The preview page uses the title's real TMDB backdrop, because its API answer already carries one; the library pages keep the blurred poster (ui-detail-head).
