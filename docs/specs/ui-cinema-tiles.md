---
status: todo
created: 2026-11-22
size: M
design: [sofarr.md §3, sofarr.md §9 D30, sofarr.md §9 D34]
media: both
oracle: none
---

# Cinema look 2: poster tiles

## Context

The cinema look (D34) puts titles and images first. Today a poster tile carries its poster and, below it, two title lines, the year, status chips, the rating and, on discover, the library chip or the „Hinzufügen“ button (`ui-polish` AC-04, AC-05). In a grid of posters the text below every tile reads like a table under each picture.

The owner decided on 2026-11-22: at rest a tile is only its poster; on hover or keyboard focus it grows slightly and shows its information over the lower part of the poster.

This spec builds on `ui-cinema-backdrops` (same scrim idea for text over images) and revises:

- `ui-polish` AC-04 and AC-05 (title lines and foot below the poster) for poster tiles;
- `ui-list-items` AC-02 and AC-03 for cards in poster view (chips and rating below the poster);
- `ui-consistency` AC-03 and AC-04 (compact rating and equal chip and button heights on discover tiles);
- `imdb-ratings` AC-04 for poster tiles: the rating shows in the overlay and in the tile's accessible name, not at rest.

## Terms

- **Poster tile**: an item of a poster grid or shelf that stands for a movie or series: a card of the movie and series lists in poster view, and a title tile of the discover, search, shelf and person pages (`ui-polish`). Tiles of people stay as they are.
- **Hover device**: a device whose primary pointer can hover, such as a mouse or a trackpad; a device whose primary pointer is a finger is not one (also a touchscreen laptop used by touch).
- **Overlay**: the layer over the lower part of a tile's poster that shows its information on a dark gradient.
- **State mark**: a small round mark in a corner of the poster that tells a tile's state at rest.
- **Open tile**: the tile the pointer is over; while the pointer is over no tile, the tile the keyboard focus is in. At most one tile is open at a time.

## Scope

### In scope

- The look and behaviour of poster tiles on hover devices and touch devices, in the dark palette.
- The state mark at rest.
- The movie and series lists in poster view and every discover page with title tiles.

### Out of scope

- The list view of the movie and series lists — because its rows are a table, not a grid of pictures.
- Tiles of people (cast, crew, search results) — because the owner's request concerns titles.
- New data in the API — because every value the overlay shows is already in the list and discover answers.
- The light palette — because D34 designs the dark palette only; until `ui-cinema-shell` removes the switch, the light palette shows the same layout and is neither measured nor checked by eye.

## Acceptance criteria

- [ ] AC-01 (state): While a poster tile is not open on a hover device, sofarr shows only its poster: no title, year, addition, chip, rating or button is visible on or below it, and the tile is exactly as high as its poster. Only the controls of `ui-list-items` (the monitor toggle in the corner, the selection checkbox on hover, focus or in selection mode) and the state mark of AC-02 show on the poster.
- [ ] AC-02 (always): A poster tile shows a state mark in the lower left corner of its poster, with the state's German text as tooltip and accessible name:
  - a movie of the library: accent while it has a file („vorhanden“), danger while the list API counts it as missing („fehlt“: monitored, available, no file), none otherwise (e.g. not yet available, or unmonitored without a file);
  - a series of the library: danger while the list API counts missing episodes („N fehlen“ / „1 fehlt“), accent while the list API counts it as complete („komplett“), none otherwise;
  - a discover tile: accent while the title is in the library („In der Bibliothek“), danger while it is excluded („Ausgeschlossen“), none otherwise.
- [ ] AC-03 (event): When a poster tile becomes open on a hover device, then within 200 ms it grows to between 105 % and 110 % of its size, lies above its neighbours (it covers them, they never cover it), and its overlay shows over the lower part of the poster, at most over the whole poster, never outside the tile:
  - the title in up to two lines, cut with an ellipsis;
  - the year and, for a library title, its addition (`ui-polish` AC-03);
  - the rating („★ 7,8“, or „–“), while ratings are on;
  - the status chips of today (library) or the „Hinzufügen“ button (discover, where it shows today).

  When the tile is no longer open, it returns to its size and the overlay hides. Where the system asks for reduced motion, the tile does not grow and the overlay shows at once.
- [ ] AC-04 (always): Every text of an overlay meets a contrast of at least 4.5:1 against the poster under it, whatever the poster: measured over an all-white and an all-black poster at 1440 px (an open tile) and at 360 px on a touch device (AC-06).
- [ ] AC-05 (always): A poster tile's link is named with the full title, its year and its rating with the votes („Nordlicht über Tromsø, 2023, IMDb 7,6 (48.210 Stimmen)“; without a rating „IMDb –“, nothing while ratings are off), at rest and open; its tooltip is the full title. The controls in the overlay are reachable by keyboard: Tab into a tile opens it, and every control in it then has a visible focus. The „Hinzufügen“ button's accessible name stays as today.
- [ ] AC-06 (state): While the device is not a hover device, every poster tile shows its overlay at rest, without growing.
- [ ] AC-07 (state): While the movie or series list is in selection mode, the behaviour of `ui-list-items` AC-05 and AC-06 stays: every tile shows its checkbox, a selected tile keeps its accent outline, and an open tile's overlay never covers the checkbox or the monitor toggle.
- [ ] AC-08 (always): Within a row of a poster grid all tiles at rest are equally high (their posters' height), and a shelf's tiles end at one edge; an open tile does not move its neighbours.
- [ ] AC-09 (always): The movie and series lists in poster view and the discover start page, search, a shelf page and a person page meet WCAG 2.1 AA in the dark palette at 1440 px and 360 px (axe scans), with a tile open and with none.
- [ ] AC-10 (always, manual: images): The website screenshots `movies.png`, `discover.png` and `person.png` under `docs/site/screenshots/` are regenerated and show the tiles at rest; the owner checked the three files.
- [ ] AC-11 (state, manual: visual design): While the dark palette is active, the movie list in poster view and the discover start page look like a streaming service's library at 1440 px: posters carry the page, an open tile reads clearly, nothing jumps.

## Technical notes

The notes are suggestions for the build; the ACs are binding.


- **Components:** `ui/src/lib/library/components/MediaItems.svelte` (cards) and `ui/src/lib/discover/components/TitleTile.svelte`; the shared look as one component or shared CSS, so both kinds of tile behave alike.
- **Overlay text:** a gradient from transparent to the page ground behind the overlay's text; AC-04 is measured like `ui-cinema-backdrops` AC-07 (`ui/e2e/cinema-backdrops.spec.ts`).
- **Hover device:** `@media (hover: hover)` and `(prefers-reduced-motion: reduce)` in CSS; Playwright emulates both (`hasTouch`, `reducedMotion`).
- **Tests:** Playwright in a new `ui/e2e/cinema-tiles.spec.ts` on the library, ratings and discover servers; older tests that assert text below the poster (`ui-polish.spec.ts` AC-03 to AC-05 in poster view, `library-items.spec.ts`, `ui-consistency.spec.ts` AC-03/AC-04, `imdb-ratings.spec.ts`, the discover tile tests) are adjusted to read the overlay, not removed.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A tile in the first or last column grows | It is not cut by the grid's edge; in a shelf, only by the shelf's visible area |
| A shelf scrolled sideways, a tile half outside | It opens as far as it is visible; the shelf does not jump |
| A title without a poster | The placeholder of today, with the state mark and the same overlay |
| A rating that is still pending (discover) | The overlay's rating line stays empty (`ui-polish` AC-02); the link's name says „IMDb-Bewertung folgt“ |
| Ratings switched off | No rating in the overlay or the link's name |
| The pointer moves fast across a row | Each tile opens and closes; at most one tile is open at a time |
| The focus is in one tile, the pointer over another | The tile under the pointer is open |
| The add dialog opens from an open tile and is cancelled | The focus returns to the tile's button; the tile is open while it has the focus |
| The add dialog adds the title | The button gives way to the library state; the focus moves to the tile's link; the state mark turns accent |
| Unmonitored library title | Its poster stays dimmed (`ui-list-items`), also when open |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 105–110 % growth | Large enough to read as „lifted“, small enough not to cover the neighbours' marks | Low |
| The link's name carries year and rating | At rest nothing else tells a screen reader user what the visible text told before | Low |
| Touch devices show the overlay at rest | D34 keeps touch usable without designing for it; a tile without any text would be unusable there | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `at rest on a hover device (AC-01)` › `a library tile is only its poster, with its controls and its mark`, `a discover tile is only its poster and its mark` (`ui/e2e/cinema-tiles.spec.ts`) | built |  |
| AC-02 | `state mark (AC-02)` › `a movie: accent with a file, danger while missing, none otherwise`, `a series: danger while episodes are missing, accent when complete`, `a discover tile: in the library, excluded, or none` (`ui/e2e/cinema-tiles.spec.ts`); `a movie's state mark …`, `a series' state mark …` (`ui/src/lib/library/rows.test.ts`); `marks the library state on the poster` (`ui/src/lib/discover/texts.test.ts`) | built |  |
| AC-03 | `open tile on a hover device (AC-03)` › `a library tile grows and shows title, year, addition, rating and chips`, `a long title is cut after two lines with an ellipsis`, `a discover tile grows and shows title, year, rating and the add button`, `one tile at a time: the one under the pointer before the focused one`, `with reduced motion the tile does not grow and the overlay shows at once` (`ui/e2e/cinema-tiles.spec.ts`); `poster tiles (AC-04, AC-05)` (`ui/e2e/ui-polish.spec.ts`) | built |  |
| AC-04 | `contrast over an all-white / all-black poster (AC-04)` › `1440 px, an open tile` › `library tile`, `discover tile movie/502`, `discover tile movie/501`; `360 px, a touch device` › `library tiles`, `discover tiles` (`ui/e2e/cinema-tiles.spec.ts`) | built |  |
| AC-05 | `name and keyboard (AC-05)` › `a library tile is named with title, year and rating with votes`, `a discover tile is named with title, year and rating; nothing while off`, `Tab into a discover tile opens it; its button shows its focus`, `Tab into a library tile opens it; checkbox and toggle show their focus` (`ui/e2e/cinema-tiles.spec.ts`); `poster tile name (ui-cinema-tiles AC-05)` (`ui/src/lib/components/tile.test.ts`) | built |  |
| AC-06 | `touch device (AC-06)` › `library and discover tiles show their overlay at rest, without growing` (`ui/e2e/cinema-tiles.spec.ts`) | built |  |
| AC-07 | `selection mode (AC-07)` › `checkboxes show, the selected tile is outlined, the overlay leaves the controls free` (`ui/e2e/cinema-tiles.spec.ts`); `selection checkbox on a hover device (AC-05, AC-06)` (`ui/e2e/library-items.spec.ts`) | built |  |
| AC-08 | `even tiles (AC-08)` › `a grid row is as high as its posters; an open tile moves no neighbour`, `a shelf's tiles end at one edge` (`ui/e2e/cinema-tiles.spec.ts`) | built |  |
| AC-09 | `accessibility in the dark palette (AC-09)` › `<page> at 1440 / 360 px, at rest and with a tile open` for the movie and series lists in poster view, the discover start page, search, a shelf page and a person page (`ui/e2e/cinema-tiles.spec.ts`) | built |  |
| AC-10 | `bun run screenshots` regenerated `docs/site/screenshots/` on 2026-11-22; `movies.png`, `discover.png` and `person.png` show the tiles at rest with their state marks. The owner's check (`manual:`) is open | built |  |
| AC-11 |      | open   |        |

## Decisions

- 2026-11-22: At rest a tile is only its poster with a state mark; its information shows in an overlay on hover or focus (owner), because a grid of posters reads as a library only without text under every picture.
- 2026-11-22 (build): Both kinds of tile are one component, `ui/src/lib/components/PosterTile.svelte` (poster, overlay and corner controls as snippets), with the pure helpers `tileName` and `Mark` in `ui/src/lib/components/tile.ts`; the library rows carry their `mark`, the discover texts have `libraryMark`. The movie and series list keep their rows in list view unchanged.
- 2026-11-22 (build): The open tile is pure CSS: `:hover`, else `:focus-within` while no tile of the same list is hovered (`:has`), only under `@media (hover: hover)`; the frame scales by 7 % and gets `z-index: 1` over neighbours that are each their own stacking context. The overlay hides with `opacity` (not `visibility`), so its button stays in the Tab order and Tab into a tile opens it.
- 2026-11-22 (build): The overlay's title and rating are `aria-hidden`, because the link's name already carries them; year, addition, chips and the button stay readable. Older tests that read the rating's own name now read the link's name (`discover-pages`, `discover-person`, `imdb-ratings`, `ui-consistency`, `ui-polish`); the tests of `ui-polish` AC-04/AC-05 now check the overlay's two title lines and the even rows; tiles are found by their tooltip instead of an exact link name (`library-items`, the website screenshots). None removed.
- 2026-11-22 (build): The overlay is a gradient from transparent to the page ground over 1.5 rem and the full ground behind its text; the state mark sits in the overlay's lowest line, below the text, with a ring of the page ground so it reads on any poster at rest. The overlay leaves the top 2.5 rem of the poster free for checkbox and toggle (AC-07).
- 2026-11-22 (build): Red runs: before the build 17 of 18 tests of `cinema-tiles.spec.ts` (AC-01 to AC-08) failed; `a shelf's tiles end at one edge` (AC-08) was green at once, the shelves were already even. The contrast (AC-04) and axe (AC-09) tests were written after the overlay and were green at once, except the touch test of the discover tiles: „Hinzufügen“ wrapped in a 128 px tile and reached the button's border (3.3:1); the button now stays on one line with a smaller padding. To prove the contrast test can fail, the overlay's ground was set to 60 % for one run: five of ten tests failed (2.3:1 and 4.4:1), then restored.
- 2026-11-22 (build): Shorter tiles exposed a gap of the shelf page: its sentinel stayed in reach after the first page and the observer never reported it again, so the page stopped at 20 titles. The sentinel is now observed afresh after every load; `"Mehr" opens the whole shelf` (`discover-pages.spec.ts`) no longer expects exactly 20 tiles first, because a window that still shows the end loads the next page at once.
- 2026-11-22 (build): A pending rating keeps an empty line in the overlay (`.pending`, which the screenshots' wait for ratings looks for). After the add dialog added a title from a tile, the focus moves to the tile's link (edge case, not covered by its own test).
