---
status: implemented
created: 2026-11-17
size: M
design: [sofarr.md §3, sofarr.md §9 D24, sofarr.md §9 D30]
media: both
oracle: none
---

# UI redesign 5: boxes and seasons of the movie and series pages

## Context

Below the head, the detail pages pour up to seven boxes into an auto-fit grid. The order of reading and the size of each box depend on the window width: the file can land next to the trailer, and the settings next to the grabs. Every box header has its own ground and a 20 px heading, long file paths overflow their column, and a season repeats its monitor and search actions as a separate row with large text buttons (screenshot of 2026-11-16).

This is the second half of the detail redesign (D30). It builds on `ui-detail-head` on the same branch. It gives the boxes a fixed two-column layout, quieter headers, readable file paths, and compact seasons and episodes.

What the boxes show and do stays as specified:

- `web-ui-library` AC-05, AC-06, AC-09 to AC-17
- `subtitles` AC-12
- `trailers` AC-09, AC-10

Terms:

- **Main boxes**: the content of the title.
  - Movie: Datei, Untertitel, Grabs, Blocklist.
  - Series: the seasons (Staffeln), then Grabs and Blocklist.
- **Side boxes**: what one sets or does: Einstellungen, Ordner und Dateiaktionen, Trailer.
- **Season header**: the always visible line of a season that opens and closes it.
- **Episode row**: one episode inside an open season.

## Scope

### In scope

- The layout of the boxes below the head.
- The look of every box header on the detail pages.
- File paths in the file box.
- Season headers and episode rows.

### Out of scope

- The head and the cast — because they are `ui-detail-head`.
- New actions (e.g. an interactive search with release choice) — because the redesign changes the look, not what can be done.
- The dialogs (remove, rename, move) — because they already use the shared dialog and buttons.

## Acceptance criteria

- [x] AC-01 (state): While the window is at least 1200 px wide, the boxes below the head and the cast (the cast stays full width between head and boxes) sit in two columns: the main boxes in a wider left column (about two thirds) and the side boxes in a right column.
  - Each column keeps its order whatever the boxes' heights.
  - Below 1200 px, all boxes stand in one column: the main boxes first, then the side boxes.
- [x] AC-02 (always): Every box on a detail page (including „Besetzung“, „Grabs“ and „Blocklist“; the season headers are AC-04, and „Staffeln“ stays a section heading outside any box) has a header line with its heading in 16 px semibold on the box's own `surface` ground (no separate header ground), separated from the content by a hairline.
- [x] AC-03 (always): A file path in the file box is set in the monospace font, wraps inside its box at any character instead of overflowing, and carries the full path as its tooltip.
- [x] AC-04 (always): A season header shows:
  - the season's name
  - a progress bar of episodes with a file out of all episodes, with its numbers as text („x von y“) and the counts of today in `season-counts`
  - the labels („N fehlen“ bzw. „1 fehlt“, „N unter Cutoff“)
  - the season's monitor toggle and its search action as icon buttons at the line's end, with their German names of today („Überwachen „{Serie} – Staffel N““, „Staffel N suchen“)
- [x] AC-05 (event): When the user activates a season header (click, Enter or Space on it), then the season opens or closes, and a chevron in the header shows the state.
  - Its toggle and search buttons do not open or close the season.
  - Which seasons are open at load stays as today (those whose missing count from the API is above 0, and season 1).
- [x] AC-06 (always): An episode row shows, on one line from 768 px as long as its column is wide enough (otherwise wrapping onto a second line):
  - the episode code
  - the title
  - the air date („Ausgestrahlt …“ or „Sendetermin unbekannt“)
  - a status chip: the quality in the accent tone while it has a file; „fehlt“ in the danger tone while it is missing (as the API counts it); otherwise „keine Datei“ in the neutral tone
  - the other labels of today (e.g. „Cutoff nicht erreicht“); the status chip replaces today's „fehlt“ label, so „fehlt“ shows once
  - the monitor toggle and the search action as icon buttons with their German names of today

  The disclosures „Datei“ and „Untertitel“ of an episode with a file stay below the line.
- [x] AC-07 (state): While the window is narrow (below 768 px), boxes, season headers and episode rows fit 360 px without horizontal page scrolling, and the icon buttons are at least 32 × 32 px.
- [x] AC-08 (always): Both detail pages meet WCAG 2.1 AA in both palettes with seasons open and closed, at 1440 px and 360 px (the axe scans of the library pages), and every control is reachable by keyboard with a visible focus.
- [x] AC-09 (state, manual: visual design): While either palette is active, a movie page with a file and a series page with several seasons look complete at 1440 px and 360 px: columns balanced, headers quiet, file paths readable, seasons scannable.

## Technical notes

- **Layout:** `MoviePage`/`SeriesPage` wrap their boxes in two column containers (`.detail-main`, `.detail-side`) inside a grid with `grid-template-columns: minmax(0, 2fr) minmax(0, 1fr)` from 1200 px; below that, one column.
- **Box header:** `.box-header` in `app.css` takes the `am-panel > header` look (no ground, 16 px semibold heading, hairline). Box headings on other pages change with it; `ui-calendar`, `ui-overview-pages` and `ui-discover-polish` relies on that.
- **Season:**
  - stays a `<details>`
  - the summary holds name, progress (`am-progress`), counts, labels and a chevron
  - the action buttons are not inside the summary (a button in a summary toggles the details in some browsers); they sit in a container next to it, positioned over the end of the header line.
  - `MonitorButton` with `icon`, and the search as an icon button with `aria-label` „Staffel N suchen“.
- **Episode:** `Label` chips; `MonitorButton icon`; the existing icon search button.
- **Tests:** Playwright in a new `ui/e2e/library-detail-boxes.spec.ts`; the existing tests keep their selectors (`[data-episode]`, `season-counts`, names of toggles and buttons, `summary` of the disclosures).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A movie without file | The file box says „fehlt – es gibt noch keine Datei.“ as today; the columns stay |
| Discover/trailers off, so no trailer box | The side column holds the remaining boxes |
| A season with 0 episodes | Progress „0 von 0“, an empty bar, no labels |
| Specials (season 0) | Named „Specials“, as today |
| A season with 100 episodes | The open season lists them all; the page scrolls, not the box |
| A path without spaces or slashes, 300 characters | Wraps inside the box |
| A season or episode search is started from its icon button | The button is disabled while the search runs; the result shows as the notice of today (web-ui-library AC-09) |
| Shadow mode | The file actions note of web-ui-library AC-17 shows in the side box as today |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 1200 px for two columns | From 1024 px the sidebar shows (240 px, 56 px collapsed); at 1200 px that leaves about 900 px: about 600 for main boxes and 300 for side boxes | Low |
| Main boxes first on narrow windows | The content of the title is what one opens the page for | Low |

## Summary

> Completed: 2026-11-17

- From 1200 px the boxes of the detail pages stand in two fixed columns (main boxes left, side boxes right); below that in one column, main boxes first.
- Box headers are quiet: a 16 px semibold heading on the box ground with a hairline below.
- File paths are monospace, wrap inside their box and carry the full path as tooltip.
- Season headers show name, a progress bar with „x von y“, labels, a chevron and icon buttons for monitor and search; episode rows show code, title, date, a status chip, labels and icon buttons on one line.

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `two columns (AC-01)` › `from 1200 px: main boxes left, side boxes right, each in its order`, `below 1200 px: one column, main boxes first`, `a series page: seasons in the main column, settings at the side` (`ui/e2e/library-detail-boxes.spec.ts`) | verified | a7bca22 |
| AC-02 | `box headers (AC-02)` › `16 px semibold heading on the box ground, a hairline below` (`ui/e2e/library-detail-boxes.spec.ts`); `grounds, text and hairlines are the tokens (AC-02)` (`ui/e2e/theme.spec.ts`) | verified | a7bca22 |
| AC-03 | `file path (AC-03)` › `monospace, the full path as tooltip, wraps inside its box` (`ui/e2e/library-detail-boxes.spec.ts`); `ui/e2e/library-files.spec.ts` (unchanged) | verified | a7bca22 |
| AC-04 | `seasons (AC-04, AC-05)` › `a season header: name, progress with numbers, labels, icon buttons at its end` (`ui/e2e/library-detail-boxes.spec.ts`); the season tests of `ui/e2e/library-detail.spec.ts` | verified | a7bca22 |
| AC-05 | `seasons (AC-04, AC-05)` › `the header opens and closes the season; its buttons do not; a chevron shows it` (`ui/e2e/library-detail-boxes.spec.ts`) | verified | a7bca22 |
| AC-06 | `episode rows (AC-06)` › `one line: code, title, date, status chip, labels and icon buttons` (`ui/e2e/library-detail-boxes.spec.ts`); episode tests of `ui/e2e/library-detail.spec.ts` and `ui/e2e/subtitles.spec.ts` | verified | a7bca22 |
| AC-07 | `narrow window (AC-07)` › `boxes, seasons and episodes fit; icon buttons are 32 px` (`ui/e2e/library-detail-boxes.spec.ts`) | verified | a7bca22 |
| AC-08 | `axe, dark theme` and `axe, light theme` › `series page with seasons open and closed, wide and narrow (ui-detail-boxes AC-08)`, `Film`, `Serie`, `remove dialog and settings preview` (`ui/e2e/library-a11y.spec.ts`) | verified | a7bca22 |
| AC-09 | manual: the owner checked a movie page with a file and a series page with seasons, both palettes, 1440 px and 360 px („Sichtcheck passt“), 2026-11-17 | verified | d51afe1 |

## Decisions

- 2026-11-17: Fixed two columns (main and side) instead of the auto-fit grid, so the order of reading no longer depends on the window width.
- 2026-11-17 (build): Red run against aa26a2c: `bunx playwright test e2e/library-detail-boxes.spec.ts` failed 7 of 9. Green at once: the narrow-window check (the old buttons were already 32 px and nothing scrolled) and a first, too weak version of „seasons in the main column“, which was then tightened to require the settings beside the seasons.
- 2026-11-17 (build): The season's actions sit in a container next to the `summary`, absolutely placed over the end of its first line, so a click on them never opens or closes the season; the summary reserves room for them on the right.
- 2026-11-17 (build): The progress is a `role="progressbar"` with `aria-valuenow`/`aria-valuemax` (episodes with file, all episodes) and the text „x von y“ beside it; the counts of today stay as smaller text (`season-counts`).
- 2026-11-17 (build): An episode's status chip: its quality (accent) with a file, „fehlt“ (danger) while missing, otherwise „keine Datei“ (neutral); today's separate „fehlt“ label and the „· quality“ text went.
- 2026-11-17 (build): A first `box-row` right after a `box-header` loses its top hairline (`.box-header + .box-row`), and a `p.box-row` its margin; before, the empty Grabs and Blocklist boxes showed a double line.

