# sofarr web UI

The browser interface of sofarr (spec `docs/specs/web-ui-shell.md`): a SvelteKit single-page
app (Svelte 5, strict TypeScript) built with Bun into `ui/build`, which the `amigo-sofarr`
binary embeds and serves at `/` (D16). Look: Primer primitives and Octicons (D23).

## Scripts

Run in `ui/`:

| Command                | What it does                                                   |
| ---------------------- | -------------------------------------------------------------- |
| `bun install`          | Install the dependencies (`bun.lock` is committed)             |
| `bun run dev`          | Development server; `/api` goes to `127.0.0.1:7676`            |
| `bun run lint`         | Oxlint                                                         |
| `bun run format:check` | Oxfmt check (`bun run format` writes)                          |
| `bun run check`        | `svelte-kit sync` and `svelte-check`, zero errors and warnings |
| `bun run test`         | Vitest unit tests (`src/**/*.test.ts`)                         |
| `bun run build`        | Static build into `ui/build` (gzip and brotli next to files)   |
| `bun run e2e`          | Playwright end-to-end tests against the built binary           |

## End-to-end tests

The e2e tests run in Chromium against real `amigo-sofarr serve` instances, so the UI build
and the debug binary must exist first:

```sh
cd ui
bun run build
cargo build -p sofarr-server     # from the repository root
bunx playwright install chromium # once per machine
bun run e2e
```

Playwright starts seven instances through `e2e/serve.mjs` (fresh configuration and database
in the temp folder; the login password is generated per run in `e2e/env.mjs` and hashed with
`amigo-sofarr hash-password`) on fixed ports:

| Port  | Instance                                                            |
| ----- | ------------------------------------------------------------------- |
| 47811 | auth off, with a download client and an indexer (no answers)        |
| 47812 | auth on                                                             |
| 47813 | auth on, `127.0.0.1/32` a trusted network                           |
| 47814 | auth on, only for the failed-login and throttling test              |
| 47815 | auth off, shadow mode, 2,501 movies and 23 series (`e2e/seed.mjs`)  |
| 47816 | auth off, live mode, movies with real files for the file actions    |
| 47817 | the library of 47815 again, only read by the AC-04 time measurement |

sofarr has no seed command, so `e2e/seed.mjs` imports the test libraries the way a real one
comes in: `import-radarr --exports` reads a Radarr export it writes (quality profiles from
`fixtures/public/config/radarr`), and `import-sonarr` reads a fake Sonarr API it serves on
port 47830/47831 for the duration of the import. The time measurement of AC-04
(`e2e/library-perf.spec.ts`) runs in the Playwright project `perf` after all other tests.

A debug binary reads `ui/build` at runtime, so after a UI change `bun run build` is enough; a
release binary embeds the build at compile time.

## Layout

- `src/routes/` – the pages: `/` (Übersicht), `/filme`, `/filme/{id}`, `/serien`,
  `/serien/{id}`, `/kalender`, `/fehlend`, `/blocklist`, `/backups`, `/system`, `/anmelden`;
  `+error.svelte` for unknown addresses and failed assets.
- `src/lib/api/generated/` – API types generated from the Rust types (`ts-rs`); never edit them,
  a server test fails when they drift (`SOFARR_UPDATE_TYPES=1` rewrites them).
- `src/lib/` – API client, login state, theme, the system page's parts, shared components;
  `src/lib/library/` the library pages' logic (filters in the page address, calendar ranges,
  German texts) and components.
- `e2e/` – Playwright tests and the server start script.

Every user-visible text is German.
