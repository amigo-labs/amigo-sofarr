// The library pages under the shell's rules: axe (WCAG 2.1 A/AA) in light and dark (shell
// AC-11), German texts (shell AC-14), at most 200 KB of JavaScript and CSS per directly
// opened page (shell AC-16), and no horizontal scrolling at 360 px (web-ui-library AC-22).

import { AxeBuilder } from '@axe-core/playwright';
import { expect, test, type Browser, type Page } from '@playwright/test';
import { expectNoHorizontalScroll, json } from './helpers';
import { LIBRARY, movieId, seriesId } from './library-helpers';

type Target = { readonly name: string; readonly path: (page: Page) => Promise<string> };

const fixed = (path: string) => async (): Promise<string> => path;

const TARGETS: readonly Target[] = [
  { name: 'Übersicht', path: fixed('/') },
  { name: 'Filme', path: fixed('/filme') },
  {
    name: 'Film',
    path: async (page) => `/filme/${await movieId(page.request, LIBRARY, 'Das Boot')}`,
  },
  { name: 'Serien', path: fixed('/serien') },
  {
    name: 'Serie',
    path: async (page) => `/serien/${await seriesId(page.request, LIBRARY, 'Kleine Insel')}`,
  },
  { name: 'Kalender', path: fixed('/kalender') },
  { name: 'Fehlend', path: fixed('/fehlend') },
  { name: 'Blocklist', path: fixed('/blocklist') },
  { name: 'Backups', path: fixed('/backups') },
];

/** Gives the blocklist entries to scan (none can be seeded). */
async function withBlocklist(page: Page): Promise<void> {
  await page.route('**/api/blocklist?*', (route) =>
    json(route, {
      page: 1,
      page_size: 50,
      total: 1,
      entries: [
        {
          id: 1,
          movie_id: 1,
          series_id: null,
          media_title: 'Film 0001',
          title: 'Film.0001.1951.1080p-GRP',
          indexer: 'Testindexer',
          reason: 'Download fehlgeschlagen',
          blocklisted_at: '2026-11-10T08:00:00Z',
        },
      ],
    }),
  );
}

async function settle(page: Page): Promise<void> {
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
  await expect(page.getByText(/wird geladen|werden geladen/)).toHaveCount(0);
}

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
  test.describe(`axe, ${scheme} theme`, () => {
    test.beforeEach(async ({ page }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await withBlocklist(page);
    });

    for (const target of TARGETS) {
      test(target.name, async ({ page }) => {
        await page.goto(`${LIBRARY}${await target.path(page)}`);
        await settle(page);
        await expectNoViolations(page);
      });
    }

    test('poster grid with a selection and the bulk dialog', async ({ page }) => {
      await page.goto(`${LIBRARY}/filme?suche=Film%20000`);
      await page.getByRole('button', { name: 'Poster' }).click();
      await page.getByLabel('„Film 0001“ auswählen').check();
      await expectNoViolations(page);
      await page
        .getByRole('region', { name: 'Auswahl' })
        .getByRole('button', { name: 'Überwachen', exact: true })
        .click();
      await expect(page.getByRole('dialog')).toBeVisible();
      await expectNoViolations(page);
    });

    test('remove dialog and settings preview', async ({ page }) => {
      await page.goto(`${LIBRARY}/filme/${await movieId(page.request, LIBRARY, 'Bald verfügbar')}`);
      await page.getByLabel('Mindestverfügbarkeit').selectOption({ label: 'angekündigt' });
      await expect(page.getByTestId('change-preview')).toBeVisible();
      await expectNoViolations(page);
      await page.getByRole('button', { name: 'Entfernen …' }).click();
      await expect(page.getByRole('dialog')).toBeVisible();
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
  await page.goto(url, { waitUntil: 'networkidle' });
  await settle(page);
  const total = (await Promise.all(sizes)).reduce((a, b) => a + b, 0);
  await context.close();
  return total;
}

test.describe('transfer budget (shell AC-16)', () => {
  for (const target of TARGETS) {
    test(`${target.name}: at most 200 KB of JavaScript and CSS (gzip)`, async ({
      browser,
      page,
    }) => {
      const bytes = await scriptAndStyleBytes(browser, `${LIBRARY}${await target.path(page)}`);
      test.info().annotations.push({ type: 'bytes', description: String(bytes) });
      expect(bytes).toBeGreaterThan(0);
      expect(bytes).toBeLessThanOrEqual(200 * 1024);
    });
  }
});

test.describe('360 px (AC-22)', () => {
  test.use({ viewport: { width: 360, height: 740 } });

  for (const target of TARGETS) {
    test(`${target.name} fits without horizontal scrolling`, async ({ page }) => {
      await withBlocklist(page);
      await page.goto(`${LIBRARY}${await target.path(page)}`);
      await settle(page);
      await expectNoHorizontalScroll(page);
    });
  }
});

/** Words an English UI would show (as in layout.spec.ts). */
const ENGLISH =
  /\b(Loading|Error|Retry|Search|Monitored|Missing|Remove|Delete|Cancel|Save|Rename|Settings|Not found|Menu|Theme|Week|Month|Today|Next|Previous|Restore|Poster view|List view)\b/;

test.describe('German texts (shell AC-14)', () => {
  for (const target of TARGETS) {
    test(`${target.name} speaks German`, async ({ page }) => {
      await withBlocklist(page);
      await page.goto(`${LIBRARY}${await target.path(page)}`);
      await settle(page);
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
