// Helpers of the theme tests (spec amigo-theme): the tokens of the generated theme files
// and the palette a test wants.

import type { Locator, Page } from '@playwright/test';
import { tokensCss } from './theme-files.mjs';

export const PALETTES = ['dark', 'light'] as const;
export type Palette = (typeof PALETTES)[number];

/** The key the UI remembers the theme choice under (`src/lib/theme/theme.ts`). */
export const THEME_KEY = 'sofarr-theme';

function declarations(block: string): Record<string, string> {
  const found: Record<string, string> = {};
  for (const match of block.matchAll(/--am-([a-z0-9-]+):\s*([^;]+);/g)) {
    found[match[1] ?? ''] = (match[2] ?? '').trim();
  }
  return found;
}

function rgb(hex: string): string {
  const value = Number.parseInt(hex.slice(1), 16);
  return `rgb(${value >> 16}, ${(value >> 8) & 255}, ${value & 255})`;
}

function readTokens(): Record<Palette, Record<string, string>> {
  const css = tokensCss();
  const dark = /:root \{([^}]*)\}/.exec(css)?.[1];
  const light = /:root\[data-theme='light'\] \{([^}]*)\}/.exec(css)?.[1];
  if (dark === undefined || light === undefined) {
    throw new Error('unexpected layout of src/theme/tokens.css');
  }
  const all = declarations(dark);
  return { dark: all, light: { ...all, ...declarations(light) } };
}

const TOKENS = readTokens();

/** A token's raw value in a palette, e.g. `8px` for `radius-md`. */
export function tokenValue(palette: Palette, name: string): string {
  const value = TOKENS[palette][name];
  if (value === undefined) throw new Error(`no token ${name} in the theme files`);
  return value;
}

/** A colour token as the browser reports computed colours: `rgb(r, g, b)`. */
export function token(palette: Palette, name: string): string {
  const value = tokenValue(palette, name);
  if (!/^#[0-9a-f]{6}$/.test(value)) throw new Error(`token ${name} is not a colour: ${value}`);
  return rgb(value);
}

/** Stores a theme choice before the page's own scripts run, on every load of `page`. */
export async function storeTheme(page: Page, choice: string): Promise<void> {
  await page.addInitScript(
    ([key, value]) => {
      try {
        window.localStorage.setItem(key, value);
      } catch {
        // A test that blocks storage has no stored choice.
      }
    },
    [THEME_KEY, choice] as const,
  );
}

/** A computed style property of the first element matching `selector`. */
export async function styleOf(page: Page, selector: string, property: string): Promise<string> {
  return page
    .locator(selector)
    .first()
    .evaluate((el, name) => getComputedStyle(el).getPropertyValue(name), property);
}

/** The button of a theme choice in the switch (`Dunkel`, `Hell`, `System`). */
export function themeButton(page: Page, label: string): Locator {
  return page.getByRole('group', { name: 'Farbschema' }).getByRole('button', { name: label });
}

/** Picks a theme choice in the switch. */
export async function pickTheme(page: Page, label: string): Promise<void> {
  await themeButton(page, label).click();
}
