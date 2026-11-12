// The discover pages under the shell's rules: axe (WCAG 2.1 A/AA) in light and dark (shell
// AC-11), German texts (shell AC-14, discover AC-16), at most 200 KB of JavaScript and CSS
// per directly opened page (shell AC-16), and no horizontal scrolling at 360 px.

import { AxeBuilder } from '@axe-core/playwright';
import { expect, test, type Browser, type Page } from '@playwright/test';
import { DISCOVER, cards, settle } from './discover-helpers';
import { expectNoHorizontalScroll } from './helpers';
import { LIBRARY } from './library-helpers';

type Target = { readonly name: string; readonly url: string };

const TARGETS: readonly Target[] = [
  { name: 'Entdecken', url: `${DISCOVER}/entdecken` },
  { name: 'Ergebnisliste', url: `${DISCOVER}/entdecken?art=filme&genre=18&bewertung=6` },
  { name: 'Regal', url: `${DISCOVER}/entdecken/regal/beliebte-serien` },
  { name: 'Suche', url: `${DISCOVER}/entdecken/suche?begriff=Stern` },
  { name: 'Film-Vorschau', url: `${DISCOVER}/entdecken/filme/502` },
  { name: 'Serien-Vorschau', url: `${DISCOVER}/entdecken/serien/605` },
  { name: 'Person', url: `${DISCOVER}/entdecken/personen/9000` },
  { name: 'Ohne TMDB', url: `${LIBRARY}/entdecken` },
];

async function expectNoViolations(page: Page): Promise<void> {
  const result = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
    .analyze();
  const summary = result.violations.map(
    (v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`,
  );
  expect(summary).toEqual([]);
}

async function open(page: Page, url: string): Promise<void> {
  await page.goto(url);
  await settle(page);
  await expect(
    page
      .locator(
        '[data-testid="title-card"], [data-testid="discover-unavailable"], [data-testid="credits"], [data-testid="preview"]',
      )
      .first(),
  ).toBeVisible();
}

for (const scheme of ['light', 'dark'] as const) {
  test.describe(`axe, ${scheme} theme`, () => {
    test.beforeEach(async ({ page }) => {
      await page.emulateMedia({ colorScheme: scheme });
    });

    for (const target of TARGETS) {
      test(target.name, async ({ page }) => {
        await open(page, target.url);
        await expectNoViolations(page);
      });
    }

    test('add dialogs and the import list dialog', async ({ page }) => {
      await open(page, `${DISCOVER}/entdecken/serien/605`);
      await page.getByRole('button', { name: 'Hinzufügen' }).click();
      await expect(page.getByRole('dialog').getByLabel('Qualitätsprofil')).toBeVisible();
      await expectNoViolations(page);
      await page.getByRole('dialog').getByRole('button', { name: 'Abbrechen' }).click();

      await open(page, `${DISCOVER}/entdecken`);
      await cards(page, 'movie/505')
        .first()
        .getByRole('button', { name: /Hinzufügen/ })
        .click();
      await expect(page.getByRole('dialog').getByLabel('Mindestverfügbarkeit')).toBeVisible();
      await expectNoViolations(page);
      await page.getByRole('dialog').getByRole('button', { name: 'Abbrechen' }).click();

      await open(page, `${DISCOVER}/entdecken/personen/9100`);
      await page.getByRole('button', { name: 'Import-Liste anlegen' }).click();
      await expect(page.getByRole('dialog').getByLabel('Qualitätsprofil')).toBeVisible();
      await expectNoViolations(page);
    });
  });
}

/** Encoded bytes of every script and style a fresh browser loads for `url`. */
async function scriptAndStyleBytes(browser: Browser, url: string): Promise<number> {
  const context = await browser.newContext();
  const page = await context.newPage();
  const sizes: Promise<number>[] = [];
  page.on('requestfinished', (request) => {
    if (['script', 'stylesheet'].includes(request.resourceType())) {
      sizes.push(request.sizes().then((s) => s.responseBodySize));
    }
  });
  await open(page, url);
  const total = (await Promise.all(sizes)).reduce((a, b) => a + b, 0);
  await context.close();
  return total;
}

test.describe('transfer budget (shell AC-16)', () => {
  for (const target of TARGETS) {
    test(`${target.name}: at most 200 KB of JavaScript and CSS (gzip)`, async ({ browser }) => {
      const bytes = await scriptAndStyleBytes(browser, target.url);
      test.info().annotations.push({ type: 'bytes', description: String(bytes) });
      expect(bytes).toBeGreaterThan(0);
      expect(bytes).toBeLessThanOrEqual(200 * 1024);
    });
  }
});

test.describe('360 px', () => {
  test.use({ viewport: { width: 360, height: 740 } });

  for (const target of TARGETS) {
    test(`${target.name} fits without horizontal scrolling`, async ({ page }) => {
      await open(page, target.url);
      await expectNoHorizontalScroll(page);
    });
  }
});

/** Words an English UI would show (as in layout.spec.ts). */
const ENGLISH =
  /\b(Loading|Error|Retry|Search|Monitored|Missing|Remove|Delete|Cancel|Save|Add|More|Cast|Crew|Seasons|Trending today|Popular|Upcoming|Now playing|Not found|Menu|Theme)\b/;

test.describe('German texts (shell AC-14, discover AC-16)', () => {
  for (const target of TARGETS) {
    test(`${target.name} speaks German`, async ({ page }) => {
      await open(page, target.url);
      const text = await page.evaluate(() => {
        const labels = [...document.querySelectorAll('[aria-label], [title]')].map(
          (el) => `${el.getAttribute('aria-label') ?? ''} ${el.getAttribute('title') ?? ''}`,
        );
        return [document.body.innerText, ...labels, document.title].join(' | ');
      });
      expect(text).not.toMatch(ENGLISH);
    });
  }
});
