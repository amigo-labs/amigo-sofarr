// The palette (spec ui-cinema-shell AC-02, revising amigo-theme AC-04 .. AC-08): sofarr is
// always dark. `app.html` carries `data-theme="dark"` before the first paint; the light values
// stay in the generated theme files (D29) but nothing selects them. A choice an older sofarr
// stored under `THEME_KEY` is left in the browser and ignored.

/** The key under which an older sofarr remembered the theme choice; never read or written. */
export const THEME_KEY = 'sofarr-theme';

/** The only palette. */
export const PALETTE = 'dark';

/** Puts the document element in the dark palette (`theme/tokens.css` reads `data-theme`). */
export function applyPalette(root: Pick<HTMLElement, 'setAttribute'>): void {
  root.setAttribute('data-theme', PALETTE);
}
