---
status: todo
created: 2026-11-17
size: M
design: [sofarr.md §3, sofarr.md §9 D24, sofarr.md §9 D30]
media: both
oracle: none
---

# UI redesign 3: cards and rows of the movie and series lists

## Context

Second half of the list redesign (D30; the first half is `ui-list-toolbar`). In the movie and series lists, every title shows today:

- its title as an underlined link (changed by `ui-shell-redesign`)
- the year, then the quality or "fehlt" as plain text
- "IMDb 7,4 (24.000 Stimmen)" over two lines
- a checkbox and an outlined "Überwachen" button under every poster

The grid reads busy, and the poster, which carries the card, gets less weight than the controls.

It is built after `ui-shell-redesign` and `ui-list-toolbar` on the same branch and uses their status chips (`Label`, tone accent) and toolbar. This spec makes cards and rows quieter. The quality becomes a chip, the rating becomes compact, monitoring becomes an icon on the poster, and the selection checkbox shows when it is needed. What the lists show and do stays as `web-ui-library` (AC-02, AC-06, AC-10, AC-21, AC-22) specifies it.

The owner chose (2026-11-17) to show the rating as „★ 7,4“ with the votes only in its tooltip and accessible name. That revises `imdb-ratings` AC-04 for the two library lists, in the same change. The detail and discover pages keep showing the votes.

Terms:

- **Card**: one title in the poster grid.
- **Row**: one title in the list view.
- **Item**: a card or a row.
- **Selection mode**: at least one item of the list is selected (the "Auswahl" bar of web-ui-library AC-10 shows).
- **Hover device**: a device whose main pointer can hover (`(hover: hover)`). Other devices are **touch devices**.

## Scope

### In scope

- What a card and a row show, and how: title, year, quality or missing state, labels and rating.
- The monitor toggle as an icon on the poster (card) or at the end of the row.
- When the selection checkbox shows.
- The revision of `imdb-ratings` AC-04 for the library lists.

### Out of scope

- The toolbar, filters and attribution — because they are `ui-list-toolbar`.
- The detail pages and the discover cards — because they are `ui-detail-redesign` and `ui-pages-redesign`; they keep the full rating text until then.
- The bulk actions themselves (the "Auswahl" bar, its dialogs) — because they work and only their trigger visibility changes here.
- Poster sizes and the number of grid columns — because the current auto-fill grid already fills 1440 px with seven columns.

## Acceptance criteria

- [ ] AC-01 (always): A card shows its poster (or the placeholder of web-ui-library AC-21), and below it the title (a UI link), the year, the subtitle where one exists, and one status chip.
  - The chip shows the quality in the accent tone, „fehlt“ in the danger tone, or for a series „x von y Dateien“ in the neutral tone.
  - The labels of today (e.g. „Cutoff nicht erreicht“, „3 fehlen“, „komplett“) follow as chips in their tone.
  - A movie without a file shows „fehlt“ once, not twice: the status chip replaces today's „fehlt“ label.
  - While ratings are shown, the rating (AC-03) follows the chips.
- [ ] AC-02 (always): A row shows a small poster, the title with year and subtitle in one line, and the same chips as the card in a second line.
  - From 768 px wide, the size (movies), the rating and the monitor toggle sit at the end of the row, right-aligned.
  - Below 768 px, the rating follows the chips and the toggle stays at the row's end.
  - Each row is at most 64 px high at 1280 px, as long as its title line fits one line.
  - A selected row has the `accent-subtle` ground.
- [ ] AC-03 (state): While ratings are shown, each item shows its rating as „★ 7,4“, or „–“ without a rating.
  - The rating's tooltip and accessible name are the full text „IMDb 7,4 (24.000 Stimmen)“, or „IMDb –“.
  - This revises imdb-ratings AC-04 for the movie and series lists.
- [ ] AC-04 (always): The monitor toggle of an item is an icon button with the German name „Überwachen „{Titel}““.
  - It is pressed (`aria-pressed="true"`) while the item is monitored.
  - Its icon is a filled bookmark while monitored and an outline bookmark otherwise.
  - Its tooltip says the state and what a press does, as today.
  - On a card it sits in the poster's top right corner on its own ground, against which the icon has a contrast of at least 3:1 in both palettes; in a row it sits at the row's end.
  - Pressing it saves at once, as web-ui-library AC-10 requires.
- [ ] AC-05 (state): While no item is selected, on a hover device an item's selection checkbox is visible only while the pointer is over the item or the focus is inside it; on a touch device it is always visible.
  - The checkbox keeps its name „„{Titel}“ auswählen“ and stays in the Tab order, so keyboard users reach it.
  - On a card it sits in the poster's top left corner.
- [ ] AC-06 (state): While in selection mode, every item shows its selection checkbox, and a selected card shows an accent outline around its poster. When the last item is deselected, the checkboxes hide again except where the pointer or focus is.
- [ ] AC-07 (always): Monitored and unmonitored items look different without colour alone: an unmonitored item (card or row) shows its poster dimmed and its toggle as an outline bookmark; the toggle and the checkbox themselves are not dimmed.
- [ ] AC-08 (state): While the window is narrow (below 768 px), cards and rows fit 360 px without horizontal scrolling, every control stays reachable, and the touch targets of the toggle and the checkbox are at least 32 × 32 px.
- [ ] AC-09 (always): The lists meet WCAG 2.1 AA in both palettes in list and poster view, with an item selected and on narrow screens (the axe scans of the list pages), and every item's controls are reachable by keyboard with a visible focus.
- [ ] AC-10 (always): The speed limits of web-ui-library AC-04 and imdb-ratings AC-11 still hold (the perf test with 2,500 movies).
- [ ] AC-11 (state, manual: visual design): While either palette is active, the movie and series lists look complete at 1440 px and 360 px in list and poster view, with and without a selection: posters carry the grid, nothing overlaps, the chips and icons read clearly over light and dark posters.

## Technical notes

- **Card layout:**
  - The poster is the card's top with an 8 px corner (`radius-md`).
  - The toggle and checkbox are absolutely placed over it on a `surface` ground at about 85 % (a mix of `surface` and transparent, as the dialog backdrop does).
  - The dimmed poster of AC-07 uses `opacity` around 0.55.
- **Rating:**
  - `ratingCompactText` next to `ratingWithVotesText` in `ui/src/lib/imdb/imdb.ts`, unit-tested.
  - `rows.ts` carries both texts.
  - `data-testid="rating"` stays on the element; its accessible name carries the full text (`aria-label` plus `title`).
- **Monitor toggle:** `MonitorButton.svelte` gets an `icon` variant for the lists. The detail pages keep the variant with text until `ui-detail-redesign`.
- **Chips:** `Label` with the tone `accent` from `ui-shell-redesign`; `rows.ts` returns the status chip as the first label, so `file` as plain text goes.
- **Hover:** done in CSS with `@media (hover: hover)` and `:focus-within`, plus a `selecting` class on the list while in selection mode.
- **Tests:**
  - Playwright: `ui/e2e/library-lists.spec.ts`, which today reads „Bluray-1080p“, „fehlt“ and the outlined border at 360 px, and a new `ui/e2e/library-items.spec.ts`.
  - The checks of imdb-ratings AC-04 for the lists move from the visible text to the accessible name.
  - Hover devices are emulated with the default desktop project; touch with `hasTouch`/`isMobile` (`devices['Pixel 7']`).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A very long title in a card | Wraps up to three lines, then ends with "…"; the full title is the link's text for screen readers and its tooltip |
| A movie with a file and „Cutoff nicht erreicht“ | Quality chip (accent) and the cutoff chip (neutral) |
| A series with no episodes yet | Status chip „0 von 0 Dateien“ in neutral |
| A poster that fails to load | Placeholder (AC-21); toggle and checkbox still sit on it |
| Ratings off | No rating, nothing in its place (imdb-ratings AC-09) |
| Saving the toggle fails | As today: the state goes back and the notice of web-ui-library says so |
| Keyboard user tabs into a card | The checkbox shows with focus; Tab order: title link, checkbox, toggle |
| A click on the poster or title in selection mode | The title link opens the item as today; the poster is not a link; only the checkbox selects |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| A dimmed poster marks "not monitored" well enough | Paired with the outline icon, it doesn't rely on colour alone | Medium: a user may miss it; the icon stays |
| 64 px rows | A 36 px poster thumbnail (2:3 → 54 px high) plus 4 px padding; the title line (20 px) and the chip line (22 px) fit beside it | Low |
| Touch devices always show the checkbox | They cannot hover; a long press would be hidden behavior | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `cards (AC-01, AC-04)` › `a card shows the quality chip in accent, "fehlt" once, labels after it`, `a series card shows its files as a neutral chip first` (`ui/e2e/library-items.spec.ts`); `list rows (AC-02, AC-06)` (`ui/src/lib/library/rows.test.ts`) | built |  |
| AC-02 | `rows (AC-02)` › `a row is at most 64 px high; rating and toggle at its end`, `a selected row has the accent-subtle ground` (`ui/e2e/library-items.spec.ts`); `cards on narrow screens …` (`ui/e2e/library-lists.spec.ts`) | built |  |
| AC-03 | `every movie shows rating and votes, "–" without; attribution with the date`, `every series shows rating and votes, "–" without`, `cards on narrow screens show the rating` (`ui/e2e/imdb-ratings.spec.ts`, now `★ x,y` with the full text as name and tooltip); `shows the compact rating of the library lists` (`ui/src/lib/imdb/imdb.test.ts`); `rows carry the compact rating and the full text` (`rows.test.ts`) | built |  |
| AC-04 | `cards (AC-01, AC-04)` › `the monitor toggle is an icon in the top right corner of the poster`, `the toggle's icon has 3:1 against its own ground (AC-04, dark/light)` (`ui/e2e/library-items.spec.ts`); `monitoring from the list (AC-10)` (`ui/e2e/library-lists.spec.ts`) | built |  |
| AC-05 | `selection checkbox on a hover device (AC-05, AC-06)` › `shows on hover and focus only, stays in the Tab order`; `touch device (AC-05, AC-08)` (`ui/e2e/library-items.spec.ts`) | built |  |
| AC-06 | `selection checkbox on a hover device (AC-05, AC-06)` › `in selection mode every checkbox shows; a selected card is outlined` (`ui/e2e/library-items.spec.ts`) | built |  |
| AC-07 | `monitored or not (AC-07)` › `an unmonitored poster is dimmed, its controls are not` (`ui/e2e/library-items.spec.ts`) | built |  |
| AC-08 | `touch device (AC-05, AC-08)` › `checkboxes always show; targets are at least 32 px; nothing scrolls sideways` (`ui/e2e/library-items.spec.ts`); `narrow screens …` (`ui/e2e/library-lists.spec.ts`) | built |  |
| AC-09 | `axe, dark theme` and `axe, light theme` › `list view with a selection, poster grid on a narrow screen (ui-list-items AC-09)`, `poster grid with a selection and the bulk dialog`, `Filme`, `Serien` (`ui/e2e/library-a11y.spec.ts`) | built |  |
| AC-10 | `movie list speed (AC-04)` (`ui/e2e/library-perf.spec.ts`) | built |  |
| AC-11 |      | open   |        |

## Decisions

- 2026-11-17: The rating shows as „★ 7,4“ with the votes in tooltip and accessible name (owner); this revises imdb-ratings AC-04 for the library lists.
- 2026-11-17: The monitor toggle is an icon in the poster's corner, the checkbox shows on hover, focus or in selection mode, and always on touch devices (owner).
- 2026-11-17 (build): Red runs against 30680b4: `bunx vitest run src/lib/library/rows.test.ts src/lib/imdb/imdb.test.ts` failed 4 of 11 (no `ratingCompactText`, no status chip, no `ratingFull`); `bunx playwright test e2e/library-items.spec.ts` failed 9 of 11. Green at once: the toggle's 3:1 contrast in both palettes, because today's outlined button already had it; it stays as the check for the new ground.
- 2026-11-17 (build): The checkbox is hidden with `opacity: 0` (not `display`/`visibility`), so it stays in the Tab order and in the accessibility tree; the tests measure its opacity, because Playwright counts a transparent element as visible.
- 2026-11-17 (build): The rating is a `span` with `role="img"`, its `aria-label` and `title` the full text, because `aria-label` on a span without a role is not allowed.
- 2026-11-17 (build): `Row.file` went; the status chip is the first entry of `Row.labels`. A movie without a file shows „fehlt“ whether or not it counts as missing, as the plain text did before.
- 2026-11-17 (build): New public API: `ratingCompactText` (`ui/src/lib/imdb/imdb.ts`), `Row.ratingFull`, the `icon` variant of `MonitorButton`.

