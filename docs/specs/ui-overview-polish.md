---
status: todo
created: 2026-11-17
size: S
design: [sofarr.md §3, sofarr.md §9 D30]
media: both
oracle: none
---

# UI redesign 10: overview polish

## Context

The overview (`/`, `web-ui-library` AC-01, `ui-overview-pages` AC-02 to AC-04) is the first page after login. In the screenshots of 2026-11-17 it reads as a plain list:

- The key figures are bare numbers with a label.
- The queue box shows a yellow warning „Kein Download-Client konfiguriert“ and below it „Nichts in der Warteschlange.“, although running without a client (shadow mode) is a normal setup.
- The days of „Die nächsten 7 Tage“ are full dates („19.11.2026“) where „Morgen“ or „Donnerstag, 19.11.“ would read faster.
- The empty boxes are a single grey sentence each.

There is also a bug: while the client is unreachable and a last state is known, the queue notice names that state twice („Download-Client nicht erreichbar – Stand von 17.11.2026 10:23. Stand: 17.11.2026, 10:23:41“).

This spec builds on `ui-consistency` (date-times to the minute, neutral info notices).

## Scope

### In scope

- The key figures, the queue box's notice and empty state, the day headings of the next 7 days, the empty states of the four boxes.
- The queue answer's new field that says whether a download client is configured.

### Out of scope

- New figures or boxes (e.g. library size, disk space) — because the overview answer has no such data and adding it is a feature of its own.
- A hero or backdrop on the overview — because the overview shows states, not a title; the library heads carry the artwork.
- The order and columns of the boxes — because `ui-overview-pages` AC-04 settled them.

## Acceptance criteria

- [ ] AC-01 (always): sofarr shows each key figure with an icon before its label (Warteschlange: download, Probleme: alert, Nächste 7 Tage: calendar), the number in tabular figures, and the whole tile as its link of today (ui-overview-pages AC-02). The icons are decorative (hidden from assistive technology).
- [ ] AC-02 (state): While no download client is configured (in shadow or live mode), the queue box shows one line of muted text „Kein Download-Client eingerichtet – sofarr lädt nichts herunter.“ and neither a warning nor „Nichts in der Warteschlange.“. Grabs that are active in the database still show as rows below it, as today.
- [ ] AC-03 (error): If the configured download client is unreachable, then the queue box shows the warning of today with the time of the last known state exactly once (to the minute), or „Download-Client nicht erreichbar“ without a time when no state is known.
- [ ] AC-04 (always): The queue answer (`GET /api/queue` and the overview's `queue`) carries `client_configured` (true while `[nzbget]` is configured). All fields of today stay, so existing clients keep working.
- [ ] AC-05 (always): A day heading in „Die nächsten 7 Tage“ reads:
  - „Heute“ for today
  - „Morgen“ for tomorrow
  - otherwise the weekday and the date without year (e.g. „Donnerstag, 19.11.“)

  An entry's date is a calendar day without time (`YYYY-MM-DD`), compared with the browser's local date. The full date („19.11.2026“) stays available as the heading's tooltip and as a machine-readable date. A date that cannot be read shows as it came.
- [ ] AC-06 (always): An empty box of the overview (queue without client is AC-02) shows its sentence of today with a decorative icon before it (Warteschlange: download, Die nächsten 7 Tage: calendar, Probleme: check, Letzte Aktivität: history), in the muted text colour. The problems box while empty says „Keine Probleme in den letzten 7 Tagen.“
- [ ] AC-07 (always): The overview meets WCAG 2.1 AA in both palettes at 1440 px and 360 px with empty boxes and with a queue item, a problem and upcoming entries (axe scans).
- [ ] AC-08 (state, manual: visual design): While either palette is active, the overview looks complete and calm at 1440 px and 360 px, with no client and with one.

## Technical notes

- **Queue:** `client_state` in `crates/sofarr-server/src/overview.rs` already builds the „Stand von …“ notice; the UI's `QueueBox.svelte` appends a second „Stand“. One of the two goes.
- **Day headings:** a pure, unit-tested helper `dayHeading(date, today)` next to `dateText` (`ui/src/lib/library/format.ts`); `today` is passed in, so tests need no clock.
- **Icons:** octicons as elsewhere (`download-16`, `alert-16`, `calendar-16`).
- **Tests:** server tests in `crates/sofarr-server/src/overview_tests.rs` (AC-02 to AC-04); Playwright in `ui/e2e/overview-polish.spec.ts`; the e2e server of today runs without a client.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| No client configured, but grabs from an earlier live run are active | The neutral line, then the grab rows without client progress |
| Client unreachable, a last state known | Warning with that state's time once |
| Client unreachable, no state known | Warning without a time |
| An upcoming entry on today's date | Heading „Heute“ |
| The overview stays open past midnight | The next refresh (every 15 s) recomputes „Heute“ and „Morgen“ |
| Three problems | Problems figure in the danger tone (ui-overview-pages AC-03) with its icon |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `key figures (AC-01)` › `each figure has a decorative icon before its label and tabular figures` (which Octicon per figure, icon before the label) (`ui/e2e/overview-polish.spec.ts`); `overview (AC-02 .. AC-04)` (`ui/e2e/pages-redesign.spec.ts`) | built |  |
| AC-02 | `queue without and with a client (AC-02, AC-03)` › `without a client: one muted line, no warning, no empty text`, `without a client, active grabs still show as rows below the line` (`ui/e2e/overview-polish.spec.ts`); `live_mode_without_a_client_says_no_client_is_configured` (`crates/sofarr-server/src/overview_tests.rs`); `overview (AC-01)` (`ui/e2e/library-pages.spec.ts`) | built |  |
| AC-03 | `queue without and with a client (AC-02, AC-03)` › `an unreachable client with a known state names its time once`, `an unreachable client without a state shows no time` (`ui/e2e/overview-polish.spec.ts`); `queue_shows_progress_size_and_remaining_time` (`crates/sofarr-server/src/overview_tests.rs`) | built |  |
| AC-04 | `queue_shows_progress_size_and_remaining_time`, `overview_shows_activity_problems_and_the_next_days` (`crates/sofarr-server/src/overview_tests.rs`); `ui_types_match_the_server` (`crates/sofarr-server/src/ui_types.rs`) | built |  |
| AC-05 | `day headings (ui-overview-polish AC-05)` (`ui/src/lib/library/format.test.ts`); `day headings (AC-05)` › `„Heute“, „Morgen“, then weekday and date; the full date as tooltip` (`ui/e2e/overview-polish.spec.ts`) | built |  |
| AC-06 | `empty states (AC-06)` › `empty boxes show their sentence after a decorative icon, muted` (all four boxes, the queue with a configured client; which Octicon per box, muted colour, icon before the sentence) (`ui/e2e/overview-polish.spec.ts`) | built |  |
| AC-07 | `accessibility (AC-07), dark/light` › `empty boxes at 1440 px / 360 px`, `a queue item, a problem and upcoming entries at 1440 px / 360 px` (`ui/e2e/overview-polish.spec.ts`) | built |  |
| AC-08 | manual: the owner checked the overview from the before/after screenshots of this branch and the regenerated `docs/site/screenshots/overview.png` („Visual check passt“), 2026-11-17 | built |  |

## Decisions

- 2026-11-17: Running without a download client is a normal setup (shadow mode needs none, sofarr.md §5 step 3), so it is a muted line, not a warning, in either mode; whether sofarr may download at all is the System page's mode, not the queue's business. An unreachable configured client stays a warning in either mode.
- 2026-11-17: `client_configured` is a new field rather than a check of the notice text, because the UI must not depend on a German sentence; the field is additive, so the API stays compatible.
- 2026-11-17 (build): Red runs: `cargo test -p sofarr-server overview` failed on the missing `client_configured`; against bf5b0dd `bunx vitest run src/lib/library/format.test.ts` failed 2 (no `dayHeading`) and `bunx playwright test e2e/overview-polish.spec.ts` failed 10 of 14. Green at once: „an unreachable client without a state shows no time“ and the scans with a queue item, which stay as guards.
- 2026-11-17 (build): The UI no longer appends „Stand: …“ to the queue notice, because the server's notice already names the time of the last state („– Stand von 17.11.2026 10:23“), to the minute.
- 2026-11-17 (build): With a configured client the empty queue keeps „Nichts in der Warteschlange.“, now with the download icon like the other empty boxes (AC-06).
- 2026-11-17 (build): „Heute“ is computed from the browser's clock at every refresh of the overview (every 15 s), so the headings move on after midnight without a reload.
- 2026-11-17 (build): The regenerated overview screenshot showed the figure labels as status chips (dot and ground): their class `label` collided with the global chip class. The figure test now checks that the label has no dot and no ground (red against f558a6f), and the class is `figure-label`.
- 2026-11-17 (spec-check, round 1: NOT READY): AC-06 lacked the empty queue with a configured client and the muted colour, AC-02 the muted colour. The tests now cover both; they were green at once, because the behaviour was already built (`EmptyLine` with the `muted` class).
- 2026-11-17 (spec-check, round 2: NOT READY): the tests did not say which icon each figure and empty box shows. They now compare the icon's path with the Octicon from the package (`ui/e2e/octicons.mjs`); green at once, because the icons were built as specified.
- 2026-11-17 (spec-check, round 3: NOT READY): AC-02 had no case in live mode and AC-03 checked only the notice's prefix. `live_mode_without_a_client_says_no_client_is_configured` and the full notice shape „… – Stand von 99.99.9999 99:99“ in `queue_shows_progress_size_and_remaining_time` now cover both; green at once.
