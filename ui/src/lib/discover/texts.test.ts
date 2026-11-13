import { describe, expect, it } from 'vitest';
import type { TitleCard } from '../api/generated/TitleCard';
import { fresher, newCards } from './pages.svelte';
import { batches } from './states.svelte';
import {
  departmentText,
  fetchedText,
  jobText,
  libraryLabel,
  ratingLine,
  runtimeText,
  seasonText,
  statusText,
} from './texts';

const card = (kind: 'movie' | 'series', id: number): TitleCard => ({
  kind,
  tmdb_id: id,
  title: 't',
  original_title: 't',
  year: null,
  poster: null,
  popularity: 0,
  library: { status: 'none', id: null },
  rating: { state: 'off', rating: null, votes: null },
});

describe('discover texts (discover AC-01, AC-13, AC-16)', () => {
  it('names the library state', () => {
    expect(libraryLabel('in_library')?.text).toBe('In der Bibliothek');
    expect(libraryLabel('excluded')?.text).toBe('Ausgeschlossen');
    expect(libraryLabel('none')).toBeNull();
  });

  it('shows ratings with a placeholder while loading and "–" without one', () => {
    expect(ratingLine({ state: 'rated', rating: 7.7, votes: 4242 })).toBe('7,7 (4.242 Stimmen)');
    expect(ratingLine({ state: 'pending', rating: null, votes: null })).toBe('…');
    expect(ratingLine({ state: 'none', rating: null, votes: null })).toBe('–');
    expect(ratingLine({ state: 'off', rating: null, votes: null })).toBeNull();
  });

  it('dates cached answers; a stale page wins', () => {
    const freshness = { fetched_at: '2026-11-12T09:00:00Z', stale: false, notice: null };
    expect(fetchedText(freshness, 'Europe/Berlin')).toBe('Stand: 12.11.2026, 10:00:00');
    const stale = { ...freshness, stale: true, notice: 'alt' };
    expect(fresher(freshness, stale)).toBe(stale);
    expect(fresher(stale, freshness)).toBe(stale);
    expect(fresher(null, freshness)).toBe(freshness);
  });

  it('speaks German for TMDB terms it knows and keeps the others', () => {
    expect(departmentText('Directing')).toBe('Regie');
    expect(departmentText('Unbekannt')).toBe('Unbekannt');
    expect(jobText('Writer, Story')).toBe('Autor, Geschichte');
    expect(statusText('Returning Series')).toBe('Läuft');
    expect(runtimeText(105)).toBe('1 Std. 45 Min.');
    expect(runtimeText(45)).toBe('45 Min.');
    expect(seasonText('', 0, 1)).toBe('Specials (1 Folge)');
    expect(seasonText('Staffel 2', 2, 8)).toBe('Staffel 2 (8 Folgen)');
  });

  it('shows a title once and asks for states in batches of 500', () => {
    const known = [card('movie', 1)];
    expect(newCards(known, [card('movie', 1), card('series', 1), card('series', 1)])).toEqual([
      card('series', 1),
    ]);
    const refs = Array.from({ length: 1001 }, (_, i) => ({ kind: 'movie' as const, tmdb_id: i }));
    expect(batches(refs).map((b) => b.length)).toEqual([500, 500, 1]);
  });
});
