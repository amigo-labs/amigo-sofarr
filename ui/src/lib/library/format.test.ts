import { describe, expect, it } from 'vitest';
import {
  availabilityText,
  bulkSearchText,
  bytesText,
  dateText,
  durationText,
  episodeCode,
  itemCount,
  percentText,
  removeText,
  searchText,
  statusText,
} from './format';

describe('library texts', () => {
  it('writes sizes with German decimals', () => {
    expect(bytesText(0)).toBe('0 Bytes');
    expect(bytesText(1536)).toBe('1,5 KB');
    expect(bytesText(8_000_000_000)).toBe('7,5 GB');
    expect(bytesText(45_000_000_000)).toBe('42 GB');
    expect(bytesText(null)).toBe('–');
  });

  it('writes remaining time and progress, "–" when unknown (AC-01)', () => {
    expect(durationText(3900)).toBe('1 Std. 5 Min.');
    expect(durationText(125)).toBe('2 Min.');
    expect(durationText(42)).toBe('42 Sek.');
    expect(durationText(null)).toBe('–');
    expect(percentText(0.5)).toBe('50 %');
    expect(percentText(1.2)).toBe('100 %');
    expect(percentText(null)).toBe('–');
  });

  it('writes dates, states and availabilities in German', () => {
    expect(dateText('2026-11-12')).toBe('12.11.2026');
    expect(dateText(null)).toBe('–');
    expect(statusText('downloading')).toBe('lädt herunter');
    expect(statusText('failed')).toBe('fehlgeschlagen');
    expect(statusText(null)).toBe('unbekannt');
    expect(availabilityText('inCinemas')).toBe('im Kino');
    expect(episodeCode(1, 2)).toBe('S01E02');
  });

  it('counts in singular and plural', () => {
    expect(itemCount(1, 'movie')).toBe('1 Film');
    expect(itemCount(3, 'movie')).toBe('3 Filme');
    expect(itemCount(1, 'episode')).toBe('1 Episode');
    expect(itemCount(2, 'file')).toBe('2 Dateien');
    expect(itemCount(0, 'series')).toBe('0 Serien');
  });

  it('reports search results in German (AC-09)', () => {
    expect(searchText({ releases: 4, grabs: 1, errors: [] })).toBe(
      'Suche beendet: 4 Releases gefunden, 1 Grab.',
    );
    expect(
      searchText({ releases: 0, grabs: 0, errors: [], note: 'Kein Indexer konfiguriert' }),
    ).toBe('Suche beendet: Kein Indexer konfiguriert');
    expect(searchText({ releases: 1, grabs: 0, errors: ['A: Zeitüberschreitung'] })).toBe(
      'Suche beendet: 1 Release gefunden, 0 Grabs. Fehler: A: Zeitüberschreitung',
    );
    expect(
      bulkSearchText({ searched: 3, releases: 2, grabs: 2, errors: [], notes: ['2 übersprungen'] }),
    ).toBe('Suche beendet: 3 Suchen, 2 Releases gefunden, 2 Grabs. 2 übersprungen');
  });

  it('reports removing an item (AC-12)', () => {
    expect(removeText('Weg', { files_removed: 0, recycle_bin: false, downloads_removed: 0 })).toBe(
      '„Weg“ wurde aus der Bibliothek entfernt.',
    );
    expect(removeText('Weg', { files_removed: 2, recycle_bin: true, downloads_removed: 1 })).toBe(
      '„Weg“ wurde aus der Bibliothek entfernt. 2 Dateien in den Papierkorb verschoben. 1 Download beim Download-Client entfernt.',
    );
  });
});
