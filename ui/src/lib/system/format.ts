// German texts for the system page (spec web-ui-shell AC-12).

import type { ConnectionAnswer } from '../api/generated/ConnectionAnswer';
import type { ContactState } from '../api/generated/ContactState';
import type { JobResult } from '../api/generated/JobResult';
import type { Mode } from '../api/generated/Mode';

/** The tone of a status label: Primer's semantic colors. */
export type Tone = 'success' | 'danger' | 'neutral';

export type StatusText = { readonly text: string; readonly tone: Tone };

const MINUTES_PER_HOUR = 60;
const MINUTES_PER_DAY = 24 * MINUTES_PER_HOUR;

/** A job's interval: "aus" when switched off. */
export function intervalText(minutes: number | null): string {
  if (minutes === null) return 'aus';
  if (minutes === 1) return 'jede Minute';
  if (minutes === MINUTES_PER_HOUR) return 'stündlich';
  if (minutes === MINUTES_PER_DAY) return 'täglich';
  if (minutes % MINUTES_PER_DAY === 0) return `alle ${minutes / MINUTES_PER_DAY} Tage`;
  if (minutes % MINUTES_PER_HOUR === 0) return `alle ${minutes / MINUTES_PER_HOUR} Stunden`;
  return `alle ${minutes} Minuten`;
}

/** A point in time as German date and time, in the browser's (or the given) time zone. */
export function dateTimeText(iso: string, timeZone?: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return new Intl.DateTimeFormat('de-DE', {
    dateStyle: 'medium',
    timeStyle: 'medium',
    ...(timeZone === undefined ? {} : { timeZone }),
  }).format(date);
}

export function lastRunText(iso: string | null, timeZone?: string): string {
  return iso === null ? 'noch nicht gelaufen' : dateTimeText(iso, timeZone);
}

export function nextRunText(
  iso: string | null,
  interval: number | null,
  timeZone?: string,
): string {
  if (interval === null) return 'aus';
  return iso === null ? 'noch nicht geplant' : dateTimeText(iso, timeZone);
}

export function resultText(result: JobResult | null): StatusText {
  if (result === null) return { text: 'noch kein Ergebnis', tone: 'neutral' };
  return result.state === 'ok'
    ? { text: 'OK', tone: 'success' }
    : { text: 'Fehler', tone: 'danger' };
}

export function modeText(mode: Mode): { readonly name: string; readonly hint: string } {
  return mode === 'live'
    ? { name: 'Live', hint: 'sofarr lädt herunter, importiert und benennt um.' }
    : { name: 'Shadow', hint: 'sofarr protokolliert nur, was es tun würde.' };
}

export function contactText(state: ContactState): StatusText {
  switch (state) {
    case 'ok':
      return { text: 'erreichbar', tone: 'success' };
    case 'error':
      return { text: 'Fehler', tone: 'danger' };
    case 'no_contact':
      return { text: 'noch kein Kontakt', tone: 'neutral' };
    case 'not_configured':
      return { text: 'nicht konfiguriert', tone: 'neutral' };
  }
}

/** The heading of a connection, e.g. "Download-Client nzbget" or "Indexer". */
export function connectionTitle(connection: ConnectionAnswer): string {
  const kind = connection.kind === 'download_client' ? 'Download-Client' : 'Indexer';
  return connection.name === null ? kind : `${kind} ${connection.name}`;
}
