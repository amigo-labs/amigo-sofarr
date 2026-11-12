// The pages of IMDb ratings, subtitles and trailers under the shell's rules: axe (WCAG 2.1
// A/AA) in light and dark (shell AC-11), German texts (shell AC-14) and no horizontal
// scrolling at 360 px (web-ui-library AC-22).

import { AxeBuilder } from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { expectNoHorizontalScroll } from './helpers';
import { EXTRAS, RATINGS, waitForRatings, waitForSubtitleRun } from './extras-helpers';
import { movieId, seriesId } from './library-helpers';

type Target = {
  readonly name: string;
  readonly open: (page: Page) => Promise<void>;
};

const at = (url: string) => async (page: Page) => {
  await page.goto(url);
};

const TARGETS: readonly Target[] = [
  { name: 'Untertitel', open: at(`${EXTRAS}/untertitel`) },
  { name: 'Untertitel ausgeschaltet', open: at(`${RATINGS}/untertitel`) },
  { name: 'Filme mit Bewertungen', open: at(`${RATINGS}/filme`) },
  {
    name: 'Filme als Poster mit Bewertungs-Filtern',
    open: async (page) => {
      await page.goto(`${RATINGS}/filme?mindestbewertung=5&sortierung=bewertung`);
      await page.getByRole('button', { name: 'Poster' }).click();
    },
  },
  { name: 'Serien mit Bewertungen', open: at(`${RATINGS}/serien`) },
  {
    name: 'Film mit Bewertung',
    open: async (page) => {
      await page.goto(`${RATINGS}/filme/${await movieId(page.request, RATINGS, 'Bestnote')}`);
    },
  },
  {
    name: 'Serie mit Bewertung',
    open: async (page) => {
      await page.goto(`${RATINGS}/serien/${await seriesId(page.request, RATINGS, 'Hochserie')}`);
    },
  },
  {
    name: 'Film mit Untertiteln',
    open: async (page) => {
      await page.goto(`${EXTRAS}/filme/${await movieId(page.request, EXTRAS, 'Untertitelfilm')}`);
    },
  },
  {
    name: 'Film mit fremdem Trailer',
    open: async (page) => {
      await page.goto(`${EXTRAS}/filme/${await movieId(page.request, EXTRAS, 'Trailerfilm')}`);
    },
  },
  {
    name: 'Serie mit geöffneten Episoden-Untertiteln',
    open: async (page) => {
      const id = await seriesId(page.request, EXTRAS, 'Untertitelserie');
      await page.goto(`${EXTRAS}/serien/${id}`);
      await page.locator('[data-episode="S01E01"] summary').click();
      await expect(page.getByTestId('subtitles-needed')).toBeVisible();
    },
  },
  { name: 'System', open: at(`${EXTRAS}/system`) },
];

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

test.beforeEach(async ({ request }) => {
  await waitForRatings(request);
  await waitForSubtitleRun(request);
});

for (const scheme of ['light', 'dark'] as const) {
  test.describe(`axe, ${scheme} theme`, () => {
    for (const target of TARGETS) {
      test(target.name, async ({ page }) => {
        await page.emulateMedia({ colorScheme: scheme });
        await target.open(page);
        await settle(page);
        await expectNoViolations(page);
      });
    }
  });
}

test.describe('360 px', () => {
  test.use({ viewport: { width: 360, height: 740 } });

  for (const target of TARGETS) {
    test(`${target.name} fits without horizontal scrolling`, async ({ page }) => {
      await target.open(page);
      await settle(page);
      await expectNoHorizontalScroll(page);
    });
  }
});

/** Words an English UI would show (as in layout.spec.ts). */
const ENGLISH =
  /\b(Loading|Error|Retry|Search|Monitored|Missing|Remove|Delete|Cancel|Save|Rename|Settings|Not found|Menu|Theme|Subtitles?|Rating|Votes|Download now|Forced|Embedded|Needed)\b/;

test.describe('German texts', () => {
  for (const target of TARGETS) {
    test(`${target.name} speaks German`, async ({ page }) => {
      await target.open(page);
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
