// The rows of the overview (spec ui-cinema-home): „Demnächst“ from the overview's upcoming
// dates, „Zuletzt hinzugefügt“ and „Fehlt noch“ from the movie and series lists, merged by
// the time they were added. Pure, so the rules are unit-tested.

import type { CalendarEntry } from '../api/generated/CalendarEntry';
import type { MediaKind } from '../api/generated/MediaKind';
import type { MovieListItem } from '../api/generated/MovieListItem';
import type { SeriesListItem } from '../api/generated/SeriesListItem';
import type { Mark } from '../components/tile';
import { tileName } from '../components/tile';
import { mediaHref } from './actions';
import { dayHeading, episodeCode } from './format';
import { movieRow, seriesRow } from './rows';

/** At most this many tiles per row (AC-01). */
export const ROW_SIZE = 20;

/** Where „Alle anzeigen“ of each row leads (AC-03). */
export const ROW_LINKS = {
  upcoming: '/kalender',
  added: '/filme?sortierung=hinzugefuegt&richtung=ab',
  missing: '/fehlend',
} as const;

/** One library title in a row. */
export type HomeTile = {
  /** Unique within a row: kind and id. */
  readonly key: string;
  readonly kind: MediaKind;
  readonly id: number;
  readonly title: string;
  readonly year: number | null;
  readonly hasPoster: boolean;
  readonly mark: Mark | null;
  /** An unmonitored title: its poster is dimmed. */
  readonly dimmed: boolean;
  readonly href: string;
  /** The line below the year in the overlay, e.g. „Morgen · S03E05“ (AC-02). */
  readonly when: string | null;
  /** The accessible name of the tile's link. */
  readonly name: string;
};

/**
 * The tiles of „Demnächst“ (AC-01, AC-02): each title once at its soonest entry, with the
 * day as the overview's day headings name it and, for a series, the code of its first
 * episode that day. `entries` come as the overview sends them.
 */
export function upcomingTiles(entries: readonly CalendarEntry[], today: string): HomeTile[] {
  const sorted = entries.toSorted(
    (a, b) =>
      a.date.localeCompare(b.date) ||
      (a.season ?? 0) - (b.season ?? 0) ||
      (a.episode ?? 0) - (b.episode ?? 0),
  );
  const tiles = new Map<string, HomeTile>();
  for (const entry of sorted) {
    const kind: MediaKind = entry.kind === 'movie' ? 'movie' : 'series';
    const id = kind === 'movie' ? entry.movie_id : entry.series_id;
    if (id === null) continue;
    const key = `${kind}/${id}`;
    if (tiles.has(key)) continue;
    const day = dayHeading(entry.date, today);
    const code = kind === 'series' ? episodeCode(entry.season, entry.episode) : '';
    const parts = code === '' ? [day] : [day, code];
    tiles.set(key, {
      key,
      kind,
      id,
      title: entry.title,
      year: entry.year,
      hasPoster: entry.has_poster,
      mark: null,
      dimmed: false,
      href: mediaHref(kind, id),
      when: parts.join(' · '),
      name: [tileName(entry.title, entry.year, null), ...parts].join(', '),
    });
  }
  return [...tiles.values()].slice(0, ROW_SIZE);
}

function libraryTile(row: ReturnType<typeof movieRow>): HomeTile {
  return {
    key: `${row.kind}/${row.id}`,
    kind: row.kind,
    id: row.id,
    title: row.title,
    year: row.year,
    hasPoster: row.hasPoster,
    mark: row.mark,
    dimmed: !row.monitored,
    href: row.href,
    when: null,
    name: tileName(row.title, row.year, null),
  };
}

const addedTime = (text: string): number => {
  const time = Date.parse(text);
  return Number.isNaN(time) ? 0 : time;
};

/**
 * Movies and series together, newest added first, at most [`ROW_SIZE`] (AC-01); on equal
 * times movies come first.
 */
export function newestTiles(
  movies: readonly MovieListItem[],
  series: readonly SeriesListItem[],
): HomeTile[] {
  const all = [
    ...movies.map((m) => ({ at: addedTime(m.added_at), tile: libraryTile(movieRow(m)) })),
    ...series.map((s) => ({ at: addedTime(s.added_at), tile: libraryTile(seriesRow(s)) })),
  ];
  return all
    .toSorted((a, b) => b.at - a.at)
    .slice(0, ROW_SIZE)
    .map((a) => a.tile);
}

/** The list API query of one row: newest added first, one page of [`ROW_SIZE`]. */
export function rowQuery(missing: boolean): string {
  const params = new URLSearchParams();
  if (missing) params.set('missing', 'yes');
  params.set('sort', 'added');
  params.set('order', 'desc');
  params.set('page', '1');
  params.set('page_size', String(ROW_SIZE));
  return `?${params.toString()}`;
}
