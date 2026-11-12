// Whether IMDb ratings are on and the date of their data (spec imdb-ratings AC-08, AC-09),
// read by every page that shows ratings when it opens.

import type { ImdbInfo } from '../api/generated/ImdbInfo';
import { getImdbInfo } from './imdb';

const state = $state<{ value: ImdbInfo | null; failed: boolean }>({ value: null, failed: false });

export const imdb = {
  /** `null` until the first answer. */
  get info(): ImdbInfo | null {
    return state.value;
  },
  /** Whether ratings are shown: only once the server said they are on. */
  get shown(): boolean {
    return state.value?.enabled ?? false;
  },
  /** Whether the answer arrived or failed, so a list may rely on it. */
  get settled(): boolean {
    return state.value !== null || state.failed;
  },
};

/** Reads the ratings state again; the last answer stays while a new one fails. */
export async function loadImdb(): Promise<void> {
  const result = await getImdbInfo();
  if (result.ok) {
    state.value = result.data;
    state.failed = false;
  } else {
    state.failed = true;
  }
}
