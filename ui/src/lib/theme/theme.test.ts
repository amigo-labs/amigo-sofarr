// Spec ui-cinema-shell AC-02 (revising amigo-theme AC-04 .. AC-08): the palette is always
// dark; a choice an older sofarr stored is neither read nor written.

import { describe, expect, it } from 'vitest';
import { sources, uiFile } from './sources.test-helper.mjs';
import { PALETTE, THEME_KEY, applyPalette } from './theme';

/** The part of the document element `applyPalette` touches. */
function root(initial: string | null): Pick<HTMLElement, 'setAttribute'> & {
  theme: string | null;
} {
  return {
    theme: initial,
    setAttribute(name: string, value: string) {
      if (name === 'data-theme') this.theme = value;
    },
  };
}

describe('the palette', () => {
  it('is dark', () => {
    expect(PALETTE).toBe('dark');
  });

  it('puts the document in the dark palette, whatever it carried before', () => {
    for (const before of ['light', 'dark', null]) {
      const element = root(before);
      applyPalette(element);
      expect(element.theme).toBe('dark');
    }
  });

  it('is on the document before any script runs, and the browser draws dark controls', () => {
    const html = uiFile('src/app.html');
    expect(html).toMatch(/<html lang="de" data-theme="dark">/);
    expect(html).toContain('<meta name="color-scheme" content="dark" />');
  });

  it('never reads or writes a stored theme choice', () => {
    const readers = sources()
      .filter(({ text }) => text.includes(THEME_KEY) || text.includes('THEME_KEY'))
      .map(({ path }) => path);
    expect(readers).toEqual([]);
  });

  it('offers no theme switch', () => {
    const switches = sources()
      .filter(({ text }) => text.includes('ThemeSwitch') || text.includes('Farbschema'))
      .map(({ path }) => path);
    expect(switches).toEqual([]);
  });
});
