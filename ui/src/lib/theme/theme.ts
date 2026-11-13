// The color scheme (spec web-ui-shell AC-10): light, dark or following the system,
// remembered in the browser when it lets us.

export type ThemeChoice = 'light' | 'dark' | 'system';

export const THEME_KEY = 'sofarr-theme';

export const THEME_CHOICES: readonly { readonly value: ThemeChoice; readonly label: string }[] = [
  { value: 'system', label: 'System' },
  { value: 'light', label: 'Hell' },
  { value: 'dark', label: 'Dunkel' },
];

/** The part of `Storage` the theme needs; tests pass a fake. */
export type ThemeStorage = Pick<Storage, 'getItem' | 'setItem'>;

export function parseChoice(value: string | null): ThemeChoice {
  return value === 'light' || value === 'dark' ? value : 'system';
}

/** The `data-color-mode` of Primer's theme files for a choice. */
export function colorMode(choice: ThemeChoice): 'light' | 'dark' | 'auto' {
  return choice === 'system' ? 'auto' : choice;
}

/** The stored choice; a browser that blocks storage follows the system. */
export function readChoice(storage: () => ThemeStorage): ThemeChoice {
  try {
    return parseChoice(storage().getItem(THEME_KEY));
  } catch {
    return 'system';
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

/** Applies a choice to the document element (Primer reads `data-color-mode`). */
export function applyChoice(root: HTMLElement, choice: ThemeChoice): void {
  root.dataset['colorMode'] = colorMode(choice);
}
