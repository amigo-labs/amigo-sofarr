// A list of titles TMDB pages (a shelf's further titles, the filtered result list),
// loaded page by page while the user scrolls. A new query replaces the list; answers of
// an older query are dropped, and a title already shown is not shown twice.

import type { ApiFailure, ApiResult } from '../api/http';
import type { Freshness } from '../api/generated/Freshness';
import type { ListAnswer } from '../api/generated/ListAnswer';
import type { TitleCard } from '../api/generated/TitleCard';
import { titleKey } from './states.svelte';

export type TitlePages = {
  readonly items: readonly TitleCard[];
  /** `null` until the first page of the current query arrived. */
  readonly freshness: Freshness | null;
  readonly loading: boolean;
  readonly failure: ApiFailure | null;
  /** TMDB has more pages. */
  readonly more: boolean;
  readonly reset: (load: (page: number) => Promise<ApiResult<ListAnswer>>) => Promise<void>;
  /** Loads the next page, if there is one and none is loading. */
  readonly next: () => Promise<void>;
};

/** Cards not yet in `known`, without repeats. */
export function newCards(known: readonly TitleCard[], cards: readonly TitleCard[]): TitleCard[] {
  const seen = new Set(known.map((c) => titleKey(c.kind, c.tmdb_id)));
  return cards.filter((c) => {
    const key = titleKey(c.kind, c.tmdb_id);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

/**
 * The freshness to show for several loaded pages: a stale page wins over fresh ones (its
 * notice shows), and the fetch time is the oldest of all pages (spec ui-polish AC-10).
 */
export function fresher(shown: Freshness | null, page: Freshness): Freshness {
  if (shown === null) return page;
  const base = page.stale && !shown.stale ? page : shown;
  const other = base === page ? shown : page;
  return Date.parse(other.fetched_at) < Date.parse(base.fetched_at)
    ? { ...base, fetched_at: other.fetched_at }
    : base;
}

export function createTitlePages(): TitlePages {
  const state = $state<{
    items: TitleCard[];
    freshness: Freshness | null;
    loading: boolean;
    failure: ApiFailure | null;
    page: number | null;
  }>({ items: [], freshness: null, loading: false, failure: null, page: null });
  let generation = 0;
  let load: ((page: number) => Promise<ApiResult<ListAnswer>>) | null = null;

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
    state.page = result.data.next_page;
    state.freshness = fresher(replace ? null : state.freshness, result.data.freshness);
    state.items = replace
      ? newCards([], result.data.items)
      : [...state.items, ...newCards(state.items, result.data.items)];
  };

  return {
    get items() {
      return state.items;
    },
    get freshness() {
      return state.freshness;
    },
    get loading() {
      return state.loading;
    },
    get failure() {
      return state.failure;
    },
    get more() {
      return state.page !== null;
    },
    reset: async (query) => {
      generation += 1;
      load = query;
      state.items = [];
      state.freshness = null;
      state.page = null;
      state.failure = null;
      await fetchPage(1, true);
    },
    next: async () => {
      if (state.loading || state.page === null || state.failure !== null) return;
      await fetchPage(state.page, false);
    },
  };
}
