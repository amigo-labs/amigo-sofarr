import { describe, expect, it } from 'vitest';
import { PAGES, isCurrent, navPages } from './nav';

describe('navigation', () => {
  it('keeps the order of the spec', () => {
    expect(PAGES.map((p) => p.label)).toEqual([
      'Übersicht',
      'Filme',
      'Serien',
      'Kalender',
      'Fehlend',
      'Entdecken',
      'Blocklist',
      'Untertitel',
      'Backups',
      'System',
    ]);
  });

  it('lists only built pages, in order', () => {
    expect(navPages().map((p) => p.label)).toEqual([
      'Übersicht',
      'Filme',
      'Serien',
      'Kalender',
      'Fehlend',
      'Blocklist',
      'Untertitel',
      'Backups',
      'System',
    ]);
    const built = PAGES.map((p) => ({ label: p.label, href: p.href, built: p.href !== '/filme' }));
    expect(navPages(built).map((p) => p.label)).toEqual([
      'Übersicht',
      'Serien',
      'Kalender',
      'Fehlend',
      'Entdecken',
      'Blocklist',
      'Untertitel',
      'Backups',
      'System',
    ]);
  });

  it('marks the current page', () => {
    expect(isCurrent('/system', '/system')).toBe(true);
    expect(isCurrent('/filme', '/filme/12')).toBe(true);
    expect(isCurrent('/', '/system')).toBe(false);
    expect(isCurrent('/system', '/systemx')).toBe(false);
  });
});
