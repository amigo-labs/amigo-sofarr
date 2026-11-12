// German texts of the library pages (spec web-ui-library).

import type { BulkSearchReport } from '../api/generated/BulkSearchReport';
import type { EntryState } from '../api/generated/EntryState';
import type { ReleaseStage } from '../api/generated/ReleaseStage';
import type { Release } from '../api/generated/Release';
import type { RemoveAnswer } from '../api/generated/RemoveAnswer';
import type { SearchReport } from '../api/generated/SearchReport';
import type { Tone } from '../system/format';

const UNITS = ['Bytes', 'KB', 'MB', 'GB', 'TB'] as const;

/** A size in bytes as German text with binary units, e.g. "4,2 GB". */
export function bytesText(bytes: number | null): string {
  if (bytes === null || !Number.isFinite(bytes) || bytes < 0) return '–';
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 ? 0 : value < 10 ? 1 : 0;
  const number = new Intl.NumberFormat('de-DE', {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(value);
  return `${number} ${UNITS[unit]}`;
}

/** A remaining time, e.g. "1 Std. 5 Min." or "45 Sek."; "–" when unknown. */
export function durationText(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds) || seconds < 0) return '–';
  const total = Math.round(seconds);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  if (hours > 0) return `${hours} Std. ${minutes} Min.`;
  if (minutes > 0) return `${minutes} Min.`;
  return `${total} Sek.`;
}

/** Progress 0..1 as percent text; "–" when unknown. */
export function percentText(progress: number | null): string {
  if (progress === null || !Number.isFinite(progress)) return '–';
  return `${Math.round(Math.min(1, Math.max(0, progress)) * 100)} %`;
}

/** A `YYYY-MM-DD` date as German text, e.g. "12.11.2026". */
export function dateText(date: string | null): string {
  if (date === null) return '–';
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(date);
  return match ? `${match[3]}.${match[2]}.${match[1]}` : date;
}

const GRAB_STATUS: Readonly<Record<string, string>> = {
  shadow: 'nur protokolliert (Shadow)',
  queued: 'wartet',
  downloading: 'lädt herunter',
  paused: 'pausiert',
  completed: 'heruntergeladen',
  imported: 'importiert',
  failed: 'fehlgeschlagen',
};

/** A grab's or download's state in German. */
export function statusText(status: string | null): string {
  if (status === null) return 'unbekannt';
  return GRAB_STATUS[status] ?? status;
}

export function statusTone(status: string): Tone {
  if (status === 'failed') return 'danger';
  if (status === 'imported') return 'success';
  return 'neutral';
}

export const STAGE_TEXT: Readonly<Record<ReleaseStage, string>> = {
  announced: 'angekündigt',
  in_cinemas: 'im Kino',
  released: 'veröffentlicht',
};

/** The minimum availability values of the API with their German names. */
export const AVAILABILITY_TEXT: Readonly<Record<string, string>> = {
  announced: 'angekündigt',
  inCinemas: 'im Kino',
  released: 'veröffentlicht',
};

export const availabilityText = (value: string): string => AVAILABILITY_TEXT[value] ?? value;

export const RELEASE_TEXT: Readonly<Record<Release, string>> = {
  cinema: 'Kino',
  digital: 'digital',
  physical: 'Disc',
};

export const ENTRY_STATE: Readonly<Record<EntryState, { text: string; tone: Tone }>> = {
  present: { text: 'vorhanden', tone: 'success' },
  upcoming: { text: 'noch nicht erschienen', tone: 'neutral' },
  missing: { text: 'fehlt', tone: 'danger' },
  neutral: { text: 'neutral', tone: 'neutral' },
};

const count = (n: number, one: string, many: string): string => `${n} ${n === 1 ? one : many}`;

function withErrors(text: string, errors: readonly string[], notes: readonly string[]): string {
  const parts = [text, ...notes];
  if (errors.length > 0) parts.push(`Fehler: ${errors.join('; ')}`);
  return parts.join(' ');
}

/** The German result of a search (AC-09). */
export function searchText(report: SearchReport): string {
  if (report.note !== undefined && report.releases === 0 && report.grabs === 0) {
    return withErrors(`Suche beendet: ${report.note}`, report.errors, []);
  }
  const text = `Suche beendet: ${count(report.releases, 'Release', 'Releases')} gefunden, ${count(
    report.grabs,
    'Grab',
    'Grabs',
  )}.`;
  return withErrors(text, report.errors, report.note === undefined ? [] : [report.note]);
}

/** The German result of a search for several items (AC-08, AC-09). */
export function bulkSearchText(report: BulkSearchReport): string {
  const text = `Suche beendet: ${count(report.searched, 'Suche', 'Suchen')}, ${count(
    report.releases,
    'Release',
    'Releases',
  )} gefunden, ${count(report.grabs, 'Grab', 'Grabs')}.`;
  return withErrors(text, report.errors, report.notes);
}

export function searchTone(errors: readonly string[]): 'danger' | 'info' {
  return errors.length > 0 ? 'danger' : 'info';
}

/** The German result of removing an item (AC-12). */
export function removeText(title: string, answer: RemoveAnswer): string {
  const parts = [`„${title}“ wurde aus der Bibliothek entfernt.`];
  if (answer.files_removed > 0) {
    const where = answer.recycle_bin ? 'in den Papierkorb verschoben' : 'gelöscht';
    parts.push(`${count(answer.files_removed, 'Datei', 'Dateien')} ${where}.`);
  }
  if (answer.downloads_removed > 0) {
    parts.push(
      `${count(answer.downloads_removed, 'Download', 'Downloads')} beim Download-Client entfernt.`,
    );
  }
  return parts.join(' ');
}

/** "1 Film", "3 Serien", "5 Episoden" … */
export function itemCount(
  n: number,
  kind: 'movie' | 'series' | 'episode' | 'file' | 'item',
): string {
  switch (kind) {
    case 'movie':
      return count(n, 'Film', 'Filme');
    case 'series':
      return count(n, 'Serie', 'Serien');
    case 'episode':
      return count(n, 'Episode', 'Episoden');
    case 'file':
      return count(n, 'Datei', 'Dateien');
    case 'item':
      return count(n, 'Eintrag', 'Einträge');
  }
}

/** Season and episode, e.g. "S01E02". */
export function episodeCode(season: number | null, episode: number | null): string {
  if (season === null || episode === null) return '';
  return `S${String(season).padStart(2, '0')}E${String(episode).padStart(2, '0')}`;
}
