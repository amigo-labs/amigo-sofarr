import { describe, expect, it } from 'vitest';
import { jobText } from './reencode';

describe('re-encode job text (reencode-job AC-08)', () => {
  it('says whether the job runs, what it encodes, or why not', () => {
    expect(jobText({ active: true, message: null, running: null })).toBe('aktiv');
    expect(jobText({ active: true, message: null, running: 'Heat (1995)' })).toBe(
      'aktiv – encodiert Heat (1995)',
    );
    expect(
      jobText({ active: false, message: 'in der Konfiguration ausgeschaltet', running: null }),
    ).toBe('aus – in der Konfiguration ausgeschaltet');
    expect(jobText({ active: false, message: null, running: null })).toBe('aus – Grund unbekannt');
  });
});
