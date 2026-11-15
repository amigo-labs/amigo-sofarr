// Spec amigo-theme AC-02, AC-03, AC-12: every custom property the components use is one of
// the shared theme's tokens or an alias `app.css` defines from them, colours only from
// tokens, and `@primer/primitives` is gone.

import { describe, expect, it } from 'vitest';
import { sources, uiFile } from './sources.test-helper.mjs';

const appCss = uiFile('src/app.css');
const tokensCss = uiFile('src/theme/tokens.css');
const manifestText = uiFile('package.json');
const lockfile = uiFile('bun.lock');

const tokens = new Set([...tokensCss.matchAll(/(--am-[a-z0-9-]+):/g)].map((m) => m[1]));
const manifest = JSON.parse(manifestText) as Record<string, Record<string, string> | undefined>;

/** The aliases `app.css` defines: name → value. */
function aliases(): Map<string, string> {
  const found = new Map<string, string>();
  for (const match of appCss.matchAll(/^\s*(--[A-Za-z][A-Za-z0-9-]*):\s*([^;]+);/gm)) {
    const value = (match[2] ?? '').replaceAll(/\s+/g, ' ').trim();
    found.set(match[1] ?? '', value.replaceAll('( ', '(').replaceAll(' )', ')'));
  }
  return found;
}

const COLOR_ALIAS =
  /^--(bgColor|fgColor|borderColor|button|control-(bg|border|fg)Color|focus-outline-color|header|underlineNav|overlay)/;
const TOKEN = String.raw`var\(--am-[a-z0-9-]+\)`;
const TOKEN_OR_MIX = new RegExp(
  String.raw`^(${TOKEN}|color-mix\(in srgb, ${TOKEN} \d+%, (${TOKEN}|transparent)\))$`,
);

describe('aliases of the shared theme', () => {
  it('reads the theme files', () => {
    expect(tokens.has('--am-bg')).toBe(true);
    expect(tokens.has('--am-accent-ink')).toBe(true);
    expect(aliases().size).toBeGreaterThan(40);
  });

  it('every custom property in use is a token or defined in app.css', () => {
    const defined = aliases();
    const unknown = new Set<string>();
    for (const { path, text } of sources()) {
      const local = new Set([...text.matchAll(/(--[A-Za-z][A-Za-z0-9-]*)\s*:/g)].map((m) => m[1]));
      for (const match of text.matchAll(/var\((--[A-Za-z][A-Za-z0-9-]*)/g)) {
        const name = match[1] ?? '';
        if (!tokens.has(name) && !defined.has(name) && !local.has(name)) {
          unknown.add(`${name} (${path})`);
        }
      }
    }
    expect([...unknown]).toEqual([]);
  });

  it('every token an alias names exists', () => {
    const missing: string[] = [];
    for (const [name, value] of aliases()) {
      for (const match of value.matchAll(/var\((--am-[a-z0-9-]+)\)/g)) {
        if (!tokens.has(match[1])) missing.push(`${name}: ${match[1]}`);
      }
    }
    expect(missing).toEqual([]);
  });

  it('every colour alias is a token or a mix of two tokens (AC-02)', () => {
    const wrong = [...aliases()]
      .filter(([name]) => COLOR_ALIAS.test(name))
      .filter(([, value]) => !TOKEN_OR_MIX.test(value))
      .map(([name, value]) => `${name}: ${value}`);
    expect(wrong).toEqual([]);
  });

  it('maps the roles of AC-02 and AC-03', () => {
    const defined = aliases();
    const expected: Record<string, string> = {
      '--bgColor-default': 'var(--am-bg)',
      '--bgColor-surface': 'var(--am-surface)',
      '--bgColor-muted': 'var(--am-surface-raised)',
      '--control-bgColor-rest': 'var(--am-surface-raised)',
      '--header-bgColor': 'var(--am-surface)',
      '--fgColor-default': 'var(--am-text)',
      '--fgColor-muted': 'var(--am-text-secondary)',
      '--fgColor-caption': 'var(--am-text-muted)',
      '--borderColor-default': 'var(--am-border)',
      '--borderColor-muted': 'var(--am-border)',
      '--control-borderColor-emphasis': 'var(--am-border-strong)',
      '--fgColor-accent': 'var(--am-accent-ink)',
      '--underlineNav-borderColor-active': 'var(--am-accent-ink)',
      '--button-primary-bgColor-rest': 'var(--am-accent-solid)',
      '--button-primary-fgColor-rest': 'var(--am-on-accent)',
      '--focus-outline-color': 'var(--am-focus)',
      '--fgColor-success': 'var(--am-success-ink)',
      '--fgColor-attention': 'var(--am-warning-ink)',
      '--fgColor-danger': 'var(--am-danger-ink)',
      '--bgColor-attention-muted': 'var(--am-warning-subtle)',
      '--bgColor-danger-muted': 'var(--am-danger-subtle)',
      '--bgColor-accent-muted': 'var(--am-accent-subtle)',
      '--bgColor-danger-emphasis': 'var(--am-danger-solid)',
      '--fgColor-onEmphasis': 'var(--am-on-accent)',
      '--fontStack-system': 'var(--am-font-sans)',
      '--fontStack-monospace': 'var(--am-font-mono)',
      '--control-medium-size': 'var(--am-control-md)',
      '--borderRadius-medium': 'var(--am-radius-sm)',
      '--borderRadius-large': 'var(--am-radius-md)',
    };
    const actual = Object.fromEntries(Object.keys(expected).map((k) => [k, defined.get(k)]));
    expect(actual).toEqual(expected);
  });

  it('components carry no colour of their own', () => {
    const raw: string[] = [];
    for (const { path, text } of sources()) {
      if (path === 'app.html') continue;
      const styles = path.endsWith('.css')
        ? [text]
        : [...text.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map((m) => m[1] ?? '');
      for (const css of styles) {
        for (const match of css.matchAll(/#[0-9a-fA-F]{3,8}\b|\b(rgba?|hsla?|oklch|oklab)\(/g)) {
          raw.push(`${path}: ${match[0]}`);
        }
      }
    }
    expect(raw).toEqual([]);
  });
});

describe('@primer/primitives is gone (AC-12)', () => {
  it('is no dependency of the UI', () => {
    for (const key of ['dependencies', 'devDependencies', 'peerDependencies']) {
      expect(Object.keys(manifest[key] ?? {})).not.toContain('@primer/primitives');
    }
    expect(lockfile).not.toContain('@primer/primitives');
  });

  it('no source imports its stylesheets', () => {
    const importing = sources()
      .filter(({ text }) => text.includes('@primer/primitives'))
      .map(({ path }) => path);
    expect(importing).toEqual([]);
  });

  it('Inter ships with the UI (AC-10)', () => {
    expect(Object.keys(manifest['devDependencies'] ?? {})).toContain('@fontsource-variable/inter');
    expect(appCss).toMatch(/@font-face\s*\{[^}]*font-family:\s*'Inter Variable'/);
    expect(appCss).toMatch(/inter-latin-wght-normal\.woff2/);
    // No font, stylesheet or script from another host.
    const remote = sources()
      .filter(({ text }) => /(url\(|@import|src=|href=)\s*['"]?(https?:)?\/\//.test(text))
      .map(({ path }) => path);
    expect(remote).toEqual([]);
  });
});
