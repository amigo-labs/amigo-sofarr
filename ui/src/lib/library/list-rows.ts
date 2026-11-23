// The rows above the movie and series lists (spec ui-cinema-library-rows): „Zuletzt
// hinzugefügt“, „Fehlt noch“, „Unter Cutoff“ and, while ratings are on, „Am besten
// bewertet“, each one request to the list API of today. Pure, so the rules are unit-tested.

import type { MovieListItem } from '../api/generated/MovieListItem';
import type { SeriesListItem } from '../api/generated/SeriesListItem';
import { tileName } from '../components/tile';
import { ROW_SIZE, type HomeTile } from './home';
import {
  EMPTY_FILTERS,
  apiQuery,
  filterParams,
  isFiltered,
  type ListFilters,
  type ListKind,
} from './list-query';
import { movieRow, seriesRow } from './rows';

export type ListRowId = 'added' | 'missing' | 'cutoff' | 'rated';

export type ListRow = {
  readonly id: ListRowId;
  readonly title: string;
  /** The toolbar's filters and sort that show the row's titles in the full list (AC-05). */
  readonly filters: ListFilters;
  /** Only while IMDb ratings are on (AC-01). */
  readonly ratings: boolean;
};

const NEWEST: ListFilters = { ...EMPTY_FILTERS, sort: 'added', order: 'desc' };

/** The four rows in their order on the page (AC-01, AC-02). */
export const LIST_ROWS: readonly ListRow[] = [
  { id: 'added', title: 'Zuletzt hinzugefügt', filters: NEWEST, ratings: false },
  {
    id: 'missing',
    title: 'Fehlt noch',
    filters: { ...NEWEST, tris: { missing: 'yes' } },
    ratings: false,
  },
  {
    id: 'cutoff',
    title: 'Unter Cutoff',
    filters: { ...NEWEST, tris: { cutoff_unmet: 'yes' } },
    ratings: false,
  },
  // The list API sorts by rating best first in ascending order (imdb-ratings AC-06).
  {
    id: 'rated',
    title: 'Am besten bewertet',
    filters: { ...EMPTY_FILTERS, sort: 'rating', order: 'asc' },
    ratings: true,
  },
];

/** A plain list: no search text and no filter, so the rows show (AC-04). */
export const isPlain = (filters: ListFilters): boolean => !isFiltered(filters);

/** The list API query of a row: one page of at most [`ROW_SIZE`] titles. */
export const listRowQuery = (kind: ListKind, row: ListRow): string =>
  apiQuery(kind, row.filters, 1, ROW_SIZE);

/** Where „Alle anzeigen“ of a row leads: the list's address with the row's filters. */
export function listRowHref(kind: ListKind, row: ListRow): string {
  const path = kind === 'movie' ? '/filme' : '/serien';
  const params = filterParams(kind, row.filters).toString();
  return params === '' ? path : `${path}?${params}`;
}

/** Whether „Alle anzeigen“ keeps the list plain, so the rows stay above it (AC-05). */
export const keepsRows = (row: ListRow): boolean => isPlain(row.filters);

/**
 * The tiles of a row: the list's items as poster tiles, „Am besten bewertet“ without the
 * titles that have no rating (AC-01). While ratings are on, the tile's name carries the
 * rating with votes and its overlay the compact rating, as the full list's tiles do.
 */
export function listRowTiles(
  row: ListRow,
  items: readonly (MovieListItem | SeriesListItem)[],
  ratings: boolean,
): HomeTile[] {
  return items
    .filter((item) => row.id !== 'rated' || item.imdb_rating !== null)
    .slice(0, ROW_SIZE)
    .map((item) => {
      const r = 'episodes' in item ? seriesRow(item) : movieRow(item);
      return {
        key: `${r.kind}/${r.id}`,
        kind: r.kind,
        id: r.id,
        title: r.title,
        year: r.year,
        hasPoster: r.hasPoster,
        mark: r.mark,
        dimmed: !r.monitored,
        href: r.href,
        when: ratings ? r.rating : null,
        name: tileName(r.title, r.year, ratings ? r.ratingFull : null),
      };
    });
}
