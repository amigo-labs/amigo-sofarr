// Recovery when an asset of an older UI build is gone (spec web-ui-shell AC-19): the app
// document is loaded again once; a second failure shows an error instead of a loop.

/** Remembers in the tab that the app document was already loaded again. */
export const RELOAD_KEY = 'sofarr-asset-reload';

export const ASSET_ERROR_MESSAGE =
  'Ein Teil der Oberfläche konnte nicht geladen werden. Wahrscheinlich wurde sofarr ' +
  'aktualisiert oder der Server ist nicht erreichbar.';

export type AssetDecision = 'reload' | 'give-up';

/** The part of `Storage` the recovery needs; tests pass a fake. */
export type AttemptStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;

const PATTERNS: readonly RegExp[] = [
  // Chromium
  /Failed to fetch dynamically imported module/i,
  // Firefox
  /error loading dynamically imported module/i,
  // Safari
  /Importing a module script failed/i,
  // Vite's preload helper for CSS
  /Unable to preload CSS/i,
];

/** Whether an error says that a script or style of the build could not be loaded. */
export function isAssetLoadError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : typeof error === 'string' ? error : '';
  return PATTERNS.some((pattern) => pattern.test(message));
}

/**
 * Decides what to do about a failed asset and records a reload attempt. Without usable
 * storage the attempt cannot be remembered, so there is no automatic reload at all.
 */
export function decide(storage: () => AttemptStorage, now: number): AssetDecision {
  try {
    const store = storage();
    if (store.getItem(RELOAD_KEY) !== null) return 'give-up';
    store.setItem(RELOAD_KEY, String(now));
    return 'reload';
  } catch {
    return 'give-up';
  }
}

/** Forgets the attempt once the app started without an asset failure. */
export function settle(storage: () => AttemptStorage): void {
  try {
    storage().removeItem(RELOAD_KEY);
  } catch {
    // Nothing was remembered, so there is nothing to forget.
  }
}
