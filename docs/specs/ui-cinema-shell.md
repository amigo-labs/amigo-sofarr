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

- 2026-11-22: The theme switch goes and the UI is always dark; the light values stay in the generated theme files (owner), so the palette could come back without a change to amigo-labs/website.
