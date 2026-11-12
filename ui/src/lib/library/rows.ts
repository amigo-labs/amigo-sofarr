// One row of the movie or series list, the same for list and poster grid (AC-02, AC-06).

import type { MediaKind } from '../api/generated/MediaKind';
import type { MovieListItem } from '../api/generated/MovieListItem';
import type { SeriesListItem } from '../api/generated/SeriesListItem';
import type { Tone } from '../system/format';
import { bytesText } from './format';

export type RowLabel = { readonly text: string; readonly tone: Tone };

export type Row = {
  readonly kind: MediaKind;
  readonly id: number;
  readonly title: string;
  /** A second title shown smaller (German or original title), if it differs. */
  readonly subtitle: string | null;
  readonly year: number | null;
  /** The file's quality, "fehlt", or the number of files of a series. */
  readonly file: string;
  readonly size: string;
  readonly monitored: boolean;
  readonly hasPoster: boolean;
  readonly labels: readonly RowLabel[];
  readonly href: string;
};

const other = (title: string, ...candidates: (string | null)[]): string | null =>
  candidates.find((c) => c !== null && c.trim() !== '' && c !== title) ?? null;

export function movieRow(item: MovieListItem): Row {
  const labels: RowLabel[] = [];
  if (item.missing) labels.push({ text: 'fehlt', tone: 'danger' });
  if (item.cutoff_unmet) labels.push({ text: 'Cutoff nicht erreicht', tone: 'neutral' });
  return {
    kind: 'movie',
    id: item.id,
    title: item.title,
    subtitle: other(item.title, item.title_de, item.original_title),
    year: item.year,
    file: item.quality ?? 'fehlt',
    size: item.size === null ? '–' : bytesText(item.size),
    monitored: item.monitored,
    hasPoster: item.has_poster,
    labels,
    href: `/filme/${item.id}`,
  };
}

export function seriesRow(item: SeriesListItem): Row {
  const labels: RowLabel[] = [];
  if (item.missing > 0) labels.push({ text: `${item.missing} fehlen`, tone: 'danger' });
  if (item.cutoff_unmet > 0) {
    labels.push({ text: `${item.cutoff_unmet} unter Cutoff`, tone: 'neutral' });
  }
  if (item.complete) labels.push({ text: 'komplett', tone: 'success' });
  return {
    kind: 'series',
    id: item.id,
    title: item.title,
    subtitle: other(item.title, item.title_de),
    year: item.year,
    file: `${item.files} von ${item.episodes} ${item.episodes === 1 ? 'Datei' : 'Dateien'}`,
    size: bytesText(item.size),
    monitored: item.monitored,
    hasPoster: item.has_poster,
    labels,
    href: `/serien/${item.id}`,
  };
}

/** The first letter of a title for the poster placeholder (AC-21). */
export function initial(title: string): string {
  const letter = [...title.trim()].find((c) => /[\p{L}\p{N}]/u.test(c));
  return (letter ?? '?').toLocaleUpperCase('de-DE');
}
