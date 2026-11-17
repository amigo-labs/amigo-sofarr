import { describe, expect, it } from 'vitest';
import favicon from '../../static/favicon.svg?raw';
import monogram from '../../../assets/brand/monogram-small.svg?raw';

// The UI's monogram (sidebar, top bar, tab icon) is a copy of the one the website generates
// into `assets/brand/` (D29), because the Docker build of the UI only sees `ui/`
// (ui-consistency AC-13). This keeps the copy from drifting.
describe('monogram (ui-consistency AC-13)', () => {
  it('ui/static/favicon.svg is assets/brand/monogram-small.svg byte for byte', () => {
    expect(favicon).toBe(monogram);
    expect(favicon).toContain('<title>Sofarr</title>');
  });
});
