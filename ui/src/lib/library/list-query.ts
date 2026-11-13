// Filters, search and sorting of the movie and series lists, kept in the page address
// with German names (spec web-ui-library AC-03, AC-06; IMDb ratings: spec imdb-ratings
// AC-06, AC-07, AC-09), and the matching API query.

import type { Order } from '../api/generated/Order';
import type { Sort } from '../api/generated/Sort';
import type { Tri } from '../api/generated/Tri';

export type ListKind = 'movie' | 'series';

/** The yes/no/any filters of a list, by their API name. */
export type TriKey = 'monitored' | 'missing' | 'cutoff_unmet' | 'has_file' | 'complete';

export type ListFilters = {
  readonly tris: Readonly<Partial<Record<TriKey, Tri>>>;
  readonly profile: number | null;
  readonly tag: number | null;
  readonly q: string;
  /** Lowest IMDb rating (0 to 10), `null`: any. */
  readonly minRating: number | null;
  /** Lowest number of IMDb votes, `null`: any. */
  readonly minVotes: number | null;
  readonly sort: Sort;
  readonly order: Order;
};

export type TriFilter = { readonly key: TriKey; readonly param: string; readonly label: string };

const MONITORED: TriFilter = { key: 'monitored', param: 'ueberwacht', label: 'Überwacht' };
const MISSING: TriFilter = { key: 'missing', param: 'fehlt', label: 'Fehlt' };
const CUTOFF: TriFilter = { key: 'cutoff_unmet', param: 'cutoff', label: 'Cutoff nicht erreicht' };

/** The yes/no/any filters each list offers, in their order on the page. */
export const TRI_FILTERS: Readonly<Record<ListKind, readonly TriFilter[]>> = {
  movie: [MONITORED, MISSING, CUTOFF, { key: 'has_file', param: 'datei', label: 'Mit Datei' }],
  series: [MONITORED, MISSING, CUTOFF, { key: 'complete', param: 'komplett', label: 'Komplett' }],
};

const TRI_PARAM: Readonly<Record<Tri, string | null>> = { yes: 'ja', no: 'nein', any: null };

export const SORTS: readonly {
  readonly value: Sort;
  readonly param: string;
  readonly label: string;
}[] = [
  { value: 'title', param: 'titel', label: 'Titel' },
  { value: 'year', param: 'jahr', label: 'Jahr' },
  { value: 'added', param: 'hinzugefuegt', label: 'Hinzugefügt' },
  { value: 'size', param: 'groesse', label: 'Größe' },
  { value: 'rating', param: 'bewertung', label: 'Bewertung' },
];

/** The sort choices while IMDb ratings are on or off (AC-09: no rating sort while off). */
export const sortsFor = (ratings: boolean): typeof SORTS =>
  ratings ? SORTS : SORTS.filter((s) => s.value !== 'rating');

export const EMPTY_FILTERS: ListFilters = {
  tris: {},
  profile: null,
  tag: null,
  q: '',
  minRating: null,
  minVotes: null,
  sort: 'title',
  order: 'asc',
};

function triOf(text: string | null): Tri {
  if (text === 'ja') return 'yes';
  if (text === 'nein') return 'no';
  return 'any';
}

function idOf(text: string | null): number | null {
  if (text === null || !/^\d+$/.test(text)) return null;
  const id = Number(text);
  return Number.isSafeInteger(id) ? id : null;
}

/** A minimum rating from the address, "7.5" or "7,5"; one decimal, 0 to 10. */
function ratingOf(text: string | null): number | null {
  if (text === null || !/^\d{1,2}([.,]\d)?$/.test(text)) return null;
  const rating = Number(text.replace(',', '.'));
  return rating <= 10 ? rating : null;
}

/** A minimum number of votes from the address. */
function votesOf(text: string | null): number | null {
  if (text === null || !/^\d{1,9}$/.test(text)) return null;
  return Number(text);
}

/** The filters of a page address; unknown or invalid values fall back to the default. */
export function parseFilters(kind: ListKind, params: Pick<URLSearchParams, 'get'>): ListFilters {
  const tris: Partial<Record<TriKey, Tri>> = {};
  for (const filter of TRI_FILTERS[kind]) {
    const value = triOf(params.get(filter.param));
    if (value !== 'any') tris[filter.key] = value;
  }
  const sort = SORTS.find((s) => s.param === params.get('sortierung'))?.value ?? 'title';
  return {
    tris,
    profile: idOf(params.get('profil')),
    tag: idOf(params.get('tag')),
    q: params.get('suche')?.trim() ?? '',
    minRating: ratingOf(params.get('mindestbewertung')),
    minVotes: votesOf(params.get('mindeststimmen')),
    sort,
    order: params.get('richtung') === 'ab' ? 'desc' : 'asc',
  };
}

/** The page address parameters of filters; defaults are left out. */
export function filterParams(kind: ListKind, filters: ListFilters): URLSearchParams {
  const params = new URLSearchParams();
  for (const filter of TRI_FILTERS[kind]) {
    const value = TRI_PARAM[filters.tris[filter.key] ?? 'any'];
    if (value !== null) params.set(filter.param, value);
  }
  if (filters.profile !== null) params.set('profil', String(filters.profile));
  if (filters.tag !== null) params.set('tag', String(filters.tag));
  if (filters.q.trim() !== '') params.set('suche', filters.q.trim());
  if (filters.minRating !== null) params.set('mindestbewertung', String(filters.minRating));
  if (filters.minVotes !== null) params.set('mindeststimmen', String(filters.minVotes));
  if (filters.sort !== 'title') {
    params.set('sortierung', SORTS.find((s) => s.value === filters.sort)?.param ?? 'titel');
  }
  if (filters.order === 'desc') params.set('richtung', 'ab');
  return params;
}

/** The API query of one page of the list (`/api/library/{movies|series}?…`). */
export function apiQuery(
  kind: ListKind,
  filters: ListFilters,
  page: number,
  pageSize: number,
): string {
  const params = new URLSearchParams();
  for (const filter of TRI_FILTERS[kind]) {
    const value = filters.tris[filter.key] ?? 'any';
    if (value !== 'any') params.set(filter.key, value);
  }
  if (filters.profile !== null) params.set('profile', String(filters.profile));
  if (filters.tag !== null) params.set('tag', String(filters.tag));
  if (filters.q.trim() !== '') params.set('q', filters.q.trim());
  if (filters.minRating !== null) params.set('min_rating', String(filters.minRating));
  if (filters.minVotes !== null) params.set('min_votes', String(filters.minVotes));
  params.set('sort', filters.sort);
  params.set('order', filters.order);
  params.set('page', String(page));
  params.set('page_size', String(pageSize));
  return `?${params.toString()}`;
}

/** Whether any filter or search narrows the list. */
export function isFiltered(filters: ListFilters): boolean {
  return (
    Object.values(filters.tris).some((t) => t !== 'any') ||
    filters.profile !== null ||
    filters.tag !== null ||
    filters.q.trim() !== '' ||
    filters.minRating !== null ||
    filters.minVotes !== null
  );
}

/** Whether the filters sort or filter by IMDb rating. */
export function usesRatings(filters: ListFilters): boolean {
  return filters.sort === 'rating' || filters.minRating !== null || filters.minVotes !== null;
}

/**
 * The filters without rating sort and minimums: while IMDb ratings are off the server
 * refuses them, so an old address still shows the list (AC-09).
 */
export function withoutRatings(filters: ListFilters): ListFilters {
  if (!usesRatings(filters)) return filters;
  return {
    ...filters,
    minRating: null,
    minVotes: null,
    sort: filters.sort === 'rating' ? 'title' : filters.sort,
  };
}
