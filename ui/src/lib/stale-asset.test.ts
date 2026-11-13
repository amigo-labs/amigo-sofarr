import { describe, expect, it } from 'vitest';
import { RELOAD_KEY, decide, isAssetLoadError, settle, type AttemptStorage } from './stale-asset';

function memory(): AttemptStorage & { readonly items: Map<string, string> } {
  const items = new Map<string, string>();
  return {
    items,
    getItem: (key) => items.get(key) ?? null,
    setItem: (key, value) => void items.set(key, value),
    removeItem: (key) => void items.delete(key),
  };
}

const blocked = (): AttemptStorage => {
  throw new DOMException('Der Zugriff ist blockiert', 'SecurityError');
};

describe('isAssetLoadError', () => {
  it('recognizes failed module and style loads of the browsers', () => {
    for (const message of [
      'Failed to fetch dynamically imported module: http://h/_app/immutable/nodes/4.abc.js',
      'error loading dynamically imported module: http://h/_app/immutable/nodes/4.abc.js',
      'Importing a module script failed.',
      'Unable to preload CSS for /_app/immutable/assets/4.abc.css',
    ]) {
      expect(isAssetLoadError(new TypeError(message))).toBe(true);
    }
  });

  it('ignores other errors', () => {
    expect(isAssetLoadError(new Error('x is undefined'))).toBe(false);
    expect(isAssetLoadError(null)).toBe(false);
    expect(isAssetLoadError({ message: 'Failed to fetch dynamically imported module' })).toBe(
      false,
    );
  });
});

describe('decide', () => {
  it('reloads once, then gives up instead of looping', () => {
    const storage = memory();
    expect(decide(() => storage, 1000)).toBe('reload');
    expect(storage.items.get(RELOAD_KEY)).toBe('1000');
    expect(decide(() => storage, 2000)).toBe('give-up');
    expect(decide(() => storage, 3000)).toBe('give-up');
  });

  it('reloads again after the app started cleanly', () => {
    const storage = memory();
    expect(decide(() => storage, 1000)).toBe('reload');
    settle(() => storage);
    expect(storage.items.has(RELOAD_KEY)).toBe(false);
    expect(decide(() => storage, 2000)).toBe('reload');
  });

  it('never reloads automatically when the attempt cannot be remembered', () => {
    expect(decide(blocked, 1000)).toBe('give-up');
    expect(() => settle(blocked)).not.toThrow();
  });
});
