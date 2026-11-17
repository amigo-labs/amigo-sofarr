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

- [ ] AC-01 (state): While discover is available (a TMDB key is set, discover AC-15), the discover start page shows the search field, its „Suchen“ button and a filter button in one row.
  - The filter button reads „Filter“, or „Filter (n)“ while n discover filters are active (each filter with a value from the address counts, also a genre id that is not offered).
  - It opens and closes a panel with the discover filters of today (labels, choices, „Filter zurücksetzen“), marked `aria-expanded`.
  - The panel is closed whenever the page loads.
- [ ] AC-02 (state): While a discover filter is active, a chip names it and its value (e.g. „Art: Filme“, „Jahr von: 1990“, „Originalsprache: Dänisch“, „IMDb-Bewertung: ab 7,0“), with the panel open or closed.
  - Pressing a chip removes that filter as its field's empty choice would.
  - The address parameters stay as today (discover AC-02).
  - After a chip is removed, the focus moves to the next chip, else the one before it, else the filter button (as ui-list-toolbar AC-06).
  - Removing „Jahr von“ keeps „Jahr bis“ and the other way round.
- [ ] AC-03 (always): Every shelf that shows titles has two arrow buttons beside its heading (an empty or failed shelf shows its text of today and no arrows), named „Zurück in „{Regal}““ and „Weiter in „{Regal}““.
  - Each scrolls the shelf by about its visible width.
  - The back button is disabled at the shelf's start and the forward button at its end.
  - The page itself never scrolls sideways.
- [ ] AC-04 (state): While a preview's title has a backdrop image and it has loaded, the preview page's head shows it blurred behind the head under the page ground at an opacity of at least 70 %, fading into the page ground at its lower edge, as the library head does (ui-detail-head AC-01); without one, or when it fails to load, the head has the plain page ground. The facts of the preview head are the compact panel of ui-detail-head AC-03.
- [ ] AC-05 (state): While the window is narrow (below 768 px), the search field takes the full width with „Suchen“ and the filter button on the next line, the filter panel shows its fields in at most two columns, the arrow buttons stay visible beside each shelf heading, and nothing scrolls sideways at 360 px.
- [ ] AC-06 (always): The discover start page with the panel open and chips shown, and a preview page with a backdrop, meet WCAG 2.1 AA in both palettes at 1440 px and 360 px (new axe scans), and every control is reachable by keyboard with a visible focus.
- [ ] AC-07 (always, manual: images): The screenshots in `docs/site/screenshots/` show the redesigned UI (sidebar, chips, the new heads and the month grid), taken from this spec's build; the discover shot shows the start page with its panel closed. The set:
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
| Discover without a TMDB key | The notice of discover AC-15 as today; no search row, filter button or shelves |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The overview is a worthwhile new screenshot | It shows the key figures and the next dates, and is the first page after login; the demo has no queue or activity, so those boxes show their empty texts | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `search row and filter panel (AC-01, AC-02)` › `search, Suchen and the filter button in one row; the panel closed at load` (`ui/e2e/discover-polish.spec.ts`); `filters (AC-02)` (`ui/e2e/discover-pages.spec.ts`, through the open panel) | built |  |
| AC-02 | `search row and filter panel (AC-01, AC-02)` › `chips name the filters; pressing one removes it and moves the focus` (`ui/e2e/discover-polish.spec.ts`); `discover chips` (`ui/src/lib/discover/discover-chips.test.ts`) | built |  |
| AC-03 | `shelf arrows (AC-03)` › `back and forward scroll the shelf; disabled at its ends` (`ui/e2e/discover-polish.spec.ts`) | built |  |
| AC-04 | `preview head (AC-04)` › `the backdrop blurred under at least 70 % of the page ground; compact facts`, `no backdrop when it fails to load` (`ui/e2e/discover-polish.spec.ts`) | built |  |
| AC-05 | `narrow window (AC-05)` › `the search takes the full width, two filter columns, arrows visible` (`ui/e2e/discover-polish.spec.ts`) | built |  |
| AC-06 | `axe: start page with panel and chips, preview with backdrop (AC-06, dark/light)` (`ui/e2e/discover-polish.spec.ts`); `ui/e2e/discover-a11y.spec.ts` | built |  |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |

## Decisions

- 2026-11-17: The preview page uses the title's real TMDB backdrop, because its API answer already carries one; the library pages keep the blurred poster (ui-detail-head).
- 2026-11-17 (build): Red runs against 66b2ea7: `bunx vitest run src/lib/discover/discover-chips.test.ts` failed 4 of 4 (no `discoverChips`); `bunx playwright test e2e/discover-polish.spec.ts` failed 7 of 8. Green at once: „no backdrop when it fails to load“ (there was no backdrop); kept as a guard.
- 2026-11-17 (build): The narrow-window test then found that the search field shared its row with „Suchen“ below 768 px; the search form now wraps there, with the field on its own line.
- 2026-11-17 (build): The existing filter tests of `ui/e2e/discover-pages.spec.ts` open the panel first (`openDiscoverFilters` in `ui/e2e/discover-helpers.ts`) and look the fields up inside the form „Filter“, because the chips' names („Filter Art: Serien entfernen“) also contain the labels.
- 2026-11-17 (build): A first full run failed the person page's rating sort and an a11y scan of discover too; both depend on the fake TMDB releasing a held-back rating, which the failing filter test had not done. With the filter tests fixed they pass (482 of 482).
- 2026-11-17 (build): `ui/e2e/site/screenshots.mjs` gains the overview; the seven screenshots in `docs/site/screenshots/` were regenerated with `bun run screenshots` from this build.

