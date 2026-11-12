// The current library state and rating of the titles a discover page shows (spec discover
// AC-01, AC-02, AC-05): ratings still loading are asked for every few seconds, and titles
// added meanwhile (another tab, an import list) change their state without a reload.

import { untrack } from 'svelte';
import type { LibraryState } from '../api/generated/LibraryState';
import type { Rating } from '../api/generated/Rating';
import type { TitleKind } from '../api/generated/TitleKind';
import type { TitleRef } from '../api/generated/TitleRef';
import { getStates } from './api';

/** What a page knows of a title. */
export type Shown = {
  readonly kind: TitleKind;
  readonly tmdb_id: number;
  readonly library: LibraryState;
  readonly rating: Rating;
};

export type State = { readonly library: LibraryState; readonly rating: Rating };

/** Ratings still loading are asked for this often. */
export const PENDING_MS = 2_000;
/** Library states are refreshed this often. */
export const STATES_MS = 15_000;
/** After this long without an answer for a rating, it is asked for only every STATES_MS. */
const PENDING_PATIENCE_MS = 60_000;
/** The server answers at most this many titles at once. */
export const STATES_BATCH = 500;

export const titleKey = (kind: TitleKind, tmdbId: number): string => `${kind}/${tmdbId}`;

/** The titles in batches the server accepts. */
export function batches(refs: readonly TitleRef[], size = STATES_BATCH): TitleRef[][] {
  const out: TitleRef[][] = [];
  for (let i = 0; i < refs.length; i += size) out.push(refs.slice(i, i + size));
  return out;
}

export type TitleStates = {
  /** The newest known state of a title: from the last refresh, else the page's own. */
  readonly of: (title: Shown) => State;
  /** The titles the page shows now (replaces the earlier ones). */
  readonly show: (titles: readonly Shown[]) => void;
  /** A title this page just added to the library. */
  readonly added: (kind: TitleKind, tmdbId: number, libraryId: number) => void;
  /** Starts refreshing; returns the function that stops it. */
  readonly start: () => () => void;
};

export function createTitleStates(): TitleStates {
  const known = $state<{ map: Record<string, State> }>({ map: {} });
  let shown: readonly TitleRef[] = [];
  let lastRefresh = 0;
  let pendingSince: number | null = null;
  let running = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const of = (title: Shown): State =>
    known.map[titleKey(title.kind, title.tmdb_id)] ?? {
      library: title.library,
      rating: title.rating,
    };

  const anyPending = (): boolean =>
    shown.some((t) => known.map[titleKey(t.kind, t.tmdb_id)]?.rating.state === 'pending');

  const refresh = async (): Promise<void> => {
    lastRefresh = Date.now();
    const results = await Promise.all(batches(shown).map((batch) => getStates(batch)));
    for (const result of results) {
      if (!result.ok) continue;
      for (const t of result.data.titles) {
        known.map[titleKey(t.kind, t.tmdb_id)] = { library: t.library, rating: t.rating };
      }
    }
  };

  const due = (now: number): boolean => {
    if (shown.length === 0) return false;
    if (anyPending()) {
      pendingSince ??= now;
      if (now - pendingSince < PENDING_PATIENCE_MS) return true;
    } else {
      pendingSince = null;
    }
    return now - lastRefresh >= STATES_MS;
  };

  const tick = async (): Promise<void> => {
    if (due(Date.now())) await refresh();
    if (running) timer = setTimeout(() => void tick(), PENDING_MS);
  };

  return {
    of,
    // Called from effects: what it reads of the known states must not become a dependency.
    show: (titles) =>
      untrack(() => {
        shown = titles.map((t) => ({ kind: t.kind, tmdb_id: t.tmdb_id }));
        pendingSince = null;
        for (const t of titles) {
          const key = titleKey(t.kind, t.tmdb_id);
          known.map[key] ??= { library: t.library, rating: t.rating };
        }
      }),
    added: (kind, tmdbId, libraryId) => {
      const key = titleKey(kind, tmdbId);
      const rating = known.map[key]?.rating ?? { state: 'off', rating: null, votes: null };
      known.map[key] = { library: { status: 'in_library', id: libraryId }, rating };
    },
    start: () => {
      running = true;
      lastRefresh = Date.now();
      timer = setTimeout(() => void tick(), PENDING_MS);
      return () => {
        running = false;
        clearTimeout(timer);
      };
    },
  };
}
