// Files the theme tests read (spec amigo-theme): the generated tokens and the built
// stylesheets. Plain Node, typed in `theme-files.d.mts`.

import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ui = fileURLToPath(new URL('..', import.meta.url));

/** The text of `src/theme/tokens.css`. */
export function tokensCss() {
  return readFileSync(join(ui, 'src', 'theme', 'tokens.css'), 'utf8');
}

/** Every stylesheet of `ui/build`: its path in the build and its text. */
export function builtStylesheets() {
  const build = join(ui, 'build');
  return readdirSync(build, { recursive: true })
    .filter((file) => file.endsWith('.css'))
    .map((file) => ({ file, css: readFileSync(join(build, file), 'utf8') }));
}
