// Screenshots of the web UI for amigo-labs.dev (`docs/site/screenshots/`):
// `bun run screenshots [out folder]`; `bun e2e/site/screenshots.mjs --serve` only starts the demo
// instance and keeps it running for a look by hand. Starts sofarr with the fictional demo library
// (`demo-data.mjs`), a fake TMDB, TVDB and IMDb and generated artwork, then photographs the
// pages in Chromium at 1440x900 @2x in dark mode. Prerequisites as for the e2e tests:
// `bun run build` and `cargo build -p sofarr-server`.

import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID } from 'node:crypto';
import { chromium } from '@playwright/test';
import { portBase } from '../env.mjs';
import { MOVIES, SERIES } from './demo-data.mjs';
import { SONARR_KEY, renderArtwork, seedLibrary } from './demo-seed.mjs';
import { startDemoTmdb } from './demo-tmdb.mjs';

const ui = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const root = resolve(ui, '..');
const serveOnly = process.argv[2] === '--serve';
const out = resolve(
  (serveOnly ? undefined : process.argv[2]) ?? join(root, 'docs', 'site', 'screenshots'),
);
const exe = process.platform === 'win32' ? 'amigo-sofarr.exe' : 'amigo-sofarr';
const binary = join(root, 'target', 'debug', exe);
const fixtures = join(root, 'fixtures', 'public', 'config', 'radarr');

const PORTS = { server: portBase() + 60, tmdb: portBase() + 61, sonarr: portBase() + 62 };
const BASE = `http://127.0.0.1:${PORTS.server}`;

const literal = (text) => `'${text.replaceAll('\\', '/')}'`;

function config(dir) {
  const fake = `http://127.0.0.1:${PORTS.tmdb}`;
  return [
    '[server]',
    `bind = "127.0.0.1:${PORTS.server}"`,
    '[database]',
    `path = ${literal(join(dir, 'sofarr.db'))}`,
    '[backup]',
    `dir = ${literal(join(dir, 'backups'))}`,
    '[log]',
    'level = "warn"',
    '[tmdb]',
    `api_key = "demo-${randomUUID()}"`,
    `url = "${fake}/tmdb/3"`,
    '[tvdb]',
    `api_key = "demo-${randomUUID()}"`,
    `url = "${fake}/tvdb"`,
    '[imdb]',
    `url = "${fake}/imdb/title.ratings.tsv.gz"`,
    '[sonarr]',
    `url = "http://127.0.0.1:${PORTS.sonarr}"`,
    `api_key = "${SONARR_KEY}"`,
  ].join('\n');
}

async function waitFor(check, what, seconds = 120) {
  for (let i = 0; i < seconds * 4; i += 1) {
    if (await check().catch(() => false)) return;
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error(`timed out waiting for ${what}`);
}

/** The pages to photograph: file name, path, and an optional step before the picture. */
const SHOTS = [
  { name: 'discover', path: '/entdecken', settled: 'IMDb …' },
  { name: 'movies', path: '/filme', button: 'Poster' },
  { name: 'movie', path: '/filme', open: MOVIES[0].title },
  { name: 'series', path: '/serien', open: SERIES[0].title },
  { name: 'calendar', path: '/kalender?ansicht=monat', text: SERIES[1].title },
  { name: 'person', path: '/entdecken/personen/30001', heading: 'Lena Marquardt' },
];

/** The images in the viewport that have not arrived yet (runs in the page). */
const pending = () =>
  [...document.images]
    .filter((img) => {
      const box = img.getBoundingClientRect();
      const inside = box.bottom > 0 && box.top < window.innerHeight;
      return inside && box.right > 0 && box.left < window.innerWidth && box.width > 0;
    })
    .filter((img) => !img.complete)
    .map((img) => img.src);

async function photograph(browser) {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 2,
    colorScheme: 'dark',
    locale: 'de-DE',
    timezoneId: 'Europe/Berlin',
  });
  const page = await context.newPage();
  mkdirSync(out, { recursive: true });
  for (const shot of SHOTS) {
    await page.goto(`${BASE}${shot.path}`, { waitUntil: 'networkidle' });
    if (shot.button) await page.getByRole('button', { name: shot.button }).click();
    if (shot.open) await page.getByRole('link', { name: shot.open, exact: true }).first().click();
    const heading = shot.open ?? shot.heading;
    if (heading) await page.getByRole('heading', { name: heading, level: 1 }).waitFor();
    if (shot.text) await page.getByText(shot.text).first().waitFor();
    // Ratings and library states fill in in the background: reload until they are there.
    for (let i = 0; shot.settled && i < 60; i += 1) {
      await page.waitForLoadState('networkidle');
      if ((await page.getByText(shot.settled).count()) === 0) break;
      await new Promise((r) => setTimeout(r, 1000));
      await page.reload();
    }
    await page.waitForLoadState('networkidle');
    try {
      // Posters load lazily: wait until every image in the viewport has arrived.
      await page.waitForFunction(`(${pending})().length === 0`, undefined, { timeout: 15_000 });
    } catch {
      throw new Error(
        `images of ${shot.path} did not load: ${(await page.evaluate(pending)).join(', ')}`,
      );
    }
    const file = join(out, `${shot.name}.png`);
    await page.screenshot({ path: file });
    process.stdout.write(`${file}\n`);
  }
  await context.close();
}

async function main() {
  if (!existsSync(binary))
    throw new Error(`missing ${binary}: run \`cargo build -p sofarr-server\``);
  if (!existsSync(join(ui, 'build', 'index.html')))
    throw new Error('missing ui/build: run `bun run build`');
  const dir = join(tmpdir(), 'sofarr-site', String(portBase()));
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const configPath = join(dir, 'sofarr.toml');
  writeFileSync(configPath, `${config(dir)}\n`, { encoding: 'utf8' });

  const browser = await chromium.launch();
  const tmdb = await startDemoTmdb(PORTS.tmdb);
  let server = null;
  try {
    await renderArtwork(browser, dir);
    await seedLibrary({ binary, config: configPath, dir, fixtures, sonarrPort: PORTS.sonarr });
    server = spawn(binary, ['--config', configPath, 'serve'], { stdio: 'inherit' });
    await waitFor(async () => (await fetch(`${BASE}/api/health`)).ok, 'the server');
    await waitFor(async () => {
      const body = await (await fetch(`${BASE}/api/imdb`)).json();
      return body.titles > 0;
    }, 'the IMDb ratings');
    if (serveOnly) {
      process.stdout.write(`demo instance at ${BASE} (Ctrl+C stops it)
`);
      await new Promise((r) => server.on('exit', r));
      return;
    }
    await photograph(browser);
  } finally {
    server?.kill();
    tmdb.close();
    await browser.close();
  }
}

await main();
