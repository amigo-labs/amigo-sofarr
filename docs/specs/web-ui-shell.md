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
| AC-01 | `the binary answers / with the app document and the app starts` (`ui/e2e/serving.spec.ts`); `ui_tests::pages_get_the_app_document_and_assets_their_files` (`crates/sofarr-server/src/ui_tests.rs`) | built |  |
| AC-02 | `ui_tests::pages_get_the_app_document_and_assets_their_files`, `ui_tests::unknown_assets_and_api_paths_are_not_found`, `ui_tests::with_auth_on_unknown_api_paths_ask_for_a_login_first` (`crates/sofarr-server/src/ui_tests.rs`); `a page address opens directly and survives a reload`, `unknown assets and API paths are errors, not the app document` (`ui/e2e/serving.spec.ts`) | built |  |
| AC-03 | `ui_tests::without_a_build_every_page_explains_how_to_build_it` (`crates/sofarr-server/src/ui_tests.rs`) | built |  |
| AC-04 | `a bookmarked page asks for a login and shows that page afterwards`, `a foreign target after the login leads to the start page` (`ui/e2e/auth.spec.ts`); `safeTarget`, `routeFor` (`ui/src/lib/auth/redirect.test.ts`) | built |  |
| AC-05 | `a wrong password shows the API message; throttling disables the button`, `without Retry-After the next attempt waits 60 seconds`, `a failed logout says so and the user stays logged in` (`ui/e2e/auth.spec.ts`); `retry-after` (`ui/src/lib/auth/retry-after.test.ts`) | built |  |
| AC-06 | `no login where none is needed (AC-06)` (auth off and trusted network, `ui/e2e/auth.spec.ts`); `redirect` (`ui/src/lib/auth/redirect.test.ts`) | built |  |
| AC-07 | `a refresh that gets 401 shows the login page and returns to the same page`, `a write action that got 401 is not repeated and the page says so` (`ui/e2e/auth.spec.ts`); `sends a write action once and names it after a 401` (`ui/src/lib/api/client.test.ts`) | built |  |
| AC-08 | `lists the built pages in order and marks the current one`, `every page reaches every other page through the navigation` (`ui/e2e/layout.spec.ts`); `a login without a requested page shows the system page (AC-08)` (`ui/e2e/auth.spec.ts`); `nav` (`ui/src/lib/nav.test.ts`) | built |  |
| AC-09 | `narrow screens (AC-09)` › `the navigation sits behind a menu button`, `very long names and messages wrap instead of scrolling` (`ui/e2e/layout.spec.ts`) | built |  |
| AC-10 | `theme (AC-10)` › `follows the system setting, also when it changes`, `the switch offers Hell, Dunkel and System and is remembered`, `blocked storage: the theme follows the system and is not remembered` (`ui/e2e/layout.spec.ts`); `theme` (`ui/src/lib/theme/theme.test.ts`) | built |  |
| AC-11 | `light theme` and `dark theme` › `system page with every state`, `system page with a failed refresh`, `full-page error`, `login page with an error and the open menu on a narrow screen`, `logged-in header and open menu`, `page not found` (axe, wcag2a/2aa/21a/21aa, `ui/e2e/a11y.spec.ts`); `every control is reachable by Tab with a visible focus`, `the login form works with the keyboard alone` (`ui/e2e/layout.spec.ts`) | built |  |
| AC-12 | `ui_tests::the_system_page_shows_jobs_connections_and_masked_settings`, `ui_tests::without_clients_the_system_page_says_not_configured` (`crates/sofarr-server/src/ui_tests.rs`); `system::tests::secrets_are_masked_at_any_depth`, `system::tests::urls_lose_user_info_and_query` (`crates/sofarr-server/src/system.rs`); `status::tests::jobs_and_contacts_keep_the_last_state` (`crates/sofarr-server/src/status.rs`); `shows version, mode, jobs, connections and the configuration of the server`, `shows every job result and connection state in German`, `refreshes every 15 seconds while open` (`ui/e2e/system.spec.ts`) | built |  |
| AC-13 | `failed requests (AC-13)` › `a first load with a server error shows a full-page error with a retry`, `an unreachable API on the first load says so in German`, `a failed refresh keeps the data, offers a retry and retries by itself`, `an unreachable API at the start shows a full-page error with a retry`, `a slow answer shows a loading indicator`; `without JavaScript` › `a German note says that the UI needs JavaScript` (`ui/e2e/system.spec.ts`); `resource` (`ui/src/lib/resource.test.ts`) | built |  |
| AC-14 | `pages, errors and controls speak German` (`ui/e2e/layout.spec.ts`); `format` (`ui/src/lib/system/format.test.ts`) | built |  |
| AC-15 | `ui_types::ui_types_match_the_server` (`crates/sofarr-server/src/ui_types.rs`) | built |  |
| AC-16 | `transfer budget (AC-16)` (one test per page: `/`, `/system`, `/anmelden`, `/filme`; about 77–81 KB measured), `scripts and styles arrive gzip-compressed` (`ui/e2e/assets.spec.ts`) | built |  |
| AC-17 | `ui_tests::pages_get_the_app_document_and_assets_their_files` (`crates/sofarr-server/src/ui_tests.rs`); `caching (AC-17)` › `assets are cached for a year, the app document is always checked` (`ui/e2e/serving.spec.ts`) | built |  |
| AC-18 | CI: docker — the `docker` job of `.github/workflows/ci.yml` starts the image and checks that `/` answers the app document (`/_app/immutable/` in the page); locally checked with `docker build` + `docker run` | built |  |
| AC-19 | `a missing asset reloads the app document once and the page appears`, `an asset that still fails after the reload shows an error instead of looping`, `a page changed by a newer build loads after one reload during navigation` (`ui/e2e/assets.spec.ts`); `stale-asset` (`ui/src/lib/stale-asset.test.ts`) | built |  |

## Decisions

- 2026-11-12: Two UI specs: `web-ui-shell` (this) and `web-ui-library` (owner).
- 2026-11-12: Responsive from 360 px; light and dark following the system (owner).
- 2026-11-12: GitHub look with Primer tokens and Octicons, own Svelte components (owner; D23).
- 2026-11-12: The UI files are embedded with `rust-embed` (`allow_missing`); debug builds read `ui/build` at runtime, so the e2e tests run against the debug binary; release builds and the Docker image carry the files inside (AC-01, AC-03).
- 2026-11-12: The asset folder is SvelteKit's `_app/`; unknown paths in it answer 404, `_app/immutable/` (hashed files) gets `public, max-age=31536000, immutable`, everything else `no-cache` (AC-02, AC-17).
- 2026-11-12: The build is precompressed (`adapter-static` `precompress`); the server sends `<file>.gz` with `Content-Encoding: gzip` when the browser accepts gzip, so no compression crate is needed (AC-16).
- 2026-11-12: Non-GET requests outside `/api/` answer 405; the UI is static.
- 2026-11-12: Job and connection status live in memory (`status.rs`); a contact counts at every indexer RSS/search call and every download-client call; the backup's next run comes from the newest scheduled backup plus the interval, because the backup scheduler only looks every minute (AC-12).
- 2026-11-12: Secrets are masked as `***` by key (`password`, `password_hash`, `api_key`, `key`, `token`, `pin`, `secret`) and in every URL-looking string (user info and query); `null` values stay `null` so "not set" is still visible (AC-12).
- 2026-11-12: `ts-rs` derives only in test builds (`cfg_attr(test, …)`); numbers are generated as `number`; `ErrorBody`/`OkBody` replace the ad-hoc JSON of error and ok answers so the UI has types for them (AC-15).
- 2026-11-12: Stack versions: SvelteKit 3 (config in `vite.config.ts`, imports via `#lib/*` because Kit 3 dropped `$lib`), Svelte 5.57, TypeScript 6 (Kit 3 and svelte-check require ^6), Vite 8, Vitest 5, Playwright 1.65 with Chromium only.
- 2026-11-12: `/` redirects to `/system` until the overview exists, so the current-page mark stays unambiguous (AC-08).
- 2026-11-12: Logout is offered only for a session (`method = session`); auth off, trusted networks and the API key have nothing to log out of (AC-06).
- 2026-11-12: The e2e servers are started by `ui/e2e/serve.mjs` (plain JS, as `@types/node` is not an approved dependency); a fourth server takes the throttling test so its lockout does not affect the other tests (AC-05).
- 2026-11-12: The AC-07 write-action test uses the logout with a faked 401, because the shell has no other write action yet; the general write path is covered by `client.test.ts`.
- 2026-11-12: A stale asset reload is remembered in `sessionStorage`; when that is blocked the German error shows at once instead of risking a reload loop; `src/error.html` covers a layout that cannot load at all (AC-19).
- 2026-11-12: oxlint turns `no-await-in-loop` off for `ui/e2e/**` in `.oxlintrc.json`, because browser steps must run in order; there are no inline suppressions.
- 2026-11-12: The theme switch is a native `<select>` labelled "Farbschema"; a small inline script in `app.html` applies the stored theme before the first paint (AC-10).
- 2026-11-12: The `ui` gates (`lint`, `format:check`, `check` in `fast`; plus install, unit tests, build and e2e in `full`) and the CI job `ui` were added with the owner's approval (2026-11-12).
