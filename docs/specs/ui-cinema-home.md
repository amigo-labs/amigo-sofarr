---
status: todo
created: 2026-11-22
size: M
design: [sofarr.md §3, sofarr.md §9 D30, sofarr.md §9 D34]
media: both
oracle: none
---

# Cinema look 4: rows on the overview

## Context

The overview (`/`) shows three key figures and up to four boxes: queue, the next 7 days, problems, last activity (`ui-overview-pages`, `ui-overview-polish`, `ui-polish` AC-08). It tells whether anything needs attention, but it shows no title as a picture. The cinema look (D34) asks for rows of posters to browse.

The owner decided on 2026-11-22: the overview gets the rows „Demnächst“, „Zuletzt hinzugefügt“ and „Fehlt noch“ above its boxes. The rows of the movie and series lists are the next spec (`ui-cinema-library-rows`).

This spec builds on `ui-cinema-tiles` (the tiles in the rows).

## Terms

- **Row**: a heading and one line of poster tiles that scrolls sideways, with the arrows of the discover shelves (`ui-discover-polish`) and a link „Alle anzeigen“.
- **Library title**: a movie or series of the library.

## Scope

### In scope

- Three rows on the overview, their order and their links.
- The order of the overview's parts: key figures, rows, boxes.

### Out of scope

- Rows by genre — because the library stores no genres; that needs a migration and is a spec of its own.
- Changing the key figures or the boxes — because `ui-overview-pages` and `ui-polish` settled them.
- Rows on the movie and series lists — because they are `ui-cinema-library-rows`.

## Acceptance criteria

- [ ] AC-01 (always): The overview shows, below its key figures and above its boxes, these rows in this order, each with at most 20 tiles of library titles:
  1. „Demnächst“: the titles of the entries of the box „Die nächsten 7 Tage“ (the same entries, so the same rule for monitoring), each title once, soonest first; a series once for all its episodes.
  2. „Zuletzt hinzugefügt“: movies and series together, newest added first.
  3. „Fehlt noch“: monitored movies that are missing and series with missing episodes, newest added first.
- [ ] AC-02 (always): A tile of „Demnächst“ shows the day of its first date as the overview's day headings name it („Heute“, „Morgen“, „Donnerstag, 26.11.“) and, for a series, the episode code of its first episode that day: in its overlay as one line below the year („Morgen · S03E05“), and at the end of its link's name („…, Morgen, S03E05“). A title without a poster shows the placeholder of the library's posters.
- [ ] AC-03 (event): When the user activates „Alle anzeigen“ of a row, then sofarr opens: „Demnächst“ → the calendar (week view); „Zuletzt hinzugefügt“ → the movie list sorted by added date, newest first; „Fehlt noch“ → the page „Fehlend“.
- [ ] AC-04 (state): While a row has no title, it does not show. While all three rows and all four boxes have nothing to show (a row hidden by AC-06 does not count as empty), the overview shows the line of `ui-polish` AC-08 („Alles ruhig – …“).
- [ ] AC-05 (event): When the overview refreshes (every 15 s, `web-ui-shell`), then its rows show the new state; a row keeps its sideways scroll position in pixels, and a focused tile keeps the focus. If the focused tile leaves its row, the focus moves to the row's „Alle anzeigen“ link; if the row goes, to the page's heading.
- [ ] AC-06 (error): If the data of „Zuletzt hinzugefügt“ or „Fehlt noch“ cannot be loaded (any of the requests the row needs), then that row does not show and the rest of the overview shows as today; the next refresh tries again. „Demnächst“ shares the overview's own answer and its failure handling of today (`web-ui-shell` AC-13).
- [ ] AC-07 (always): With the e2e library of 2,501 movies and 23 series, the overview shows its rows within 2 s after the page starts loading (Playwright, measured duration in the test output).
- [ ] AC-08 (always): The overview with its rows meets WCAG 2.1 AA in the dark palette at 1440 px and 360 px (axe scans); each row is a list named by its heading, and its arrows and tiles are reachable by keyboard with a visible focus.
- [ ] AC-09 (always, manual: images): The website screenshot `overview.png` is regenerated and shows the rows; the owner checked the file.
- [ ] AC-10 (state, manual: visual design, judged by the owner): While the dark palette is active, the overview looks like the home page of a streaming service at 1440 px and 360 px: the posters carry the page, the key figures and boxes still tell at a glance what needs attention, nothing overlaps.

## Technical notes

- **Data:** „Zuletzt hinzugefügt“ and „Fehlt noch“ come from the list API of today (`GET /api/library/movies` and `/series` with `sort=added`, `order=desc`, `missing=yes`, `page_size=20`), merged in the UI by `added_at`. „Demnächst“ comes from the overview answer's `upcoming`; a tile needs to know whether its title has a poster, so calendar entries get `has_poster` (an additive field, which `ui-cinema-shell` uses too).
- **Row component:** the shelf row of discover (`TitleList` with `row`, `ShelfArrows`) generalised for library tiles.
- **Tests:** Playwright in a new `ui/e2e/cinema-home.spec.ts` on the library server (the 2,501 movies for AC-07, the calendar data of `seed.mjs` for „Demnächst“); the overview's older tests stay green.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A series with three episodes in the next 7 days | One tile; its day and code are those of the soonest episode |
| A movie with a cinema and a digital date in the next 7 days | One tile with the earlier date |
| An empty library | No rows; the key figures and boxes of today |
| A title added and missing | It shows in „Zuletzt hinzugefügt“ and in „Fehlt noch“ |
| 300 missing movies | „Fehlt noch“ shows the 20 newest; „Alle anzeigen“ leads to all of them |
| A window of 360 px | Each row scrolls sideways; the page does not |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| At most 20 tiles per row | The discover shelves show 20 (`discover` AC-01); „Alle anzeigen“ leads to all of them | Low |
| Key figures stay on top, the rows follow | The figures say at a glance whether anything needs attention; the rows invite browsing | Low |
| „Alle anzeigen“ of „Zuletzt hinzugefügt“ opens the movie list | The lists are per kind; movies are the larger part of the library | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/cinema-home.spec.ts` „rows (AC-01)“ (both tests); `ui/src/lib/library/home.test.ts` | built |        |
| AC-02 | `ui/e2e/cinema-home.spec.ts` „tiles of „Demnächst“ (AC-02)“; `crates/sofarr-server/src/overview_tests.rs` `calendar_entries_say_whether_their_title_has_a_poster`; `ui/src/lib/library/home.test.ts` | built |        |
| AC-03 | `ui/e2e/cinema-home.spec.ts` „„Alle anzeigen“ (AC-03)“ | built |        |
| AC-04 | `ui/e2e/cinema-home.spec.ts` „empty rows (AC-04)“ (three tests); `ui/e2e/ui-polish.spec.ts` „overview: empty boxes (AC-08)“, `ui/e2e/overview-polish.spec.ts` „empty states“ with empty rows | built |        |
| AC-05 | `ui/e2e/cinema-home.spec.ts` „refresh (AC-05)“ | built |        |
| AC-06 | `ui/e2e/cinema-home.spec.ts` „failures (AC-06)“ | built |        |
| AC-07 | `ui/e2e/cinema-home.spec.ts` „loading time (AC-07)“ (measured 1166 ms, annotation „Messung“) | built |        |
| AC-08 | `ui/e2e/cinema-home.spec.ts` „accessibility in the dark palette (AC-08)“ at 1440 and 360 px | built |        |
| AC-09 | manual: the owner checked `overview.png` with its rows among the screenshots of the demo instance from this branch, dark palette, 1440 px and 360 px („Sichtprüfung schaut gut aus“), 2026-11-23 | built |  |
| AC-10 | manual: the owner checked the overview from the screenshots of the demo instance from this branch, dark palette, 1440 px and 360 px („Sichtprüfung schaut gut aus“), 2026-11-23 | built |  |

## Decisions

- 2026-11-22: The overview gets the rows „Demnächst“, „Zuletzt hinzugefügt“ and „Fehlt noch“ above its boxes (owner).
- 2026-11-22: The rows read the APIs of today and need no new endpoint, because each row is a sorted or filtered page of an existing list.
- 2026-11-22: The row component is `ui/src/lib/components/TileRow.svelte` (heading, „Alle anzeigen“, arrows, a list named by the heading) with `keepRowFocus` (`row-focus.ts`) for AC-05; `ShelfArrows` moved to `ui/src/lib/components/` and measures its ends again when the tile count changes, because `ui-cinema-library-rows` reuses all three outside discover.
- 2026-11-22: The focus is kept by watching the rows' DOM (a `MutationObserver`), not by Svelte effects, because a keyed list may move a focused tile out and back in, which drops the focus without any event.
- 2026-11-22: „Alle anzeigen“ carries the row's name as hidden text („Alle anzeigen: Demnächst“), because the three links lead to different pages and need distinct names.
- 2026-11-22: Tiles of „Demnächst“ carry no state mark, and no tile of the rows shows an IMDb rating: the row itself says what the titles have in common, and the overview does not read the ratings switch.
- 2026-11-22: A row whose first load has not answered yet counts as not empty, so the quiet line does not flash before the rows arrive; on equal added times movies come before series.
- 2026-11-22: The older tests of the quiet line (`ui-polish` AC-08, AC-13; `ui-overview-polish` AC-06) serve empty rows (`emptyRows` in `library-helpers.ts`), because AC-04 shows the line only while the rows are empty too.
- 2026-11-22: Poster tile links are never underlined: the placeholder's initial counted as an underlined title link (`look` AC-12).
- 2026-11-22: The screenshot script waits for „Zuletzt hinzugefügt“ on the overview, so the picture shows the rows; only `overview.png` is committed, the other pictures changed by rendering noise alone.
- 2026-11-22: The unit tests of `home.ts` were green at once (written after the module); the red run of AC-01 .. AC-08 is the e2e run before the page had rows (11 of 12 failed; „no rows and no boxes: the quiet line“ passed, the behavior of today).
