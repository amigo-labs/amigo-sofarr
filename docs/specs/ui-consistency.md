---
status: todo
created: 2026-11-17
size: M
design: [sofarr.md §3, sofarr.md §9 D29, sofarr.md §9 D30]
media: both
oracle: none
---

# UI redesign 9: consistency

## Context

The redesign series (D30, `ui-shell-redesign` to `ui-discover-polish`) gave every page the same frame. A review of the screenshots of 2026-11-17 (dark and light, 1440 px and 390 px) still found breaks between the pages:

- **The accent means too much.** Next to the states that rightly carry it (the active navigation, the monitor icon, the quality chips, „verfügbar“, the main action), every title link, the episode disclosures and the info notices are in sofarr's green too. Even the shadow-mode notice of the file actions (a restriction) looks like a success. The colour no longer tells the user anything.
- **Discover differs from the library.**
  - Its tiles show „IMDb 7,8 (112.400 Stimmen)“ over two lines where the library shows „★ 7,8“.
  - Its search field has no magnifier.
  - Its add button is taller than the „In der Bibliothek“ chip, so the tiles of a shelf end at different heights.
  - At 390 px „Suchen“ and „Filter“ sit on two lines of their own, although `ui-discover-polish` AC-05 puts them on one line below the field.
- **Wording:** English words in the German UI („Trending heute“, „Trending diese Woche“, „Blocklist“), and raw values („Serientyp standard“).
- **Technical details too loud:**
  - Time stamps with seconds („Stand: 17.11.2026, 10:23:41“).
  - The configuration syntax `([automation] mode = "live")` in the file actions note.
  - The episode counts as „19 mit Datei / 24 überwacht / 24 gesamt“.
- **Episode rows** carry two accent-coloured disclosures („Datei“, „Untertitel“) on two lines of their own, which doubles every row.
- **Folder paths** break inside a folder name („/tmp/sofarr-↵site“).
- **The sidebar** shows a generic server icon. sofarr's own monogram exists in `assets/brand/` (made by the website, D29).

This spec fixes these breaks. It changes the look only; what each page shows and does stays as its specs say, except where an AC below revises one.

Revised specs:

- AC-03 revises `imdb-ratings` AC-04: the title tiles of discover, search, shelf and person pages show the votes in the tooltip, as the library lists already do; the movie, series and preview pages keep them visible.
- AC-06 restores `ui-discover-polish` AC-05.
- AC-11 revises the last sentence of `ui-detail-boxes` AC-06.

Terms:

- **Accent**: sofarr's green (`--fgColor-accent` and the accent chip tone).
- **Title link**: a link whose text is the name of a movie, series, episode, person or shelf, in lists, tiles, rows, boxes and the calendar; also the „Mehr“ link beside a shelf heading, because it sits in the same heading row as the shelf name and should look the same.
- **Neutral muted ground**: the theme's `--bgColor-muted` with `--borderColor-default`, as the neutral status chips use them.
- **Caption style**: the theme's small text size in `--fgColor-muted`, as the field labels of the facts panels use it.
- **Preview page**: the page of a title not (or not yet) in the library (`/entdecken/filme/{id}`, `/entdecken/serien/{id}`, ui-discover-polish).
- **Narrow window**: below 768 px, the breakpoint of every redesign spec since `ui-detail-boxes`. Narrow checks run at 360 px (the smallest phone width the redesign specs test) and 390 px (the screenshots of 2026-11-17).
- **Main action**: the one primary button of a page or dialog (e.g. „Hinzufügen“, „Speichern“, „Alle 2 suchen“).
- **Date-time**: a moment shown with date and clock time (last and next runs, „Stand“, grabs, backups).

## Scope

### In scope

- The colour of title links, info notices and episode disclosures.
- Discover tiles, the discover search row.
- Visible German wording of shelves, the blocklist page and series types; the episode counts on the series page.
- The format of date-times across the UI.
- The file actions note (its server text).
- Episode rows on the series page.
- Folder and file paths.
- The sidebar brand and the browser tab icon.

### Out of scope

- The overview's layout, figures and boxes — because they get their own spec (`ui-overview-polish`); only the wording changes of AC-07 and the date-times of AC-09 reach the overview, because they apply to every page.
- Page addresses (`/blocklist` stays) — because bookmarks and the addresses in the specs of today must keep working.
- „Cutoff“ — because it is the term of the quality profiles the user knows from Radarr and Sonarr; there is no common German word for it.
- The status chip tones (accent for a quality, success for „verfügbar“, danger for „fehlt“) — because `ui-list-items` and `ui-detail-head` settled them; this spec only takes the accent away from what is not a state.
- New palettes or a new accent — because the colours come from the shared amigo theme (D29).

## Acceptance criteria

- [ ] AC-01 (always): sofarr shows title links in the default text colour, not underlined. While hovered or focused they turn to the accent and are underlined.
  - Prose links in sentences and notices stay as today (accent, underlined; ui-shell-redesign AC-12).
  - The active navigation entry, the main action, selected tabs and segments, the monitor toggle while monitored and the accent status chips keep the accent.
- [ ] AC-02 (always): An info notice uses the neutral muted ground with the default text colour, so it no longer looks like a success. Warning and danger notices stay as today.
- [ ] AC-03 (always): A discover tile shows the rating compactly as the library rows do („★ 7,8“; „–“ without one or when it could not be fetched; the placeholder of today while it is pending), with the full text („IMDb 7,8 (112.400 Stimmen)“) as its tooltip and accessible name. The rating takes one line at 1440 px and 360 px.
- [ ] AC-04 (always): Within one shelf or result list, the library state chip („In der Bibliothek“, „Ausgeschlossen“) and the „Hinzufügen“ button have the same height, so tiles with the same number of title lines end at the same height.
- [ ] AC-05 (always): The discover search field shows a magnifier at its start, as the library search field does. Enter and „Suchen“ start the search as today.
- [ ] AC-06 (state): While the window is narrow, the discover search field takes the full width and „Suchen“ and the filter button share the next line, at 360 px and 390 px (ui-discover-polish AC-05).
- [ ] AC-07 (always): The UI says „Heute beliebt“ and „Diese Woche beliebt“ instead of „Trending heute“ and „Trending diese Woche“ (shelf headings, shelf pages, arrow and „Mehr“ names), and „Sperrliste“ instead of „Blocklist“ (navigation entry, page heading, page title, the overview's links to it). The addresses stay.
- [ ] AC-08 (always): The series page shows the series type in German:
  - „Standard“, „Anime“, „Täglich“ for `standard`, `anime`, `daily`
  - the raw value for any other; „–“ when the series has none

  The episode fact reads „{mit Datei} von {gesamt} mit Datei“ with „{überwacht} überwacht“ on a second, muted line, also when a number is 0 („0 von 0 mit Datei“, „0 überwacht“).
- [ ] AC-09 (always): sofarr's web UI shows every date-time it formats itself to the minute, without seconds (e.g. „17.11.2026, 10:23“): last and next runs on System, „Stand“ on discover and the queue, backups, grabs, blocklist entries, subtitle downloads. A value that is not a valid date-time shows as it came, as today. The discover „Stand“ line uses the caption style.
- [ ] AC-10 (state): While sofarr runs in shadow mode, the file actions note reads „Dateiaktionen (Umbenennen, Ordner verschieben, Dateien löschen) gibt es nur im Live-Modus.“ followed by „Überwachung, Profil und Entfernen ohne Dateien funktionieren trotzdem.“ No configuration syntax is shown. The note follows the mode the page was loaded with, as today. The API's `file_actions.reason` carries the new text.
- [ ] AC-11 (always): An episode row with a file shows one disclosure line below its main line (a row without a file shows none, as today):
  - „Datei“ and „Untertitel“ side by side, in the muted text colour (the accent on hover and focus), with a chevron that shows whether each is open
  - each opens its content below that line in the row's full width, as today (web-ui-library AC-06, subtitles AC-12)
  - both can be open at once, as today, because file and subtitles are often checked together
  - their accessible names name the episode as today („Datei von …“, „Untertitel von …“)
- [ ] AC-12 (always): A folder or file path breaks after a „/“ or „\“ (also both in one path) when it does not fit its box, and only inside a name when that name alone is wider than the box. It keeps the monospace font and the full path as tooltip (ui-detail-boxes AC-03).
- [ ] AC-13 (always): The sidebar brand, the top bar on narrow windows and the browser tab show sofarr's own monogram (the pixel-art „So“ of the amigo brand, the same image as amigo-labs.dev shows for sofarr) instead of the server icon. The monogram is part of the built UI, so it shows without any request leaving sofarr. The brand link keeps its accessible name „sofarr“.
- [ ] AC-14 (always): The pages changed here (movie list, series page with a file opened, discover start page, a preview page, Sperrliste, System) meet WCAG 2.1 AA in both palettes at 1440 px and 360 px (the automated accessibility scans of the other UI specs). The hovered title link's accent on the page ground has a contrast of at least 4.5:1 in both palettes (computed from the theme colours in a test).
- [ ] AC-15 (state, manual: visual design): While either palette is active, the library lists, a series page, discover and the calendar read calmer than in the screenshots of 2026-11-17: green marks only states, the active place and the main action.

## Technical notes

- **Title links:** `a.ui-link` in `ui/src/app.css` gets the default colour and the accent on hover/focus. Elements that should keep the accent (navigation, chips, buttons) have their own classes today.
- **Info notice:** `.flash-info` in `ui/src/lib/components/Flash.svelte`.
- **Compact rating:** `ratingCompactText` and `ratingWithVotesText` of `ui/src/lib/imdb/imdb.ts` (the library rows' `rating` and `ratingFull`).
- **Date-times:** `dateTimeText` in `ui/src/lib/system/format.ts` (`timeStyle: 'short'`).
- **File actions note:** `NEEDS_LIVE` in `crates/sofarr-server/src/library_detail.rs`.
- **Paths:** a pure, unit-tested helper splits a path after its separators (`<wbr>` or zero-width break opportunities) with `overflow-wrap: anywhere` as fallback.
- **Monogram:** `assets/brand/monogram-small.svg` (16 px pixel art, `shape-rendering: crispEdges`) is generated by the website (D29); it is used as is, not redrawn.
- **Tests:** Playwright in a new `ui/e2e/ui-consistency.spec.ts`; unit tests next to the helpers; existing tests that assert the old texts are updated, not removed.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A title link inside a prose notice (e.g. the add dialog's „bereits in der Bibliothek“) | Stays a prose link: accent, underlined |
| A pending rating on a discover tile | Shows „…“; its tooltip and accessible name read „IMDb-Bewertung folgt“ |
| The monogram file is missing from the build | The UI build fails (it is imported, not fetched) |
| Resizing across 768 px while typing a discover search or with the filter panel open | The text and the panel state stay; only the layout changes |
| A path without any separator, 300 characters | Breaks inside the name, never overflows |
| An empty path | Shown empty, as today |
| A Windows path `D:\Filme\X` | Breaks after `\` like after `/` |
| An unknown series type from the API | Its raw value |
| A date-time with seconds ≥ 30 | Shown truncated to its minute, as `Intl` formats it without seconds (not rounded) |
| A tile in a shelf whose title takes two lines | May end lower than its one-line neighbours; only chip and button heights are equal |
| Light palette | Title links in the light default text colour; hover accent meets AA |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| „Sperrliste“ is the better German word for the blocklist | It is the usual German term in download tools; the address stays `/blocklist` | Low: a label |
| „Heute beliebt“ / „Diese Woche beliebt“ describe TMDB's trending lists | TMDB orders them by popularity in the given window | Low |
| Seconds carry no meaning for the user in any date-time | Runs are scheduled in minutes or hours; no test of today needs seconds to tell two moments apart | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `title links and notices (AC-01, AC-02), dark/light` › `a title link rests in the text colour and turns to the accent on hover`, `the active navigation entry keeps the accent` (`ui/e2e/ui-consistency.spec.ts`) | built |  |
| AC-02 | `title links and notices (AC-01, AC-02), dark/light` › `an info notice has the neutral muted ground and the text colour` (`ui/e2e/ui-consistency.spec.ts`) | built |  |
| AC-03 | `discover tiles (AC-03, AC-04)` › `a tile shows the compact rating with the full text as tooltip and name` (`ui/e2e/ui-consistency.spec.ts`); `compact tile rating (ui-consistency AC-03)` (`ui/src/lib/discover/texts.test.ts`); the tile rating checks of `ui/e2e/discover-pages.spec.ts` and `ui/e2e/discover-person.spec.ts` | built |  |
| AC-04 | `discover tiles (AC-03, AC-04)` › `the library state chip and the add button have the same height` (`ui/e2e/ui-consistency.spec.ts`) | built |  |
| AC-05 | `discover search (AC-05, AC-06)` › `a magnifier sits at the start of the search field` (`ui/e2e/ui-consistency.spec.ts`) | built |  |
| AC-06 | `discover search (AC-05, AC-06)` › `at 360 px …`, `at 390 px the field takes the width, „Suchen“ and „Filter“ share the next line` (`ui/e2e/ui-consistency.spec.ts`); `narrow window (AC-05)` (`ui/e2e/discover-polish.spec.ts`) | built |  |
| AC-07 | `wording and date-times (AC-07 to AC-10)` › `„Sperrliste“ in navigation, heading and page title; the address stays`, `discover says „Heute beliebt“ and „Diese Woche beliebt“` (`ui/e2e/ui-consistency.spec.ts`); `shelves_come_in_order_with_library_state_and_ratings_that_load` (`crates/sofarr-server/src/discover/tests.rs`); `navigation` (`ui/src/lib/nav.test.ts`) | built |  |
| AC-08 | `wording and date-times (AC-07 to AC-10)` › `the series page: type in German, episode counts on two lines, the note without syntax` (`ui/e2e/ui-consistency.spec.ts`); `series type (ui-consistency AC-08)` (`ui/src/lib/library/format.test.ts`) | built |  |
| AC-09 | `job texts` › `writes points in time as German date and time, to the minute (ui-consistency AC-09)` (`ui/src/lib/system/format.test.ts`); `wording and date-times (AC-07 to AC-10)` › `the discover „Stand“ line is a caption to the minute` (`ui/e2e/ui-consistency.spec.ts`); the date-time checks of `ui/e2e/system.spec.ts`, `ui/e2e/discover-pages.spec.ts`, `ui/e2e/library-pages.spec.ts` | built |  |
| AC-10 | `movie_page_shows_everything_about_one_movie` (`crates/sofarr-server/src/library_api_tests.rs`); `wording and date-times (AC-07 to AC-10)` › `the series page: …` (`ui/e2e/ui-consistency.spec.ts`) | built |  |
| AC-11 | `episode disclosures (AC-11)` › `„Datei“ and „Untertitel“ share one muted line; both open in the full row width` (`ui/e2e/ui-consistency.spec.ts`); the episode file test of `ui/e2e/library-detail.spec.ts` and the episode subtitles test of `ui/e2e/subtitles.spec.ts` | built |  |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |

## Decisions

- 2026-11-17: Title links lose the accent at rest, because the green otherwise marks nearly every line and no longer tells states and the active place apart. The hover accent keeps them recognisable as links.
- 2026-11-17: The user asked to do everything without further questions („Ja mach alles“); the wording choices (Sperrliste, Heute beliebt) and the date-time format were decided here and are recorded as assumptions.
- 2026-11-17: The overview's dashboard look is split off into `ui-overview-polish`, because both together exceed one M block.
- 2026-11-17 (build): Red run of block A against 9f58f78: `bunx playwright test e2e/ui-consistency.spec.ts` failed 5 of 7 (title links in the accent, the info notice on the accent ground, no episode toggle buttons); the two navigation tests were green at once (the active entry already had the accent) and stay as guards.
- 2026-11-17 (build): The episode disclosures became two toggle buttons (`aria-expanded`) with their contents below, because two `<details>` cannot share one line and still open in the row's full width. On narrow windows they are 32 px high, because `ui-detail-boxes` AC-07 measures every button of a season.
- 2026-11-17 (build): Red run of block B against e399146: the five discover tests of `ui/e2e/ui-consistency.spec.ts` failed (full rating text, chip 10 px lower than the button, no magnifier, „Suchen“ 40 px above „Filter“) and `ratingCompact` did not exist.
- 2026-11-17 (build): The pending placeholder is checked by the unit test only, because the fake TMDB releases a held-back rating for every test at once and the tests run in parallel.
- 2026-11-17 (build): Chip and add button of a tile are both 28 px high, because 32 px made the chip a heavy block; 28 px stays above the 24 px target size of WCAG 2.2.
- 2026-11-17 (build): The filter button is passed into the search form, so that on narrow windows it can share the line with „Suchen“ while the field takes its own; „Suchen“ lost its icon, because the magnifier now sits in the field.
- 2026-11-17 (build): One run of `ui/e2e/discover-a11y.spec.ts` reported a contrast violation on the add dialog's primary button in the light palette; two reruns of the file were green (42 of 42). The scan probably caught the button mid-transition after the click.
- 2026-11-17 (build): Red run of block C with its implementation stashed: `bunx vitest run` failed 7 (navigation names, date-times with seconds, no `seriesTypeText`); `cargo test -p sofarr-server discover::tests` and `library_api_tests` failed on „Trending heute“ and the configuration syntax. The discover „Stand“ line already had the caption style; its e2e check was green at once and stays as a guard.
- 2026-11-17 (build): The shelf names come from the server (`discover/shelves.rs`) and the UI (`discover/links.ts`); both changed. The slugs `trending-heute` and `trending-woche` stay, because they are addresses.
- 2026-11-17 (build): The file actions note joins the server's reason and the UI's second sentence with a full stop the busy reasons lacked („… erst danach möglich Überwachung …“), through a small `sentence` helper.
- 2026-11-17 (build): The season headers keep their counts of today („8 mit Datei / 8 überwacht / 8 gesamt“, ui-detail-boxes AC-04), because AC-08 names only the series page's episode fact.
- 2026-11-17 (build): Times the server writes into its own German messages (e.g. the login lock „Nächster Versuch ab 10:23:45 Uhr“) keep their format, because AC-09 covers the date-times the UI formats itself.
- 2026-11-17 (build): The first full gate run found 11 older e2e tests on the old look: the episode `<summary>` disclosures in `ui/e2e/extras-a11y.spec.ts`, times with seconds in `ui/e2e/subtitles.spec.ts` and `ui/e2e/trailers.spec.ts`, and the image test of discover AC-17, which now also sees sofarr's own `favicon.svg` (the monogram); it still allows only images from sofarr. Second full run: all gates green, 534 e2e tests passed.
