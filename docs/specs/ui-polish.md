---
status: todo
created: 2026-11-22
size: M
design: [sofarr.md §3, sofarr.md §9 D30]
media: both
oracle: none
---

# UI redesign 11: polish

## Context

The redesign series (D30, `ui-shell-redesign` to `ui-overview-polish`) gave the web UI its structure. A review of the website screenshots of 2026-11-17 (`docs/site/screenshots/`) with the owner on 2026-11-22 found thirteen remaining blemishes. None is a missing feature; each makes a page look unfinished or restless:

1. **Series head:** the blurred poster behind the head shows a bright vertical streak where the poster has a bright shape, so the backdrop reads as a rendering fault.
2. **Discover tiles:** a rating that is still being fetched shows as „…“, which reads as a truncated text; the website screenshots were even taken in that state.
3. **Library cards:** year and addition run together („2024· Star Voyage“).
4. **Poster tiles:** titles of one or two lines and one or two chips push year, chips, rating and button to a different height on every tile.
5. **Discover tiles:** the „In der Bibliothek“ chip and the „Hinzufügen“ button already share a height (`ui-consistency` AC-04) but end at different heights from tile to tile (same cause as 4).
6. **Episode rows:** „Datei“ and „Untertitel“ sit on a line of their own below every episode with a file, which doubles the season's height.
7. **Series head facts:** five facts in rows of four leave „Serientyp“ alone on a second row.
8. **Overview:** an empty box repeats what its key figure says („0 Warteschlange“ and „Nichts in der Warteschlange.“).
9. **Overview:** every entry of „Die nächsten 7 Tage“ that is not out yet carries the chip „noch nicht erschienen“, although the box lists upcoming dates.
10. **Movie and series page:** the line „Stand: …“ of the cast sits inside the cast box.
11. **Discover pages:** the line „Stand: …“ sits prominently right under the search.
12. **Dropdowns:** selects show the browser's own arrow and look unlike the other controls.
13. **Calendar:** in the month grid a week without entries is as tall as a full one, e.g. a last week that lies almost wholly in the next month.

The owner decided on 2026-11-22: all thirteen points; empty overview boxes are hidden and the key figures stay; every poster tile reserves two title lines; the cast strip stays as it is and only its „Stand“ line moves.

This spec changes the look only. What each page shows and does stays as its specs say, except where an AC below revises one:

- AC-02 revises `ui-consistency` AC-03 (the pending placeholder of a discover tile) and the edge case „A pending rating on a discover tile“ there.
- AC-04 revises the edge case „A tile in a shelf whose title takes two lines“ of `ui-consistency`.
- AC-06 revises `ui-consistency` AC-11 (the disclosure line below an episode row) from 768 px.
- AC-07 revises `ui-detail-head` AC-03 („up to four per row“).
- AC-08 and AC-09 revise `ui-overview-polish` AC-06 (the empty states of the overview boxes) and `ui-overview-pages` AC-02 (every key figure is a link) and AC-04 (two columns).

## Terms

- **Poster tile**: an item of a poster grid or shelf that stands for a movie or series: a card of the movie and series lists in poster view, and a title tile of the discover, search, shelf and person pages. The tiles of people (search results, cast, crew) are not poster tiles and stay as they are.
- **Tile row**: the poster tiles that stand side by side in one row of a grid, or all tiles of one shelf.
- **Foot** of a poster tile: everything below its year line: the chips, the rating and, on discover tiles, the library state chip or the „Hinzufügen“ button.
- **Freshness line**: the caption „Stand: {date-time}“ that says when TMDB's answer was fetched (`discover` AC-13). It is not the warning that TMDB cannot be reached (`discover` AC-14), which today stands directly above it.
- **Empty box** of the overview: a box that today shows only its empty sentence (`ui-overview-polish` AC-06).
- **Head facts**: the terms and values on the panel in the head of a movie or series page (`ui-detail-head`).

## Scope

### In scope

- The thirteen points above, in the dark and the light palette.
- The website screenshots (`docs/site/screenshots/`), regenerated after the change, taken only once the ratings have arrived.

### Out of scope

- New data, boxes or figures on any page — because this spec is about the look; the server's answers stay as they are.
- The size and shape of the cast portraits — because the owner keeps the strip (2026-11-22); real titles fill it, the demo's three members do not.
- The pages Fehlend, Sperrliste, Untertitel, Backups, System, Neu encodieren and the login — because the review covered only the pages on the website screenshots; they get a review of their own if needed.
- A custom dropdown menu (the opened list of a select) — because the browser's own list is accessible and works on touch devices; only the closed control gets the theme's look.
- The agenda of the calendar below 768 px and the week view — because point 13 concerns only the month grid.

## Acceptance criteria

- [ ] AC-01 (state, manual: visual design, judged by the owner): While a movie or series page shows a poster backdrop in its head, the backdrop reads as an even wash of the poster's colours: no shape, edge or streak of the poster can be made out, in either palette, at 1440 px and 360 px. Checked on a poster with a bright shape on a dark ground (the demo series „Hafenlichter“).
- [ ] AC-02 (state): While the rating of a discover tile is pending, the tile shows no visible text in its rating line and keeps the line's height, so the tile does not change height when the rating arrives; the accessible name and the tooltip of the line stay „IMDb-Bewertung folgt“. A tile with a rating shows „★ 7,8“ and one without „–“, as today.
- [ ] AC-03 (always): A library card or row that shows a year and an addition after it separates them with „ · “ (a space on both sides of the dot, e.g. „2024 · Star Voyage“); with only one of the two, no dot shows.
- [ ] AC-04 (always): Every poster tile reserves exactly two lines for its title. A title that needs more is cut at the end of the second line with an ellipsis; the full title is the tile's tooltip and the accessible name of its link. Every tile also keeps the space of its year line when it has no year, so within a tile row the top edges of all year lines are at the same vertical position.
- [ ] AC-05 (always): Within a tile row, all tiles end at the same bottom edge, and the last element of every tile's foot ends at that edge: the rating line on library cards, the library state chip or the „Hinzufügen“ button on discover tiles (their bottom edges at the same vertical position, and the rating lines above them too). This holds whether a tile carries one chip or two (also when two chips take two lines), a title of one line or two, a rating or none. The chips of a library card start directly below its year line.
- [ ] AC-06 (state): While the window is at least 768 px wide, an episode row with a file shows the toggles „Datei“ and „Untertitel“ in its main line, before the monitor and search buttons, and no line of their own below it. A closed row with a file whose main line fits on one line is then as high as a row without a file whose main line fits on one line. An opened „Datei“ or „Untertitel“ shows its content below the main line in the row's full width, as today. Names, the muted colour with the accent on hover and focus, and the chevron stay as `ui-consistency` AC-11 says. Below 768 px the toggles stay on their own line below the main line, as today.
- [ ] AC-07 (state): While the window is at least 1200 px wide, the head facts of a series page stand in one row (the five of today: Episoden, Größe, Qualitätsprofil, Tags, Serientyp; all five always show). A row of head facts holds up to five facts from 1200 px and as many as fit below. At any width, on both pages, no row of head facts holds a single fact while the row above it holds more than two.
- [ ] AC-08 (state): While an overview box is an empty box, sofarr does not show it: neither its heading nor its empty sentence. This holds for all four boxes (Warteschlange, Die nächsten 7 Tage, Probleme der letzten 7 Tage, Letzte Aktivität).
  - A queue box that carries a notice (no download client configured, client unreachable) is not an empty box and shows as today.
  - The key figure of a hidden box shows its number as plain text, not as a link; the figure of a shown box links to it as today.
  - While the boxes of one column are all hidden (from 1200 px), the other column takes the full width.
  - While all four boxes are hidden, the overview shows one muted line below the key figures: „Alles ruhig – keine Downloads, Probleme oder Termine.“
- [ ] AC-09 (always): An entry of „Die nächsten 7 Tage“ that is not out yet shows no chip „noch nicht erschienen“; entries in any other state keep their chip of today. Other pages keep the chip.
- [ ] AC-10 (always): The freshness line of a page stands once, as the last line of the page's content before the IMDb attribution (the very last line where no attribution shows), in the caption style. This holds on the discover, search, shelf, preview and person pages and on the movie and series pages (there it is the cast's). It no longer stands under the search or inside the cast box.
  - A page that shows several TMDB answers at once shows one line with the oldest fetch time.
  - The warning that TMDB cannot be reached keeps its place and wording of today (at the top of the discover pages, inside the cast box on the movie and series pages); a page with that warning shows the freshness line at the end too.
  - A page without a TMDB answer shows no freshness line.
- [ ] AC-11 (always): Every dropdown (select) of the web UI shows the theme's control look in its closed state: the height, border, radius and ground of the text fields, a chevron icon at its end instead of the browser's own arrow, the selected value not running under the chevron, the visible focus ring of the other controls, and a dimmed look while disabled. Opening, choosing and keyboard use stay the browser's own.
- [ ] AC-12 (state): While the month view is shown at 768 px or wider, a week row in which no day shows an entry (days outside the month never show entries, `ui-calendar` AC-01) is at most 72 px high, and a week row with entries is as high as its fullest day needs. Every day cell keeps its day number, and today keeps its mark.
- [ ] AC-13 (always): The pages this spec changes meet WCAG 2.1 AA in both palettes at 1440 px and 360 px (axe scans): the overview with all boxes hidden and with only some, a poster grid with long titles, a discover shelf with a pending rating, a series page with an opened episode, a page with a dropdown, the month grid.
- [ ] AC-14 (event): When the website screenshots are generated (`bun run screenshots`), then the run photographs a page only after no rating on it is pending, and ends with an error instead of a picture if one is still pending after its wait. The screenshots under `docs/site/screenshots/` are regenerated with this run after the other ACs are built.
- [ ] AC-15 (state, manual: visual design, judged by the owner): While either palette is active, the overview, the movie and series lists in poster view, a movie page, a series page, the discover page and the month calendar look calm and complete at 1440 px and 360 px: tile rows are even, nothing is said twice, and no control looks foreign.

## Technical notes

- **Backdrop (AC-01):** `DetailHead.svelte` stretches the portrait poster over the wide head (`object-fit: cover`, `blur(28px)`), so a bright shape survives as a streak. A much stronger blur relative to the image (e.g. blurring a small rendition, or a blur radius in the order of the head's height) removes the shape.
- **Pending rating (AC-02):** `RatingText.svelte` and `ratingCompact` in `ui/src/lib/discover/texts.ts`; the library lists have no pending state.
- **Separator (AC-03):** `MediaItems.svelte` renders `· {row.subtitle}` after the year without a space before the dot.
- **Tiles (AC-04, AC-05):** `MediaItems.svelte` (cards) and `TitleTile.svelte`; the grid already stretches the tiles of a row, the foot only needs to sit at the bottom.
- **Episode rows (AC-06):** `SeasonBox.svelte`, `EpisodeExtras.svelte`; the toggles are already buttons with `aria-expanded` (`ui-consistency`, build decision).
- **Facts (AC-07):** the modifier `facts-compact` of the head's list.
- **Overview (AC-08, AC-09):** `ui/src/routes/+page.svelte`, `QueueBox.svelte`, `ActivityBox.svelte`, `EmptyLine.svelte`; the chip text comes from `ui/src/lib/library/format.ts` (`upcoming`).
- **Freshness (AC-10):** `FreshnessNote.svelte` renders the stale warning and the line together; they separate.
- **Selects (AC-11):** the global rule for `select` in `ui/src/app.css`; the generated theme files under `ui/src/theme/` are not edited (D29).
- **Calendar (AC-12):** the month grid's rows in `ui/src/routes/kalender/+page.svelte`.
- **Screenshots (AC-14):** `ui/e2e/site/screenshots.mjs` waits for the text „IMDb …“ to vanish, which the compact rating no longer shows; it must wait for the pending state itself.
- **Tests:** Playwright in a new `ui/e2e/ui-polish.spec.ts` (library, discover and extras servers as the existing specs use them); unit tests where a pure helper changes. Older e2e tests that assert the replaced looks (`ui-consistency.spec.ts` AC-11 and the pending placeholder, `overview-polish.spec.ts` AC-06, `pages-redesign.spec.ts` figures and columns, `library-detail-head.spec.ts` facts, `calendar.spec.ts` rows, the „Stand“ checks of the discover tests) are adjusted to the revised behaviour, not removed.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A title of three or more lines on a poster tile | Two lines, then an ellipsis; tooltip and link name carry the full title |
| A one-word title too long for the tile's width | Breaks inside the word as today, still at most two lines |
| A tile row in which every title is short | Every tile still reserves two title lines, so all rows of a grid share one rhythm |
| A discover rating that never arrives | The rating line stays empty (name and tooltip „IMDb-Bewertung folgt“) as long as the server reports it pending; a rating the server could not fetch is reported as none and shows „–“ (`discover`, decision of 2026-11-12) |
| Ratings switched off (`[imdb] enabled = false`) | No rating line at all, as today; the other foot elements still line up |
| Queue box empty with a configured, reachable client | Hidden; its figure shows 0 without a link |
| Queue box with the no-client line and active grabs from an earlier live run | Shown with the line and the grab rows, as today |
| A hidden box gets content at the next refresh (every 15 s) | The box appears, its figure becomes a link; focus and scroll position stay |
| A shown box becomes empty at a refresh while the focus is inside it | The box hides; the focus is not moved elsewhere by sofarr (the browser's default), and no error shows |
| A focused key figure link becomes plain text at a refresh | The figure stays readable; the focus is the browser's default |
| A refresh of the overview fails | The page keeps its last state and shows the error of today; hidden boxes stay hidden |
| All boxes hidden, then one problem arrives | The „Alles ruhig“ line goes, the problems box shows |
| The cast could not be loaded on a movie page | The failure note of today in the cast's place; no freshness line |
| An episode row with a file in a narrow column at 800 px | The main line may wrap as `ui-detail-boxes` AC-06 allows; the toggles stay part of it |
| An episode without a file | No toggles, as today |
| A month with six week rows, the last without entries | The last row is at most 72 px high |
| A select with a value longer than the control | The value is cut before the chevron |
| A select inside a dialog or the filter panel | The same look |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The no-client line keeps the queue box visible | The figure „0“ does not say that sofarr has no download client; `ui-overview-polish` chose that line on purpose | Low: in live operation a client is configured and the empty box hides |
| „Letzte Aktivität“ hides while empty although no key figure counts it | It is only empty on a fresh install; the „Alles ruhig“ line or the other boxes fill the page | Low |
| 72 px for a week row without entries | The day number with the cell's padding needs about 44 px; 72 px leaves air and is clearly less than a row with an entry (about 150 px) | Low |
| 1200 px for the series facts in one row | The breakpoint of the two-column boxes (`ui-detail-boxes`), where the head is wide enough for five facts | Low |
| The freshness line moves on every discover page, not only the main one | One place for the same caption on every page (`ui-consistency`: one pattern per element) | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |  | open |  |
| AC-02 | `discover tiles: pending rating (AC-02)` › `a pending rating shows no text, keeps its line and its name` (`ui/e2e/ui-polish.spec.ts`); `ratingCompact` in `ui/src/lib/discover/texts.test.ts` | built |  |
| AC-03 | `library cards and rows: year and addition (AC-03)` › `„ · “ between year and addition, no dot with only one of them (liste / poster)` (`ui/e2e/ui-polish.spec.ts`) | built |  |
| AC-04 | `poster tiles (AC-04, AC-05)` › `library cards: two title lines, year lines level, feet end level`, `discover tiles: two title lines, year lines level, chips and buttons end level` (`ui/e2e/ui-polish.spec.ts`) | built |  |
| AC-05 | the same two tests of `poster tiles (AC-04, AC-05)` (tile bottoms, rating lines, chips and add buttons at one edge; one and two chips) (`ui/e2e/ui-polish.spec.ts`) | built |  |
| AC-06 | `episode rows (AC-06)` › `from 768 px the toggles sit in the main line, before the buttons`, `below 768 px the toggles keep their own line` (`ui/e2e/ui-polish.spec.ts`); `episode disclosures (AC-11)` (`ui/e2e/ui-consistency.spec.ts`) | built |  |
| AC-07 | `head facts (AC-07)` › `a series shows its five facts in one row from 1200 px`, `no fact stands alone below a fuller row at 360 / 600 / 800 / 1000 / 1199 / 1440 px` (series and movie page) (`ui/e2e/ui-polish.spec.ts`); `the facts are a compact surface panel with up to five per row` (`ui/e2e/library-detail-head.spec.ts`) | built |  |
| AC-08 | `overview: empty boxes (AC-08)` › `all boxes empty: no box, plain figures and the quiet line`, `a queue box with the no-client line shows; the quiet line does not`, `a queue box with an unreachable client shows its warning`, `only problems: that box takes the full width and its figure links to it`, `a box hides and shows again with the refreshes` (`ui/e2e/ui-polish.spec.ts`); `empty states (AC-06, revised by ui-polish AC-08)` (`ui/e2e/overview-polish.spec.ts`) | built |  |
| AC-09 | `overview: upcoming entries (AC-09)` › `no chip for entries that are not out yet; other states keep theirs` (`ui/e2e/ui-polish.spec.ts`); the agenda's chips in `ui/e2e/calendar.spec.ts` (unchanged) | built |  |
| AC-10 | `freshness line (AC-10)` › `discover / filtered list / search / preview / person: once, at the end of the page`, `shelf page: once, at the end of the page`, `library movie: the cast’s line stands at the end, not in the cast box`, `a stale answer keeps its warning on top and the line at the end` (`ui/e2e/ui-polish.spec.ts`) | built |  |
| AC-11 | `dropdowns (AC-11)` › `a select has the field look and a chevron of its own, dark / light`, `focused by keyboard a select shows the focus ring; disabled it is dimmed` (`ui/e2e/ui-polish.spec.ts`) | built |  |
| AC-12 | `month grid: weeks without entries (AC-12)` › `an empty week is at most 72 px high; a week with entries grows` (`ui/e2e/ui-polish.spec.ts`) | built |  |
| AC-13 | `accessibility (AC-13), dark / light, 1440 px / 360 px` › `overview with all boxes hidden and with only some`, `a poster grid with long titles and a dropdown`, `a discover shelf with a pending rating`, `a series page with an opened episode`, `the month calendar` (`ui/e2e/ui-polish.spec.ts`) | built |  |
| AC-14 | `website screenshots: the wait for ratings (AC-14)` › `ends with an error while a rating stays pending`, `returns once no rating is pending` (`ui/e2e/ui-polish.spec.ts`); `bun run screenshots` regenerated `docs/site/screenshots/` on 2026-11-22 | built |  |
| AC-15 |  | open |  |

## Decisions

- 2026-11-22: The owner chose all thirteen points of the review.
- 2026-11-22: Empty overview boxes are hidden and the key figures stay (owner), because the figures say „0“ at a glance and the boxes then only repeat it.
- 2026-11-22: Every poster tile reserves two title lines and cuts longer titles (owner), because only a fixed title height puts year, chips and rating on one line across a row; the full title stays in tooltip and link name.
- 2026-11-22: The cast strip keeps its size; only its „Stand“ line moves to the page's end (owner), because the strip looks empty only with the demo's three members.
- 2026-11-22: A pending rating shows no visible text instead of „…“, because „…“ reads as cut-off text and „–“ would claim there is no rating; the reserved line keeps the tile from jumping.
- 2026-11-22: No new entry in `sofarr.md` §9: the spec stays within D30 and adds no dependency, API field or config key.
- 2026-11-22 (build): Red run of `bunx playwright test e2e/ui-polish.spec.ts` against the unchanged UI: 30 of 52 failed (pending „…“, „2024· …“, uneven tiles, toggles below the line, 4 + 1 facts, visible empty boxes, the chip, the „Stand“ lines, the browser's select arrow, 112 px empty weeks). Green at once: the toggles' own line below 768 px, the facts at 360 px and 600 px, the unreachable client's warning, the freshness line on the preview and person pages (already at the end) and most axe scans; they stay as guards.
- 2026-11-22 (build): AC-01 is built (the backdrop's blur grows with the window, 56 px to 120 px, and the image is scaled so the blur's edge lies outside the head) and waits for the owner's eye; so does AC-15.
- 2026-11-22 (build): The episode toggles are rendered before the episode's buttons from one component, so from 768 px the Tab order is the visual order (toggles, monitor, search). Below 768 px the toggles show below the line but keep that place in the Tab order, because one DOM order has to serve both layouts.
- 2026-11-22 (build): An episode with a file and two chips („Cutoff nicht erreicht“) does not fit one line in the two-thirds column at 1440 px: its toggles and buttons wrap onto a second line (the edge case of the narrow column). The title's flexible width starts at 6 rem instead of 10 rem, so a row with one chip fits.
- 2026-11-22 (build): Facts: rows of four, five from 1200 px, two below 768 px; where exactly one fact would be left over (5 or 9 of four, 6 or 11 of five), the panel uses rows of three.
- 2026-11-22 (build): The chevron of a select is a background image and cannot read a custom property, so `ui/src/app.css` draws it once per palette in the value of `--am-text-secondary`; the generated theme files stay untouched. The rule's selector outweighs the theme's `.am-input`, which sets the whole background.
- 2026-11-22 (build): „Alles ruhig …“ takes the check icon and the muted empty-line look of `ui-overview-polish`. An unreachable client without downloads keeps „Nichts in der Warteschlange.“ below its warning, as today.
- 2026-11-22 (build): The freshness line of a library page shows only when the cast shows (an empty cast or a failed load: no line). On the discover start page the line is the filtered list's while filters are set, otherwise the shelves'; no page shows two TMDB answers at once today.
- 2026-11-22 (build): The wait of the screenshot run is `ratingsSettled` in `ui/e2e/site/settled.mjs`, so the e2e tests can run it against a page whose ratings stay pending; it now guards every picture, not only the discover page's.
- 2026-11-22 (build): Older e2e tests adjusted to the revised behaviour, none removed: the overview's figures and empty states (`overview-polish.spec.ts`, `pages-redesign.spec.ts`, `library-pages.spec.ts`), the facts' columns (`library-detail-head.spec.ts`), the one-line check of an episode row, now on an episode without a file (`library-detail-boxes.spec.ts`), and the pending placeholder's unit test.
