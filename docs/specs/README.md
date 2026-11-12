# Specs

Two kinds of spec live here (`AGENTS.md`, "Spec-driven workflow"):

- **Design spec** – [`sofarr.md`](sofarr.md): goal, scope, architecture, parity strategy,
  decision log (§9). Next to it the parity matrix [`../parity.md`](../parity.md).
- **Feature specs** – one feature each, with acceptance criteria and an Evidence table: written
  with `/spec`, built with `/spec-build`, accepted with `/spec-check` (the only step that sets
  `status: implemented`).

## Order

Phase 6 "operations and comfort" (decisions D13–D20). The specs are not written yet; each is
written with `/spec <name>` right before it is built and then moves into this table with a link.
The order is binding because later specs build on earlier ones.

| # | Spec | Size | Depends on | Content | Next |
| - | ---- | ---- | ---------- | ------- | ---- |
| 1 | [fail-handling](fail-handling.md) | M | – | Blocklist, re-search after failures, vanished downloads, series search, manual search (D13, D21) | `/spec-build fail-handling` |
| 2 | backups | S | – | `VACUUM INTO` on a schedule, snapshot before migrations, `backup`/`restore` (D14) | `/spec backups` |
| 3 | auth | M | – | Switchable login (argon2, session) and API key, generic error messages (D15) | `/spec auth` |
| 4 | mediainfo | L | – | Own MKV/MP4 reader: codecs, HDR, audio and subtitle tracks; real `{MediaInfo…}` tokens | `/spec mediainfo` |
| 5 | web-ui | L | auth | Svelte 5 + SvelteKit static, embedded; overview, movies, series, missing, blocklist, backups (D16) | `/spec web-ui` |
| 6 | discover | L | web-ui | Discover/trending, people pages with filmography, adding titles, IMDb rating from the IMDb datasets (D19, D20) | `/spec discover` |
| 7 | subtitles | L | mediainfo | OpenSubtitles.com, stored by Jellyfin's naming rules (D17) | `/spec subtitles` |
| 8 | trailers | M | – | TMDB trailers via yt-dlp into `trailers/`, Alpine runtime image (D18) | `/spec trailers` |

Large specs (L) are split while being written when they need more than a few sessions
(e.g. `web-ui-shell` / `web-ui-library`, `discover-browse` / `discover-imdb`).

## Done

No feature spec accepted yet.

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
