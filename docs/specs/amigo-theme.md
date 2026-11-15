---
status: todo
created: 2026-11-15
size: L
design: [sofarr.md §3, sofarr.md §9 D23, sofarr.md §9 D29]
media: none
oracle: none
---

# The shared amigo theme in the web UI

## Context

Every amigo app gets the same look: one theme, kept in amigo-labs/website (`theme/`), and one accent per app, the `accent` of its `docs/site/project.yml` (sofarr: `#2bd67b`). sofarr's web UI so far looks like github.com: colours, sizes and type come from `@primer/primitives` (D23), with a light and a dark palette that follow the system.

The owner decided (2026-11-15, D29): sofarr takes the shared theme. Surfaces, text, status colours, type, radii and the focus ring come from the shared tokens, the accent is sofarr's green in the theme's roles, Inter is the UI font, and dark is the default palette; light and system can be chosen. D23's icons (Octicons) stay, because the shared theme has no icon set yet.

This spec changes the look's values and the default palette. The components keep their markup; moving them onto the shared component classes (`am-*`) is a later spec (backlog). It revises `web-ui-shell` AC-10 in the same change, to: "The UI is dark until the user chooses otherwise; a switch offers dark, light and system (which follows the system's light or dark setting), and the choice is remembered in the browser."

Terms. **Theme files**: `tokens.css`, `components.css` and `theme.json`, written by amigo-labs/website `scripts/theme.ts` for sofarr's `docs/site/project.yml`. **Token**: a named value of the theme files (`bg`, `surface`, `text`, `accent-ink`, …; their meaning is in the website's `theme/README.md`). **Theme choice**: the user's setting Dunkel, Hell or System in the header's switch. **Active palette**: dark or light, what the theme choice resolves to.

## Scope

### In scope

- The theme files in the repository, checked in CI against the generator.
- Every colour, size, radius, type and focus value of the UI from the tokens; `@primer/primitives` leaves the dependencies.
- The default palette dark; the switch's choices Dunkel, Hell, System.
- Inter as the UI font, bundled.

### Out of scope

- Rebuilding buttons, fields, tables and navigation on the shared component classes — because that changes the components' markup and is a spec of its own; here they only change values, and `components.css` is generated but not loaded.
- Replacing Octicons — because the shared theme has no icon set; D23's icon half stays.
- Renaming the 69 Primer-style custom properties (`--fgColor-default`, `--base-size-8`, …) in the 64 files that use them — because it changes nothing a user sees; they stay as sofarr's aliases of the tokens (backlog).
- The screenshots on amigo-labs.dev (`docs/site/screenshots`) — because they are regenerated with `bun run screenshots` after the look is accepted, not part of the behavior.

## Acceptance criteria

- [ ] AC-01 (always, CI: site-data): The theme files in the repository are exactly what the website's theme command writes for sofarr's `docs/site/project.yml`; a pull request in which they differ fails the Site data check.
- [ ] AC-02 (always): sofarr draws its UI in the tokens of the active palette: the page ground is `bg`, the header, cards, tables and menus are `surface`, fields are `surface-raised`, body text is `text`, supporting text `text-secondary`, captions and placeholders `text-muted`, and hairlines are `border`; every colour property the components use has the value of a token (or a mix of two tokens) in both palettes.
- [ ] AC-03 (always): sofarr's accent is its brand green in the theme's roles: links and the active navigation entry are `accent-ink`; the primary button is filled with `accent-solid` and carries `on-accent`; the keyboard focus ring is `focus`. A success, warning or error message shows its words in the status' `-ink` colour, on the status' `-subtle` fill where it has one.
- [ ] AC-04 (state): While no theme choice is stored, the active palette is dark, whatever the system prefers.
- [ ] AC-05 (event): When the user picks Dunkel, Hell or System in the switch, then the whole UI has that choice's palette as soon as the pick is handled, without a reload, and the choice is remembered in the browser.
- [ ] AC-06 (state): While the theme choice is System, the active palette is light where the system prefers light and dark otherwise, and it follows a change of the system preference while the UI is open.
- [ ] AC-07 (event): When the UI loads with a stored theme choice, then the document carries that choice's palette before the app renders anything: at the moment the page's body first has content, the palette is the stored one.
- [ ] AC-08 (error): If local storage throws on reading or on writing, then the UI starts dark, a pick still applies until the page is reloaded or closed, it is not remembered, and nothing else fails.
- [ ] AC-09 (always): In both palettes text and controls meet WCAG 2.1 AA contrast on every page the accessibility scans cover (the axe scans of `ui/e2e/a11y.spec.ts`, `library-a11y.spec.ts`, `discover-a11y.spec.ts` and `extras-a11y.spec.ts` run in the dark and in the light palette and report no violation).
- [ ] AC-10 (always): UI text is set in Inter, which ships with sofarr: it shows without a connection to another host, and the UI requests no font, stylesheet or script from another host.
- [ ] AC-11 (always): A default button and a text field are 32 px high (`control-md`) with a 6 px corner (`radius-sm`); cards, tables and menus have an 8 px corner (`radius-md`).
- [ ] AC-12 (always): The built UI ships no stylesheet of `@primer/primitives`, and the package is gone from the UI's dependencies.
- [ ] AC-13 (state, manual: visual design): While either palette is active, the login page, the overview, the movie and series lists and detail pages, the calendar, discover, the system page and the error page look complete at 1280 px and 360 px width: nothing unstyled, nothing unreadable, no leftover GitHub colour.

## Technical notes

- The theme files live in `ui/src/theme/`; regenerate with `pnpm theme amigo-labs/amigo-sofarr --out ../amigo-sofarr/ui/src/theme` in amigo-labs/website. `.github/workflows/site-data.yml` passes `theme-dir: ui/src/theme` to the reusable check and triggers on that folder as well as on `docs/site/**`. The formatter and linter skip the generated folder.
- `theme/tokens.css` is dark on `:root`, light under `data-theme="light"`, and follows the system when the root has no `data-theme`. `app.html` carries `data-theme="dark"`; its inline script (which exists today and applies the stored choice before the first paint) sets `light`, or removes the attribute for `system`. `data-color-mode` and Primer's `data-*-theme` attributes go.
- `ui/src/app.css` defines the Primer-style names the components use (69 of them: `--base-size-*`, `--bgColor-*`, `--fgColor-*`, `--borderColor-*`, `--button-*`, `--text-*`, `--fontStack-*`, …) from the tokens, so the components need no rename. `--fontStack-system` becomes the theme's sans stack (Inter first).
- The stored key stays `sofarr-theme` with the values `light`, `dark` and, new, `system`; anything else counts as no choice.
- Inter comes from `@fontsource-variable/inter` (latin subset, which covers the German UI), a new dev dependency (D29). A generator change in the website shows at the next pull request that touches the theme folder or `docs/site/`; then the files are regenerated.
- Tests: Vitest for the theme helpers (`ui/src/lib/theme/theme.test.ts`); Playwright in `ui/e2e/` for the palette, the switch, the first paint and the font (`layout.spec.ts`), with `page.emulateMedia({ colorScheme })` for the system preference; the axe scans for AC-09.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A browser that had stored `light` or `dark` under the old look | Keeps that palette |
| A browser with no stored choice on a light system, which so far got the light palette | Gets dark (the default); Hell or System is one pick in the switch |
| The stored value is unknown (`auto`, an empty string) | Counts as no choice: dark |
| The system preference changes while the choice is Dunkel or Hell | Nothing changes |
| JavaScript is off | The noscript notice shows in the dark palette |
| Inter cannot be loaded | Text falls back to the system UI font; nothing else changes |
| A title or name in a non-Latin script | Its glyphs come from the system font, as today |
| JavaScript is off and `light` is stored | The noscript notice shows in the dark palette, because only the script applies a stored choice |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Dark is the default also on a light system | The owner chose "dark is the default, light can be chosen" for every app | A light-system user without a stored choice sees a changed palette once |
| The order of the choices is Dunkel, Hell, System | Dark is the default and comes first; same as the other apps | Low: wording only |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | CI: site-data — `.github/workflows/site-data.yml` passes `theme-dir: ui/src/theme` to the website's reusable check and triggers on `ui/src/theme/**`. Locally: `node scripts/theme.ts <sofarr>/docs/site/project.yml --out <sofarr>/ui/src/theme --check` in a checkout of amigo-labs/website `main` (2843e5f) answers "up to date" for all three files, 2026-11-15 | built |  |
| AC-02 | `dark palette` and `light palette` › `grounds, text and hairlines are the tokens (AC-02)`, `fields are surface-raised with muted placeholders, lists are surface (AC-02)` (`ui/e2e/theme.spec.ts`); `aliases of the shared theme` › `every custom property in use is a token or defined in app.css`, `every token an alias names exists`, `every colour alias is a token or a mix of two tokens (AC-02)`, `maps the roles of AC-02 and AC-03`, `components carry no colour of their own` (`ui/src/lib/theme/aliases.test.ts`) | built |  |
| AC-03 | `dark palette` and `light palette` › `links, the active navigation entry and the focus ring are the accent (AC-03)`, `the primary button is accent-solid with on-accent (AC-03)`, `status messages show their words in the status ink on its subtle fill (AC-03)` (`ui/e2e/theme.spec.ts`); `maps the roles of AC-02 and AC-03` (`ui/src/lib/theme/aliases.test.ts`, also the warning status) | built |  |
| AC-04 | `theme (AC-10)` › `is dark while nothing is stored, whatever the system prefers (AC-04)` (`ui/e2e/layout.spec.ts`); `theme choice` › `is dark by default (amigo-theme AC-04)`, `counts unknown values as no choice`, `remembers the choice in storage` (`ui/src/lib/theme/theme.test.ts`) | built |  |
| AC-05 | `theme (AC-10)` › `the switch offers Dunkel, Hell and System; a pick applies at once and is remembered (AC-05)` (`ui/e2e/layout.spec.ts`); `theme choice` › `offers dark, light and system with German labels, dark first`, `sets data-theme for dark and light and removes it for system` (`ui/src/lib/theme/theme.test.ts`) | built |  |
| AC-06 | `theme (AC-10)` › `System follows the system setting, also when it changes (AC-06)` (`ui/e2e/layout.spec.ts`) | built |  |
| AC-07 | `theme (AC-10)` › `a stored "light" / "dark" / "system" / "auto" / "" is on the document before the app renders (AC-07)` (five tests, `ui/e2e/layout.spec.ts`) | built |  |
| AC-08 | `theme (AC-10)` › `blocked storage: dark at the start, a pick applies but is not remembered (AC-08)`, `storage that refuses writes: a pick applies until the reload (AC-08)` (`ui/e2e/layout.spec.ts`); `theme choice` › `starts dark and remembers nothing when the browser blocks storage (AC-08)` (`ui/src/lib/theme/theme.test.ts`) | built |  |
| AC-09 | The axe scans with the palette stored as the theme choice: `dark theme` and `light theme` (`ui/e2e/a11y.spec.ts`), `axe, dark theme` and `axe, light theme` (`ui/e2e/library-a11y.spec.ts`, `ui/e2e/discover-a11y.spec.ts`, `ui/e2e/extras-a11y.spec.ts`) | built |  |
| AC-10 | `font (AC-10)` › `text is set in Inter from sofarr itself; nothing comes from another host` (`ui/e2e/theme.spec.ts`); `@primer/primitives is gone (AC-12)` › `Inter ships with the UI (AC-10)` (`ui/src/lib/theme/aliases.test.ts`) | built |  |
| AC-11 | `dark palette` and `light palette` › `buttons and fields are 32 px high with a 6 px corner, cards have 8 px (AC-11)` (`ui/e2e/theme.spec.ts`) | built |  |
| AC-12 | `Primer primitives are gone (AC-12)` › `the built UI ships none of their stylesheets` (`ui/e2e/theme.spec.ts`); `@primer/primitives is gone (AC-12)` › `is no dependency of the UI`, `no source imports its stylesheets` (`ui/src/lib/theme/aliases.test.ts`) | built |  |
| AC-13 | manual: the owner checked both palettes on the login page, the overview, the movie and series lists and detail pages, the calendar, discover, the system page and the error page ("Krake und Sofarr sind ok"), 2026-11-15 | built |        |

## Decisions

- 2026-11-15: Inter is bundled here as in Pincel and Krake. The owner chose "embed Inter: the same type in every app" when Pincel asked; D29 records it for sofarr.
- 2026-11-15: The stored key stays `sofarr-theme`, so a browser's earlier choice survives.
- 2026-11-15: The Primer-style property names stay as aliases of the tokens, because renaming them in 64 files changes nothing visible.
- 2026-11-15 (build): Red runs against the unchanged UI (dc10bf8, old build of `ui/`). `bun run test`: 14 failed in `src/lib/theme/theme.test.ts` and `src/lib/theme/aliases.test.ts` (no `DEFAULT_CHOICE`, `dataTheme`; `parseChoice(null)` is `'system'`; `readChoice` on empty storage is `'system'`; no `--am-*` alias in `app.css`; `@primer/primitives` in `package.json`). `bunx playwright test e2e/theme.spec.ts e2e/layout.spec.ts -g "palette|font|Primer|theme"`: 25 failed — every theme test of `layout.spec.ts` (body background `rgb(255, 255, 255)` or `rgb(13, 17, 23)` instead of the tokens' `bg`, options `System, Hell, Dunkel`, no `data-theme` at the first content) and every test of `theme.spec.ts` (Primer colours instead of tokens, 6 px card corner, no Inter font face, `data-color-mode` in the built stylesheet).
- 2026-11-15 (build): AC-01 cannot go red locally; its proof is the first run of the Site data workflow on the pull request. The local `--check` against the website's `main` is in Evidence.
- 2026-11-15 (build): The axe scans and the palette tests choose the palette by storing the theme choice before the page loads (`storeTheme` in `ui/e2e/theme-helpers.ts`), because the system preference no longer decides it. The tests read the expected colours from `ui/src/theme/tokens.css`, so a regenerated theme needs no test change.
- 2026-11-15 (build): Besides the 69 Primer-style names, `app.css` defines five of sofarr's own in the same style, because one Primer name stood for two of the theme's roles: `--bgColor-surface` (cards, header, dialogs, lists; `--bgColor-default` is the page ground), `--control-bgColor-rest` (fields), `--fgColor-caption` (placeholders and the terms of fact lists, `text-muted`), `--fgColor-attention` (warning words) and `--bgColor-backdrop`. `--fgColor-muted` is `text-secondary` everywhere else.
- 2026-11-15 (build): The dialog backdrop is `bg` mixed with transparent (70 %), the only colour that is not a token or a mix of two tokens: the theme has no scrim token, and a scrim has to let the page show through. The static test allows `transparent` as the second part of a mix.
- 2026-11-15 (build): The `-muted` status borders of notices are a mix of the status' `-ink` (40 %) and `-subtle`; the primary button's hover is `accent-solid` mixed with 12 % `text`, which raises the contrast to `on-accent` in both palettes (the shared `am-btn` uses a brightness filter, which is not a colour value).
- 2026-11-15 (build): Notices (`Flash`) show their words in the status' ink (info: `accent-ink` on `accent-subtle`) and get the 8 px corner of the theme's banner; the header is `surface` with a `border` hairline, its navigation entries `text-secondary`, the active one `accent-ink`; the focus ring in the header is `focus` like everywhere else (the white ring of the dark GitHub header went).
- 2026-11-15 (build): Text sizes follow the theme's text styles: h1 24 px (`title-lg`), h2 20 px (`title`), h3 16 px (`title-sm`), body 14/20 px, small text 13 px (`body-sm`; Primer's was 12 px). The `--am-text-*` tokens are `font` shorthands and cannot be split in CSS, so `app.css` repeats their sizes as numbers.
- 2026-11-15 (build): `@font-face` for Inter is written in `app.css` with the address of the package's latin file (same family name, weight range and unicode range as the package's `wght.css`), because importing `wght.css` would ship six more subsets. Vite copies the file into `_app/immutable/assets/`; the server already serves `.woff2` as `font/woff2`. The server sets no Content-Security-Policy header, so the inline script of `app.html` needed no change in Rust.
- 2026-11-15 (build): Without JavaScript Chromium under Playwright renders the notice's paragraph; the test checks it by `p.noscript` and its colour (`text` of the dark palette).
- 2026-11-15 (build): New public API of `ui/src/lib/theme/theme.ts`: `DEFAULT_CHOICE`, `dataTheme` (replaces `colorMode`); `parseChoice` returns `null` for no choice; `applyChoice` sets or removes `data-theme`.
