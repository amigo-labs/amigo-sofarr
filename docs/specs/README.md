# Specs

Two kinds of spec live here (`AGENTS.md`, "Spec-driven workflow"):

- **Design spec** – [`sofarr.md`](sofarr.md): goal, scope, architecture, parity strategy,
  decision log (§9). Next to it the parity matrix [`../parity.md`](../parity.md).
- **Feature specs** – one feature each, with acceptance criteria and an Evidence table: written
  with `/spec`, built with `/spec-build`, accepted with `/spec-check` (the only step that sets
  `status: implemented`).

## Order

Phase 6 "operations and comfort" and after (decisions D13–D33). All specs are written and committed;
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
| 21 | [ui-consistency](ui-consistency.md) | M | ui-discover-polish | UI redesign 9: accent only for states and the main action, discover tiles and search like the library, German wording, date-times to the minute, calmer episode rows, paths break at slashes, sofarr's monogram (D30) | done |
| 22 | [ui-overview-polish](ui-overview-polish.md) | S | ui-consistency | UI redesign 10: key figures with icons, a neutral line without download client, the queue's „Stand“ once, „Heute“/„Morgen“ day headings, empty states (D30) | done |
| 23 | [ui-shell-redesign](ui-shell-redesign.md), [web-ui-shell](web-ui-shell.md) (revision) | S | ui-consistency | Übersicht and Entdecken first in the sidebar without a group heading; ui-shell-redesign AC-01 and web-ui-shell AC-08 revised | done |
| 24 | [reencode-rule](reencode-rule.md) | M | mediainfo | Re-encoding 1: duration in the media info, size-per-minute thresholds per resolution, candidate list by API and CLI, largest first (D31) | done |
| 25 | [reencode-file](reencode-file.md) | M | reencode-rule, trailers | Re-encoding 2: one file on request by CLI and API: ffmpeg (libx265, MKV, low priority), checked result, swap with the original to the recycle bin, outcome of every attempt (D31) | done |
| 26 | [reencode-job](reencode-job.md) | M | reencode-file | Re-encoding 3: background job around the clock through the candidate list, largest first, skips settled and resting files, repairs interrupted attempts, stops with the server, system page (D31) | done |
| 27 | [ui-reencode](ui-reencode.md) | M | reencode-job | Re-encoding 4: page "Neu encodieren" with the job state, candidates with their job state, history and saved space; encode now, exclude a file from the job (D31) | done |
| 28 | [reencode-audio](reencode-audio.md) | M | reencode-file | Re-encoding 5: lossless audio without object audio (TrueHD without Atmos, PCM, FLAC; DTS-HD MA stays, as DTS:X cannot be ruled out) to E-AC-3 640 kbit/s in the same encode; an HEVC video is then copied (owner 2026-11-17); revises reencode-file AC-03 | done |
| 29 | [parser-season-in-title](parser-season-in-title.md) | S | parser-episode-gaps | Episode parser: a season word before ` - 06` belongs to the anime title; `[Grp][Title.S2][05]` is S02E05 (deliberate deviation from Sonarr) | done |
| 30 | [release-oracle-history](release-oracle-history.md) | S | – | Release oracle: `already_imported` in the decision engine, imported release names in the release fixtures; "Raw Bluray/DVD release" as deliberate deviation | `/spec-check` |
| 31 | [krake-download-client](krake-download-client.md) | M | – | Krake (formerly amigo-downloader) as download client through its NZBGet-compatible API (D32) | `/spec-check` |

Large specs (L) are split while being written when they need more than a few sessions
(as `web-ui` became `web-ui-shell` and `web-ui-library`, and `imdb-ratings` left `discover`).

## Done

| Spec | Completed | Content |
| ---- | --------- | ------- |
| [fail-handling](fail-handling.md) | 2026-11-12 | Blocklist, re-search after failures, vanished downloads, series search, manual search (D13, D21) |
| [backups](backups.md) | 2026-11-12 | Scheduled, manual and pre-migration backups, restore at the next start or by CLI (D14) |
| [auth](auth.md) | 2026-11-12 | Switchable login, sessions, API key, trusted networks and proxies, generic error messages (D15) |
| [web-ui-shell](web-ui-shell.md) | 2026-11-12, AC-10 revised 2026-11-15, AC-08 revised 2026-11-17 | SvelteKit static UI in the binary, login, navigation, shared amigo theme, system page (D16, D23, D29) |
| [trailers](trailers.md) | 2026-11-12 | TMDB trailers via yt-dlp into `trailers/` within a daily budget, Alpine runtime image (D18) |
| [imdb-ratings](imdb-ratings.md) | 2026-11-12, AC-04 revised 2026-11-17 (ui-list-items, ui-consistency) | IMDb ratings from the IMDb datasets in every list, sortable and filterable; TMDB ids for series (D20, D26) |
| [subtitles](subtitles.md) | 2026-11-12 | Needed subtitles (de, en, forced) from OpenSubtitles.com, stored by Jellyfin's naming rules (D17, D25) |
| [discover](discover.md) | 2026-11-12 | Discover/trending, search, title previews, people pages with filmography, adding movies and series, person import lists (D19) |
| [mediainfo](mediainfo.md) | 2026-11-13 | Own MKV/MP4 reader, media info for every file, real `{MediaInfo…}` tokens, `compare-mediainfo` (D22) |
| [web-ui-library](web-ui-library.md) | 2026-11-13 | Overview, movies, series, calendar, missing, blocklist, backups; monitor, profile, remove, rename, move folder; poster cache (D24) |
| [parser-episode-gaps](parser-episode-gaps.md) | 2026-11-15 | Episode parser: year before ` - 06`, `E06` without a season, batches `(23-26)`, `[03]` after a dotted anime title |
| [amigo-theme](amigo-theme.md) | 2026-11-15 | The shared amigo app theme from amigo-labs/website with sofarr's accent, dark by default, Inter (D29) |
| [ui-shell-redesign](ui-shell-redesign.md) | 2026-11-17, AC-01 revised 2026-11-17 | UI redesign 1: sidebar navigation (collapsible, drawer on narrow windows), segmented theme switch, shared component classes, filled status labels, UI links without underline (D30) |
| [ui-list-items](ui-list-items.md) | 2026-11-17 | UI redesign 3: quieter cards and rows, status chips, compact rating, monitor icon on the poster, checkbox on hover (D30) |
| [ui-detail-head](ui-detail-head.md) | 2026-11-17 | UI redesign 4: head of the movie and series pages with blurred poster backdrop, compact facts, cast strip, attribution at the end (D30) |
| [ui-detail-boxes](ui-detail-boxes.md) | 2026-11-17 | UI redesign 5: two-column boxes, quiet box headers, readable file paths, compact seasons and episodes (D30) |
| [ui-list-toolbar](ui-list-toolbar.md) | 2026-11-17 | UI redesign 2: list header, one-row toolbar, filters in a panel, active filters as chips, IMDb attribution below the list (D30) |
| [ui-calendar](ui-calendar.md) | 2026-11-17 | UI redesign 6: calendar as a month grid with weekday headings, week columns, agenda on narrow windows, compact entries with state dots (D30) |
| [ui-overview-pages](ui-overview-pages.md) | 2026-11-17 | UI redesign 7: one page header, key figures and two columns on the overview, shared tabs on Fehlend, calm list rows, compact system facts, centred login card (D30) |
| [ui-discover-polish](ui-discover-polish.md) | 2026-11-17 | UI redesign 8: discover filters in a panel with chips, shelf arrows, preview head with backdrop, new website screenshots (D30) |
| [ui-consistency](ui-consistency.md) | 2026-11-17 | UI redesign 9: accent only for states and the main action, discover tiles and search like the library, German wording, date-times to the minute, calmer episode rows, paths break at slashes, sofarr's monogram (D30) |
| [ui-overview-polish](ui-overview-polish.md) | 2026-11-17 | UI redesign 10: key figures with icons, a neutral line without download client, the queue's „Stand“ once, „Heute“/„Morgen“ day headings, empty states (D30) |
| [reencode-rule](reencode-rule.md) | 2026-11-17 | Re-encoding 1: duration from Matroska and MP4 headers, size-per-minute thresholds per resolution (`[reencode]`), candidate list by `GET /api/reencode/candidates` and `amigo-sofarr reencode-candidates`, largest first, read-only in both modes (D31) |
| [reencode-file](reencode-file.md) | 2026-11-18 | Re-encoding 2: one file on request by `amigo-sofarr reencode` and `POST /api/reencode/encode`: ffmpeg (libx265, 10-bit, MKV, niceness 19), one encode slot, checked result, swap with the original to the recycle bin, every attempt recorded, `[reencode] crf/preset/threads/min_saving_percent/timeout_hours` (D31) |
| [reencode-job](reencode-job.md) | 2026-11-18 | Re-encoding 3: background job `[reencode] enabled` (off by default, live mode), largest due file first, settled and resting files skipped, interrupted attempts repaired at start, encodes stopped with the server, "Neu encodieren" on the system page (D31) |
| [ui-reencode](ui-reencode.md) | 2026-11-18 | Re-encoding 4: page "Neu encodieren" (`GET /api/reencode`) with the job state, the running encode, saved space, candidates with their job state and the latest 50 attempts; encode now, keep a library file out of the job (`/api/reencode/exclusions`), read-only in shadow mode (D31) |
| [reencode-audio](reencode-audio.md) | 2026-11-18 | Re-encoding 5: lossless audio without object audio (TrueHD without Atmos, PCM, FLAC) to E-AC-3 in the same encode (`[reencode] convert_lossless_audio`, `audio_bitrate_kbps`, default 640, stereo 256), TrueHD Atmos and every DTS track copied, an HEVC video copied when audio is converted, every audio track checked against its plan (D31) |
| [parser-season-in-title](parser-season-in-title.md) | 2026-11-19 | Episode parser: a season word before ` - 06` belongs to the anime title (absolute number, no pack); `[Grp][Title.S2][05]` is S02E05, a deliberate deviation from Sonarr's full season |
| [series](series.md) | 2026-11-19 | Series: Sonarr import, TVDB refresh (German title, aliases, episodes, D28), refresh schedule, release-to-series matching, covered/wanted episodes, season packs, grabs in flight, episode import |
| [import-lists](import-lists.md) | 2026-11-19 | TMDb person import list sync: cast/crew selection (a movie through either credit, once), exclusions, adding with the list's settings, search on add, TMDB errors, schedule, CLI `sync-import-lists` |

## Phase 1–5 behavior

Phases 1–5 (before this workflow) built what `sofarr.md` §2 lists for v1 and v2. These specs
record that behavior after the fact (`/spec-extract`, 2026-11-19); the state per feature against
the oracles stays in [`../parity.md`](../parity.md).

| Spec | Size | Content | Next |
| ---- | ---- | ------- | ---- |
| [parser](parser.md) | L | Release-name parser: title/AKA, year, group, source, resolution, remux/full disc/DVD/regional, revision, edition, languages, burnt-in subs, season/episode/pack/daily/anime episodes, movie-oracle ratchet | `/spec-check` |
| [decision-engine](decision-engine.md) | L | Named quality, profile ranks, custom formats and score, year/unparsable rule, size, language, burnt-in subs, indexer limits, upgrades and cutoff, propers/repacks, best-release order, availability, series episode rules, release-oracle ratchet | `/spec-check` |
| [naming](naming.md) | M | File and folder naming: token syntax and case, colon/illegal characters, folder levels, clean titles, movie/series/episode tokens, Quality Full, multi-episode styles, `config_naming` defaults, movie and episode import naming | `/spec-check` |
| [library-import](library-import.md) | M | Takeover of settings and library from Radarr/Sonarr (API or export folder): movies by TMDB id, series/episodes/files by TVDB id, German CLI summary and errors | `/spec-check` |
| [rss-sync](rss-sync.md) | M | Scheduled RSS sync over all Newznab indexers: matching to movies and series, best release per movie/episode set, open grabs count as the file, indexer tags, shadow grabs vs. hand-over by category | `/spec-check` |
| [download-import](download-import.md) | M | Download check: queue/history states, moving and renaming finished downloads, recycle bin on upgrade, waiting for episode titles, retry after a failed import, free-space check, remove completed | `/spec-check` |
| [import-lists](import-lists.md) | M | TMDb person import list sync: cast/crew selection, exclusions, adding with the list's settings, search on add, TMDB errors, schedule, CLI `sync-import-lists` | done |
| [disk-scan](disk-scan.md) | M | Disk scan of movies and series: forget missing files (auto-unmonitor), take up found videos, skip unmounted root folders, delete empty folders (not in shadow mode), `rescanAfterRefresh`, scan report | `/spec-check` |
| [series](series.md) | L | Series: Sonarr import, TVDB refresh (German title, aliases, episodes, D28), refresh schedule, release-to-series matching, covered/wanted episodes, season packs, grabs in flight, episode import | done |

## Backlog

Gaps not yet written as specs. Not commitments.

None at the moment. Closed on 2026-11-19: the parser gaps (`parser-season-in-title`), the
release-oracle gaps (`release-oracle-history`; raw disc releases a deliberate deviation in
`parity.md`), amigo-downloader (now Krake, `krake-download-client`, D32), the v3 API shim (not
planned, D33), and the feature specs for phases 1–5 (table above).

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
