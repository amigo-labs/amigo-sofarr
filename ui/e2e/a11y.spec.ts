// AC-11: no accessibility violations (WCAG 2.1 A/AA, including contrast) in both themes.

import { AxeBuilder } from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { SAMPLE_SYSTEM, expectSystemPage, json, logIn } from './helpers';
import { SERVERS } from './servers';

async function expectNoViolations(page: Page): Promise<void> {
  const result = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
    .analyze();
  const summary = result.violations.map(
    (v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`,
  );
  expect(summary).toEqual([]);
}

for (const scheme of ['light', 'dark'] as const) {
  test.describe(`${scheme} theme`, () => {
    test.beforeEach(async ({ page }) => {
      await page.emulateMedia({ colorScheme: scheme });
    });

    test('system page with every state', async ({ page }) => {
      await page.route('**/api/system', (route) => json(route, SAMPLE_SYSTEM));
      await page.goto(`${SERVERS.open}/system`);
      await expect(page.getByTestId('version')).toHaveText('9.8.7');
      await expectNoViolations(page);
    });

    test('system page with a failed refresh', async ({ page }) => {
      await page.clock.install();
      await page.goto(`${SERVERS.open}/system`);
      await expectSystemPage(page);
      await page.route('**/api/system', (route) =>
        json(route, { error: 'Datenbank gesperrt' }, 503),
      );
      await page.clock.runFor(15_000);
      await expect(page.getByRole('alert')).toContainText('Datenbank gesperrt');
      await expectNoViolations(page);
    });

    test('full-page error', async ({ page }) => {
      await page.route('**/api/system', (route) =>
        json(route, { error: 'Datenbank gesperrt' }, 503),
      );
      await page.goto(`${SERVERS.open}/system`);
      await expect(page.getByRole('alert')).toContainText('Datenbank gesperrt');
      await expectNoViolations(page);
    });

    test('login page with an error and the open menu on a narrow screen', async ({ page }) => {
      await page.goto(`${SERVERS.auth}/anmelden`);
      await page.route('**/api/auth/login', (route) =>
        json(route, { error: 'Benutzername oder Passwort falsch' }, 401),
      );
      await logIn(page, 'falsch');
      await expect(page.getByRole('alert')).toBeVisible();
      await expectNoViolations(page);
    });

    test('logged-in header and open menu', async ({ page }) => {
      await page.setViewportSize({ width: 360, height: 740 });
      await page.goto(`${SERVERS.auth}/system`);
      await logIn(page);
      await expectSystemPage(page);
      await page.getByRole('button', { name: 'Menü öffnen' }).click();
      await expectNoViolations(page);
    });

    test('page not found', async ({ page }) => {
      await page.goto(`${SERVERS.open}/filme`);
      await expect(page.getByRole('heading', { name: 'Seite nicht gefunden' })).toBeVisible();
      await expectNoViolations(page);
    });
  });
}
