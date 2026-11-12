// AC-04 .. AC-07: login, logout, no login page where none is needed, expired sessions.

import { expect, test } from '@playwright/test';
import { expectSystemPage, json, logIn } from './helpers';
import { SERVERS } from './servers';

const BAD_LOGIN = 'Benutzername oder Passwort falsch';

test.describe('login (AC-04)', () => {
  test('a bookmarked page asks for a login and shows that page afterwards', async ({ page }) => {
    await page.goto(`${SERVERS.auth}/system?ansicht=kurz`);
    await expect(page).toHaveURL(`${SERVERS.auth}/anmelden?weiter=%2Fsystem%3Fansicht%3Dkurz`);
    await expect(page.getByRole('navigation', { name: 'Hauptnavigation' })).toHaveCount(0);
    await logIn(page);
    await expect(page).toHaveURL(`${SERVERS.auth}/system?ansicht=kurz`);
    await expectSystemPage(page);
  });

  test('a login without a requested page shows the system page (AC-08)', async ({ page }) => {
    await page.goto(`${SERVERS.auth}/anmelden`);
    await logIn(page);
    await expect(page).toHaveURL(`${SERVERS.auth}/system`);
    await expectSystemPage(page);
  });

  test('a foreign target after the login leads to the start page', async ({ page }) => {
    await page.goto(`${SERVERS.auth}/anmelden?weiter=${encodeURIComponent('//evil.example/x')}`);
    await logIn(page);
    await expect(page).toHaveURL(`${SERVERS.auth}/system`);
  });
});

test.describe('failed login and logout (AC-05)', () => {
  test('a wrong password shows the API message; throttling disables the button', async ({
    page,
  }) => {
    // The throttle server is used by this test alone: five failures lock out 127.0.0.1.
    await page.goto(`${SERVERS.throttle}/system`);
    for (let attempt = 1; attempt <= 5; attempt += 1) {
      await logIn(page, `falsch-${attempt}`);
      await expect(page.getByRole('alert')).toContainText(BAD_LOGIN);
    }
    const throttled = page.waitForResponse((r) => r.url().endsWith('/api/auth/login'));
    await logIn(page);
    const response = await throttled;
    expect(response.status()).toBe(429);
    const wait = Number(response.headers()['retry-after']);
    const alert = page.getByRole('alert');
    await expect(alert).toContainText('Zu viele fehlgeschlagene Anmeldungen');
    await expect(alert).toContainText(/Nächster Versuch ab \d{2}:\d{2}:\d{2} Uhr möglich/);
    await expect(alert).toContainText(new RegExp(`in (${wait}|${wait - 1}) Sekunden`));
    await expect(page.getByRole('button', { name: 'Anmelden' })).toBeDisabled();
  });

  test('without Retry-After the next attempt waits 60 seconds', async ({ page }) => {
    await page.clock.install();
    await page.route('**/api/auth/login', (route) =>
      json(route, { error: 'Zu viele fehlgeschlagene Anmeldungen' }, 429),
    );
    await page.goto(`${SERVERS.auth}/anmelden`);
    await logIn(page);
    await expect(page.getByRole('alert')).toContainText('(in 60 Sekunden)');
    const button = page.getByRole('button', { name: 'Anmelden' });
    await expect(button).toBeDisabled();
    await page.clock.runFor(59_000);
    await expect(button).toBeDisabled();
    await page.clock.runFor(2_000);
    await expect(button).toBeEnabled();
    await expect(page.getByRole('alert')).toContainText('Ein neuer Versuch ist jetzt möglich.');
  });

  test('a failed logout says so and the user stays logged in', async ({ page }) => {
    await page.goto(`${SERVERS.auth}/system`);
    await logIn(page);
    await expectSystemPage(page);
    await page.route('**/api/auth/logout', (route) =>
      json(route, { error: 'Interner Fehler – Details stehen im Log' }, 500),
    );
    await page.getByRole('button', { name: 'Abmelden' }).click();
    await expect(page.getByRole('alert')).toContainText(
      'Abmelden fehlgeschlagen: Interner Fehler – Details stehen im Log',
    );
    await expect(page).toHaveURL(`${SERVERS.auth}/system`);
    await expect(page.getByRole('button', { name: 'Abmelden' })).toBeVisible();
    await page.unroute('**/api/auth/logout');
    await page.reload();
    await expectSystemPage(page);
  });

  test('a logout ends the session and shows the login page', async ({ page }) => {
    await page.goto(`${SERVERS.auth}/system`);
    await logIn(page);
    await expectSystemPage(page);
    await page.getByRole('button', { name: 'Abmelden' }).click();
    await expect(page.getByRole('heading', { name: 'Bei sofarr anmelden' })).toBeVisible();
    await page.reload();
    await expect(page.getByRole('heading', { name: 'Bei sofarr anmelden' })).toBeVisible();
  });
});

test.describe('no login where none is needed (AC-06)', () => {
  for (const [name, base] of [
    ['auth off', SERVERS.open],
    ['trusted network', SERVERS.trusted],
  ] as const) {
    test(`${name}: no login page and no logout; the login address leads to /`, async ({ page }) => {
      await page.goto(`${base}/anmelden?weiter=%2Fsystem`);
      await expectSystemPage(page);
      expect(new URL(page.url()).pathname).toBe('/system');
      await expect(page.getByRole('heading', { name: 'Bei sofarr anmelden' })).toHaveCount(0);
      await expect(page.getByRole('button', { name: 'Abmelden' })).toHaveCount(0);
    });
  }
});

test.describe('expired session (AC-07)', () => {
  test('a refresh that gets 401 shows the login page and returns to the same page', async ({
    page,
  }) => {
    await page.clock.install();
    await page.goto(`${SERVERS.auth}/system`);
    await logIn(page);
    await expectSystemPage(page);
    // The session ends elsewhere (same cookie, logged out through the API).
    const logout = await page.request.post(`${SERVERS.auth}/api/auth/logout`);
    expect(logout.ok()).toBe(true);
    await page.clock.runFor(15_000);
    await expect(page).toHaveURL(`${SERVERS.auth}/anmelden?weiter=%2Fsystem`);
    await expect(
      page.getByText('Die Anmeldung ist abgelaufen. Bitte erneut anmelden.'),
    ).toBeVisible();
    await logIn(page);
    await expect(page).toHaveURL(`${SERVERS.auth}/system`);
    await expectSystemPage(page);
  });

  test('a write action that got 401 is not repeated and the page says so', async ({ page }) => {
    await page.goto(`${SERVERS.auth}/system`);
    await logIn(page);
    await expectSystemPage(page);
    let calls = 0;
    await page.route('**/api/auth/logout', (route) => {
      calls += 1;
      return json(route, { error: 'Anmeldung erforderlich' }, 401);
    });
    await page.getByRole('button', { name: 'Abmelden' }).click();
    await expect(page).toHaveURL(`${SERVERS.auth}/anmelden?weiter=%2Fsystem`);
    await expect(page.getByText('„Abmelden“ wurde nicht ausgeführt.')).toBeVisible();
    await logIn(page);
    await expectSystemPage(page);
    await expect(
      page.getByText(/„Abmelden“ wurde nicht ausgeführt, weil die Anmeldung/),
    ).toBeVisible();
    expect(calls).toBe(1);
  });
});
