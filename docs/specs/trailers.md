---
status: todo
created: 2026-11-12
size: M
design: [sofarr.md §7, sofarr.md §8, sofarr.md §9 D12, sofarr.md §9 D18, sofarr.md §9 D26]
media: both
oracle: none
---

# Trailers

## Context

Jellyfin plays local trailers on every client when they sit in a `trailers` folder next to the
movie or series (or carry a `-trailer` suffix); the YouTube links it gets from TMDB often do not
play on TV clients. D18 decides that sofarr downloads trailers with yt-dlp into `trailers/` and
that the runtime image becomes Alpine with yt-dlp and ffmpeg, because the `scratch` image cannot
run them. The owner's library has about 2,400 movies and 150 series; downloading everything at
once would be hundreds of gigabytes and hammer YouTube, so the backlog is filled slowly.

Terms used below:

- **Trailer candidates** – the videos TMDB lists for a movie or series on YouTube of type Trailer
  or Teaser (no clips or featurettes) in a wanted language (`[trailers] languages`, default
  `["de", "en"]`) or without a language. Ranked strictly in this order: language (the wanted
  languages in their order, then no language), type Trailer before Teaser, official before
  unofficial, then newest — so a German teaser beats an English trailer.
- **Language name** – the German name of the trailer's language in file names and the UI
  (`de` → Deutsch, `en` → Englisch, others by their German name), `Original` without a language.
- **Has a trailer** – the item's folder has a `trailers` folder with a video in it, or a video with a
  `-trailer`, `.trailer` or `_trailer` suffix (Jellyfin's rules), wherever it came from.
- **Eligible item** – a movie or series that has at least one video file and no trailer; a series
  needs its TMDB id (D26, spec `imdb-ratings` AC-12).
- **Newest added** – by the date the item was added to the library.
- **Daily budget** – `[trailers] per_day` downloads per day (default 20); an item counts once when
  a download was attempted for it, an item without candidates does not count.

## Scope

### In scope

- Downloading one trailer per eligible movie and series, newest added first, within the daily budget
- Picking the best candidate and falling back to the next when one fails
- Storing it in the item's `trailers` folder for Jellyfin
- The Alpine runtime image with yt-dlp and ffmpeg
- Keeping trailers out of the import and the disk scan
- API and UI: trailer state per item, a manual download

### Out of scope

- Playing trailers in sofarr's UI — because Jellyfin plays them; the UI links to the YouTube page
- More than one trailer per item, season trailers — because one trailer is what Jellyfin shows first
- Trailers for items without a file — because the owner watches what is in the library; a trailer for a wanted movie can be fetched by hand
- Sources other than YouTube via TMDB — because TMDB lists YouTube almost exclusively
- Updating yt-dlp inside the running container — because the image is rebuilt with a pinned newer version; the container stays read-only

## Acceptance criteria

- [ ] AC-01 (event): When the trailer job runs (daily), then sofarr downloads the trailer of eligible items, newest added first, until the daily budget is used.
- [ ] AC-02 (event): When a movie or series becomes eligible by an import, then it is first in line for the next trailer job.
- [ ] AC-03 (always): sofarr picks the best trailer candidate (AC terms), downloads it at most at `max_height` (default 1080) and stores it as one video file in the item's `trailers` folder, named `<item folder name> - Trailer (<language name>).mkv`, e.g. `Heat (1995) - Trailer (Deutsch).mkv`.
- [ ] AC-15 (error): If the free space on the item's file system is below the import's minimum (`minimumFreeSpaceWhenImporting`), then sofarr downloads no trailer there, counts neither an attempt (AC-05) nor the budget, and the item shows "zu wenig Speicherplatz" (AC-10).
- [ ] AC-04 (error): If a candidate cannot be downloaded (removed, private, blocked in the country, yt-dlp fails, takes longer than 10 minutes, or would exceed 500 MB), then sofarr removes any partial file, tries the next candidate, and counts one download against the budget per item, not per attempt.
- [ ] AC-05 (state): While an item has no candidate left or all failed, it is tried again 7 days later, at most four attempts in total; after that it is left alone until the user starts a manual download.
- [ ] AC-06 (always): sofarr never overwrites, renames or deletes a trailer it did not write — except when the user removes the item with its files, which removes its whole folder; the trailer job never downloads for an item that has a trailer.
- [ ] AC-07 (always): The import and the disk scan never treat a trailer as the movie or episode video: files in a `trailers` folder or with a trailer suffix are neither imported, nor added as library files, nor counted as the largest video.
- [ ] AC-08 (event): When an item is removed with its files, or its folder moved (spec `web-ui-library`), then its trailers go with it.
- [ ] AC-09 (event): When the user starts a trailer download for an item (also one without a file) through the API or the UI, then sofarr downloads it at once, outside the daily budget, following AC-03 and AC-04; if every candidate fails, the item's attempts start over at one and it is retried per AC-05; an own trailer is replaced by the new one; a trailer it did not write makes it refuse with a German message; an item without a folder on disk is refused with a German message.
- [ ] AC-10 (event): When the user opens a movie or series in the UI, then it shows whether it has a trailer, its language and source, or why there is none (no candidate, failed with the reason, waiting for the budget).
- [ ] AC-11 (state): While sofarr runs in shadow mode, it downloads no trailers and writes nothing.
- [ ] AC-12 (option): Where `[trailers] enabled = false` is set, or yt-dlp or ffmpeg cannot be found, sofarr downloads no trailers; the system page (spec `web-ui-shell`) says which applies.
- [ ] AC-13 (always): sofarr starts yt-dlp without a shell, with fixed arguments and a validated YouTube video id; nothing from TMDB or YouTube reaches the command line unchecked.
- [ ] AC-14 (event, CI: docker): When the Docker image is built, then it runs sofarr as UID/GID 1000, `amigo-sofarr healthcheck` reports healthy, and `yt-dlp --version` and `ffmpeg -version` work in it.

## Technical notes

- TMDB: `/movie/{id}/videos` and `/tv/{id}/videos` with `language=de-DE` and
  `include_video_language=de,en,null`; fields `site`, `type`, `official`, `iso_639_1`,
  `published_at`, `key`.
- yt-dlp: format selection `bv*[height<=1080]+ba/b[height<=1080]`, merge to MKV with ffmpeg,
  into a temporary folder under `/config/cache/trailers/`, then moved into `trailers/` (so a disk
  scan never sees a partial download). Timeout and size limit per AC-04.
- Config: `[trailers] enabled` (default true), `languages`, `max_height`, `per_day`, `yt_dlp`
  (path, default `yt-dlp`).
- Dockerfile: runtime `alpine` (pinned) with `yt-dlp`, `ffmpeg`, `ca-certificates`; the musl
  binary stays the same. `compose.example.yml` and `sofarr.md` §7 already name the change.
- Table or columns per item: trailer state, language, video key, attempts, next attempt.
- Video id check: YouTube ids are 11 characters of `A-Z a-z 0-9 _ -`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A movie has only an English trailer | The English trailer is downloaded |
| A series' TMDB entry lists no videos | No candidate: retried after 7 days, at most three times (AC-05) |
| A trailer folder exists but is empty | No trailer: the item is eligible |
| The owner put an own `Film-trailer.mkv` next to the movie | The item has a trailer; sofarr does nothing (AC-06) |
| yt-dlp breaks because YouTube changed something | Every attempt fails with the reason; items wait per AC-05 until an updated image ships |
| The disk is nearly full | No trailer downloads on that file system (AC-15) |
| The movie folder is renamed while its trailer downloads | The download finishes in the temporary folder; it is moved into the current folder or discarded if the folder is gone |
| A series without a TMDB id | Not eligible until the metadata refresh finds one |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 20 trailers a day | Fills a 2,500-item library in about four months without drawing attention | The backlog takes long; the owner can raise `per_day` |
| Downloading trailers for private use in the own library is acceptable | The owner's personal media setup; the trailers are promotional material | YouTube's terms restrict downloading; YouTube may block it |
| 10 minutes and 500 MB per trailer | A 1080p trailer is 2–3 minutes and 50–200 MB | Very long trailers are skipped |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

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

- 2026-11-12: Trailers for items with a file, newest added first, within a daily budget; series get one trailer too (owner).
- 2026-11-12: The runtime image becomes Alpine with yt-dlp and ffmpeg (owner; D18).
