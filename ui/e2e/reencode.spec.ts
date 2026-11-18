// spec reencode-job AC-08 in the browser: the system page says whether the re-encode job
// runs, what it encodes, or why not. The test servers run without `[reencode] enabled`;
// a running job comes as a simulated API answer (the server side is covered by the Rust
// tests of spec reencode-job).

import { expect, test } from '@playwright/test';
import { SAMPLE_SYSTEM, expectSystemPage, json } from './helpers';
import { LIBRARY } from './library-helpers';

test.describe('system page (reencode-job AC-08)', () => {
  test('says whether files are re-encoded, which one, or why not', async ({ page }) => {
    await page.goto(`${LIBRARY}/system`);
    await expectSystemPage(page);
    await expect(page.getByTestId('reencode')).toHaveText(
      'aus – in der Konfiguration ausgeschaltet',
    );
    await expect(page.locator('[data-job="reencode"]')).toContainText('Neu encodieren');
    await page.route('**/api/system', (route) =>
      json(route, {
        ...SAMPLE_SYSTEM,
        reencode: { active: true, message: null, running: 'Heat (1995)' },
      }),
    );
    await page.reload();
    await expectSystemPage(page);
    await expect(page.getByTestId('reencode')).toHaveText('aktiv – encodiert Heat (1995)');
  });
});
