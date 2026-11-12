---
status: todo
created: 2026-11-12
size: L
design: [sofarr.md §3, sofarr.md §7, sofarr.md §9 D15, sofarr.md §9 D16, sofarr.md §9 D23]
media: none
oracle: none
---

# Web UI: shell

## Context

sofarr has no user interface; everything runs through the CLI and a small JSON API. D16 decides a
web UI built with Svelte 5 and SvelteKit as a static single-page app, built with Bun and embedded
into the `amigo-sofarr` binary, so the container stays one binary. D23 decides the look: GitHub's
(Primer design tokens and Octicons, MIT), light and dark following the system.

This spec builds the shell every later page lives in: build and embedding, login (spec `auth`),
navigation, theme, the system page, error handling, and the checks in CI. The library pages are
spec `web-ui-library`; discover and people pages are spec `discover`.

Terms used below:

- **UI** – the web interface served by sofarr at `/` (every path outside `/api/`).
- **Page** – one view of the UI with its own address (e.g. `/filme`), reachable by URL and by navigation.
- **Narrow screen** – a browser window from 360 px up to 767 px wide; **wide** from 768 px.
- **UI build** – the static files the Bun build produces; the binary embeds them when they exist.
  Its **app document** is the HTML file every page starts from; its **assets** are the hashed
  script, style, font and icon files under one asset folder.
- **Refresh** – a page re-reading data that changes on its own (e.g. job status) every
  15 seconds while it is open. Write actions (a click that changes something) are never retried
  on their own.
- **Secret** – a configuration value whose key is `password`, `password_hash`, `api_key`, `key`,
  `token`, `pin` or `secret`, and the user-info and query part of any configured URL.

## Scope

### In scope

- Building the UI with Bun and embedding it into the binary; serving it with client-side routing
- Login and logout pages on top of spec `auth`
- Layout with header and navigation to every page, responsive from 360 px
- Light and dark theme following the system, switchable, GitHub look (Primer tokens, Octicons)
- System page: version, mode, jobs, connections, read-only configuration overview
- Shared handling of loading, errors and an expired session
- API types for the UI generated from the Rust types, with a drift check
- CI job for the UI and the UI build in the Docker image

### Out of scope

- The library pages (overview, movies, series, missing, blocklist, backups) — because they are spec `web-ui-library`
- Editing settings in the UI — because `sofarr.toml` stays the source of configuration (D15, spec `auth`); the system page only shows it
- Serving the UI under a URL prefix (e.g. `/sofarr/`) — because sofarr runs on its own port or host name; a prefix can be added when a setup needs it
- Languages other than German — because user-visible texts are German (AGENTS.md, Konventionen) and there is one user
- Offline use or installing as an app (PWA) — because sofarr is only useful while the server is reachable
- GitHub branding (logo, name) — because only the visual language is borrowed, not the identity

## Acceptance criteria

- [ ] AC-01 (always): The `amigo-sofarr` binary serves the UI from its own content at `/`; no extra files, web server or runtime are needed in the container.
- [ ] AC-02 (event): When a browser requests a path outside `/api/` and outside the asset folder that is no file of the UI build, then sofarr answers with the app document, so every page address can be opened directly and reloaded; an unknown path in the asset folder answers status 404; an unknown path under `/api/` answers status 404 as JSON — or 401 while auth is on and the request has no credential, because authentication is checked first.
- [ ] AC-03 (state): While the binary was built without a UI build, `cargo build` and `cargo test` still succeed and `/` answers a German page saying that the UI was not built and how to build it.
- [ ] AC-04 (event): When the UI is opened while auth is on and the browser has no valid session (and is not in a trusted network), then it shows the login page; after a successful login it shows the page that was requested.
- [ ] AC-05 (error): If the login fails, then the login page shows the German message of the API; if the API throttles (status 429), it shows when the next attempt is possible (from the answer's `Retry-After`, or 60 seconds without it) and keeps the button disabled until then; if a logout fails, a German notice says so and the user stays logged in.
- [ ] AC-06 (state): While auth is off, or the browser is in a trusted network, the UI shows neither the login page nor a logout action; opening the login address leads to `/`.
- [ ] AC-07 (event): When the API answers status 401 while the user works in the UI (session expired or logged out elsewhere), then the UI shows the login page and returns to the same page after the login; a write action that got the 401 is not repeated, and the page says that it was not carried out.
- [ ] AC-08 (always): Every page that exists is reachable from every other page through the navigation, in the order Übersicht, Filme, Serien, Kalender, Fehlend, Entdecken, Blocklist, Backups, System; the navigation lists only pages that are built (after this spec: System), and the current page is marked. Until the overview exists (spec `web-ui-library`), `/` and the page after a login without a requested page is the system page.
- [ ] AC-09 (state): While the window is narrow, every page fits the width without horizontal scrolling and the navigation is reachable through a menu button.
- [ ] AC-10 (always): The UI follows the system's light or dark setting; a switch offers light, dark and system, and the choice is remembered in the browser.
- [ ] AC-11 (always): Text and controls meet WCAG 2.1 AA contrast in both themes, every action is reachable by keyboard with a visible focus, and every icon-only control has a German accessible name.
- [ ] AC-12 (event): When the user opens the system page, then it shows the sofarr version, the mode (shadow or live), each scheduled job with interval (or "aus" when switched off), last run (or "noch nicht gelaufen"), its result (ok, or error with the message) and next run, whether the download client and each indexer answered at the last contact (or "nicht konfiguriert" / "noch kein Kontakt"), and the configuration in effect with every secret masked; the page refreshes.
- [ ] AC-13 (error): If the API cannot be reached or answers a server error (5xx), then the UI shows a German notice with the API's message and a retry action, keeps the last shown data visible (or, on a first load without data, a full-page German error with a retry action), and retries a failed refresh by itself after at most 30 seconds.
- [ ] AC-19 (error): If an asset of an older UI build fails to load (sofarr was updated while the page was open), then the UI reloads the app document once, so the new build is used; if the asset still fails after that reload, the UI shows a German error with a reload action instead of reloading again.
- [ ] AC-14 (always): Every user-visible text of the UI is German.
- [ ] AC-15 (always): The UI's types for API requests and answers cannot drift from the server's: a test fails when the committed UI types differ from the types the server code defines.
- [ ] AC-16 (always): Opening any page directly with an empty browser cache transfers at most 200 KB of JavaScript and CSS, gzip-compressed (Playwright request log per page against the built binary).
- [ ] AC-17 (always): Assets are served with a cache lifetime of one year; the app document is served so that the browser always checks for a new version.
- [ ] AC-18 (event, CI: docker): When the Docker image is built, then it contains the UI, and the container answers `/` with the app document.

## Technical notes

- Stack (D16): SvelteKit with `adapter-static` and a fallback page (SPA), Svelte 5 runes, strict
  TypeScript, Bun as package manager and runner; Oxlint, Oxfmt, Vitest, `svelte-check`; Playwright
  for end-to-end tests against the built binary with the fake services of the server tests.
- Look (D23): `@primer/primitives` CSS variables for colors, spacing and type (light and dark
  sets), `@primer/octicons` for icons; components (header, boxes, labels, tables, buttons, flash
  notices) are own Svelte components in Primer's visual language. System font stack.
- Embedding: `rust-embed` in `sofarr-server` with `allow_missing = true` (AC-03); the folder is
  `ui/build`.
- Types: `ts-rs` derives on the API types; a test compares them with `ui/src/lib/api/generated/`
  and fails on a difference; `SOFARR_UPDATE_TYPES=1` makes it write them instead (like the ratchet
  baselines).
- Page addresses in German: `/`, `/filme`, `/serien`, `/fehlend`, `/blocklist`, `/backups`,
  `/system`, `/anmelden`.
- The system page needs a new endpoint, e.g. `GET /api/system` (version, mode, jobs, connections,
  redacted configuration). The server does not record job runs or connection results today
  (`app::every` only logs): this spec adds an in-memory record of each job's last run and result
  and of the last contact per download client and indexer.
- CI: a job `ui` with `bun install --frozen-lockfile`, lint, check, unit tests, build, e2e; the
  Dockerfile gets a Bun build stage before the Cargo stage. The gates in `.agents/gates.json` get
  matching `ui/` entries (owner's approval for that file, AGENTS.md "Quality gates").
- Playwright tests check AC-09 at 360 px and AC-11 with an accessibility scan (axe) in both themes.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A bookmarked page address is opened while logged out | Login page, then that page (AC-04) |
| A page address of a page not built yet (e.g. `/filme` before `web-ui-library`) | The app document shows a German "Seite nicht gefunden" with a link to the overview |
| sofarr is updated while the UI is open | The next page that needs an asset of the old build fails to load it and reloads the app document once (AC-19) |
| The system color scheme changes while the UI is open | The theme follows at once when set to "system" |
| A very long movie title in a narrow window | Wraps or truncates with the full title on hover/focus; no horizontal scrolling (AC-09) |
| The browser blocks local storage | The theme follows the system; the choice is not remembered |
| The API answers slowly (several seconds) | A loading indicator appears after 300 ms; the page stays usable |
| JavaScript is disabled | A German note that the UI needs JavaScript |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 360 px as the smallest supported width | The smallest common phone width | Older, smaller phones scroll horizontally |
| 200 KB gzip for the first load | Svelte compiles to small bundles; Primer tokens and the used Octicons are small | Icons imported wholesale would break the budget |
| Job and connection status live in memory | Only the system page shows them; after a restart "noch nicht gelaufen" is true | The status of the last run before a restart is lost |

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
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |

## Decisions

- 2026-11-12: Two UI specs: `web-ui-shell` (this) and `web-ui-library` (owner).
- 2026-11-12: Responsive from 360 px; light and dark following the system (owner).
- 2026-11-12: GitHub look with Primer tokens and Octicons, own Svelte components (owner; D23).
