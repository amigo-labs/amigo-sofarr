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
| 7 | [imdb-ratings](imdb-ratings.md) | M | web-ui-library | IMDb ratings from the IMDb datasets in every list, sortable and filterable; TMDB ids for series (D20, D26) | AC-04 revised by ui-list-items |
| 8 | [discover](discover.md) | L | web-ui-library, imdb-ratings | Discover/trending, search, title previews, people pages with filmography, adding movies and series, person import lists (D19) | done |
| 9 | [subtitles](subtitles.md) | L | mediainfo, web-ui-library | Needed subtitles (de, en, forced) from OpenSubtitles.com, stored by Jellyfin's naming rules (D17, D25) | done |
| 10 | [trailers](trailers.md) | M | imdb-ratings | TMDB trailers via yt-dlp into `trailers/` within a daily budget, Alpine runtime image (D18) | done |
| 11 | [parser-episode-gaps](parser-episode-gaps.md) | M | – | Episode parser: year before ` - 06`, `E06` without a season, batches `(23-26)`, `[03]` after a dotted anime title | done |
| 12 | [amigo-theme](amigo-theme.md) | L | web-ui-library | The shared amigo app theme from amigo-labs/website with sofarr's accent, dark by default, Inter (D29); revises web-ui-shell AC-10 | done |
| 13 | [ui-shell-redesign](ui-shell-redesign.md) | M | amigo-theme | UI redesign 1: sidebar navigation (collapsible, drawer on narrow windows), segmented theme switch, shared component classes loaded, filled status labels, UI links without underline (D30); moves web-ui-shell's menu breakpoint to 1024 px | spec-build |
| 14 | [ui-list-toolbar](ui-list-toolbar.md) | M | ui-shell-redesign | UI redesign 2: list header, one-row toolbar, filters in a panel, active filters as chips, IMDb attribution below the list (D30) | spec-build |
| 15 | [ui-list-items](ui-list-items.md) | M | ui-list-toolbar | UI redesign 3: quieter cards and rows, status chips, compact rating, monitor icon on the poster, checkbox on hover (D30); revises imdb-ratings AC-04 | spec-build |

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
| [imdb-ratings](imdb-ratings.md) | 2026-11-12 | IMDb ratings from the IMDb datasets in every list, sortable and filterable; TMDB ids for series (D20, D26) |
| [subtitles](subtitles.md) | 2026-11-12 | Needed subtitles (de, en, forced) from OpenSubtitles.com, stored by Jellyfin's naming rules (D17, D25) |
| [discover](discover.md) | 2026-11-12 | Discover/trending, search, title previews, people pages with filmography, adding movies and series, person import lists (D19) |
| [mediainfo](mediainfo.md) | 2026-11-13 | Own MKV/MP4 reader, media info for every file, real `{MediaInfo…}` tokens, `compare-mediainfo` (D22) |
| [web-ui-library](web-ui-library.md) | 2026-11-13 | Overview, movies, series, calendar, missing, blocklist, backups; monitor, profile, remove, rename, move folder; poster cache (D24) |
| [parser-episode-gaps](parser-episode-gaps.md) | 2026-11-15 | Episode parser: year before ` - 06`, `E06` without a season, batches `(23-26)`, `[03]` after a dotted anime title |
| [amigo-theme](amigo-theme.md) | 2026-11-15 | The shared amigo app theme from amigo-labs/website with sofarr's accent, dark by default, Inter (D29) |

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
