import { describe, expect, it } from 'vitest';
import {
  DEFAULT_CHOICE,
  THEME_CHOICES,
  THEME_KEY,
  applyChoice,
  dataTheme,
  parseChoice,
  readChoice,
  storeChoice,
} from './theme';

function memory(): Storage {
  const items = new Map<string, string>();
  return {
    get length() {
      return items.size;
    },
    clear: () => items.clear(),
    getItem: (key) => items.get(key) ?? null,
    key: (index) => [...items.keys()][index] ?? null,
    removeItem: (key) => void items.delete(key),
    setItem: (key, value) => void items.set(key, value),
  };
}

const blocked = (): Storage => {
  throw new DOMException('Der Zugriff ist blockiert', 'SecurityError');
};

/** The part of the document element `applyChoice` touches. */
function root(initial: string | null): Pick<HTMLElement, 'setAttribute' | 'removeAttribute'> & {
  theme: string | null;
} {
  return {
    theme: initial,
    setAttribute(name: string, value: string) {
      if (name === 'data-theme') this.theme = value;
    },
    removeAttribute(name: string) {
      if (name === 'data-theme') this.theme = null;
    },
  };
}

describe('theme choice', () => {
  it('offers dark, light and system with German labels, dark first', () => {
    expect(THEME_CHOICES.map((c) => c.label)).toEqual(['Dunkel', 'Hell', 'System']);
    expect(THEME_CHOICES.map((c) => c.value)).toEqual(['dark', 'light', 'system']);
  });

  it('is dark by default (amigo-theme AC-04)', () => {
    expect(DEFAULT_CHOICE).toBe('dark');
  });

  it('maps choices to the data-theme of the theme files', () => {
    expect(dataTheme('light')).toBe('light');
    expect(dataTheme('dark')).toBe('dark');
    expect(dataTheme('system')).toBeNull();
  });

  it('sets data-theme for dark and light and removes it for system', () => {
    const element = root('dark');
    applyChoice(element, 'light');
    expect(element.theme).toBe('light');
    applyChoice(element, 'system');
    expect(element.theme).toBeNull();
    applyChoice(element, 'dark');
    expect(element.theme).toBe('dark');
  });

  it('counts unknown values as no choice', () => {
    expect(parseChoice(null)).toBeNull();
    expect(parseChoice('blau')).toBeNull();
    expect(parseChoice('auto')).toBeNull();
    expect(parseChoice('')).toBeNull();
    expect(parseChoice('dark')).toBe('dark');
    expect(parseChoice('light')).toBe('light');
    expect(parseChoice('system')).toBe('system');
  });

  it('remembers the choice in storage', () => {
    const storage = memory();
    expect(readChoice(() => storage)).toBe('dark');
    expect(storeChoice(() => storage, 'light')).toBe(true);
    expect(storage.getItem(THEME_KEY)).toBe('light');
    expect(readChoice(() => storage)).toBe('light');
    expect(storeChoice(() => storage, 'system')).toBe(true);
    expect(readChoice(() => storage)).toBe('system');
    storage.setItem(THEME_KEY, 'auto');
    expect(readChoice(() => storage)).toBe('dark');
  });

  it('starts dark and remembers nothing when the browser blocks storage (AC-08)', () => {
    expect(readChoice(blocked)).toBe('dark');
    expect(storeChoice(blocked, 'light')).toBe(false);
    const throwing = { ...memory(), setItem: () => blocked().setItem('', '') };
    expect(storeChoice(() => throwing, 'light')).toBe(false);
  });
});
