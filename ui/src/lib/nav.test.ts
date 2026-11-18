import { describe, expect, it } from 'vitest';
import { NAV_GROUPS, PAGES, isCurrent, navGroups, navPages } from './nav';

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
      'Neu encodieren',
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
      'Entdecken',
      'Blocklist',
      'Untertitel',
      'Neu encodieren',
      'Backups',
      'System',
    ]);
    const built = PAGES.map((p) => ({ ...p, built: p.href !== '/filme' }));
    expect(navPages(built).map((p) => p.label)).toEqual([
      'Übersicht',
      'Serien',
      'Kalender',
      'Fehlend',
      'Entdecken',
      'Blocklist',
      'Untertitel',
      'Neu encodieren',
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

  it('groups the pages without changing their order (ui-shell-redesign AC-01)', () => {
    expect(NAV_GROUPS).toEqual(['Bibliothek', 'Entdecken', 'Verwaltung']);
    expect(navGroups().map((g) => [g.label, g.pages.map((p) => p.label)])).toEqual([
      ['Bibliothek', ['Übersicht', 'Filme', 'Serien', 'Kalender', 'Fehlend']],
      ['Entdecken', ['Entdecken']],
      ['Verwaltung', ['Blocklist', 'Untertitel', 'Neu encodieren', 'Backups', 'System']],
    ]);
  });

  it('gives every page an icon (ui-shell-redesign AC-02)', () => {
    expect(PAGES.filter((page) => !page.icon.startsWith('<svg')).map((page) => page.label)).toEqual(
      [],
    );
  });
});
