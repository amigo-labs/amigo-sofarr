---
status: todo
created: 2026-11-17
size: M
design: [sofarr.md §3, sofarr.md §9 D24, sofarr.md §9 D30]
media: both
oracle: none
---

# UI redesign 6: calendar as a real week and month grid

## Context

The calendar (`/kalender`, web-ui-library AC-07) shows its days as a wall of equal cards in an auto-fill grid. In the month view only the days with entries appear, so the month has no shape. The days lose their weekday column, and every entry carries a full outlined state label (screenshot of 2026-11-16). The redesign series (D30) turns it into a calendar one can read at a glance:

- the month as a 7-column grid with weekday headings
- the week as seven columns side by side
- an agenda list on narrow windows

What the calendar shows, its address parameters and its navigation stay as web-ui-library AC-07 specifies them.

Terms:

- **Month grid**: the weeks of a month, Monday to Sunday, as rows of seven day cells.
- **Outside day**: a cell of the month grid that belongs to the previous or next month.
- **Week columns**: the seven days of a week side by side.
- **Agenda**: the days of the shown range one below the other, each with its entries.
- **Entry**: one release or air date (a movie's cinema, digital or disc date, or an episode).
- **State**: the entry's `vorhanden`, `noch nicht erschienen`, `fehlt` or `neutral` (web-ui-library AC-07).

## Scope

### In scope

- The month view as a month grid.
- The week view as week columns.
- The agenda below 768 px for both views.
- How an entry shows inside a day.
- The toolbar look.

### Out of scope

- The overview's „Die nächsten 7 Tage“ — because it belongs to `ui-overview-pages`.
- Entries of outside days — because the API is asked for the month only (web-ui-library AC-07); outside days stay empty and are only there to give the grid its shape.
- New filters (e.g. movies only) — because nothing asks for them.

## Acceptance criteria

- [ ] AC-01 (state): While the month view is shown at 768 px or wider, the calendar shows a grid of seven columns headed Mo, Di, Mi, Do, Fr, Sa, So, with one row per week that touches the month.
  - Every day of the month has a cell with its day number, whether or not it has entries; the cell's heading is named with weekday and date for screen readers („Mo, 16.11.“).
  - Outside days show their number dimmed and no entries.
  - Today's cell, also when it is an outside day, is marked with the accent and its name ends in „· heute“.
- [ ] AC-02 (state): While the week view is shown at 768 px or wider, the seven days of the week stand side by side as columns, each headed with weekday and date („Mo, 16.11.“, today with „· heute“ and the accent) and listing its entries below; a day without entries says „Nichts.“ as today. Entries in a week column wrap instead of being cut.
- [ ] AC-03 (state): While the window is narrower than 768 px, both views show the agenda.
  - Week: all seven days.
  - Month: only the days with entries, as today.
  - Each day has a heading with weekday and date (today with „· heute“ and the accent) and its entries below.
  - An empty month shows only the note, no agenda.
  - Nothing scrolls sideways at 360 px.
- [ ] AC-04 (always): An entry shows:
  - its title as a UI link (or plain text without a library item)
  - its detail (Kino, digital, Disc, Film, or „S01E03 · Titel“)
  - its state as a coloured dot in the state's tone, whose German text („vorhanden“, „noch nicht erschienen“, „fehlt“, „neutral“) is the dot's accessible name and tooltip

  In a month cell, title and detail are cut to one line each with the full text as tooltip.
- [ ] AC-05 (always): Every entry of the answer shows exactly once, in the order of the answer (web-ui-library AC-07). A movie with cinema, digital and disc dates shows in each of those days.
- [ ] AC-06 (event): When the user presses Woche, Monat, Heute or the previous and next buttons, then the calendar changes range as today: same address parameters, same names of the buttons, same range title. Woche and Monat are a segmented control (the shared `am-seg` look) with the pressed one marked.
- [ ] AC-07 (always): The calendar meets WCAG 2.1 AA in both palettes: new axe scans of the month grid and the week columns at 1440 px and of the agenda at 360 px report no violation, and every link and button is reachable by keyboard with a visible focus.
- [ ] AC-08 (state, manual: visual design): While either palette is active, the week and the month view look complete at 1440 px and 360 px: the month reads as a calendar at a glance, the week columns are balanced, the agenda is calm.

## Technical notes

- **Grid building:** `calendar.ts` gets a pure, unit-tested `monthGrid(day)` that returns the weeks of a month (Monday first) with each cell's date and whether it is an outside day. The existing `rangeOf`, `step` and `rangeTitle` stay.
- **Markup:**
  - The days stay an `ol.days` with `li[data-day]` per day (outside days: `data-outside`), so the existing tests keep finding days and entries.
  - The weekday headings are `aria-hidden` (each cell's own label names its weekday and date).
  - The agenda is the same markup with a different layout below 768 px; outside days and, in the month, empty days are hidden there.
- **Entry:**
  - `CalendarEntryLine` gets a compact form for cells.
  - The dot is a `span` with `role="img"`, `aria-label` and `title` of the state's text, coloured by the state's tone of today (`ENTRY_STATE`): `success`, `danger`, or `border-strong` for the neutral tone („noch nicht erschienen“ and „neutral“).
  - `data-state` and `data-testid="calendar-entry"` stay.
- **Overview:** the overview's „Die nächsten 7 Tage“ also uses `CalendarEntryLine` and keeps the full label form.
- **Tests:**
  - Vitest for `monthGrid` (months starting on Monday and on Sunday, February in a leap year, 4, 5 and 6 week rows).
  - Playwright in a new `ui/e2e/calendar.spec.ts`; `library-pages.spec.ts` keeps its tests.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A month starting on Monday (e.g. June 2026) | No leading outside days |
| February of a non-leap year starting on Monday | Exactly four rows |
| A month needing six rows | Six rows, the grid grows |
| Ten entries on one day | The month cell lists them all; the row grows |
| A very long title in a month cell | One line, cut with „…“, the full title as tooltip |
| An empty month | The note „In diesem Zeitraum erscheint nichts.“ above an empty grid |
| Today outside the shown range and outside the grid | No cell is marked |
| The calendar is loading | The loading indicator of today, no grid |
| Loading fails | The German error notice of today, no grid |
| The answer carries a note and entries | The note above the grid, columns or agenda |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The month grid shows outside days without entries | Their entries would need a wider API request; dimmed numbers keep the grid's shape | Low |
| 768 px for the agenda | Between 768 and 1023 px the sidebar is a drawer, so the content is about 730 px wide (≈ 100 px per column); from 1024 px the 240 px sidebar leaves about 750 px; below 768 px columns would get narrower than about 100 px | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `month grid (AC-01)` › `seven columns headed Mo .. So, one row per week, every day of the month`, `outside days are dimmed and empty; today is marked and named` (dimmed by the muted text colour) (`ui/e2e/calendar.spec.ts`); `month grid (ui-calendar AC-01)` (`ui/src/lib/library/calendar.test.ts`) | built |  |
| AC-02 | `week columns (AC-02)` › `seven days side by side, headed with weekday and date, empty days say Nichts.` (`ui/e2e/calendar.spec.ts`) | built |  |
| AC-03 | `agenda (AC-03)` › `the week stacks all seven days; the month only days with entries`, `an empty month shows only the note` (`ui/e2e/calendar.spec.ts`) | built |  |
| AC-04 | `entries (AC-04)` › `title, detail and a state dot named by its German text`; `month grid (AC-01)` › `entries in a month cell are cut to one line with the full text as tooltip` (`ui/e2e/calendar.spec.ts`) | built |  |
| AC-05 | `calendar (AC-07)` › `starts at the current week with one entry per date, marked by state`, `a movie with cinema, digital and disc dates appears three times in its month` (`ui/e2e/library-pages.spec.ts`) | built |  |
| AC-06 | `toolbar (AC-06)` › `Woche and Monat are a segmented control; the range changes as before` (`ui/e2e/calendar.spec.ts`); `steps back and forth; an empty month shows a German note` (`ui/e2e/library-pages.spec.ts`) | built |  |
| AC-07 | `axe, dark theme` and `axe, light theme` › `calendar: month grid, week columns and agenda (ui-calendar AC-07)`, `Kalender` (`ui/e2e/library-a11y.spec.ts`) | built |  |
| AC-08 | manual: the owner checked week and month view in both palettes at 1440 px and 360 px („Sichtcheck passt“), 2026-11-17 | built |  |

## Decisions

- 2026-11-17: The plan's spec 4 (remaining pages) is split into `ui-calendar`, `ui-overview-pages` and `ui-discover-polish`, because together they exceed one M block.
- 2026-11-17 (build): Red runs against f57437b: `bunx vitest run src/lib/library/calendar.test.ts` failed 4 of 10 (no `monthGrid`); `bunx playwright test e2e/calendar.spec.ts` failed 6 of 8. Green at once: the two agenda tests (the narrow grid was already one column, and the empty month already showed only the note); they stay as guards.
- 2026-11-17 (build): A month cell's heading shows only the day number on wide windows and the full „Mo, 16.11.“ in the agenda; its `aria-label` always names weekday and date (and „· heute“). Today's number sits in an accent circle.
- 2026-11-17 (build): `CalendarEntryLine` takes `dot` (state as a named dot, detail below the title) and `cut` (one line each, full text as tooltip); the overview keeps the label form.
- 2026-11-17 (build): The month grid is shown also for an empty month (with the note above), so the month keeps its shape; on narrow windows the empty cells hide and only the note remains.
- 2026-11-17 (fix): The outside days were dimmed with `opacity: 0.45`, which let axe report their numbers' contrast below AA once the cells rendered (seen as an intermittent failure of the calendar scan in a full run, reproduced 3 of 16 with `--repeat-each`). They now take the muted text colour (`text-muted`) on the page ground; the scan passed 16 of 16 repeats.
