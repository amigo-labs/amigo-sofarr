// German texts of the discover pages (spec discover AC-01, AC-05, AC-13, AC-14, AC-16).

import type { Freshness } from '../api/generated/Freshness';
import type { LibraryStatus } from '../api/generated/LibraryStatus';
import type { Rating } from '../api/generated/Rating';
import type { TitleKind } from '../api/generated/TitleKind';
import { ratingWithVotesText } from '../imdb/imdb';
import { dateTimeText } from '../system/format';
import type { Tone } from '../system/format';

export const KIND_TEXT: Readonly<Record<TitleKind, string>> = { movie: 'Film', series: 'Serie' };

/** The label of a title's library state; `null` for a title that is neither. */
export function libraryLabel(status: LibraryStatus): { text: string; tone: Tone } | null {
  if (status === 'in_library') return { text: 'In der Bibliothek', tone: 'success' };
  if (status === 'excluded') return { text: 'Ausgeschlossen', tone: 'danger' };
  return null;
}

/** What the rating line shows; `null` while ratings are off. */
export function ratingLine(rating: Rating): string | null {
  switch (rating.state) {
    case 'off':
      return null;
    case 'pending':
      return '…';
    case 'none':
      return '–';
    case 'rated':
      return ratingWithVotesText(rating.rating, rating.votes);
  }
}

/** When an answer was fetched, e.g. "Stand: 12.11.2026, 10:00:00" (AC-13). */
export function fetchedText(freshness: Freshness, timeZone?: string): string {
  return `Stand: ${dateTimeText(freshness.fetched_at, timeZone)}`;
}

const DEPARTMENTS: Readonly<Record<string, string>> = {
  Acting: 'Schauspiel',
  Art: 'Ausstattung',
  Camera: 'Kamera',
  'Costume & Make-Up': 'Kostüm und Maske',
  Creator: 'Idee',
  Crew: 'Crew',
  Directing: 'Regie',
  Editing: 'Schnitt',
  Lighting: 'Licht',
  Production: 'Produktion',
  Sound: 'Ton',
  'Visual Effects': 'Visuelle Effekte',
  Writing: 'Drehbuch',
};

/** TMDB's department in German; an unknown one stays as TMDB names it. */
export const departmentText = (department: string): string => DEPARTMENTS[department] ?? department;

const JOBS: Readonly<Record<string, string>> = {
  Director: 'Regie',
  Creator: 'Idee',
  Screenplay: 'Drehbuch',
  Writer: 'Autor',
  Novel: 'Romanvorlage',
  Story: 'Geschichte',
  Producer: 'Produktion',
  'Executive Producer': 'Ausführende Produktion',
  'Original Music Composer': 'Musik',
  'Director of Photography': 'Kamera',
  Editor: 'Schnitt',
  Characters: 'Figuren',
  Teleplay: 'Drehbuch',
};

/** TMDB's job (or several, "Writer, Story") in German; unknown ones stay as they are. */
export const jobText = (job: string): string =>
  job
    .split(', ')
    .map((j) => JOBS[j] ?? j)
    .join(', ');

const STATUS: Readonly<Record<string, string>> = {
  Released: 'Erschienen',
  'In Production': 'In Produktion',
  'Post Production': 'In der Postproduktion',
  Planned: 'Geplant',
  Rumored: 'Angekündigt',
  Canceled: 'Abgesetzt',
  'Returning Series': 'Läuft',
  Ended: 'Beendet',
  Pilot: 'Pilotfolge',
};

/** TMDB's status of a title in German. */
export const statusText = (status: string): string => STATUS[status] ?? status;

/** A runtime in minutes, e.g. "1 Std. 45 Min.". */
export function runtimeText(minutes: number): string {
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (hours === 0) return `${rest} Min.`;
  return rest === 0 ? `${hours} Std.` : `${hours} Std. ${rest} Min.`;
}

/** A season's name and episode count, e.g. "Staffel 1 (10 Folgen)". */
export function seasonText(name: string, number: number, episodes: number): string {
  const label = name.trim() === '' ? (number === 0 ? 'Specials' : `Staffel ${number}`) : name;
  return `${label} (${episodes} ${episodes === 1 ? 'Folge' : 'Folgen'})`;
}
