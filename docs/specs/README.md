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
| 2 | [backups](backups.md) | M | – | Scheduled, manual and pre-migration backups, restore at the next start or by CLI (D14) | `/spec-build backups` |
| 3 | [auth](auth.md) | M | – | Switchable login, sessions, API key, trusted networks and proxies, generic error messages (D15) | `/spec-build auth` |
| 4 | [mediainfo](mediainfo.md) | L | – | Own MKV/MP4 reader, media info for every file, real `{MediaInfo…}` tokens, `compare-mediainfo` (D22) | `/spec-build mediainfo` |
| 5 | [web-ui-shell](web-ui-shell.md) | L | auth | SvelteKit static UI in the binary, login, navigation, GitHub look, system page (D16, D23) | `/spec-build web-ui-shell` |
| 6 | [web-ui-library](web-ui-library.md) | L | web-ui-shell, fail-handling, backups | Overview, movies, series, calendar, missing, blocklist, backups; monitor, profile, remove, rename, move folder; poster cache (D24) | `/spec-build web-ui-library` |
| 7 | [imdb-ratings](imdb-ratings.md) | M | web-ui-library | IMDb ratings from the IMDb datasets in every list, sortable and filterable; TMDB ids for series (D20, D26) | `/spec-build imdb-ratings` |
| 8 | [discover](discover.md) | L | web-ui-library, imdb-ratings | Discover/trending, search, title previews, people pages with filmography, adding movies and series, person import lists (D19) | `/spec-build discover` |
| 9 | [subtitles](subtitles.md) | L | mediainfo, web-ui-library | Needed subtitles (de, en, forced) from OpenSubtitles.com, stored by Jellyfin's naming rules (D17, D25) | `/spec-build subtitles` |
| 10 | [trailers](trailers.md) | M | imdb-ratings | TMDB trailers via yt-dlp into `trailers/` within a daily budget, Alpine runtime image (D18) | `/spec-build trailers` |

Large specs (L) are split while being written when they need more than a few sessions
(as `web-ui` became `web-ui-shell` and `web-ui-library`, and `imdb-ratings` left `discover`).

## Done

| Spec | Completed | Content |
| ---- | --------- | ------- |
| [fail-handling](fail-handling.md) | 2026-11-12 | Blocklist, re-search after failures, vanished downloads, series search, manual search (D13, D21) |

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
| Parser: anime titles with a year (`Title (2026) - 06`) | Episode oracle: 9 cases without a match (series title, absolute number) |
| Parser: titles without a season (`Title.E06.…`) | Episode oracle: Sonarr assumes season 1 |
| Parser: anime batches `(23-26)` | Episode oracle: only the last number is found |
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
