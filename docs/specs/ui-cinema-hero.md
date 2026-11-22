---
status: todo
created: 2026-11-22
size: M
design: [sofarr.md §3, sofarr.md §9 D19, sofarr.md §9 D24, sofarr.md §9 D34]
media: both
oracle: none
---

# Cinema look 3: hero banner on discover

## Context

The discover start page (`/entdecken`, spec `discover` AC-01) opens with its heading, the search and seven shelves. Nothing on it is large: every title is a tile. The cinema look (D34) asks for a hero banner, one featured title across the page with its backdrop.

The owner decided on 2026-11-22: the featured title is the most popular title of „Heute beliebt“ that has a backdrop and is not in the library yet; it changes with TMDB's daily answer.

The shelves answer (`GET /api/discover/shelves`) carries title cards without backdrop or overview. The preview answer of one title (`GET /api/discover/titles/{kind}/{id}`) carries both and is cached for 24 hours (`discover` AC-13).

This spec builds on `ui-cinema-backdrops` (`HeadBackdrop`, its scrim and contrast rule) and `ui-cinema-tiles`.

## Terms

- **Hero**: the banner above the shelves of the discover start page.
- **Featured title**: the title the hero shows.
- **Candidate**: a title of the shelf „Heute beliebt“ that is neither in the library nor excluded.

## Scope

### In scope

- The hero on the discover start page while no filter is set, in the dark palette.
- The featured title in the shelves answer (an additive field).

### Out of scope

- A hero on the search, shelf, person or library pages — because the owner asked for it on discover.
- Rotating or choosing between several featured titles — because the owner chose one title per day.
- Trailers or moving images in the hero — because they need a player in the page, which no spec has asked for.
- The light palette — because D34 designs the dark palette only.

## Acceptance criteria

- [ ] AC-01 (event): When the discover start page shows its shelves (no filter set), then above the first shelf a hero shows the featured title: the first candidate, in the order of „Heute beliebt“, whose preview has a backdrop. sofarr looks at no more than the first 10 candidates.
- [ ] AC-02 (always): The hero shows:
  - the featured title's backdrop, sharp, across the content's full width, under the scrim of `ui-cinema-backdrops` AC-05, at least 420 px high from 1024 px and as high as its content below;
  - the title as a level-2 heading, the kind and the year („Film · 2024“) and the rating of the tiles („★ 7,8“), while ratings are on;
  - the overview, cut after three lines;
  - the button „Hinzufügen“, which opens the add dialog of today for the title, and the link „Details“ to the title's preview page.
- [ ] AC-03 (always): Every text of the hero meets a contrast of at least 4.5:1 over any backdrop: measured over an all-white and an all-black backdrop at 1440 px and 360 px.
- [ ] AC-04 (state): While no candidate among the first 10 has a backdrop, or „Heute beliebt“ could not be loaded, or a filter is set, the page shows no hero and the shelves stand as today.
- [ ] AC-05 (event): When the featured title is added from the hero, then the hero shows „In der Bibliothek“ instead of „Hinzufügen“ and a link „Zur Bibliothek“ to the title's library page; the next load of the page features the next candidate.
- [ ] AC-06 (always): The shelves answer carries the featured title (`hero`: kind, TMDB id, title, year, overview, sofarr's backdrop address, rating, library state), or `null` when there is none. All fields of today stay. Its backdrop is served through sofarr's image cache (D24); the browser asks no other host.
- [ ] AC-07 (error): If the preview of a candidate cannot be loaded (TMDB unreachable, an error, no cached answer), then sofarr skips that candidate and looks at the next one; the shelves answer is not delayed by more than the preview requests of today's 24-hour cache.
- [ ] AC-08 (always): The page's freshness line (`ui-polish` AC-10) names the oldest fetch time of the shelves and of the featured title's preview.
- [ ] AC-09 (always): The discover start page with a hero meets WCAG 2.1 AA in the dark palette at 1440 px and 360 px (axe scans), and the hero's button and link are reachable by keyboard with a visible focus.
- [ ] AC-10 (always, manual: images): The website screenshot `discover.png` is regenerated and shows the hero; the owner checked the file.
- [ ] AC-11 (state, manual: visual design): While the dark palette is active, the discover start page with its hero looks like the start page of a streaming service at 1440 px.

## Technical notes

- **Server:** `crates/sofarr-server/src/discover/shelves.rs` builds the shelves answer; the featured title is chosen there from the cached „Heute beliebt“ page and the cached previews (`titles.rs`), so the browser needs one request as today. `ShelvesAnswer.hero` is a new optional field; `ui_types_match_the_server` covers it.
- **UI:** `ui/src/lib/discover/pages/DiscoverPage.svelte`; the backdrop with `HeadBackdrop.svelte`; the add dialog of today (`AddDialog.svelte`).
- **Tests:** server tests with the fake TMDB (`tmdb_fake.rs`) for AC-01, AC-04, AC-06 to AC-08; Playwright in a new `ui/e2e/cinema-hero.spec.ts` on the discover server for AC-02 to AC-05 and AC-09.
- **Screenshots:** the demo TMDB (`ui/e2e/site/demo-tmdb.mjs`) already serves backdrops for its titles.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Every title of „Heute beliebt“ is in the library | No hero |
| The first candidate has no backdrop, the second has one | The second is featured |
| TMDB is unreachable, the cache holds yesterday's answers | The hero shows the cached featured title; the stale notice of today shows on top |
| The featured title is added in another tab | The next load features the next candidate |
| The featured title is excluded | It is no candidate; the next one is featured |
| No TMDB key | The discover pages show their German message of today (`discover` AC-15); no hero |
| A very long overview | Three lines, then an ellipsis; the full text is on the preview page |
| A window of 360 px | The hero stacks: backdrop, then title, data, overview and actions; no horizontal scrolling |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| At most 10 candidates are looked at | Each needs a preview; ten keep a cold start within a few TMDB requests | Low: a day where none of ten has a backdrop shows no hero |
| The hero has no poster | The backdrop and the text carry it; the poster is on the preview page | Low |

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

## Decisions

- 2026-11-22: The featured title is the first title of „Heute beliebt“ with a backdrop that is neither in the library nor excluded (owner), because the hero should suggest something new and the shelf already orders by today's popularity.
- 2026-11-22: The server chooses the featured title and sends it with the shelves, because the choice needs previews from TMDB's cache, and the page should stay one request.
