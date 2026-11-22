---
status: todo
created: 2026-11-22
size: M
design: [sofarr.md §3, sofarr.md §9 D24, sofarr.md §9 D34]
media: both
oracle: none
---

# Cinema look 1: real backdrops on the title pages

## Context

The owner wants the web UI to feel more like a streaming service than like an administration tool (2026-11-22, D34): large images, rows to browse, titles first. The cinema look is a series of five specs (spec index); this is the first.

Today the head of a movie or series page (`ui-detail-head`) shows the title's poster, blurred beyond recognition, as its ground (`ui-polish` AC-01). The preview page of discover shows the title's wide TMDB image the same way, blurred (`ui-discover-polish` AC-04). Nothing on these pages looks like the film.

sofarr knows a wide image only for discover previews. For library titles it stores the poster the metadata provider offers (migration 0007, `web-ui-library`), nothing else.

This spec gives library titles a backdrop and shows it sharp behind the head of the three title pages. It revises:

- `ui-detail-head` AC-01 and `ui-polish` AC-01 (the blurred poster stays only as the fallback of AC-06 here),
- `ui-discover-polish` AC-04 (the preview's backdrop is no longer blurred).

Decided in D34: the look stays within the shared amigo theme (D29: its colours, radii and controls), only the dark palette is designed, and touch layouts are not a goal.

## Terms

- **Backdrop**: the wide image (about 16:9) a metadata provider offers for a title, as opposed to the upright poster.
- **Title page**: the movie page, the series page and the preview page of discover.
- **Head**: the top section of a title page with poster, title, chips, rating, overview, actions and facts (`ui-detail-head`).
- **Scrim**: a layer in the colour of the page ground between the backdrop and the head's content, which covers the whole backdrop.
- **Sharp**: shown without any blur filter.

## Scope

### In scope

- A backdrop for library movies and for library series with a TMDB id, taken from TMDB at the metadata refresh.
- The backdrop of a library title served by sofarr from its image cache (D24).
- `has_backdrop` in the answers of the library API.
- The head of the three title pages with the sharp backdrop, in the dark palette.

### Out of scope

- A backdrop for series without a TMDB id — because TVDB's wide images need another provider path; such series keep the fallback of AC-06.
- Backdrops in lists, rows or a hero banner — because they are `ui-cinema-hero` and `ui-cinema-home`.
- The light palette — because D34 designs the dark palette only. Until `ui-cinema-shell` removes it, the light palette shows the same layout, its scrim in the light page ground; its contrast is covered by an axe scan (AC-10), not by AC-07 or the owner's eye.
- Logos, trailers or moving images in the head — because they need new provider data and are not part of the owner's request.
- A choice between several backdrops of a title — because the provider's first choice is good enough for one user.

## Acceptance criteria

- [ ] AC-01 (event): When sofarr refreshes the metadata of a library movie, then it stores the backdrop TMDB names for the movie; a movie for which TMDB's answer names none has none afterwards. A refresh that gets no movie details from TMDB (unreachable, an error status, an unreadable answer) keeps the stored one, as it keeps the other metadata today.
- [ ] AC-02 (event): When sofarr refreshes a library series that has a TMDB id, then it stores the backdrop TMDB names for the series; a series without a TMDB id, or for which TMDB names none, has none. If TMDB gives no usable answer for the series (unreachable, an error status, an unreadable answer), then the refresh of the series still succeeds as today and the stored backdrop stays.
- [ ] AC-03 (always): The library API tells for every movie and series, in the lists and on the detail answers, whether it has a backdrop (`has_backdrop`). All fields of today stay, so existing clients keep working.
- [ ] AC-04 (event): When a client asks sofarr for the backdrop of a library movie or series, then sofarr answers with the image at a width of 1280 px from its image cache, fetching it from TMDB only when the cache has none; the browser never asks TMDB itself (D24). A title without a backdrop, an unknown title and an image TMDB does not deliver are answered 404; such an answer is not cached, so a later request tries again. The answer for a title changes when its stored backdrop changes (the address the UI uses names the stored image, so a browser never shows an outdated one). The request needs the same login as the poster's.
  - The preview page keeps the image address its answer carries today (`TitlePreview.backdrop`), also for a title that is in the library.
- [ ] AC-05 (state): While a title page's title has a backdrop and the image has loaded, the head shows it sharp (not blurred) across the head's full width behind the head's content.
  - The image fills the head without distortion (cropped, never stretched) and is anchored to the head's top.
  - A scrim lies over the whole image: at most 45 % of the page ground at the head's top right, at least 80 % under the head's text column, and the full page ground at the head's lower edge, so the head ends in the page without an edge.
  - The poster, the text and the facts panel of the head stay as they are (`ui-detail-head` AC-02, AC-03).
  - The head is at least 420 px high at window widths from 1024 px, so the image has room; below 1024 px the head is as high as its content and the image fills that.
  - The image is decorative: hidden from assistive technology, without alternative text.
- [ ] AC-06 (state): While a movie or series page's title has no backdrop, or the backdrop fails to load, the head shows the blurred poster of today (`ui-polish` AC-01); without a poster too, the plain page ground. A preview page without a backdrop shows the plain page ground, as today.
- [ ] AC-07 (always): Over a backdrop, every text of the head that does not stand on a panel of its own (title, year, other titles, rating, overview) meets a contrast of at least 4.5:1, whatever the image. Measured in the dark palette at 1440 px and 360 px with an all-white and an all-black backdrop: the text colour against the colour that results from the scrim's weakest point under that text over the image.
- [ ] AC-08 (always): The backdrop does not delay the head: while the image request is held back, the head's text is visible, and when the image then arrives the head has the same height and its text the same position as before.
- [ ] AC-09 (state): While sofarr runs in shadow mode, AC-01 to AC-08 hold unchanged (metadata and images are read-only towards the media folders).
- [ ] AC-10 (always): The three title pages with a backdrop meet WCAG 2.1 AA in the dark palette at 1440 px and 360 px (axe scans), and every control of the head is reachable by keyboard with a visible focus.
- [ ] AC-11 (always): The website screenshots `movie.png` and `series.png` under `docs/site/screenshots/` are regenerated and show the demo titles with their backdrops.
- [ ] AC-12 (state, manual: visual design, judged by the owner): While the dark palette is active, a movie page, a series page and a preview page with a backdrop look like a title's page of a streaming service at 1440 px: the image is recognisable, the text is easy to read, nothing overlaps.

## Technical notes

- **Storage:** a `backdrop_path` column next to `poster_path` on `movies` and `series` (new migration; it only adds columns). `crates/sofarr-server/src/metadata.rs` already reads `poster_path` from TMDB's movie details, which also carry `backdrop_path`.
- **Series:** the TVDB refresh (`series.rs`) stays the source of the series' data (D28); the backdrop is one extra TMDB request by the series' TMDB id (D26) inside the refresh, whose failure is logged and does not fail the refresh.
- **Serving:** `images.rs` serves `/api/library/{kind}/{id}/poster` from the cache; the backdrop gets the sibling address and the size `w1280`, which discover already uses for previews.
- **UI:** `DetailHead.svelte` (library pages) and the head of `PreviewPage.svelte` share the look; `posterAddress` in `ui/src/lib/library/` gets a sibling for the backdrop.
- **Tests:** server tests with the fake TMDB (`tmdb_fake.rs`) for AC-01 to AC-04 and AC-09; Playwright in a new `ui/e2e/cinema-backdrops.spec.ts` against the discover server (a library with TMDB data), with the image request answered by a white and a black picture for AC-07; `ui_types_match_the_server` for the new field.
- **Screenshots:** the demo instance (`ui/e2e/site/`) renders its artwork; it needs wide pictures for the demo titles.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A library that was imported before this spec | Titles get their backdrop at the next metadata refresh (the scheduled one or `refresh-metadata` / `refresh-series`); until then AC-06 |
| TMDB names a different backdrop for a title | The next refresh stores it; pages show the new image at once; the old cache file may stay (the cache is not tidied here) |
| Two requests for the same uncached image at once | Both are answered with the image; fetching twice is acceptable |
| TMDB removes a movie's backdrop | The next successful refresh stores none; the page shows the fallback |
| The cached image file is deleted on disk | The next request fetches it again from TMDB |
| TMDB is unreachable and the cache has no image | 404; the page shows the fallback of AC-06 |
| A very bright backdrop | The scrim keeps the text readable (AC-07) |
| A window of 360 px | The backdrop spans the stacked head; the text stays readable; no horizontal scrolling |
| No TMDB key configured (`[tmdb]` missing) | No backdrops are fetched; `has_backdrop` stays as stored; pages show the fallback |
| The backdrop arrives after the page was shown | It appears without moving the head's content |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Series take their backdrop from TMDB, not TVDB | sofarr already serves TMDB images in fixed sizes through its cache; series carry a TMDB id since D26 | Series without a TMDB id have no backdrop (out of scope) |
| 1280 px wide is enough | The size discover's previews already use; the head is at most about 1700 px wide and the image lies under a scrim | Low: slightly soft on very wide windows |
| 420 px minimum head height from 1024 px | Enough for a 16:9 image to read as an image next to the 14 rem poster | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 | `movie page / series page / preview page: sharp backdrop (AC-05)` › `fills the head sharp, cropped, anchored to the top, under a scrim`, `below 1024 px the head is as high as its content` (`ui/e2e/cinema-backdrops.spec.ts`); `preview head (AC-04)` › `the backdrop sharp under a scrim in the page ground; compact facts` (`ui/e2e/discover-polish.spec.ts`) | built |  |
| AC-06 | `fallbacks (AC-06)` › `a library title without a backdrop shows its poster, blurred`, `a backdrop that fails to load falls back to the blurred poster`, `without backdrop and poster the head has the plain ground`, `a preview whose backdrop fails to load has the plain ground` (`ui/e2e/cinema-backdrops.spec.ts`); `backdrop (AC-01)` (`ui/e2e/library-detail-head.spec.ts`) | built |  |
| AC-07 | `movie page / series page / preview page: contrast over any image (AC-07)` › `at least 4.5:1 over an all-white / all-black backdrop at 1440 px / 360 px` (`ui/e2e/cinema-backdrops.spec.ts`) | built |  |
| AC-08 | `movie page / series page / preview page: the head does not wait or move (AC-08)` › `text first; same height and text position when the image arrives` (`ui/e2e/cinema-backdrops.spec.ts`) | built |  |
| AC-09 |      | open   |        |
| AC-10 | `movie page / series page / preview page: accessibility (AC-10)` › `no violation and a visible focus at 1440 px / 360 px` (`ui/e2e/cinema-backdrops.spec.ts`) | built |  |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |

## Decisions

- 2026-11-22: The owner chose all four elements of the cinema look (hero banner, tiles with a hover state, real backdrops, the library as rows) for the whole UI; within the shared theme, dark palette only, touch not a goal (D34).
- 2026-11-22: The backdrop is stored as a path at the metadata refresh, like the poster, and not asked from TMDB when a page opens, because a page of the library must not wait for a provider (D24) and the lists of the later specs need `has_backdrop` for many titles at once.
- 2026-11-22 (build): The head's ground is one component, `HeadBackdrop.svelte`, for the library pages and the preview page. The UI tests were written with it; red runs on the way: the contrast tests failed over an all-white backdrop (3.3:1 for the rating line, 2.0:1 for „Originaltitel“ on the preview), because those lines reached into the weaker part of the scrim.
- 2026-11-22 (build): The contrast is measured from pixels: the head's texts are made transparent, each text's box is photographed and its lightest and darkest pixel compared with the text's colour.
- 2026-11-22 (build): The scrim is 84 % of the page ground under the head's content (the poster column and a measure of 42 rem), falls to 15 % within the next 16 rem, and is the full page ground at the lower edge. All content of the head, the facts panel too, keeps to the 42 rem measure, so the image is free beside it.
- 2026-11-22 (build): The demo instance of the website screenshots runs `refresh-metadata` and `refresh-series` after its imports, because only the refresh stores backdrops. AC-11 (the regenerated screenshots) and AC-12 are open: at 1440 px the head's content and its scrim leave only a narrow strip of the image visible, and the owner has been asked whether the head should change its layout (a revision of AC-05).
- 2026-11-22 (build): Older e2e tests adjusted, none removed: the preview's backdrop is sharp (`discover-polish.spec.ts`), the blurred poster is the fallback and its veil is the component's (`library-detail-head.spec.ts`).
- 2026-11-22: The scrim, not the choice of image, guarantees the contrast (AC-07), because sofarr cannot know how bright a provider's image is.
