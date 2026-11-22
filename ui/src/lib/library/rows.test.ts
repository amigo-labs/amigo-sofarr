import { describe, expect, it } from 'vitest';
import type { MovieListItem } from '../api/generated/MovieListItem';
import type { SeriesListItem } from '../api/generated/SeriesListItem';
import { idOf } from './detail/detail.svelte';
import { missingText } from './format';
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
  has_backdrop: false,
  quality_profile_id: 4,
  tags: [],
  added_at: '2026-11-12T00:00:00Z',
  imdb_rating: null,
  imdb_votes: null,
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
  has_backdrop: false,
  quality_profile_id: 4,
  tags: [],
  added_at: '2026-11-12T00:00:00Z',
  imdb_rating: null,
  imdb_votes: null,
};

describe('list rows (AC-02, AC-06)', () => {
  it('a movie without file shows "fehlt" once, as its status chip (ui-list-items AC-01)', () => {
    const row = movieRow(movie);
    expect(row.size).toBe('–');
    expect(row.subtitle).toBeNull();
    expect(row.labels).toEqual([{ text: 'fehlt', tone: 'danger' }]);
    expect(row.href).toBe('/filme/7');
    const below = movieRow({
      ...movie,
      quality: 'Bluray-1080p',
      missing: false,
      cutoff_unmet: true,
    });
    expect(below.labels).toEqual([
      { text: 'Bluray-1080p', tone: 'accent' },
      { text: 'Cutoff nicht erreicht', tone: 'neutral' },
    ]);
    // Without a file but not missing (not yet available): still "fehlt".
    expect(movieRow({ ...movie, missing: false }).labels).toEqual([
      { text: 'fehlt', tone: 'danger' },
    ]);
  });

  it('a series shows its number of files and episode labels', () => {
    const row = seriesRow(series);
    expect(row.subtitle).toBe('Die kleine Insel');
    expect(row.labels).toEqual([
      { text: '1 von 5 Dateien', tone: 'neutral' },
      { text: '1 fehlt', tone: 'danger' },
      { text: '2 unter Cutoff', tone: 'neutral' },
    ]);
    expect(
      seriesRow({ ...series, missing: 0, cutoff_unmet: 0, complete: true }).labels.slice(1),
    ).toEqual([{ text: 'komplett', tone: 'success' }]);
    expect(seriesRow({ ...series, episodes: 0, files: 0 }).labels[0]?.text).toBe('0 von 0 Dateien');
  });

  it('the missing count reads "1 fehlt" but "3 fehlen"', () => {
    expect(missingText(1)).toBe('1 fehlt');
    expect(missingText(3)).toBe('3 fehlen');
    expect(seriesRow({ ...series, missing: 3 }).labels[1]?.text).toBe('3 fehlen');
  });

  it('rows carry the compact rating and the full text, "–" without (imdb-ratings AC-04)', () => {
    expect(movieRow(movie).rating).toBe('–');
    expect(movieRow(movie).ratingFull).toBe('IMDb –');
    const rated = movieRow({ ...movie, imdb_rating: 8.4, imdb_votes: 271_000 });
    expect(rated.rating).toBe('★ 8,4');
    expect(rated.ratingFull).toBe('IMDb 8,4 (271.000 Stimmen)');
    expect(seriesRow({ ...series, imdb_rating: 7, imdb_votes: 1 }).ratingFull).toBe(
      'IMDb 7,0 (1 Stimme)',
    );
    expect(seriesRow(series).rating).toBe('–');
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
