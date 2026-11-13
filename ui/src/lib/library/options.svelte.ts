// Quality profiles, tags and availabilities the filters and settings offer, read once.

import type { LibraryOptions } from '../api/generated/LibraryOptions';
import { getOptions } from '../api/library';

const state = $state<{ value: LibraryOptions | null }>({ value: null });
let loading: Promise<void> | null = null;

export const options = {
  get value(): LibraryOptions | null {
    return state.value;
  },
};

/** Reads the options unless they are there or on their way; a failure tries again later. */
export function ensureOptions(): Promise<void> {
  loading ??= (async () => {
    const result = await getOptions();
    if (result.ok) state.value = result.data;
    else loading = null;
  })();
  return loading;
}
