import { describe, expect, it } from 'vitest';
import type { MovieListItem } from '../api/generated/MovieListItem';
import type { SeriesListItem } from '../api/generated/SeriesListItem';
import { idOf } from './detail/detail.svelte';
import { initial, movieRow, seriesRow } from './rows';
import { readView, storeView, viewKey } from './view';

const movie: MovieListItem = {
  id: 7,
  tmdb_id: 1,
  title: 'Das Boot',
  title_de: null,
  original_title: 'Das Boot',
  year: 1981,
  monitored: true,
  quality: null,
  size: null,
  missing: true,
  cutoff_unmet: false,
  available: true,
  has_poster: false,
  quality_profile_id: 4,
  tags: [],
  added_at: '2026-11-12T00:00:00Z',
};

const series: SeriesListItem = {
  id: 3,
  tvdb_id: 1,
  title: 'Kleine Insel',
  title_de: 'Die kleine Insel',
  year: 2020,
  monitored: true,
  series_type: 'standard',
  episodes: 5,
  monitored_episodes: 3,
  files: 1,
  missing: 1,
  cutoff_unmet: 2,
  complete: false,
  size: 4_500_000_000,
  has_poster: true,
  quality_profile_id: 4,
  tags: [],
  added_at: '2026-11-12T00:00:00Z',
};

describe('list rows (AC-02, AC-06)', () => {
  it('a movie without file shows "fehlt" and its labels', () => {
    const row = movieRow(movie);
    expect(row.file).toBe('fehlt');
    expect(row.size).toBe('–');
    expect(row.subtitle).toBeNull();
    expect(row.labels).toEqual([{ text: 'fehlt', tone: 'danger' }]);
    expect(row.href).toBe('/filme/7');
    expect(
      movieRow({ ...movie, quality: 'Bluray-1080p', missing: false, cutoff_unmet: true }).file,
    ).toBe('Bluray-1080p');
  });

  it('a series shows its number of files and episode labels', () => {
    const row = seriesRow(series);
    expect(row.file).toBe('1 von 5 Dateien');
    expect(row.subtitle).toBe('Die kleine Insel');
    expect(row.labels.map((l) => l.text)).toEqual(['1 fehlen', '2 unter Cutoff']);
    expect(seriesRow({ ...series, missing: 0, cutoff_unmet: 0, complete: true }).labels).toEqual([
      { text: 'komplett', tone: 'success' },
    ]);
  });

  it('the placeholder shows the first letter or digit (AC-21)', () => {
    expect(initial('Das Boot')).toBe('D');
    expect(initial(' „über“ alles')).toBe('Ü');
    expect(initial('12 Monkeys')).toBe('1');
    expect(initial('…')).toBe('?');
  });
});

const blocked = (): Storage => {
  throw new DOMException('blockiert', 'SecurityError');
};

describe('remembered view (AC-02)', () => {
  it('reads and stores the view per list, the list by default', () => {
    const items = new Map<string, string>();
    const storage = () => ({
      getItem: (k: string) => items.get(k) ?? null,
      setItem: (k: string, v: string) => void items.set(k, v),
    });
    expect(readView(storage, viewKey('filme'))).toBe('liste');
    storeView(storage, viewKey('filme'), 'poster');
    expect(readView(storage, viewKey('filme'))).toBe('poster');
    expect(readView(storage, viewKey('serien'))).toBe('liste');
  });

  it('a browser that blocks storage shows the list and keeps working', () => {
    expect(readView(blocked, viewKey('filme'))).toBe('liste');
    expect(() => storeView(blocked, viewKey('filme'), 'poster')).not.toThrow();
  });
});

describe('page address ids', () => {
  it('accepts positive numbers only', () => {
    expect(idOf('12')).toBe(12);
    expect(idOf('0')).toBeNull();
    expect(idOf('12a')).toBeNull();
    expect(idOf(undefined)).toBeNull();
  });
});
