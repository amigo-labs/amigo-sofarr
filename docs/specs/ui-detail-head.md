---
status: todo
created: 2026-11-17
size: M
design: [sofarr.md §3, sofarr.md §9 D24, sofarr.md §9 D30]
media: both
oracle: none
---

# UI redesign 4: head of the movie and series pages

## Context

The movie and series pages (`/filme/{id}`, `/serien/{id}`) open with a plain head: poster and title, then a loose grid of eight facts in large type. IMDb's attribution sits in the middle of the head, and the cast box shows a few portraits at full card size over the whole width (screenshot of 2026-11-16).

The redesign series (D30) gives the pages two specs:

- **This one:** the head of the page and the cast.
- **`ui-detail-boxes`:** the boxes below the head and the seasons.

What the pages show stays as `web-ui-library` (AC-05, AC-06, AC-10), `discover` (AC-06), `imdb-ratings` (AC-04, AC-08) and `trailers` specify it. Only the arrangement changes. Library titles have no backdrop image (the API carries only the poster), so the head uses the title's own poster, blurred, as its background. That needs no new request and no API change.

Terms:

- **Detail page**: a movie page or a series page of the library.
- **Head**: the top section of a detail page, from the backdrop to the facts.
- **Backdrop**: the blurred, darkened poster behind the head.
- **Status chips**: the labels of the head (e.g. „fehlt“, „Cutoff nicht erreicht“, „verfügbar“, „3 fehlen“, „komplett“).
- **Facts**: the title's terms and values (Status, Mindestverfügbarkeit, Im Kino, Digital, Disc, Qualitätsprofil, Tags, Laufzeit; for series: Episoden, Größe, Qualitätsprofil, Tags, Serientyp).
- **Cast strip**: the list of cast members as one horizontal row.

## Scope

### In scope

- The head of both detail pages: backdrop, poster, titles, status chips, rating, overview, actions, facts.
- IMDb's attribution moved to the end of the page.
- The cast as a strip.

### Out of scope

- The boxes below the cast and the seasons — because they are `ui-detail-boxes`.
- A real backdrop image from TMDB — because it would need the API and the image cache to carry a second image per title; the blurred poster gives the effect without that.
- The discover preview and person pages — because they belong to `ui-pages-redesign`, although they share the people list.

## Acceptance criteria

- [ ] AC-01 (state): While a detail page's title has a poster and it has loaded, the head shows that poster blurred behind the whole head, fading into the page ground at its lower edge.
  - The image is covered by the page ground (`bg`) at an opacity of at least 70 % (darkened in the dark palette, lightened in the light palette), so text and controls on it keep the contrast they have on the page ground.
  - The page's axe scans report no contrast violation in the head.
  - The backdrop comes from the same image address as the poster and adds no request to another host.
  - While the title has no poster (or it fails to load), the head has the plain page ground.
- [ ] AC-02 (always): The head shows, in this order:
  1. the poster (or the placeholder of web-ui-library AC-21)
  2. the title with its year
  3. the other titles („Auch: …“)
  4. the status chips in one row, wrapping when they do not fit
  5. the IMDb rating while ratings are shown (rating and votes visible, imdb-ratings AC-04; „–“ for a title without rating, as today)
  6. the overview
  7. the actions (Überwachen, Suchen)
  8. the facts
- [ ] AC-03 (always): The facts show as a compact list of terms and values on a `surface` panel.
  - Each term sits small above its value.
  - Up to four per row from 768 px; below that they wrap to fewer per row.
  - Each value stays next to its term (the `dt`/`dd` pairs of today).
- [ ] AC-04 (always): While ratings are shown (the ratings setting is on, whether or not this title has a rating), IMDb's attribution with the date of the ratings data stands once at the end of the detail page, below every box, and not in the head (imdb-ratings AC-08).
- [ ] AC-05 (always): While the title has cast (discover AC-06), the cast shows as a strip under the heading „Besetzung“.
  - One row of round portraits (or the photo placeholder), each with the name as a link to the person page and the role below it.
  - The strip scrolls horizontally when it is wider than the page, and the page itself does not.
  - The strip is a list named „Besetzung“.
  - Every link in it is reachable by Tab, and a link that receives focus scrolls into view.
- [ ] AC-06 (state): While the window is narrow (below 768 px), the head stacks: the poster (at most 9 rem wide) above the text, all of it fitting 360 px without horizontal page scrolling.
- [ ] AC-07 (always): Both detail pages meet WCAG 2.1 AA in both palettes, with and without a poster, at 1440 px and 360 px (the axe scans of the library pages), and every control is reachable by keyboard with a visible focus.
- [ ] AC-08 (state, manual: visual design): While either palette is active, the movie page of a title with and without a poster and a series page look complete at 1440 px and 360 px: the backdrop reads as atmosphere, not as noise, the facts are calm, and the cast strip fits.

## Technical notes

- **Backdrop:** an `aria-hidden` `<img>` of the detail poster (`posterAddress(kind, id, 'detail')`, the image the poster already loaded, so the browser cache serves it) with `filter: blur(…)`, under an overlay of `bg` at 75 % that fades to `bg` at the lower edge. Its `error` event hides it, which is how AC-01's failure case is seen. It sits inside the head's section with `overflow: hidden`.
- **Rating:** `RatingLine` keeps its text („IMDb-Bewertung 7,6 (48.210 Stimmen)“); the attribution moves out of it into the page (`MoviePage`/`SeriesPage`), once, after the boxes.
- **Facts:** only the head's list gets the compact panel, through a modifier class (`facts facts-compact`); every other `.facts` (file box, trailer box, system page, queue, discover pages) keeps its look.
- **Cast:** `PeopleList` gets a `strip` variant; `CastBox` uses it. Portraits are 80 px round crops (`object-fit: cover`) of the existing TMDB photos.
- **Tests:** Playwright in a new `ui/e2e/library-detail-head.spec.ts`; the existing tests in `library-detail.spec.ts`, `imdb-ratings.spec.ts` and `discover-pages.spec.ts` keep their selectors (`dl > div` with `dt`/`dd`, test ids, the list „Besetzung“).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A very light or very dark poster | The `bg` overlay of at least 70 % keeps the text on page-ground contrast |
| The cast is still loading, or loading it failed | Nothing while loading; the short German note of today (`cast-failure`) on failure (discover AC-14) |
| No poster, or the poster fails to load | No backdrop; the head on the page ground |
| A title without overview | No empty paragraph; the actions follow the rating |
| A series without TMDB id, or discover off | No cast heading and no strip (discover AC-06, AC-15) |
| Ten cast members on a 360 px window | The strip scrolls; the page does not |
| Ratings off | No rating line and no attribution (imdb-ratings AC-09) |
| A very long title or many alternative titles | Wraps; no horizontal scrolling |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The blurred poster is enough as a backdrop | No API change; the plan of 2026-11-16 names it | Low: a real backdrop can follow later |
| The attribution may move to the end of the page | imdb-ratings AC-08 requires it on the page, not in a place | Low |

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

## Decisions

- 2026-11-17: The detail redesign of the plan's spec 3 is split into this spec and `ui-detail-boxes`, because together they exceed one M block.
- 2026-11-17: The backdrop is the title's own poster, blurred, because library titles carry no backdrop image and adding one needs API and cache work.
