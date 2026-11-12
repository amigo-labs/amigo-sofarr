// Filters, search and sorting of the movie and series lists, kept in the page address
// with German names (spec web-ui-library AC-03, AC-06), and the matching API query.

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
];

export const EMPTY_FILTERS: ListFilters = {
  tris: {},
  profile: null,
  tag: null,
  q: '',
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
    filters.q.trim() !== ''
  );
}
