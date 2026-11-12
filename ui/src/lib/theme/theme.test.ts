import { describe, expect, it } from 'vitest';
import { THEME_CHOICES, THEME_KEY, colorMode, parseChoice, readChoice, storeChoice } from './theme';

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

describe('theme choice', () => {
  it('offers system, light and dark with German labels', () => {
    expect(THEME_CHOICES.map((c) => c.label)).toEqual(['System', 'Hell', 'Dunkel']);
  });

  it('maps choices to Primer color modes', () => {
    expect(colorMode('light')).toBe('light');
    expect(colorMode('dark')).toBe('dark');
    expect(colorMode('system')).toBe('auto');
  });

  it('follows the system for unknown values', () => {
    expect(parseChoice(null)).toBe('system');
    expect(parseChoice('blau')).toBe('system');
    expect(parseChoice('dark')).toBe('dark');
  });

  it('remembers the choice in storage', () => {
    const storage = memory();
    expect(readChoice(() => storage)).toBe('system');
    expect(storeChoice(() => storage, 'dark')).toBe(true);
    expect(storage.getItem(THEME_KEY)).toBe('dark');
    expect(readChoice(() => storage)).toBe('dark');
  });

  it('follows the system when the browser blocks storage', () => {
    expect(readChoice(blocked)).toBe('system');
    expect(storeChoice(blocked, 'light')).toBe(false);
    const throwing = { ...memory(), setItem: () => blocked().setItem('', '') };
    expect(storeChoice(() => throwing, 'light')).toBe(false);
  });
});
