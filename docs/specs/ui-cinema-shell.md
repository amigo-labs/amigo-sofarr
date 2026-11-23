---
status: todo
created: 2026-11-22
size: M
design: [sofarr.md §3, sofarr.md §9 D29, sofarr.md §9 D30, sofarr.md §9 D34]
media: none
oracle: none
---

# Cinema look 6: dark only, a quieter shell, flatter boxes

## Context

The last spec of the cinema look (D34) covers what the others leave: the shell, the calendar and the administration pages (Fehlend, Sperrliste, Untertitel, Neu encodieren, Backups, System).

Today:

- the sidebar is a `surface` panel with a border, and the active entry has a filled accent ground (`ui-shell-redesign` AC-02);
- the theme switch (Dunkel, Hell, System) sits at the sidebar's foot (`amigo-theme` AC-05, `ui-shell-redesign` AC-09, `web-ui-shell` AC-10);
- every box has a border, and its header a line below it;
- the month grid of the calendar draws a border around every day.

The owner decided on 2026-11-22: the theme switch goes and the UI is always dark; the light values stay in the generated theme files (D29), so the light palette could come back later.

This spec revises `amigo-theme` AC-05 to AC-08 (choosing, remembering and applying a palette; AC-02 of this spec replaces them), AC-09 and AC-13; `ui-shell-redesign` AC-02, AC-09, AC-13 and AC-15; `web-ui-shell` AC-10 and AC-11; `ui-detail-boxes` AC-02 (the header line); `ui-calendar` AC-01 (the borders of the month grid). Wherever these and later specs say „both palettes“, from now on the dark palette is meant.

## Terms

- **Shell**: the sidebar, the collapsed sidebar, the top bar and the drawer (`ui-shell-redesign`).
- **Box**: an element of the global class `box` (a section with a header and rows).

## Scope

### In scope

- The removal of the theme switch, and the dark palette at every load.
- The look of the shell, the boxes and the calendar's month and week views.

### Out of scope

- Removing the light values from `ui/src/theme/` — because they are generated from amigo-labs/website (D29) and the owner keeps them.
- New pages or new data — because this spec changes the look only.
- The title pages, tiles and rows — because they are the specs before this one.

## Acceptance criteria

- [ ] AC-01 (always): sofarr shows no theme switch: not in the sidebar, the collapsed sidebar, the drawer or the top bar of the login page.
- [ ] AC-02 (always): Every page is in the dark palette from the first moment its body has content, whatever choice the browser stored before (Hell, System) and whatever the system prefers; a stored choice is left in the browser and ignored.
- [ ] AC-03 (always): The sidebar (expanded and collapsed) and the drawer have the page ground (`bg`) and no border towards the content. Its entries show the current page with the accent text and a 3 px accent bar at the entry's left edge, without a filled ground; other entries are in the muted text colour and take the text colour on hover and focus.
- [ ] AC-04 (always): A box has no border; it stands on the `surface` ground with the theme's card radius (`radius-md`, 8 px, `amigo-theme` AC-11), and its header has no line below it (header and rows are told apart by spacing). The rows inside a box keep their hairline separators.
- [ ] AC-05 (state): While the calendar shows the month view from 768 px, its day cells stand on the `surface` ground separated by 2 px gaps of the page ground, without borders; days outside the month stand on the page ground; today's cell has a 2 px accent ring.
- [ ] AC-06 (state): While the calendar shows the week view from 768 px, each entry shows its title's poster (2:3, 40 px wide) before its text; a title without a poster, or whose poster fails to load, shows the placeholder of the library's posters (the title's initial on a neutral ground) in the same 2:3 size.
- [ ] AC-07 (always): Every page the axe scans of `ui/e2e/a11y.spec.ts`, `library-a11y.spec.ts`, `discover-a11y.spec.ts`, `extras-a11y.spec.ts` and `calendar.spec.ts` cover, plus the week view with posters and the login page, meets WCAG 2.1 AA in the dark palette at 1440 px and 360 px; every control of the shell is reachable by keyboard with a visible focus, and every icon-only control has a German accessible name.
- [ ] AC-08 (always): Opening any page directly with an empty browser cache still transfers at most 200 KB of JavaScript and CSS, gzip-compressed (`web-ui-shell` AC-16).
- [ ] AC-09 (always, manual: images): The website screenshots `calendar.png` and `overview.png` are regenerated; the owner checked the files.
- [ ] AC-10 (state, manual: visual design, judged by the owner): While the UI is open, the calendar, Fehlend, Sperrliste, Untertitel, Neu encodieren, Backups, System and the login page look of one piece with the cinema pages at 1440 px and 360 px: no box has a frame, the navigation has no filled entry, nothing overlaps, and the month reads as a calendar at a glance.

## Technical notes

- **Theme:** `ui/src/app.html` sets `data-theme` before the app renders, and `ui/src/lib/theme/theme.ts` applies a choice; both always set the dark palette. `ThemeSwitch.svelte` and its uses in `AppNav.svelte` go.
- **Boxes:** the global `.box` and `.box-header` rules in `ui/src/app.css`.
- **Calendar:** `ui/src/routes/kalender/+page.svelte`; the week view's posters need to know whether a title has a poster: calendar entries carry `has_poster` (an additive field). `ui-cinema-home` may add it first; if not, this spec adds it.
- **Tests:** older tests that run in both palettes (`PALETTES` in `ui/e2e/theme-helpers.ts`) run in the dark palette; the tests of the switch (`theme.spec.ts`, `theme.test.ts`, `shell.spec.ts`) are changed to prove that no switch shows and a stored light or system choice is ignored. Tests of removed behaviour are changed, not skipped.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| The browser stored „Hell“ before the update | The page is dark from the first paint; the stored value stays |
| The system prefers light | Dark |
| Local storage throws | Dark; nothing fails |
| A box inside a dialog | The same flat look |
| A month with six week rows | The gaps and rings as in AC-05; the row heights of `ui-polish` AC-12 hold |
| A week entry of a title without a poster | The placeholder of today at 40 px |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The stored theme choice is ignored, not deleted | Deleting it would be a write nobody asked for; leaving it costs nothing | Low |
| Hairlines between a box's rows stay | Long lists (Sperrliste, Backups) need them to be read | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/cinema-shell.spec.ts`: „no theme switch (AC-01)“ (3 tests); `ui/e2e/shell.spec.ts`: „theme switch (AC-09)“, „the login page has a top bar …“, „the drawer slides in …“; `ui/src/lib/theme/theme.test.ts`: „offers no theme switch“ | built |  |
| AC-02 | `ui/e2e/cinema-shell.spec.ts`: „always the dark palette (AC-02)“ (6 tests); `ui/e2e/layout.spec.ts`: „theme (AC-10)“ (10 tests); `ui/src/lib/theme/theme.test.ts`: „the palette“ | built |  |
| AC-03 | `ui/e2e/cinema-shell.spec.ts`: „a quieter shell (AC-03)“ (3 tests); `ui/e2e/shell.spec.ts`: „every entry has an icon; the current one is the accent text without a ground (AC-02, dark)“ | built |  |
| AC-04 | `ui/e2e/cinema-shell.spec.ts`: „flat boxes (AC-04)“; `ui/e2e/look.spec.ts`: „a box looks like its shared counterpart without the frame (AC-10)“; `ui/e2e/library-detail-boxes.spec.ts`: „box headers (AC-02)“; `ui/e2e/theme.spec.ts`: „grounds, text and hairlines are the tokens (AC-02)“ | built |  |
| AC-05 | `ui/e2e/cinema-shell.spec.ts`: „the month as tiles (AC-05)“; `ui/e2e/calendar.spec.ts`, `ui/e2e/ui-polish.spec.ts` (row heights) still green | built |  |
| AC-06 | `ui/e2e/cinema-shell.spec.ts`: „posters in the week (AC-06)“ | built |  |
| AC-07 | `ui/e2e/cinema-shell.spec.ts`: „accessibility in the dark palette (AC-07)“ (week with posters, month and login page at 1440 and 360 px; keyboard through sidebar, top bar and drawer); `a11y.spec.ts`, `library-a11y.spec.ts`, `discover-a11y.spec.ts`, `extras-a11y.spec.ts` in the dark palette (`PALETTES` in `ui/e2e/theme-helpers.ts`) | built |  |
| AC-08 | `ui/e2e/assets.spec.ts`: „transfer budget (AC-16)“; the budget tests of `library-a11y.spec.ts` and `discover-a11y.spec.ts` | built |  |
| AC-09 | manual: the owner checked `calendar.png` and `overview.png` among the screenshots of the demo instance from this branch, dark palette, 1440 px and 360 px („Sichtprüfung schaut gut aus“), 2026-11-23 | built |  |
| AC-10 |      | open   |        |

## Decisions

- 2026-11-22: The theme switch goes and the UI is always dark; the light values stay in the generated theme files (owner), so the palette could come back without a change to amigo-labs/website.
- 2026-11-23: This spec revises `amigo-theme` AC-05 .. AC-09 and AC-13, `ui-shell-redesign` AC-02, AC-09, AC-13 and AC-15, `web-ui-shell` AC-10 and AC-11, `ui-detail-boxes` AC-02 and `ui-calendar` AC-01 (see Context). Their tests were changed, not skipped: `PALETTES` in `ui/e2e/theme-helpers.ts` is only `dark`, so every test that ran in both palettes runs in the dark one; the switch tests in `layout.spec.ts`, `shell.spec.ts` and `theme.test.ts` now prove that no switch shows and that a stored „Hell“/„System“ and a light system preference are ignored (and the stored value kept); `theme.spec.ts` expects the sidebar on `bg`, a frameless box and the raised look on the calendar's segmented control instead of the switch; `look.spec.ts` compares a box with `am-panel` without its frame; `library-detail-boxes.spec.ts` expects no line below a box header.
- 2026-11-23: `app.html` keeps `data-theme="dark"` on `<html>` and no longer reads the stored choice; `<meta name="color-scheme">` is `dark` only, so native controls and scrollbars are dark under a light system preference too. `theme.ts` shrinks to `THEME_KEY` (documented as ignored), `PALETTE` and `applyPalette`, which `AppNav` calls on mount; `ThemeSwitch.svelte` is deleted. The light rules in `app.css` (the select chevron) stay with the light tokens (D29).
- 2026-11-23: Navigation entries change only their text colour on hover and focus, no ground (the shell has no filled entry, AC-10); the accent bar is the current entry's `::before`, 3 px wide with a 4 px inset top and bottom and rounded right corners.
- 2026-11-23: The narrow top bar also stands on the page ground without its bottom line, so the shell is one surface at every width; the sidebar footer's line above „Abmelden“ and the separators of the collapsed groups stay.
- 2026-11-23: A box header keeps its padding; header and first row are apart by that spacing alone. The week columns and the narrow agenda's day cards lose their frame too and mark today with the same 2 px accent ring as the month; the month keeps today's filled number.
- 2026-11-23: The week's posters show at every width (also in the narrow agenda), through `CalendarEntryLine`'s new `poster` option, which wraps the existing `Poster` (size `list`) in a 40 px column; the month cells stay without posters. An entry with neither movie nor series id shows the placeholder.
- 2026-11-23: Tests green at once: in `cinema-shell.spec.ts` a stored „dark“, nothing stored and blocked storage were dark before this spec (the default of `amigo-theme` AC-04); all other tests of the spec were red first. The rewritten `theme.test.ts` was written with the change.
- 2026-11-23: The screenshot run also changed `discover.png`, `movie.png`, `movies.png`, `person.png` and `series.png` by the new sidebar (not rendering noise); they are committed with `calendar.png` and `overview.png` so the website shows one shell.
