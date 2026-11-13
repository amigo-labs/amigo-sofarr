---
status: draft
---

# amigo-sofarr: design

> Living document. Decisions are appended to the decision log (§9), never overwritten.

This is the design spec: goal, scope, architecture, parity strategy and decisions. What a feature
does, its acceptance criteria and the evidence for them (and so its progress) live in the feature
specs next to this file, indexed in [README.md](README.md). Each feature spec names the sections it
builds on in its `design:` front matter and never contradicts them; a feature that needs a new
architectural decision gets a row in §9 first.

## 1. Goal

amigo-sofarr replaces Sonarr and Radarr with **one** service: manage a library of movies and
series, find new releases through Usenet indexers, decide by quality rules, hand them to a download
client, import and rename finished downloads.

Success criterion: in the owner's home setup (Docker on a mini PC, nzbget, Jellyfin) sofarr
replaces first Radarr, then Sonarr – with no loss in day-to-day use.

## 2. Scope

**The parity target is the owner's setup, not Sonarr/Radarr as a whole.** What the own
configuration does not use (see `docs/parity.md`, generated from `scripts/export-arr-config.sh`)
is out of scope until further notice.

| Stage | Content |
|---|---|
| v1 | Movies: parser, decision engine, Newznab RSS, TMDB, nzbget, import/rename, import lists (TMDb persons) |
| v2 | Series: episode parsing, seasons, specials, series metadata |
| v3 (phase 6) | Operations and comfort: failure handling, backups, optional auth, media info, web UI, discover/people/IMDb rating, subtitles, local trailers |
| later | amigo-downloader as download client, integrations with v3 API compatibility |

**Non-goals:** music, books, torrents (Usenet only until a real need comes up), multiple users and
requests with approval, full Sonarr/Radarr API compatibility.

## 3. Architecture

One binary (`amigo-sofarr`), one container, SQLite. Cargo workspace:

| Crate | Responsibility | IO? |
|---|---|---|
| `sofarr-parser` | Release name → `ParsedRelease` | no |
| `sofarr-core` | Domain model, qualities, profiles, custom formats, decision engine | no |
| `sofarr-indexer` | Newznab: caps, RSS, search | yes |
| `sofarr-metadata` | TMDB (movies), TVDB (series, D10) | yes |
| `sofarr-download` | `DownloadClient` trait, nzbget implementation | yes |
| `sofarr-db` | SQLite via sqlx, migrations | yes |
| `sofarr-mediainfo` | Media info from Matroska/MP4 headers on `Read + Seek`; depends on nothing of sofarr, used by `sofarr-server` (D22) | no |
| `sofarr-subtitles` | OpenSubtitles.com REST client, movie hash, candidate ranking, UTF-8 conversion, Jellyfin subtitle names; depends on `sofarr-parser`, used by `sofarr-server` (D25) | yes |
| `sofarr-server` | axum API, scheduler, config, binary `amigo-sofarr` | yes |
| `sofarr-parity` | CLI: collect oracle fixtures, diff behavior | yes |

Dependency direction: `parser` ← `core` ← IO crates ← `server`.
The core logic (parsing, decisions) is therefore fully testable without a network.

## 4. Domain model (draft)

Terms follow the Servarr user documentation so configurations stay transferable. Fields are made
precise by the features that use them.

- **Movie** – `tmdb_id`, `imdb_id?`, title, original title, year, `monitored`, quality profile, path, status.
- **Release** – raw indexer data: GUID, title, size, indexer, publish time, download URL, categories.
- **ParsedRelease** – title candidates, year, quality (source, resolution, modifier), revision
  (version, proper/repack/real), edition, release group, languages.
- **Quality / QualityDefinition** – named quality level with size limits per minute.
- **QualityProfile** – ordered allowed qualities, cutoff, upgrades allowed?,
  custom format scores, minimum score, cutoff score.
- **CustomFormat** – named set of conditions (type, value, `negate`, `required`).
- **Decision** – release + ParsedRelease + `approved` + list of rejections
  (reason, permanent vs. temporary) + custom format score.

## 5. Parity strategy

Sonarr/Radarr serve as **black-box oracles** through their HTTP API. Their code is never read
(see §6).

| What | Oracle endpoint | Phase |
|---|---|---|
| Parsing | `GET /api/v3/parse?title=…` | 1 |
| Grab decisions | `GET /api/v3/release?movieId=…` (`approved`, `rejections`, score) | 2 |
| Library state | `GET /api/v3/movie`, `/api/v3/series` | 3 |
| Actual grabs | `GET /api/v3/history` | 3 |
| Import matching | `GET /api/v3/manualimport?folder=…` | 4 |
| Naming | `GET /api/v3/rename?movieId=…`, `/api/v3/config/naming` | 4 |

Tools:

1. **Fixtures:** `sofarr-parity collect` collects real release names and stores the oracle's
   answer raw as JSONL. Public fixtures (from RSS) live in `fixtures/public/`, private ones
   (history, library) in `fixtures/private/` (gitignored).
2. **Ratchet:** pass rates per field are recorded in `fixtures/*-baseline.toml`. Tests fail when a
   rate drops. A baseline may only rise.
3. **Shadow mode:** sofarr runs next to Radarr with a read-only `/data` and logs what it would
   grab. Compared against Radarr's history.
4. **Deliberate deviations** are documented in `docs/parity.md` and excluded from the diff.

## 6. Clean room

sofarr is MIT, the Servarr projects are GPL-3.0. Binding rules: `AGENTS.md`, "Clean Room".
Summary: no Servarr code, no Servarr tests, no regexes reconstructed from memory; the same holds
for HolyArrs (AGPL-3.0) and Bazarr (GPL-3.0). Allowed: API observation, user documentation,
public specifications, general knowledge of release names.

## 7. Operations

- Container: static musl binary, port **7676**, UID/GID 1000. Base image Alpine (pinned by
  digest) with `yt-dlp` and `ffmpeg` for the trailers (D18).
- Backups: SQLite via `VACUUM INTO` to `/config/backups`, snapshot before migrations (D14).
- Auth: can be switched off (`[auth] enabled`), login for the UI and an API key for scripts (D15).
- Volumes: `/config` (config, SQLite), `/data` (downloads + library under **one** mount for
  hardlinks and atomic moves – same layout as Radarr).
- Config: TOML at `/config/sofarr.toml` (`SOFARR_CONFIG`); unknown keys are an error.
- Health check: `amigo-sofarr healthcheck` (no curl needed in the image), endpoint `GET /api/health`.
- Logging: `tracing`, `RUST_LOG` overrides the config, format `pretty` or `json`.

## 8. Integrations

| Integration | When | How |
|---|---|---|
| nzbget | phase 4 | JSON-RPC behind the `DownloadClient` trait |
| Jellyfin | – | not needed: Jellyfin watches the library itself (D12); subtitles and trailers are stored by Jellyfin's naming rules (D17, D18) |
| OpenSubtitles.com | spec `subtitles` | official REST API (movie hash, IMDb/TMDb id) |
| IMDb datasets | spec `discover` | `title.ratings.tsv.gz`, daily, personal non-commercial use |
| amigo-downloader | after Radarr is replaced | second `DownloadClient` implementation (needs a headless mode / library crate) |
| Prowlarr, Jellyseerr, Bazarr | only if needed | exactly the v3 endpoints these tools call |

## 9. Decision log

| # | Decision | Reason |
|---|---|---|
| D1 | Rewrite in Rust instead of a .NET 10 fork | A fork means rebasing forever with no gain; a rewrite means an own model, resources, learning |
| D2 | One service for movies + series | Shared pipeline (indexer → decision → download → import) |
| D3 | MIT + clean room | Freedom within the amigo-labs ecosystem; GPL code is never touched |
| D4 | SQLite (sqlx) | Single user, one container, mini PC |
| D5 | Movies before series | The Radarr domain is much simpler; series build on a finished core |
| D6 | nzbget as the first download client | Already running; never switch two new components live at once |
| D7 | Usenet only | Matches the owner's setup |
| D8 | Own API, v3 compatibility only for concrete integrations | Don't rebuild the legacy of the Sonarr API |
| D9 | Indexers directly via Newznab, no Prowlarr | Matches the owner's setup; `sofarr-indexer` speaks Newznab directly, Prowlarr compatibility stays "only if needed" (§8) |
| D10 | Series metadata from TVDB, movies from TMDB | Same episode order as Sonarr → parity in phase 5; a second provider is accepted |
| D11 | Import lists (TMDb persons) before replacing Radarr (phase 4) | 5 active lists in the owner's setup; without them no new movies would arrive after the switch-off |
| D12 | No Jellyfin refresh after an import | Jellyfin watches the library itself; Radarr has no notification configured today either |
| D13 | Failure handling per app by `autoRedownloadFailed`; failed releases go on a blocklist (GUID and title) | Sonarr has it on, Radarr off; without a blocklist the same release would come back through another indexer |
| D14 | Backups via `VACUUM INTO`, snapshot before migrations; the config (secrets) is not part of a backup | Consistent copy while running, without extra tools |
| D15 | Optional auth: one user (argon2, session cookie) plus an API key | Often unnecessary at home, needed behind a reverse proxy or on the road |
| D16 | Web UI: Svelte 5 + SvelteKit (`adapter-static`, SPA), Bun, strict TS, embedded with `rust-embed` | A pure static build without SSR; smallest bundles, little boilerplate; one binary stays one binary |
| D17 | sofarr fetches subtitles itself through the OpenSubtitles.com REST API | Jellyfin's plugin is unreliable (limits, no control); Bazarr is GPL and one more service |
| D18 | Trailers stored locally in `trailers/` via yt-dlp; the runtime image becomes Alpine | Jellyfin plays local trailers on every client, remote YouTube links often not |
| D19 | Discover and people pages via TMDB, without requests/approvals | One user; adding directly instead of requesting |
| D20 | IMDb ratings from the official IMDb datasets | No key, no limit, every title, sortable and filterable locally |
| D21 | Re-search after a failure is set per media kind in `sofarr.toml`, defaulting to the imported `autoRedownloadFailed` | Parity with the own setup by default, but the owner can deviate without touching Radarr/Sonarr |
| D22 | Own media info reader as a pure crate `sofarr-mediainfo` (Matroska, MP4) on `Read + Seek`, used by `sofarr-server`; no ffprobe/MediaInfo | Keeps the single static binary and offline operation; pure like `sofarr-parser`, so it is tested with byte fixtures |
| D23 | UI look after github.com: `@primer/primitives` (design tokens, light/dark) and `@primer/octicons` (icons), both MIT, with own Svelte components | A familiar, calm, dense look for lists and status; tokens and icons ready-made, components fit Svelte; no GitHub branding |
| D24 | Posters and images reach the browser only through sofarr, which caches them under `/config/cache` | The browser talks to sofarr alone (privacy, works behind auth), images load fast and once |
| D26 | Series also carry their TMDB id, found through TMDB's lookup by TVDB id at the metadata refresh; TVDB stays the source of episodes (D10) | IMDb fallback, cast, discover and trailers come from TMDB; one id per series avoids lookups everywhere |
| D25 | Subtitle provider client as an IO crate `sofarr-subtitles` (depends on `sofarr-parser` for release-name matching), used by `sofarr-server` | Keeps the provider's HTTP API and quota logic out of the server's orchestration and testable against a fake provider, like `sofarr-indexer` |
| D27 | Auth building blocks: `argon2` for the password (D15), `sha2` for stored session-token hashes, `subtle` for constant-time key comparison, `rand` for tokens, `rpassword` for password input without echo (owner) | Established, small crates; no secret is stored or compared in plain form |
| D28 | Series refresh: a new episode of a known season is monitored only when an episode of that season is monitored; a new season follows the per-series switch `monitor_new_seasons` (default on) | Adding a series with chosen seasons (spec discover AC-08) must not monitor the left-out seasons at the next refresh |

## 10. Open questions

- amigo-downloader: library crate or daemon with an API?
