// The theme choice (specs web-ui-shell AC-10, amigo-theme AC-04 .. AC-08): dark until the
// user picks light or system, remembered in the browser when it lets us.

export type ThemeChoice = 'dark' | 'light' | 'system';

export const THEME_KEY = 'sofarr-theme';

/** The palette while nothing is stored. */
export const DEFAULT_CHOICE: ThemeChoice = 'dark';

export const THEME_CHOICES: readonly { readonly value: ThemeChoice; readonly label: string }[] = [
  { value: 'dark', label: 'Dunkel' },
  { value: 'light', label: 'Hell' },
  { value: 'system', label: 'System' },
];

/** The part of `Storage` the theme needs; tests pass a fake. */
export type ThemeStorage = Pick<Storage, 'getItem' | 'setItem'>;

/** A stored value as a choice; anything unknown is no choice. */
export function parseChoice(value: string | null): ThemeChoice | null {
  return value === 'dark' || value === 'light' || value === 'system' ? value : null;
}

/** The `data-theme` of the theme files for a choice; none lets them follow the system. */
export function dataTheme(choice: ThemeChoice): 'dark' | 'light' | null {
  return choice === 'system' ? null : choice;
}

/** The stored choice; without one, or in a browser that blocks storage, the default. */
export function readChoice(storage: () => ThemeStorage): ThemeChoice {
  try {
    return parseChoice(storage().getItem(THEME_KEY)) ?? DEFAULT_CHOICE;
  } catch {
    return DEFAULT_CHOICE;
  }
}

/** Remembers the choice; returns whether the browser kept it. */
export function storeChoice(storage: () => ThemeStorage, choice: ThemeChoice): boolean {
  try {
    storage().setItem(THEME_KEY, choice);
    return true;
  } catch {
    return false;
  }
}

/** Applies a choice to the document element (`theme/tokens.css` reads `data-theme`). */
export function applyChoice(
  root: Pick<HTMLElement, 'setAttribute' | 'removeAttribute'>,
  choice: ThemeChoice,
): void {
  const theme = dataTheme(choice);
  if (theme === null) root.removeAttribute('data-theme');
  else root.setAttribute('data-theme', theme);
}
