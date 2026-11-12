// Whether the discover pages can work (TMDB key, spec discover AC-15) and the IMDb data
// for ratings and their attribution (spec imdb-ratings AC-08), read once per app start.

import type { ApiFailure } from '../api/http';
import type { DiscoverStatus } from '../api/generated/DiscoverStatus';
import type { ImdbInfo } from '../api/generated/ImdbInfo';
import { getImdbInfo } from '../imdb/imdb';
import { getStatus } from './api';

const state = $state<{
  status: DiscoverStatus | null;
  imdb: ImdbInfo | null;
  failure: ApiFailure | null;
}>({ status: null, imdb: null, failure: null });
let loading: Promise<void> | null = null;

export const discover = {
  get status(): DiscoverStatus | null {
    return state.status;
  },
  /** `null` until known, and while ratings are off. */
  get imdb(): ImdbInfo | null {
    return state.imdb;
  },
  get failure(): ApiFailure | null {
    return state.failure;
  },
};

/** Reads the status unless it is there or on its way; a failure tries again next time. */
export function ensureStatus(): Promise<void> {
  loading ??= (async () => {
    const result = await getStatus();
    if (!result.ok) {
      if (result.failure.kind !== 'unauthorized') state.failure = result.failure;
      loading = null;
      return;
    }
    state.failure = null;
    state.status = result.data;
    if (result.data.ratings) {
      const imdb = await getImdbInfo();
      if (imdb.ok) state.imdb = imdb.data;
    }
  })();
  return loading;
}
