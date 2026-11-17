// The Octicons the UI uses, read from the package, so a test can tell which icon a page shows.
// Plain Node, typed in `octicons.d.mts`.

import { readFileSync } from 'node:fs';

/** The path data of the 16 px Octicon `name`, e.g. `download`. */
export function octiconPath(name) {
  const svg = readFileSync(
    new URL(`../node_modules/@primer/octicons/build/svg/${name}-16.svg`, import.meta.url),
    'utf8',
  );
  return /<path d="([^"]+)"/.exec(svg)?.[1] ?? '';
}
