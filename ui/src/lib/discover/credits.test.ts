import { describe, expect, it } from 'vitest';
import type { CreditCard } from '../api/generated/CreditCard';
import type { LibraryStatus } from '../api/generated/LibraryStatus';
import type { Rating } from '../api/generated/Rating';
import type { TitleKind } from '../api/generated/TitleKind';
import { DEFAULT_CHOICES, arrange, choiceParams, parseChoices } from './credits';

function credit(
  id: number,
  date: string | null,
  rating: Rating,
  status: LibraryStatus = 'none',
  kind: TitleKind = 'movie',
): CreditCard {
  return {
    title: {
      kind,
      tmdb_id: id,
      title: `Titel ${id}`,
      original_title: `Titel ${id}`,
      year: date === null ? null : Number(date.slice(0, 4)),
      poster: null,
      popularity: 1,
      library: { status, id: status === 'in_library' ? id : null },
      rating,
    },
    date,
    character: null,
    job: null,
  };
}

const rated = (rating: number, votes: number): Rating => ({ state: 'rated', rating, votes });
const pending: Rating = { state: 'pending', rating: null, votes: null };
const none: Rating = { state: 'none', rating: null, votes: null };
const stateOf = (c: CreditCard) => ({ library: c.title.library, rating: c.title.rating });
const ids = (list: readonly CreditCard[]) => list.map((c) => c.title.tmdb_id);
/** Title 3's rating became known meanwhile. */
const later = (c: CreditCard) =>
  c.title.tmdb_id === 3 ? { library: c.title.library, rating: rated(9.5, 1) } : stateOf(c);

const CREDITS = [
  credit(1, '2001-05-01', rated(6, 100)),
  credit(2, null, rated(9, 5)),
  credit(3, '2020-01-01', pending, 'in_library'),
  credit(4, '1999-12-31', rated(8, 50), 'excluded', 'series'),
  credit(5, '2010-07-07', rated(8, 900)),
  credit(6, '2015-01-01', none, 'none', 'series'),
];

describe('person page credits (discover AC-05)', () => {
  it('sorts by year, newest or oldest first, undated last', () => {
    expect(ids(arrange(CREDITS, DEFAULT_CHOICES, stateOf))).toEqual([3, 6, 5, 1, 4, 2]);
    expect(ids(arrange(CREDITS, { ...DEFAULT_CHOICES, sort: 'aelteste' }, stateOf))).toEqual([
      4, 1, 5, 6, 3, 2,
    ]);
  });

  it('sorts by rating with votes breaking ties; unknown ratings last', () => {
    expect(ids(arrange(CREDITS, { ...DEFAULT_CHOICES, sort: 'bewertung' }, stateOf))).toEqual([
      2, 5, 4, 1, 3, 6,
    ]);
  });

  it('uses the newest state of each title', () => {
    expect(ids(arrange(CREDITS, { ...DEFAULT_CHOICES, sort: 'bewertung' }, later))[0]).toBe(3);
  });

  it('filters to missing titles and to movies or series', () => {
    const missing = { ...DEFAULT_CHOICES, missingOnly: true };
    expect(ids(arrange(CREDITS, missing, stateOf))).toEqual([6, 5, 1, 2]);
    expect(ids(arrange(CREDITS, { ...missing, kind: 'series' }, stateOf))).toEqual([6]);
    expect(ids(arrange(CREDITS, { ...DEFAULT_CHOICES, kind: 'series' }, stateOf))).toEqual([6, 4]);
  });

  it('keeps the choices in the page address; rating sort only while ratings are on', () => {
    const choices = { sort: 'bewertung' as const, missingOnly: true, kind: 'movie' as const };
    const text = choiceParams(choices).toString();
    expect(text).toBe('sortierung=bewertung&nur=fehlende&art=filme');
    expect(parseChoices(new URLSearchParams(text), true)).toEqual(choices);
    expect(parseChoices(new URLSearchParams(text), false).sort).toBe('neueste');
    expect(choiceParams(DEFAULT_CHOICES).toString()).toBe('');
  });
});
