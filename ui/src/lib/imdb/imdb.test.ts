import { describe, expect, it } from 'vitest';
import type { ImdbInfo } from '../api/generated/ImdbInfo';
import { dataDateText, ratingText, ratingWithVotesText, votesText } from './imdb';

const info: ImdbInfo = {
  enabled: true,
  loaded_at: '2026-11-12T03:00:00Z',
  titles: 1_500_000,
  attribution: 'Information courtesy of IMDb (https://www.imdb.com). Used with permission.',
  link: 'https://www.imdb.com',
};

describe('IMDb texts (imdb-ratings AC-04, AC-08)', () => {
  it('shows rating and votes in German', () => {
    expect(ratingText(7.7)).toBe('7,7');
    expect(ratingText(8)).toBe('8,0');
    expect(votesText(4242)).toBe('4.242 Stimmen');
    expect(votesText(1)).toBe('1 Stimme');
    expect(ratingWithVotesText(7.7, 4242)).toBe('7,7 (4.242 Stimmen)');
  });

  it('shows "–" without a rating', () => {
    expect(ratingText(null)).toBe('–');
    expect(votesText(null)).toBe('–');
    expect(ratingWithVotesText(null, 12)).toBe('–');
    expect(ratingWithVotesText(6.1, null)).toBe('6,1');
  });

  it('dates the ratings data, and says nothing before the first load', () => {
    expect(dataDateText(info, 'Europe/Berlin')).toBe('Stand der Bewertungen: 12.11.2026, 04:00:00');
    expect(dataDateText({ ...info, loaded_at: null })).toBeNull();
  });
});
