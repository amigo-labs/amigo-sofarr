// Spec ui-shell-redesign AC-03 .. AC-05: the sidebar's collapsed state, remembered in the
// browser when it lets us.

import { describe, expect, it } from 'vitest';
import { SIDEBAR_KEY, applyCollapsed, readCollapsed, storeCollapsed } from './sidebar';

function memory(): Pick<Storage, 'getItem' | 'setItem' | 'removeItem'> {
  const items = new Map<string, string>();
  return {
    getItem: (key) => items.get(key) ?? null,
    setItem: (key, value) => void items.set(key, value),
    removeItem: (key) => void items.delete(key),
  };
}

const blocked = (): Storage => {
  throw new DOMException('Der Zugriff ist blockiert', 'SecurityError');
};

describe('sidebar state', () => {
  it('is expanded while nothing is stored', () => {
    expect(readCollapsed(() => memory())).toBe(false);
  });

  it('remembers collapsed and expanded', () => {
    const storage = memory();
    expect(storeCollapsed(() => storage, true)).toBe(true);
    expect(storage.getItem(SIDEBAR_KEY)).toBe('collapsed');
    expect(readCollapsed(() => storage)).toBe(true);
    expect(storeCollapsed(() => storage, false)).toBe(true);
    expect(storage.getItem(SIDEBAR_KEY)).toBeNull();
    expect(readCollapsed(() => storage)).toBe(false);
  });

  it('counts unknown values as nothing stored', () => {
    const storage = memory();
    storage.setItem(SIDEBAR_KEY, 'zu');
    expect(readCollapsed(() => storage)).toBe(false);
  });

  it('starts expanded and remembers nothing when the browser blocks storage (AC-05)', () => {
    expect(readCollapsed(blocked)).toBe(false);
    expect(storeCollapsed(blocked, true)).toBe(false);
  });

  it('marks the document', () => {
    const attributes = new Map<string, string>();
    const root = {
      setAttribute: (name: string, value: string) => void attributes.set(name, value),
      removeAttribute: (name: string) => void attributes.delete(name),
    };
    applyCollapsed(root, true);
    expect(attributes.get('data-sidebar')).toBe('collapsed');
    applyCollapsed(root, false);
    expect(attributes.has('data-sidebar')).toBe(false);
  });
});
