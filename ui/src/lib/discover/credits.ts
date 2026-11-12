// The credits of a person page (spec discover AC-05): sorting by year or IMDb rating,
// "nur fehlende", movies or series, and paging of long lists – all in the browser, as the
// server delivers every credit at once. The choices live in the page address.

import type { CreditCard } from '../api/generated/CreditCard';
import type { TitleKind } from '../api/generated/TitleKind';
import type { State } from './states.svelte';

export type CreditSort = 'neueste' | 'aelteste' | 'bewertung';

export type CreditChoices = {
  readonly sort: CreditSort;
  readonly missingOnly: boolean;
  readonly kind: TitleKind | null;
};

export const DEFAULT_CHOICES: CreditChoices = { sort: 'neueste', missingOnly: false, kind: null };

/** Credits shown per section before "Weitere anzeigen". */
export const CREDITS_PAGE = 50;

export function parseChoices(
  params: Pick<URLSearchParams, 'get'>,
  ratings: boolean,
): CreditChoices {
  const sort = params.get('sortierung');
  const kind = params.get('art');
  return {
    sort: sort === 'aelteste' || (sort === 'bewertung' && ratings) ? sort : DEFAULT_CHOICES.sort,
    missingOnly: params.get('nur') === 'fehlende',
    kind: kind === 'filme' ? 'movie' : kind === 'serien' ? 'series' : null,
  };
}

export function choiceParams(choices: CreditChoices): URLSearchParams {
  const params = new URLSearchParams();
  if (choices.sort !== DEFAULT_CHOICES.sort) params.set('sortierung', choices.sort);
  if (choices.missingOnly) params.set('nur', 'fehlende');
  if (choices.kind !== null) params.set('art', choices.kind === 'movie' ? 'filme' : 'serien');
  return params;
}

/** Undated titles come last, whatever the direction. */
function byDate(a: CreditCard, b: CreditCard, newest: boolean): number {
  if (a.date === null || b.date === null) {
    if (a.date === b.date) return a.title.title.localeCompare(b.title.title, 'de');
    return a.date === null ? 1 : -1;
  }
  const order = a.date.localeCompare(b.date);
  return newest ? -order : order;
}

/** Higher rating first, votes break ties; titles without a known rating come last. */
function byRating(a: State, b: State): number {
  const ra = a.rating.state === 'rated' ? a.rating.rating : null;
  const rb = b.rating.state === 'rated' ? b.rating.rating : null;
  if (ra === null || rb === null) return ra === rb ? 0 : ra === null ? 1 : -1;
  if (ra !== rb) return rb - ra;
  return (b.rating.votes ?? 0) - (a.rating.votes ?? 0);
}

/** The credits of one section as chosen, with each title's current state. */
export function arrange(
  credits: readonly CreditCard[],
  choices: CreditChoices,
  stateOf: (credit: CreditCard) => State,
): CreditCard[] {
  const kept = credits.filter(
    (c) =>
      (choices.kind === null || c.title.kind === choices.kind) &&
      (!choices.missingOnly || stateOf(c).library.status === 'none'),
  );
  return kept.toSorted((a, b) => {
    if (choices.sort === 'bewertung') {
      const order = byRating(stateOf(a), stateOf(b));
      if (order !== 0) return order;
    }
    return byDate(a, b, choices.sort !== 'aelteste');
  });
}
