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

## Decisions

- 2026-11-15: Inter is bundled here as in Pincel and Krake. The owner chose "embed Inter: the same type in every app" when Pincel asked; D29 records it for sofarr.
- 2026-11-15: The stored key stays `sofarr-theme`, so a browser's earlier choice survives.
- 2026-11-15: The Primer-style property names stay as aliases of the tokens, because renaming them in 64 files changes nothing visible.
