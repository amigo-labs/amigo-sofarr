// A list the server pages, loaded page by page while the user scrolls (AC-02). A new
// query replaces the list; answers of an older query are dropped.

import type { ApiFailure, ApiResult } from '../api/http';
import type { Page } from '../api/generated/Page';

export const PAGE_SIZE = 50;

export type Paged<T> = {
  readonly items: readonly T[];
  /** `null` until the first page of the current query arrived. */
  readonly total: number | null;
  readonly loading: boolean;
  readonly failure: ApiFailure | null;
  readonly done: boolean;
  /** Loads the first page of a query (filters, search, sorting changed). */
  readonly reset: (query: (page: number) => Promise<ApiResult<Page<T>>>) => Promise<void>;
  /** Loads the next page, if there is one and none is loading. */
  readonly more: () => Promise<void>;
  /** Replaces one item in place (e.g. after switching its monitoring). */
  readonly update: (match: (item: T) => boolean, change: (item: T) => T) => void;
};

export function createPaged<T>(): Paged<T> {
  const state = $state<{
    items: T[];
    total: number | null;
    loading: boolean;
    failure: ApiFailure | null;
    page: number;
  }>({ items: [], total: null, loading: false, failure: null, page: 0 });
  let generation = 0;
  let load: ((page: number) => Promise<ApiResult<Page<T>>>) | null = null;

  const fetchPage = async (page: number, replace: boolean): Promise<void> => {
    if (load === null) return;
    const mine = generation;
    state.loading = true;
    const result = await load(page);
    if (mine !== generation) return;
    state.loading = false;
    if (!result.ok) {
      if (result.failure.kind !== 'unauthorized') state.failure = result.failure;
      return;
    }
    state.failure = null;
    state.page = page;
    state.total = result.data.total;
    state.items = replace ? result.data.items : [...state.items, ...result.data.items];
  };

  const done = (): boolean => state.total !== null && state.items.length >= state.total;

  return {
    get items() {
      return state.items;
    },
    get total() {
      return state.total;
    },
    get loading() {
      return state.loading;
    },
    get failure() {
      return state.failure;
    },
    get done() {
      return done();
    },
    reset: async (query) => {
      generation += 1;
      load = query;
      await fetchPage(1, true);
    },
    more: async () => {
      if (state.loading || done() || state.failure !== null) return;
      await fetchPage(state.page + 1, false);
    },
    update: (match, change) => {
      state.items = state.items.map((item) => (match(item) ? change(item) : item));
    },
  };
}
