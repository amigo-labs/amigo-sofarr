import { describe, expect, it } from 'vitest';
import type { Me } from '../api/generated/Me';
import { canLogOut, loginAddress, loginOffered, routeFor, safeTarget } from './redirect';

const OFF: Me = { auth_enabled: false, authenticated: false, method: null };
const LOGGED_OUT: Me = { auth_enabled: true, authenticated: false, method: null };
const SESSION: Me = { auth_enabled: true, authenticated: true, method: 'session' };
const TRUSTED: Me = { auth_enabled: true, authenticated: true, method: 'trusted_network' };

describe('safeTarget', () => {
  it('keeps relative paths with query and hash', () => {
    expect(safeTarget('/system')).toBe('/system');
    expect(safeTarget('/system?tab=jobs#rss')).toBe('/system?tab=jobs#rss');
    expect(safeTarget('/filme/12')).toBe('/filme/12');
  });

  it('falls back to the start page for anything off this origin', () => {
    for (const raw of [
      null,
      undefined,
      '',
      'system',
      'https://evil.example/',
      '//evil.example/x',
      '/\\evil.example',
      '\\\\evil.example',
      'javascript:alert(1)',
      '/sys\ntem',
      '/sys\ttem',
    ]) {
      expect(safeTarget(raw)).toBe('/');
    }
  });

  it('keeps encoded characters encoded: they belong to the path', () => {
    expect(safeTarget('/%2F%2Fevil.example')).toBe('/%2F%2Fevil.example');
  });

  it('never returns to the login page or the API', () => {
    expect(safeTarget('/anmelden?weiter=/system')).toBe('/');
    expect(safeTarget('/api/system')).toBe('/');
  });

  it('normalizes dot segments', () => {
    expect(safeTarget('/a/../system')).toBe('/system');
    expect(safeTarget('/../../anmelden')).toBe('/');
  });
});

describe('loginAddress', () => {
  it('carries the requested page in "weiter"', () => {
    expect(loginAddress('/system?x=1')).toBe('/anmelden?weiter=%2Fsystem%3Fx%3D1');
  });

  it('omits the start page', () => {
    expect(loginAddress('/')).toBe('/anmelden');
    expect(loginAddress('//evil.example')).toBe('/anmelden');
  });
});

describe('routeFor', () => {
  it('sends a logged-out browser to the login page with the requested page', () => {
    expect(routeFor(LOGGED_OUT, '/system', '', '')).toBe('/anmelden?weiter=%2Fsystem');
    expect(routeFor(LOGGED_OUT, '/filme', '?q=a', '#x')).toBe(
      '/anmelden?weiter=%2Ffilme%3Fq%3Da%23x',
    );
    expect(routeFor(LOGGED_OUT, '/anmelden', '?weiter=/system', '')).toBeNull();
  });

  it('leaves the login page after a login, to the requested page', () => {
    expect(routeFor(SESSION, '/anmelden', '?weiter=%2Fsystem', '')).toBe('/system');
    expect(routeFor(SESSION, '/anmelden', '?weiter=//evil.example', '')).toBe('/');
    expect(routeFor(SESSION, '/anmelden', '', '')).toBe('/');
    expect(routeFor(SESSION, '/system', '', '')).toBeNull();
  });

  it('has no login page while auth is off or in a trusted network', () => {
    expect(routeFor(OFF, '/anmelden', '?weiter=/system', '')).toBe('/');
    expect(routeFor(TRUSTED, '/anmelden', '?weiter=/system', '')).toBe('/');
    expect(routeFor(OFF, '/system', '', '')).toBeNull();
    expect(routeFor(TRUSTED, '/system', '', '')).toBeNull();
  });
});

describe('login and logout offers', () => {
  it('offers logout only for a session', () => {
    expect(canLogOut(SESSION)).toBe(true);
    expect(canLogOut(TRUSTED)).toBe(false);
    expect(canLogOut(OFF)).toBe(false);
    expect(canLogOut(LOGGED_OUT)).toBe(false);
    expect(canLogOut({ auth_enabled: true, authenticated: true, method: 'api_key' })).toBe(false);
  });

  it('offers the login page only with auth on outside trusted networks', () => {
    expect(loginOffered(LOGGED_OUT)).toBe(true);
    expect(loginOffered(SESSION)).toBe(true);
    expect(loginOffered(TRUSTED)).toBe(false);
    expect(loginOffered(OFF)).toBe(false);
  });
});
