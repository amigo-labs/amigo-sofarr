# Specs

Two kinds of spec live here (`AGENTS.md`, "Spec-driven workflow"):

- **Design spec** – [`sofarr.md`](sofarr.md): goal, scope, architecture, parity strategy,
  decision log (§9). Next to it the parity matrix [`../parity.md`](../parity.md).
- **Feature specs** – one feature each, with acceptance criteria and an Evidence table: written
  with `/spec`, built with `/spec-build`, accepted with `/spec-check` (the only step that sets
  `status: implemented`).

## Order

Phase 6 "operations and comfort" (decisions D13–D26). All specs are written and committed;
the next step for each is `/spec-build <name>`, then `/spec-check <name>`. The order is binding
because later specs build on earlier ones.

| # | Spec | Size | Depends on | Content | Next |
| - | ---- | ---- | ---------- | ------- | ---- |
| 1 | [fail-handling](fail-handling.md) | M | – | Blocklist, re-search after failures, vanished downloads, series search, manual search (D13, D21) | done |
| 2 | [backups](backups.md) | M | – | Scheduled, manual and pre-migration backups, restore at the next start or by CLI (D14) | done |
| 3 | [auth](auth.md) | M | – | Switchable login, sessions, API key, trusted networks and proxies, generic error messages (D15) | done |
| 4 | [mediainfo](mediainfo.md) | L | – | Own MKV/MP4 reader, media info for every file, real `{MediaInfo…}` tokens, `compare-mediainfo` (D22) | done |
| 5 | [web-ui-shell](web-ui-shell.md) | L | auth | SvelteKit static UI in the binary, login, navigation, shared amigo theme, system page (D16, D23, D29) | done |
| 6 | [web-ui-library](web-ui-library.md) | L | web-ui-shell, fail-handling, backups | Overview, movies, series, calendar, missing, blocklist, backups; monitor, profile, remove, rename, move folder; poster cache (D24) | done |
| 7 | [imdb-ratings](imdb-ratings.md) | M | web-ui-library | IMDb ratings from the IMDb datasets in every list, sortable and filterable; TMDB ids for series (D20, D26) | done |
| 8 | [discover](discover.md) | L | web-ui-library, imdb-ratings | Discover/trending, search, title previews, people pages with filmography, adding movies and series, person import lists (D19) | done |
| 9 | [subtitles](subtitles.md) | L | mediainfo, web-ui-library | Needed subtitles (de, en, forced) from OpenSubtitles.com, stored by Jellyfin's naming rules (D17, D25) | done |
| 10 | [trailers](trailers.md) | M | imdb-ratings | TMDB trailers via yt-dlp into `trailers/` within a daily budget, Alpine runtime image (D18) | done |
| 11 | [parser-episode-gaps](parser-episode-gaps.md) | M | – | Episode parser: year before ` - 06`, `E06` without a season, batches `(23-26)`, `[03]` after a dotted anime title | done |
| 12 | [amigo-theme](amigo-theme.md) | L | web-ui-library | The shared amigo app theme from amigo-labs/website with sofarr's accent, dark by default, Inter (D29); revises web-ui-shell AC-10 | done |
| 13 | [ui-shell-redesign](ui-shell-redesign.md) | M | amigo-theme | UI redesign 1: sidebar navigation (collapsible, drawer on narrow windows), segmented theme switch, shared component classes loaded, filled status labels, UI links without underline (D30); moves web-ui-shell's menu breakpoint to 1024 px | done |
| 14 | [ui-list-toolbar](ui-list-toolbar.md) | M | ui-shell-redesign | UI redesign 2: list header, one-row toolbar, filters in a panel, active filters as chips, IMDb attribution below the list (D30) | done |
| 15 | [ui-list-items](ui-list-items.md) | M | ui-list-toolbar | UI redesign 3: quieter cards and rows, status chips, compact rating, monitor icon on the poster, checkbox on hover (D30); revises imdb-ratings AC-04 | done |
| 16 | [ui-detail-head](ui-detail-head.md) | M | ui-list-items | UI redesign 4: head of the movie and series pages with blurred poster backdrop, compact facts, cast strip, attribution at the end (D30) | done |
| 17 | [ui-detail-boxes](ui-detail-boxes.md) | M | ui-detail-head | UI redesign 5: two-column boxes, quiet box headers, readable file paths, compact seasons and episodes (D30) | done |
| 18 | [ui-calendar](ui-calendar.md) | M | ui-detail-boxes | UI redesign 6: calendar as a month grid with weekday headings, week columns, agenda on narrow windows, compact entries with state dots (D30) | done |
| 19 | [ui-overview-pages](ui-overview-pages.md) | M | ui-calendar | UI redesign 7: one page header, key figures and two columns on the overview, shared tabs on Fehlend, calm list rows, compact system facts, centred login card (D30) | done |
| 20 | [ui-discover-polish](ui-discover-polish.md) | M | ui-overview-pages | UI redesign 8: discover filters in a panel with chips, shelf arrows, preview head with backdrop, new website screenshots (D30) | done |
| 21 | [reencode-rule](reencode-rule.md) | M | mediainfo | Re-encoding 1: duration in the media info, size-per-minute thresholds per resolution, candidate list by API and CLI, largest first (D31) | done |
| 22 | [reencode-file](reencode-file.md) | M | reencode-rule, trailers | Re-encoding 2: one file on request by CLI and API: ffmpeg (libx265, MKV, low priority), checked result, swap with the original to the recycle bin, outcome of every attempt (D31) | `/spec-build reencode-file` |
| 23 | [reencode-job](reencode-job.md) | M | reencode-file | Re-encoding 3: background job around the clock through the candidate list, largest first, skips settled and resting files, repairs interrupted attempts, stops with the server, system page (D31) | `/spec-build reencode-job` after reencode-file AC-17 |
| 24 | [ui-reencode](ui-reencode.md) | M | reencode-job | Re-encoding 4: page "Neu encodieren" with the job state, candidates with their job state, history and saved space; encode now, exclude a file from the job (D31) | `/spec-build ui-reencode` |
| 25 | [reencode-audio](reencode-audio.md) | M | reencode-file | Re-encoding 5: lossless audio without object audio (TrueHD without Atmos, PCM, FLAC; DTS-HD MA stays, as DTS:X cannot be ruled out) to E-AC-3 640 kbit/s in the same encode; an HEVC video is then copied (owner 2026-11-17); revises reencode-file AC-03 | `/spec-build reencode-audio` |

Large specs (L) are split while being written when they need more than a few sessions
(as `web-ui` became `web-ui-shell` and `web-ui-library`, and `imdb-ratings` left `discover`).

## Done

| Spec | Completed | Content |
| ---- | --------- | ------- |
| [fail-handling](fail-handling.md) | 2026-11-12 | Blocklist, re-search after failures, vanished downloads, series search, manual search (D13, D21) |
| [backups](backups.md) | 2026-11-12 | Scheduled, manual and pre-migration backups, restore at the next start or by CLI (D14) |
| [auth](auth.md) | 2026-11-12 | Switchable login, sessions, API key, trusted networks and proxies, generic error messages (D15) |
| [web-ui-shell](web-ui-shell.md) | 2026-11-12, AC-10 revised 2026-11-15 | SvelteKit static UI in the binary, login, navigation, shared amigo theme, system page (D16, D23, D29) |
| [trailers](trailers.md) | 2026-11-12 | TMDB trailers via yt-dlp into `trailers/` within a daily budget, Alpine runtime image (D18) |
| [imdb-ratings](imdb-ratings.md) | 2026-11-12, AC-04 revised 2026-11-17 | IMDb ratings from the IMDb datasets in every list, sortable and filterable; TMDB ids for series (D20, D26) |
| [subtitles](subtitles.md) | 2026-11-12 | Needed subtitles (de, en, forced) from OpenSubtitles.com, stored by Jellyfin's naming rules (D17, D25) |
| [discover](discover.md) | 2026-11-12 | Discover/trending, search, title previews, people pages with filmography, adding movies and series, person import lists (D19) |
| [mediainfo](mediainfo.md) | 2026-11-13 | Own MKV/MP4 reader, media info for every file, real `{MediaInfo…}` tokens, `compare-mediainfo` (D22) |
| [web-ui-library](web-ui-library.md) | 2026-11-13 | Overview, movies, series, calendar, missing, blocklist, backups; monitor, profile, remove, rename, move folder; poster cache (D24) |
| [parser-episode-gaps](parser-episode-gaps.md) | 2026-11-15 | Episode parser: year before ` - 06`, `E06` without a season, batches `(23-26)`, `[03]` after a dotted anime title |
| [amigo-theme](amigo-theme.md) | 2026-11-15 | The shared amigo app theme from amigo-labs/website with sofarr's accent, dark by default, Inter (D29) |
| [ui-shell-redesign](ui-shell-redesign.md) | 2026-11-17 | UI redesign 1: sidebar navigation (collapsible, drawer on narrow windows), segmented theme switch, shared component classes, filled status labels, UI links without underline (D30) |
| [ui-list-items](ui-list-items.md) | 2026-11-17 | UI redesign 3: quieter cards and rows, status chips, compact rating, monitor icon on the poster, checkbox on hover (D30) |
| [ui-detail-head](ui-detail-head.md) | 2026-11-17 | UI redesign 4: head of the movie and series pages with blurred poster backdrop, compact facts, cast strip, attribution at the end (D30) |
| [ui-detail-boxes](ui-detail-boxes.md) | 2026-11-17 | UI redesign 5: two-column boxes, quiet box headers, readable file paths, compact seasons and episodes (D30) |
| [ui-list-toolbar](ui-list-toolbar.md) | 2026-11-17 | UI redesign 2: list header, one-row toolbar, filters in a panel, active filters as chips, IMDb attribution below the list (D30) |
| [ui-calendar](ui-calendar.md) | 2026-11-17 | UI redesign 6: calendar as a month grid with weekday headings, week columns, agenda on narrow windows, compact entries with state dots (D30) |
| [ui-overview-pages](ui-overview-pages.md) | 2026-11-17 | UI redesign 7: one page header, key figures and two columns on the overview, shared tabs on Fehlend, calm list rows, compact system facts, centred login card (D30) |
| [ui-discover-polish](ui-discover-polish.md) | 2026-11-17 | UI redesign 8: discover filters in a panel with chips, shelf arrows, preview head with backdrop, new website screenshots (D30) |
| [reencode-rule](reencode-rule.md) | 2026-11-17 | Re-encoding 1: duration from Matroska and MP4 headers, size-per-minute thresholds per resolution (`[reencode]`), candidate list by `GET /api/reencode/candidates` and `amigo-sofarr reencode-candidates`, largest first, read-only in both modes (D31) |

## Existing behavior without a feature spec

Phases 1–5 (before this workflow) built what `sofarr.md` §2 lists for v1 and v2: the parser for
movies and episodes, the decision engine, the release oracle, the SQLite library, TMDB and TVDB,
Newznab RSS, shadow and live mode, nzbget, import and naming, import lists, the disk scan,
availability, tags and the `compare-*` oracle comparisons. The state per feature is in
[`../parity.md`](../parity.md). This behavior is proven by tests and the oracle ratchets but not
recorded as acceptance criteria; `/spec-extract <area>` does that (see Backlog).

## Backlog

Gaps not yet written as specs. Not commitments.

| Gap | Note |
| --- | ---- |
| Feature specs for the behavior of phases 1–5 | `/spec-extract` per area: parser, decision-engine, library-import, rss-sync, download-import, naming, import-lists, disk-scan, series |
| Parser: `…2nd.Season.-.06` read as a season pack | Episode oracle: 4 cases; out of scope of [parser-episode-gaps](parser-episode-gaps.md) |
| Parser: `[Grp][Title.S2][05]` | Episode oracle: Sonarr answers full season 2 together with an episode number |
| Release oracle: "Raw Bluray/DVD release", "same release name as imported" | `parity.md`, "Noch nicht abgedeckt" |
| amigo-downloader as a second download client | `sofarr.md` §8, open question §10 |
| v3 API shim for Jellyseerr/Prowlarr/Bazarr | `sofarr.md` §8, only if needed (D8) |

## Open items outside the code

Only a person can do these.

| Item | How |
| ---- | --- |
| Run the container in the own Compose setup | Adopt `compose.example.yml`, status `healthy` |
| Clean-room review of phase 2 | No Servarr code, no copied regexes |
| Movies in shadow mode | `sofarr.toml` with `[radarr]`, `[tmdb]`, `[[indexer]]`; `import-radarr`, `refresh-metadata`; container in shadow mode; `compare-library` green; after two weeks `compare-history` without unexplained deviation |
| Replace Radarr | `compare-rename`, `compare-import-lists`, `compare-manual-import --folder …` without unexplained deviation; `/data` without `:ro`, `[nzbget]`, `mode = "live"`, pause Radarr (don't delete it); four weeks of sole operation, then switch Radarr off |
| Series in shadow mode | `import-sonarr`, `refresh-series`, `compare-series-library` green; shadow operation for series |
| Replace Sonarr | After a stable shadow phase pause Sonarr, series in live mode |
