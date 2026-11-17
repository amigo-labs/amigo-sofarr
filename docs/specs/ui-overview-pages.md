---
status: todo
created: 2026-11-17
size: M
design: [sofarr.md §3, sofarr.md §9 D24, sofarr.md §9 D30]
media: both
oracle: none
---

# UI redesign 7: overview and the management pages

## Context

After the shell, the lists, the detail pages and the calendar, the remaining pages still have the old arrangement:

- **Übersicht**: four equal boxes in an auto-fit grid.
- **Fehlend**: its own tabs and buttons.
- **Blocklist, Backups, Untertitel and System**: each places heading, count and main action in its own way.
- **Login page**: a form at the left edge of the page.

The redesign series (D30) gives them one page header and calmer rows. The overview also gets a row of key figures that says at a glance whether anything needs attention.

What the pages show and do stays as specified:

- web-ui-library AC-01, AC-08, AC-18, AC-19
- web-ui-shell AC-04, AC-05, AC-12
- subtitles AC-11, AC-13, AC-15
- backups

Terms:

- **Management pages**: Fehlend, Blocklist, Untertitel, Backups and System.
- **Page header**: the row with a page's heading, its count where it has one, and its main action where it has one.
- **Key figures**: the tiles at the top of the overview, each a number with a label.
- **List row**: one entry of a list on these pages (a missing item, a blocklist entry, a backup, a needed or found subtitle, an activity, a queue item).

## Scope

### In scope

- The page header of the overview and the management pages.
- The key figures and the two-column layout of the overview.
- The tabs and the type switch on Fehlend.
- The look of list rows.
- The system page's overview facts.
- The login page's layout.

### Out of scope

- Discover — because it is `ui-discover-polish`.
- New figures that need new API data (e.g. library size on the overview) — because the overview's API answer carries the queue, the activity, the problems and the next dates, and the figures come from those.
- The error page — because its layout (heading, message, action) already fits.

## Acceptance criteria

- [ ] AC-01 (always): On the overview and every management page, the page header shows the heading as the first row of the page.
  - Where the page has a count (Fehlend, Blocklist), the count sits in the same row, right after the heading.
  - Where the page has a main action (Backups: „Sicherung erstellen“), the action sits at the end of the same row on windows from 768 px.
  - Below 768 px the action wraps below the heading.
- [ ] AC-02 (always): The overview shows three key figures above its boxes, each a number above a label, in this order:
  1. „Warteschlange“ (the number of items in the queue box)
  2. „Probleme (7 Tage)“ (the number of entries in the problems box)
  3. „Nächste 7 Tage“ (the number of release and air dates in the next 7 days, i.e. entries, not days)

  Each tile links to its box on the same page. While the overview loads for the first time, no figures show (the loading indicator of today); after a failed first load, the page error of today shows instead.
- [ ] AC-03 (state): While the number of problems is above 0, the problems figure is shown in the danger tone (the danger status colours, as a failed status message has them); otherwise all figures are neutral.
- [ ] AC-04 (state): While the window is at least 1200 px wide, the overview's boxes sit in two columns: Warteschlange and Die nächsten 7 Tage on the left, Probleme der letzten 7 Tage and Letzte Aktivität on the right. Each column stacks its boxes by their own heights. Below 1200 px they stand in one column in that order.
- [ ] AC-05 (always): On Fehlend:
  - The two lists „Fehlend“ and „Cutoff nicht erreicht“ are tabs in the shared tab look: the selected tab has the accent underline and `aria-selected="true"`.
  - „Filme“ / „Episoden“ is a segmented control (the shared look, as the theme switch) whose pressed button carries `aria-pressed="true"`.
  - Behavior and address parameters stay as today.
- [ ] AC-06 (always): A list row on the management pages and the overview shows its title (a UI link where it leads to an item) in semibold, its details in small secondary text below, and its actions at the right end of the row on windows from 768 px.
  - Rows get the shared hover ground while the pointer is over them. The day groups of „Die nächsten 7 Tage“ are not list rows (their entries are) and get no hover.
  - Icon-only actions are at least 32 × 32 px.
- [ ] AC-07 (always): The system page's „Überblick“ shows version, mode and trailers as terms and values, each term small above its value, on the box's ground in up to three columns.
- [ ] AC-08 (always): The login page shows its form as a card centred horizontally, at most 400 px wide, with the sofarr icon above the heading; the login behavior of web-ui-shell AC-04 and AC-05 stays.
- [ ] AC-09 (always): The overview, the management pages and the login page meet WCAG 2.1 AA in both palettes at 1440 px and 360 px: new axe scans of each of these pages in both palettes and both widths report no violation, and nothing scrolls sideways at 360 px.
- [ ] AC-10 (state, manual: visual design): While either palette is active, the overview, Fehlend, Blocklist, Untertitel, Backups, System and the login page look complete at 1440 px and 360 px: one header pattern, calm rows, the key figures readable at a glance.

## Technical notes

- **Page header:** a `PageHeader.svelte` with the props `title`, an optional `count` (text with the existing `data-testid`s) and an `actions` snippet. The pages keep their test ids (`wanted-total`, `blocklist-total`).
- **Key figures:** computed in the page from the overview answer (the queue's items, the problems and the upcoming entries); each tile is an `a href="#…"` to its box's heading id.
- **Tabs:** `am-tabs` look with the existing `role="tablist"`/`role="tab"`.
- **Rows:** the shared `.box-row` gets the hover ground (`am-hover`) where it holds a list entry (`li.box-row`); rows lay out as title and details on the left, actions on the right.
- **System:** `facts facts-compact`.
- **Login:** the existing `section.login` is centred with `max-width: 400px` and the brand icon above the heading.
- **Tests:** Playwright in a new `ui/e2e/pages-redesign.spec.ts`; existing selectors and test ids stay.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Empty queue, no problems, no dates | Figures 0, 0, 0, all neutral; the boxes show their empty texts of today |
| 25 queue items | The figure says 25 |
| The overview's refresh fails | The figures keep the last shown values with the notice of today (web-ui-shell AC-13) |
| Backups page on a 360 px window | „Sicherung erstellen“ below the heading, full width or its own width; nothing scrolls sideways |
| Login while throttled | The countdown notice of web-ui-shell AC-05 inside the card |
| A page is loading for the first time, or its first load fails | The page header shows the heading (without count); below it the loading indicator or the page error of today |
| The overview or system page refreshes slowly | „Wird aktualisiert …“ stays in the page header row, after the heading |
| A list row with a very long release name | Wraps inside the row; the actions stay at the end |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Three figures from the overview answer are enough | They answer „is anything waiting, failing or coming“ without new API data | Low |
| 1200 px for two overview columns | The same breakpoint as the detail pages' boxes (`ui-detail-boxes`) | Low |

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

## Decisions

- 2026-11-17: The key figures come from the overview answer only, so the overview needs no new API data.
