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
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |

## Decisions

- 2026-11-17: Title links lose the accent at rest, because the green otherwise marks nearly every line and no longer tells states and the active place apart. The hover accent keeps them recognisable as links.
- 2026-11-17: The user asked to do everything without further questions („Ja mach alles“); the wording choices (Sperrliste, Heute beliebt) and the date-time format were decided here and are recorded as assumptions.
- 2026-11-17: The overview's dashboard look is split off into `ui-overview-polish`, because both together exceed one M block.
