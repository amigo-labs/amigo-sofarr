import { describe, expect, it } from 'vitest';
import {
  connectionTitle,
  contactText,
  dateTimeText,
  intervalText,
  lastRunText,
  modeText,
  nextRunText,
  resultText,
} from './format';

const TZ = 'Europe/Berlin';

describe('job texts', () => {
  it('says "aus" for a switched-off job', () => {
    expect(intervalText(null)).toBe('aus');
    expect(nextRunText(null, null, TZ)).toBe('aus');
  });

  it('writes intervals in German', () => {
    expect(intervalText(1)).toBe('jede Minute');
    expect(intervalText(2)).toBe('alle 2 Minuten');
    expect(intervalText(30)).toBe('alle 30 Minuten');
    expect(intervalText(60)).toBe('stündlich');
    expect(intervalText(90)).toBe('alle 90 Minuten');
    expect(intervalText(360)).toBe('alle 6 Stunden');
    expect(intervalText(1440)).toBe('täglich');
    expect(intervalText(10080)).toBe('alle 7 Tage');
  });

  it('says "noch nicht gelaufen" before the first run', () => {
    expect(lastRunText(null, TZ)).toBe('noch nicht gelaufen');
  });

  it('writes points in time as German date and time', () => {
    expect(dateTimeText('2026-11-12T13:24:04.5741636Z', TZ)).toBe('12.11.2026, 14:24:04');
    expect(lastRunText('2026-07-01T08:00:00Z', TZ)).toBe('01.07.2026, 10:00:00');
    expect(nextRunText('2026-11-13T13:24:04Z', 1440, TZ)).toBe('13.11.2026, 14:24:04');
    expect(nextRunText(null, 30, TZ)).toBe('noch nicht geplant');
    expect(dateTimeText('kein Datum', TZ)).toBe('kein Datum');
  });

  it('labels results', () => {
    expect(resultText({ state: 'ok' })).toEqual({ text: 'OK', tone: 'success' });
    expect(resultText({ state: 'error', message: 'Indexer antwortet nicht' })).toEqual({
      text: 'Fehler',
      tone: 'danger',
    });
    expect(resultText(null)).toEqual({ text: 'noch kein Ergebnis', tone: 'neutral' });
  });

  it('names the mode', () => {
    expect(modeText('shadow').name).toBe('Shadow');
    expect(modeText('live').name).toBe('Live');
  });
});

describe('connection texts', () => {
  it('labels each contact state in German', () => {
    expect(contactText('ok')).toEqual({ text: 'erreichbar', tone: 'success' });
    expect(contactText('error')).toEqual({ text: 'Fehler', tone: 'danger' });
    expect(contactText('no_contact')).toEqual({ text: 'noch kein Kontakt', tone: 'neutral' });
    expect(contactText('not_configured')).toEqual({ text: 'nicht konfiguriert', tone: 'neutral' });
  });

  it('titles download client and indexers', () => {
    const base = { state: 'ok', at: null, message: null } as const;
    expect(connectionTitle({ ...base, kind: 'download_client', name: 'nzbget' })).toBe(
      'Download-Client nzbget',
    );
    expect(connectionTitle({ ...base, kind: 'indexer', name: 'NZBgeek' })).toBe('Indexer NZBgeek');
    expect(connectionTitle({ ...base, kind: 'indexer', name: null })).toBe('Indexer');
  });
});
