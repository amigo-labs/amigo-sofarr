// Spec ui-shell-redesign AC-10 .. AC-12: sofarr's buttons, boxes, fields, checkboxes and status
// labels look like the shared component classes; status labels are filled chips in four tones;
// UI links are not underlined, prose links are.

import { expect, test, type Page } from '@playwright/test';
import { expectSystemPage } from './helpers';
import { LIBRARY } from './library-helpers';
import { SERVERS } from './servers';
import { PALETTES, storeTheme, token } from './theme-helpers';

const LOOK = [
  'height',
  'border-top-left-radius',
  'border-top-color',
  'border-top-width',
  'background-color',
  'color',
] as const;

/** Puts `ours` and `theirs` (markup) side by side into the page and returns their looks. */
async function lookOf(
  page: Page,
  ours: string,
  theirs: string,
  pick: string,
  pseudo: string | null = null,
): Promise<{ ours: Record<string, string>; theirs: Record<string, string> }> {
  return page.evaluate(
    (args) => {
      const read = (markup: string): Record<string, string> => {
        const host = document.createElement('div');
        host.innerHTML = markup;
        document.querySelector('main')?.append(host);
        const el = host.querySelector(args.pick) ?? host.firstElementChild;
        if (el === null) throw new Error(`nothing to measure in ${markup}`);
        const style = getComputedStyle(el, args.pseudo);
        const look = Object.fromEntries(args.props.map((p) => [p, style.getPropertyValue(p)]));
        host.remove();
        return look;
      };
      return { ours: read(args.ours), theirs: read(args.theirs) };
    },
    { ours, theirs, pick, pseudo, props: [...LOOK] },
  );
}

for (const palette of PALETTES) {
  test.describe(`${palette} palette`, () => {
    test.beforeEach(async ({ page }) => {
      await storeTheme(page, palette);
      await page.goto(`${SERVERS.open}/system`);
      await expectSystemPage(page);
    });

    for (const [name, ours, theirs] of [
      ['button', '<button class="btn">A</button>', '<button class="am-btn">A</button>'],
      [
        'disabled button',
        '<button class="btn" disabled>A</button>',
        '<button class="am-btn" disabled>A</button>',
      ],
      [
        'primary button',
        '<button class="btn btn-primary">A</button>',
        '<button class="am-btn am-btn--primary">A</button>',
      ],
      [
        'disabled primary button',
        '<button class="btn btn-primary" disabled>A</button>',
        '<button class="am-btn am-btn--primary" disabled>A</button>',
      ],
      [
        'danger button',
        '<button class="btn btn-danger">A</button>',
        '<button class="am-btn am-btn--danger">A</button>',
      ],
      ['link as a button', '<a class="btn" href="/">A</a>', '<button class="am-btn">A</button>'],
      ['box', '<div class="box"><p>A</p></div>', '<div class="am-panel"><p>A</p></div>'],
    ] as const) {
      test(`a ${name} looks like its shared counterpart (AC-10)`, async ({ page }) => {
        const look = await lookOf(page, ours, theirs, ':scope > *');
        expect(look.ours).toEqual(look.theirs);
      });
    }

    for (const control of [
      '<input type="text" value="A">',
      '<select><option>A</option></select>',
      '<textarea>A</textarea>',
    ]) {
      test(`a field (${control.slice(1, control.indexOf(' ') > 0 ? control.indexOf(' ') : control.indexOf('>'))}) looks like am-input (AC-10)`, async ({
        page,
      }) => {
        const tag = control.startsWith('<input')
          ? 'input'
          : control.startsWith('<select')
            ? 'select'
            : 'textarea';
        const theirs = control.replace(`<${tag}`, `<${tag} class="am-input"`);
        const look = await lookOf(
          page,
          `<label class="form-field">A ${control}</label>`,
          theirs,
          tag,
        );
        // A textarea grows with its rows; the rest of its look is the shared field's.
        if (tag === 'textarea') {
          delete look.ours.height;
          delete look.theirs.height;
        }
        expect(look.ours).toEqual(look.theirs);
      });
    }

    test('a field without a form-field wrapper looks like am-input (AC-10)', async ({ page }) => {
      const look = await lookOf(
        page,
        '<input type="search" value="A">',
        '<input type="search" class="am-input" value="A">',
        'input',
      );
      expect(look.ours).toEqual(look.theirs);
    });

    for (const checked of [false, true]) {
      test(`a ${checked ? 'checked' : 'clear'} checkbox looks like am-check's box (AC-10)`, async ({
        page,
      }) => {
        const box = `<input type="checkbox"${checked ? ' checked' : ''}>`;
        const look = await lookOf(
          page,
          `<label class="check">${box} A</label>`,
          `<label class="am-check">${box} A</label>`,
          'input',
        );
        expect(look.ours).toEqual(look.theirs);
        // Also a checkbox outside a .check label, as in lists and tables.
        const bare = await lookOf(page, box, `<label class="am-check">${box}</label>`, 'input');
        expect(bare.ours).toEqual(bare.theirs);
      });
    }

    for (const [tone, chip, ground, text, dot] of [
      ['neutral', 'am-chip', 'surface-raised', 'text-secondary', 'border-strong'],
      ['success', 'am-chip am-chip--success', 'success-subtle', 'success-ink', 'success'],
      ['danger', 'am-chip am-chip--danger', 'danger-subtle', 'danger-ink', 'danger'],
      ['accent', 'am-chip am-chip--accent', 'accent-subtle', 'accent-ink', 'accent'],
    ] as const) {
      test(`a ${tone} status label is a filled chip with a dot (AC-10, AC-11)`, async ({
        page,
      }) => {
        const ours = `<span class="label${tone === 'neutral' ? '' : ` label-${tone}`}">fehlt</span>`;
        const look = await lookOf(page, ours, `<span class="${chip}">fehlt</span>`, 'span');
        expect(look.ours).toEqual(look.theirs);
        expect(look.ours['background-color']).toBe(token(palette, ground));
        expect(look.ours.color).toBe(token(palette, text));
        const dots = await lookOf(
          page,
          ours,
          `<span class="${chip}">fehlt</span>`,
          'span',
          '::before',
        );
        expect(dots.ours['background-color']).toBe(token(palette, dot));
        expect(dots.ours).toEqual(dots.theirs);
      });
    }

    test('the labels in use keep their tone (AC-11)', async ({ page }) => {
      // The system page shows "ok" in success and an error in danger.
      const success = page.locator('main .label-success').first();
      const danger = page.locator('main .label-danger').first();
      await expect(success).toBeVisible();
      await expect(danger).toBeVisible();
      expect(await success.evaluate((el) => getComputedStyle(el).backgroundColor)).toBe(
        token(palette, 'success-subtle'),
      );
      expect(await danger.evaluate((el) => getComputedStyle(el).backgroundColor)).toBe(
        token(palette, 'danger-subtle'),
      );
    });
  });
}

const decoration = (page: Page, selector: string) =>
  page
    .locator(selector)
    .first()
    .evaluate((el) => getComputedStyle(el).textDecorationLine);

test.describe('links (AC-12)', () => {
  test('navigation entries and the brand are not underlined', async ({ page }) => {
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    expect(await decoration(page, '.sidebar nav a')).toBe('none');
    expect(await decoration(page, '.sidebar .brand')).toBe('none');
  });

  test('titles in lists are underlined only on hover and focus; prose links always', async ({
    page,
  }) => {
    await page.goto(`${LIBRARY}/filme`);
    const title = page.locator('main .items a').first();
    await expect(title).toBeVisible();
    expect(await title.evaluate((el) => getComputedStyle(el).textDecorationLine)).toBe('none');
    await title.hover();
    expect(await title.evaluate((el) => getComputedStyle(el).textDecorationLine)).toBe('underline');
    await page.mouse.move(0, 0);
    await title.focus();
    await page.keyboard.press('Shift+Tab');
    await page.keyboard.press('Tab');
    expect(await title.evaluate((el) => getComputedStyle(el).textDecorationLine)).toBe('underline');
    // The IMDb attribution is a sentence.
    const attribution = page.locator('.imdb-attribution a');
    if ((await attribution.count()) > 0) {
      expect(
        await attribution.first().evaluate((el) => getComputedStyle(el).textDecorationLine),
      ).toBe('underline');
    }
  });

  test('the overview, the calendar and the missing page show titles without underline', async ({
    page,
  }) => {
    for (const path of ['/', '/kalender', '/fehlend', '/blocklist']) {
      await page.goto(`${LIBRARY}${path}`);
      await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
      const links = page.locator('main a:not(.btn)');
      await page.waitForLoadState('networkidle');
      const underlined = await links.evaluateAll((all) =>
        all
          .filter((el) => getComputedStyle(el).textDecorationLine !== 'none')
          .filter((el) => el.closest('p, .imdb-attribution, [role="alert"]') === null)
          .map((el) => el.textContent?.trim()),
      );
      expect(underlined, path).toEqual([]);
    }
  });

  test('a prose link in a paragraph is underlined', async ({ page }) => {
    await page.goto(`${SERVERS.open}/gibt-es-nicht`);
    const link = page.getByRole('link', { name: 'Zur Startseite' });
    await expect(link).toBeVisible();
    expect(await link.evaluate((el) => getComputedStyle(el).textDecorationLine)).toBe('underline');
  });
});
