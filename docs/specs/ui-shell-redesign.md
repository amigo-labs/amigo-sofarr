---
status: todo
created: 2026-11-16
size: M
design: [sofarr.md §3, sofarr.md §9 D16, sofarr.md §9 D23, sofarr.md §9 D29, sofarr.md §9 D30]
media: none
oracle: none
---

# UI redesign 1: shell, sidebar navigation and base styles

## Context

The web UI works (specs `web-ui-shell`, `web-ui-library`, `discover`, `amigo-theme` are implemented) but looks raw. Ten navigation entries crowd one header row, the theme switch is a plain select, links are underlined everywhere, and status labels are outlined pills with no shared look. The owner asked for a larger redesign of every area (2026-11-16) and left the direction to the agent.

The redesign is a series of four specs, built in this order: **this spec** (shell, navigation, base styles), then `ui-list-toolbar` and `ui-list-items` (movie and series lists), `ui-detail-head` and `ui-detail-boxes` (movie and series detail pages) and `ui-calendar`, `ui-overview-pages` and `ui-discover-polish` (calendar, overview, discover, the remaining pages, new screenshots). This spec lays the ground the other three build on. It changes no page's content or layout inside the main area beyond what the shared base styles change.

Decided (D30): the navigation moves from a top bar to a left sidebar, and the UI loads the generated shared component classes (`am-*`, `ui/src/theme/components.css`), which `amigo-theme` left unloaded. sofarr's existing global classes (buttons, boxes, labels, fields) keep their names but take the look of their shared counterparts. Renaming them in the page markup happens in specs 2–4, when those pages are rebuilt anyway.

Terms:

- **Wide window**: a viewport at least 1024 px wide. **Narrow window**: below 1024 px. This replaces the 768 px menu breakpoint of `web-ui-shell` AC-09.
- **Sidebar**: the navigation area at the left edge on wide windows.
- **Collapsed sidebar**: the sidebar showing only the entries' icons.
- **Drawer**: the sidebar shown over the page on narrow windows after the menu button is pressed.
- **Navigation order**: Übersicht, Entdecken, Filme, Serien, Kalender, Fehlend, Sperrliste, Untertitel, Backups, System — the order of `web-ui-shell` AC-08 (revised 2026-11-17) with the Untertitel page that spec `subtitles` added.
- **Logged-in state**: the user is logged in, or auth needs no login (auth off or trusted network); the navigation shows only then, as today. A **logout exists** where it does today (auth on and the browser not in a trusted network).
- **Navigation groups**: a first group without a heading (Übersicht, Entdecken), Bibliothek (Filme, Serien, Kalender, Fehlend), Verwaltung (Sperrliste, Untertitel, Backups, System).
- **Status label**: the small marker that states a state such as `fehlt`, `vorhanden`, `noch nicht erschienen` or a quality, today the `Label` component.
- **UI link**: a navigation entry, the brand, a link that is the title of an item in a list, card, table or calendar, and a "Mehr" link. Every other link is a **prose link** (links in sentences of paragraphs, notices, list items and table cells, such as the IMDb attribution).

## Scope

### In scope

- A left sidebar on wide windows, collapsible to icons, with the collapsed state remembered in the browser.
- On narrow windows: a top bar with the menu button and the brand, and the sidebar as a drawer over the page.
- Navigation groups and one icon per entry; the entry order of `web-ui-shell` AC-08 stays.
- The theme switch as a segmented control with Dunkel, Hell and System in the sidebar's footer, together with the logout action.
- Loading the shared component classes, and the look of sofarr's buttons, boxes, status labels and fields aligned with them.
- Filled status labels in four tones (accent is new, because the list redesign shows qualities in it).
- UI links without underline; prose links stay underlined.

### Out of scope

- A shared page header (title, count and actions in one row) — because it changes every page's markup; it comes with `ui-list-toolbar` and `ui-list-items` and `ui-calendar`, `ui-overview-pages` and `ui-discover-polish`, which rebuild those pages.
- Renaming `.btn`, `.box`, `.label` and the field classes to `am-*` in the page markup — because it touches 33 files; specs 2–4 rename them per page as they rebuild it.
- Filter controls, poster cards, detail pages and the calendar layout — because they belong to specs 2–4.
- New screenshots for amigo-labs.dev — because they are regenerated once the whole redesign is accepted (`ui-calendar`, `ui-overview-pages` and `ui-discover-polish`).
- Changes to the theme files under `ui/src/theme/` — because they are generated (amigo-theme AC-01).
- A user setting for the sidebar's side or width — because nothing asks for it.

## Acceptance criteria

- [ ] AC-01 (state): While the window is wide and in the logged-in state, sofarr shows the navigation as a sidebar along the left edge of the window, next to the page content (also on the error and not-found pages): no menu button, the brand at the top, the entries in the navigation order — Übersicht and Entdecken first without a heading, then the others under the visible group headings Bibliothek and Verwaltung — and the theme switch and (where a logout exists) the logout action at the bottom.
- [x] AC-02 (always): sofarr shows every navigation entry with an icon and its German label, and marks the current page's entry (`aria-current="page"`) with the accent (`accent-ink` text on a `accent-subtle` ground).
- [x] AC-03 (event): When the user presses the sidebar's collapse button on a wide window, then the sidebar shows only the icons (the group headings and the brand's name are hidden, a hairline separates the groups), the page content takes the freed width, each entry keeps its label as its accessible name and shows it as a visible tooltip next to the entry on hover and on keyboard focus, and the button's German name changes from "Seitenleiste einklappen" to "Seitenleiste ausklappen"; pressing it again restores the full sidebar.
- [x] AC-04 (event): When the UI is opened or reloaded after the sidebar was collapsed or expanded, then it shows the sidebar in the last chosen state: at the moment the page's body first has content, the document already carries that state, so the content area never starts at the other width; with nothing stored the sidebar is expanded.
- [x] AC-05 (error): If local storage throws on reading or on writing, then the sidebar starts expanded, collapsing and expanding still work until the page is reloaded, the state is not remembered, and nothing else fails.
- [x] AC-06 (state): While the window is narrow and in the logged-in state, sofarr shows a top bar with the menu button ("Menü öffnen") and the brand instead of the sidebar; while not in the logged-in state (login page, or an error page before the login), a top bar with only the brand and the theme switch shows at any width. Every page fits the width without horizontal scrolling down to 360 px.
- [x] AC-07 (event): When the user presses the menu button on a narrow window, then the drawer slides in from the left within 200 ms (at once where the system asks for reduced motion) over a dimmed page with the full navigation, the theme switch and the logout action, focus moves to the drawer's first navigation entry and stays inside the drawer while it is open, and the button is named "Menü schließen".
- [x] AC-08 (event): When the drawer is open and the user presses Escape or clicks the dimmed page, or the page changes (a navigation entry, browser back, any link or redirect), then the drawer closes; after Escape or a click on the dimmed page the focus returns to the menu button, after a page change it goes where a page change puts it today.
- [x] AC-09 (event): When the user picks Dunkel, Hell or System in the theme switch, which is a group of three buttons named "Farbschema" with the pressed one marked (`aria-pressed="true"`), then the palette changes as `amigo-theme` AC-05 to AC-08 require, under the same stored key, with the same behavior when storage fails.
- [x] AC-10 (always): A button, a box, a field, a checkbox and a status label of sofarr look like their counterpart among the shared component classes (button: `am-btn`, primary `am-btn--primary`, danger `am-btn--danger`; box: `am-panel`; field: `am-input`, for text inputs, selects and textareas; checkbox: the box of `am-check`; status label: `am-chip`): in the resting state, and for buttons also disabled, the same height, corner radius, border colour and width, background and text colour, as measured by computed styles in both palettes. Notices (`Flash`) are not boxes and keep their look.
- [x] AC-11 (always): A status label is a filled chip with a dot in one of four tones: success (`success-subtle` ground, `success-ink` text, `success` dot), danger (`danger-subtle`, `danger-ink`, `danger` dot), accent (`accent-subtle`, `accent-ink`, `accent` dot) and neutral (`surface-raised`, `text-secondary`, `border-strong` dot); every label keeps the tone it has today (e.g. „vorhanden“ and „verfügbar“ success, „fehlt“ danger, „noch nicht erschienen“ neutral); no label uses accent yet, it is proven by rendering the label component in that tone.
- [x] AC-12 (always): UI links (navigation entries, titles in lists, cards and tables, "Mehr" links) are not underlined until hovered or focused; prose links in paragraphs and notices stay underlined.
- [x] AC-13 (always): In both palettes the shell (sidebar expanded and collapsed, top bar, open drawer) and every page the existing axe scans cover meet WCAG 2.1 AA without a violation, every control of the shell is reachable by Tab with a visible focus, and every icon-only control has a German accessible name.
- [x] AC-14 (always): Opening any page directly with an empty browser cache still transfers at most 200 KB of JavaScript and CSS, gzip-compressed (`web-ui-shell` AC-16).
- [x] AC-15 (state, manual: visual design): While either palette is active, the overview, the movie and series lists and detail pages, the calendar, discover, the system page, the login page and the error page look complete with the new shell at 1440 px (sidebar expanded and collapsed) and at 360 px (drawer closed and open): nothing unstyled, nothing overlapping, no leftover underline on a UI link.

## Technical notes

- **Groups and icons** (Octicons, 16 px):

  | Group | Entry | Icon |
  | --- | --- | --- |
  | Bibliothek | Übersicht | `home` |
  | Bibliothek | Filme | `video` |
  | Bibliothek | Serien | `device-desktop` |
  | Bibliothek | Kalender | `calendar` |
  | Bibliothek | Fehlend | `inbox` |
  | Entdecken | Entdecken | `telescope` |
  | Verwaltung | Blocklist | `circle-slash` |
  | Verwaltung | Untertitel | `comment` |
  | Verwaltung | Backups | `database` |
  | Verwaltung | System | `server` |

  `ui/src/lib/nav.ts` gains the group and icon per page; the order of `PAGES` stays.
- **Sidebar size:** 240 px expanded and 56 px collapsed; the brand row shows the sofarr icon only when collapsed. When the entries do not fit the window height, they scroll inside the sidebar and the footer stays visible. Collapsed, the theme switch shows its three buttons as icons (`moon`, `sun`, `device-desktop`) stacked, each named Dunkel, Hell, System; expanded they show icon and label in a row.
- **Content width:** the main content stays centred at most 80 rem wide within the width beside the sidebar, so the pages keep their layout until specs 2–4.
- **Stored state:** the collapsed state goes under `sofarr-sidebar` (`collapsed`, or nothing). The first-paint script in `app.html`, which already applies the stored theme, also sets a `data-sidebar="collapsed"` attribute so AC-04 holds before the app renders.
- **Styles:** `ui/src/theme/components.css` is imported in `app.css`. The rules of `.btn`, `.btn-primary`, `.btn-danger`, `.box`, `.label`, `.form-field` and `.check` take the values of their `am-*` counterparts, written from the tokens (aliases test in `ui/src/lib/theme/aliases.test.ts` keeps holding). The primary button's hover keeps the colour mix of amigo-theme (no `filter`).
- **Label:** `Label.svelte` gains the `accent` tone next to `neutral`, `success` and `danger`.
- **Tests:**
  - Playwright in `ui/e2e/layout.spec.ts` for AC-01 to AC-09 and AC-12, at 1440 px and 360 px, plus the 1024 px and 1023 px boundary.
  - The amigo-theme switch tests move from `selectOption` to the buttons.
  - Computed-style comparisons for AC-10 and AC-11 go in `ui/e2e/theme.spec.ts`, with expected values read from `tokens.css` as today.
  - The axe scans of the four a11y specs gain the collapsed sidebar and the open drawer.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| The window is resized from wide to narrow while the sidebar is collapsed | Top bar with menu button; the drawer always shows the full navigation (labels); the stored collapsed state stays for the next wide window |
| The window is resized from narrow to wide while the drawer is open | The drawer closes, the sidebar appears in the stored state, nothing stays dimmed |
| The login page, or auth requires a login | No sidebar, no top-bar menu button (as `web-ui-shell` AC-08 today); the theme switch stays reachable |
| A stored sidebar value other than `collapsed` | Counts as nothing stored: expanded |
| JavaScript is off | The noscript notice shows as today; no sidebar |
| A very long page title or a long German label at 360 px | Wraps; no horizontal scrolling |
| Two tabs are open and the sidebar is collapsed in one | The other tab keeps its state until it is reloaded |
| A page dialog (e.g. adding a title) is open, at any width | The dialog is modal; the sidebar, menu button and drawer cannot be reached until it closes |
| The drawer is open and the browser goes back | The page changes and the drawer closes (AC-08) |
| Logout is pressed in the drawer | While pending: the drawer stays open, the action is disabled. Success: the login page shows, no drawer. Failure: the drawer closes and the German notice of `web-ui-shell` AC-05 shows |
| Keyboard user on a collapsed sidebar | Tab reaches every icon entry; its tooltip shows on focus; Enter follows it |
| The drawer is open and the user tabs past the last control | Focus wraps to the first control of the drawer |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 1024 px is the breakpoint for the sidebar | Below that a 240 px sidebar leaves less than 784 px for posters and tables, where the lists already switch toward their narrow layout | Tablets in portrait get the drawer |
| The icons in the table fit sofarr's pages | Octicons has no film strip or subtitle icon; `video` and `comment` are the closest | Low: one-line change |
| The sidebar sits at the left with the brand on top and theme and logout at the bottom | The usual app layout: page entries first, settings-like controls apart from them | Low |
| The shared component classes and the icons stay within the 200 KB budget | `components.css` is about 5 KB gzip and the ten 16 px icons about 3 KB; pages measure about 80 KB today | Low; AC-14 measures it |
| Restyled global classes cover the pages until specs 2–4 rename them | Their markup is the same; only the look changes | A page with a local override looks mixed until its spec |

## Summary

> Completed: 2026-11-17

- A left sidebar on wide windows (240 px, collapsible to 56 px icons with tooltips, state remembered under `sofarr-sidebar` and applied before the first paint), with the pages in three groups and an Octicon per entry.
- On narrow windows (below 1024 px) a top bar with menu button and brand; the navigation opens as a modal drawer from the left that keeps the focus and closes on Escape, a click outside or a page change.
- The theme switch is a segmented control (Dunkel, Hell, System) in the sidebar footer next to the logout.
- The shared component classes (`components.css`) are loaded; sofarr's buttons, boxes, fields, checkboxes and status labels take their look, and status labels are filled chips with a dot in four tones (accent new).
- UI links (navigation, titles, "Mehr") carry `ui-link` and are underlined only on hover and focus; prose links stay underlined.

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `wide window` › `the sidebar sits at the left with groups, entries, theme switch and logout (AC-01)` (first group without heading: Übersicht, Entdecken; headings Bibliothek, Verwaltung), `the sidebar also shows on the page-not-found page (AC-01)` (`ui/e2e/shell.spec.ts`); `groups the pages without changing their order (ui-shell-redesign AC-01)` (`ui/src/lib/nav.test.ts`) | built |  |
| AC-02 | `wide window` › `every entry has an icon; the current one is the accent (AC-02, dark/light)` (`ui/e2e/shell.spec.ts`); `gives every page an icon` (`ui/src/lib/nav.test.ts`) | verified | c7ca8fb |
| AC-03 | `wide window` › `collapses to icons with names and tooltips, and expands again (AC-03)` (`ui/e2e/shell.spec.ts`) | verified | c7ca8fb |
| AC-04 | `wide window` › `the collapsed state is on the document before the app renders (AC-04)`, `an unknown stored value counts as expanded (AC-04)` (`ui/e2e/shell.spec.ts`); `sidebar state` (`ui/src/lib/sidebar.test.ts`) | verified | c7ca8fb |
| AC-05 | `wide window` › `blocked storage: expanded at the start, collapsing works, nothing is remembered (AC-05)`, `storage that refuses writes: collapsed until the reload (AC-05)` (`ui/e2e/shell.spec.ts`); `starts expanded and remembers nothing when the browser blocks storage (AC-05)` (`ui/src/lib/sidebar.test.ts`) | verified | c7ca8fb |
| AC-06 | `narrow window` › `a top bar with menu button and brand instead of the sidebar (AC-06)`, `the login page has a top bar with brand and theme switch, no menu (AC-06)`; `breakpoint` › `1024 px shows the sidebar, 1023 px the menu button` (`ui/e2e/shell.spec.ts`); `narrow screens (AC-09)` (`ui/e2e/layout.spec.ts`) | verified | c7ca8fb |
| AC-07 | `narrow window` › `the drawer slides in from the left with everything and keeps the focus (AC-07)`, `the drawer opens without sliding where reduced motion is asked for (AC-07)` (`ui/e2e/shell.spec.ts`) | verified | c7ca8fb |
| AC-08 | `narrow window` › `Escape and a click on the dimmed page close the drawer and return the focus (AC-08)`, `a page change closes the drawer (AC-08)`, `growing the window to wide closes the drawer and shows the sidebar` (`ui/e2e/shell.spec.ts`) | verified | c7ca8fb |
| AC-09 | `theme switch (AC-09)` › `three buttons named Farbschema, the pressed one marked, a pick applies and is remembered` (`ui/e2e/shell.spec.ts`); `theme (AC-10)` (all amigo-theme AC-04 .. AC-08 tests, now through the buttons, `ui/e2e/layout.spec.ts`) | verified | c7ca8fb |
| AC-10 | `dark palette` and `light palette` › `a button / disabled button / primary button / disabled primary button / danger button / link as a button / box looks like its shared counterpart (AC-10)`, `a field (input / select / textarea) looks like am-input (AC-10)`, `a field without a form-field wrapper looks like am-input (AC-10)`, `a clear / checked checkbox looks like am-check's box (AC-10)` (`ui/e2e/look.spec.ts`); `buttons and fields are 32 px high with a 6 px corner, cards have 8 px (AC-11)` (`ui/e2e/theme.spec.ts`); `aliases of the shared theme` (`ui/src/lib/theme/aliases.test.ts`) | verified | b9abc06 |
| AC-11 | `dark palette` and `light palette` › `a neutral / success / danger / accent status label is a filled chip with a dot (AC-10, AC-11)`, `the labels in use keep their tone (AC-11)` (`ui/e2e/look.spec.ts`) | verified | b9abc06 |
| AC-12 | `links (AC-12)` › `navigation entries and the brand are not underlined`, `titles in lists are underlined only on hover and focus; prose links always`, `the overview, the calendar and the missing page show titles without underline`, `a prose link in a paragraph is underlined` (`ui/e2e/look.spec.ts`) | verified | b9abc06 |
| AC-13 | `dark theme` and `light theme` › `collapsed sidebar with a tooltip (ui-shell-redesign AC-13)`, `top bar of the login page on a wide window (ui-shell-redesign AC-13)`, `logged-in header and open menu` (open drawer) and every other axe scan of `ui/e2e/a11y.spec.ts`, `library-a11y.spec.ts`, `discover-a11y.spec.ts`, `extras-a11y.spec.ts`; `keyboard (AC-11)` › `every control is reachable by Tab with a visible focus` (`ui/e2e/layout.spec.ts`) | verified | b9abc06 |
| AC-14 | `transfer budget (AC-16)` (`ui/e2e/assets.spec.ts`) | verified | b9abc06 |
| AC-15 | manual: the owner checked the new shell in both palettes at 1440 px (sidebar expanded and collapsed) and 360 px (drawer closed and open) on the screenshots of 2026-11-16 and in PR #18 („Sichtcheck passt“), 2026-11-17 | verified | d51afe1 |

## Decisions

- 2026-11-16: The global classes keep their names and take the shared look; the markup is renamed per page in specs 2–4 (owner: "Stil angleichen"), so this spec stays one M block.
- 2026-11-16: The shared page header comes with the page specs, not here (owner).
- 2026-11-16: The sidebar can be collapsed to icons; the state is remembered (owner).
- 2026-11-16: On narrow windows the navigation is a drawer from the left over a dimmed page (owner).
- 2026-11-16 (build): Red runs against the unchanged UI (9fb8b63): `bunx vitest run src/lib/nav.test.ts src/lib/sidebar.test.ts` failed (no `./sidebar` module, no `navGroups`/`icon`); `bunx playwright test e2e/shell.spec.ts` failed 18 of 18 (no sidebar, no drawer, no theme buttons, no collapse button).
- 2026-11-16 (build): The drawer is a `<dialog>` opened with `showModal()`: the page behind is inert, Escape closes it natively and the browser returns the focus to the menu button. Chromium still lets Tab leave a modal dialog for the browser's own UI, so the drawer wraps Tab and Shift+Tab itself (AC-07).
- 2026-11-16 (build): The tooltip of the collapsed sidebar is one fixed-position element placed next to the hovered or focused entry, because the sidebar's entries scroll (`overflow-y: auto`) and would clip a tooltip inside them.
- 2026-11-16 (build): The sidebar and the drawer render the same entries, so the DOM holds two `Hauptnavigation` landmarks; only one is ever displayed (the other is `display: none`), so assistive technology and the tests see one.
- 2026-11-16 (build): The keyboard test of web-ui-shell AC-11 now presses Tab up to 24 times, because ten navigation entries come before the theme switch and logout; its assertions are unchanged.
- 2026-11-16 (build): `components.css` is loaded with block A, because the theme switch uses the shared segmented control (`am-seg`).
- 2026-11-16 (build): Red run for block B against c7ca8fb: `bunx playwright test e2e/look.spec.ts` failed 32 of 40 (buttons, checkboxes, the textarea, a bare field, every status label, list titles underlined). Green at once: the box, the input and the select inside `.form-field` (already matching `am-panel` and `am-input` from amigo-theme), the navigation and brand without underline (block A) and the prose link in a paragraph (unchanged behavior).
- 2026-11-16 (build): AC-10 is measured by putting sofarr's class and its `am-*` counterpart side by side into the same page and comparing the computed height, corner, border colour and width, background and text colour; the textarea's height is left out, because it grows with its rows.
- 2026-11-16 (build): Fields and checkboxes without a look of their own get the shared look through `:where(...)` rules in `app.css`, so a component's own styles still win; buttons keep the colour-mix hover of amigo-theme instead of the shared `filter`, and the danger button's hover mixes the same way.
- 2026-11-16 (build): A disabled button of any kind shows `text-muted` on `surface-raised` like `am-btn:disabled`; amigo-theme's comment that the disabled label keeps AA contrast went, because WCAG exempts disabled controls and the shared look does the same.
- 2026-11-16 (build): UI links carry the global class `ui-link` (titles in lists, cards, tables and the calendar, cast names, "Mehr"); every other link stays underlined. The accent tone is proven by rendering the label's classes, because the UI has no component test setup and no label uses accent yet.
- 2026-11-16 (build): New public API: `Tone` gains `'accent'` (`ui/src/lib/system/format.ts`); `navGroups`, `NAV_GROUPS` and `NavPage.group`/`icon` (`ui/src/lib/nav.ts`); `ui/src/lib/sidebar.ts`.
- 2026-11-17: AC-01 revised (owner): Übersicht and Entdecken stand first without a group heading, because a group „Entdecken“ with the single entry „Entdecken“ repeated itself and the two entry pages belong at the top; Bibliothek keeps Filme, Serien, Kalender and Fehlend. The navigation order of `web-ui-shell` AC-08 changes with it. Back to `status: todo` until spec-check accepts the revision.
- 2026-11-17 (build): Red run of the revision against ff08f13: `bunx vitest run src/lib/nav.test.ts` failed 3 of 5 (order and groups). The first group is named „Start“ in the code and „Einstieg“ as the list's accessible name, without a visible heading.

