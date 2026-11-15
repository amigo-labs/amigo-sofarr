// The UI's own sources as text, for `aliases.test.ts` (spec amigo-theme). Plain Node,
// typed in `sources.test-helper.d.mts`; Vitest's `?raw` leaves stylesheets empty.

import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ui = fileURLToPath(new URL('../../..', import.meta.url));

/** A file of `ui/` as text. */
export function uiFile(path) {
  return readFileSync(join(ui, path), 'utf8');
}

/** Stylesheets, components and documents under `ui/src`, without the generated theme folder. */
export function sources() {
  const src = join(ui, 'src');
  return readdirSync(src, { recursive: true })
    .map((file) => file.replaceAll('\\', '/'))
    .filter((file) => !file.startsWith('theme/') && /\.(svelte|css|html)$/.test(file))
    .map((path) => ({ path, text: readFileSync(join(src, path), 'utf8') }));
}
