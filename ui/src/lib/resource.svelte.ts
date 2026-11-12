// Data a page shows and refreshes on its own (spec web-ui-shell AC-12, AC-13): the last
// data stays visible when a refresh fails, the next attempt follows by itself, and a
// loading indicator appears only when an answer takes longer than a moment.

import type { ApiFailure, ApiResult } from './api/http';

/** A page re-reads data that changes on its own this often. */
export const REFRESH_MS = 15_000;
/** A loading indicator appears when an answer takes longer than this. */
export const LOADING_DELAY_MS = 300;

export type Resource<T> = {
  readonly data: T | null;
  readonly failure: ApiFailure | null;
  /** True while a request is running for longer than [`LOADING_DELAY_MS`]. */
  readonly slow: boolean;
  /** Loads now (the retry action); the regular refresh continues afterwards. */
  readonly reload: () => Promise<void>;
  /** Starts loading and refreshing; returns the function that stops it. */
  readonly start: () => () => void;
};

export function createResource<T>(load: () => Promise<ApiResult<T>>): Resource<T> {
  const state = $state<{ data: T | null; failure: ApiFailure | null; slow: boolean }>({
    data: null,
    failure: null,
    slow: false,
  });
  let timer: ReturnType<typeof setTimeout> | undefined;
  let running = false;
  let generation = 0;

  const schedule = (): void => {
    clearTimeout(timer);
    // A failed refresh is tried again by the same timer, well within 30 seconds.
    if (running) timer = setTimeout(() => void reload(), REFRESH_MS);
  };

  const reload = async (): Promise<void> => {
    const mine = ++generation;
    const slowTimer = setTimeout(() => {
      if (mine === generation) state.slow = true;
    }, LOADING_DELAY_MS);
    const result = await load();
    clearTimeout(slowTimer);
    if (mine !== generation) return;
    state.slow = false;
    if (result.ok) {
      state.data = result.data;
      state.failure = null;
    } else if (result.failure.kind !== 'unauthorized') {
      // A 401 leads to the login page; the data shown so far stays until then.
      state.failure = result.failure;
    }
    schedule();
  };

  return {
    get data() {
      return state.data;
    },
    get failure() {
      return state.failure;
    },
    get slow() {
      return state.slow;
    },
    reload,
    start: () => {
      running = true;
      void reload();
      return () => {
        running = false;
        generation += 1;
        clearTimeout(timer);
      };
    },
  };
}
