// Starts one sofarr instance for the end-to-end tests: `bun e2e/serve.mjs <variant>`.
// Writes a fresh configuration (with an Argon2id hash from `amigo-sofarr hash-password`)
// into the temp folder and runs the debug binary of the workspace, which serves `ui/build`.
// Playwright's `webServer` starts one per variant and stops it after the run.

import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { e2ePassword, portBase } from './env.mjs';
import { discoverConfig, seedDiscover } from './discover-seed.mjs';
import { seed, seedConfig } from './seed.mjs';
import { EXTRA_VARIANTS, extrasConfig, seedExtras } from './seed-extras.mjs';
import { SERVERS, USERNAME } from './servers.ts';

const variant = process.argv[2];
if (!variant || !(variant in SERVERS)) {
  process.stderr.write(`usage: bun e2e/serve.mjs <${Object.keys(SERVERS).join('|')}>\n`);
  process.exit(2);
}

const ui = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const root = resolve(ui, '..');
const exe = process.platform === 'win32' ? 'amigo-sofarr.exe' : 'amigo-sofarr';
const binary = join(root, 'target', 'debug', exe);
if (!existsSync(binary)) {
  process.stderr.write(`missing ${binary}: run \`cargo build -p sofarr-server\` first\n`);
  process.exit(1);
}
if (!existsSync(join(ui, 'build', 'index.html'))) {
  process.stderr.write('missing ui/build: run `bun run build` first\n');
  process.exit(1);
}

// Per port base, so two checkouts running their tests at once keep apart.
const dir = join(tmpdir(), 'sofarr-e2e', String(portBase()), variant);
rmSync(dir, { recursive: true, force: true });
mkdirSync(dir, { recursive: true });

/** A TOML literal string: no escapes, so Windows paths stay as they are. */
const literal = (text) => `'${text.replaceAll('\\', '/')}'`;

function hashPassword() {
  const result = spawnSync(binary, ['hash-password'], {
    input: `${e2ePassword()}\n`,
    encoding: 'utf8',
  });
  if (result.status !== 0) {
    process.stderr.write(`hash-password failed: ${result.stderr}\n`);
    process.exit(1);
  }
  return result.stdout.trim();
}

const port = new URL(SERVERS[variant]).port;
const lines = [
  '[server]',
  `bind = "127.0.0.1:${port}"`,
  '[database]',
  `path = ${literal(join(dir, 'sofarr.db'))}`,
  '[backup]',
  `dir = ${literal(join(dir, 'backups'))}`,
  '[log]',
  'level = "warn"',
];

if (variant === 'open') {
  // A download client and an indexer nobody answers for: the system page shows them, and
  // their secrets must arrive masked.
  lines.push(
    '[nzbget]',
    'url = "http://127.0.0.1:9"',
    'username = "nzbget"',
    'password = "nzbget-passwort-e2e"',
    '[[indexer]]',
    'name = "Testindexer"',
    'url = "http://indexer-user:indexer-pass@127.0.0.1:9/api?apikey=abc123"',
    'api_key = "indexer-schluessel-e2e"',
  );
} else if (['library', 'bench', 'live'].includes(variant)) {
  lines.push(...seedConfig(variant));
} else if (EXTRA_VARIANTS.includes(variant)) {
  lines.push(...extrasConfig(variant));
} else if (['discover', 'discoverPlain'].includes(variant)) {
  lines.push(...discoverConfig(variant));
} else {
  lines.push(
    '[auth]',
    'enabled = true',
    `username = "${USERNAME}"`,
    `password_hash = "${hashPassword()}"`,
  );
  if (variant === 'trusted') lines.push('trusted_networks = ["127.0.0.1/32"]');
}

// The e2e servers never fetch IMDb's real dataset: `ratings` and `discover` load a local
// one, `extras` and `discoverPlain` have ratings off, and the others try a closed port.
if (![...EXTRA_VARIANTS, 'discover', 'discoverPlain'].includes(variant)) {
  lines.push('[imdb]', 'url = "http://127.0.0.1:9/title.ratings.tsv.gz"');
}

const config = join(dir, 'sofarr.toml');
writeFileSync(config, `${lines.join('\n')}\n`, { encoding: 'utf8' });

if (['library', 'bench', 'live'].includes(variant)) {
  const fixtures = join(root, 'fixtures', 'public', 'config', 'radarr');
  await seed(variant, { binary, config, dir, fixtures });
}
if (EXTRA_VARIANTS.includes(variant)) {
  const fixtures = join(root, 'fixtures', 'public', 'config', 'radarr');
  await seedExtras(variant, { binary, config, dir, fixtures });
}
if (['discover', 'discoverPlain'].includes(variant)) {
  const fixtures = join(root, 'fixtures', 'public', 'config', 'radarr');
  await seedDiscover(variant, { binary, config, dir, fixtures });
}

const child = spawn(binary, ['--config', config, 'serve'], { stdio: 'inherit' });
for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => child.kill(signal));
}
child.on('exit', (code) => process.exit(code ?? 0));
