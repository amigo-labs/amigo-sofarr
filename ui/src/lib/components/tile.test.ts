import { describe, expect, it } from 'vitest';
import { tileName } from './tile';

describe('poster tile name (ui-cinema-tiles AC-05)', () => {
  it('names the title, its year and its rating with the votes', () => {
    expect(tileName('Nordlicht über Tromsø', 2023, 'IMDb 7,6 (48.210 Stimmen)')).toBe(
      'Nordlicht über Tromsø, 2023, IMDb 7,6 (48.210 Stimmen)',
    );
    expect(tileName('Das Boot', 1981, 'IMDb –')).toBe('Das Boot, 1981, IMDb –');
  });

  it('leaves out a missing year and the rating while ratings are off', () => {
    expect(tileName('Das Boot', null, 'IMDb –')).toBe('Das Boot, IMDb –');
    expect(tileName('Das Boot', 1981, null)).toBe('Das Boot, 1981');
    expect(tileName('Das Boot', null, null)).toBe('Das Boot');
  });
});
