// The browser side of the stale-asset recovery (spec web-ui-shell AC-19); the decision
// itself is in `stale-asset.ts`.

import { decide, settle, type AssetDecision } from './stale-asset';

const storage = (): Storage => window.sessionStorage;

/** What this document decided; one decision per document, however many assets fail. */
let outcome: AssetDecision | null = null;

/**
 * Handles a failed asset while showing `target`: loads the app document for it again
 * once, or returns `give-up` so the caller shows the error with a reload action.
 */
export function recover(target: URL): AssetDecision {
  if (outcome !== null) return outcome;
  outcome = decide(storage, Date.now());
  if (outcome === 'reload') {
    const here = `${location.pathname}${location.search}`;
    if (`${target.pathname}${target.search}` === here) {
      location.reload();
    } else {
      location.assign(target.href);
    }
  }
  return outcome;
}

/** Called once the app shows a page: a later failure may reload again. */
export function startedCleanly(): void {
  if (outcome === null) settle(storage);
}
