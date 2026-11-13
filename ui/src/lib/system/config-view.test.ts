import { describe, expect, it } from 'vitest';
import { GENERAL, configGroups, valueText } from './config-view';

describe('valueText', () => {
  it('writes values in German', () => {
    expect(valueText(null)).toBe('nicht gesetzt');
    expect(valueText(true)).toBe('ja');
    expect(valueText(false)).toBe('nein');
    expect(valueText('')).toBe('(leer)');
    expect(valueText('***')).toBe('***');
    expect(valueText(30)).toBe('30');
    expect(valueText([])).toBe('keine');
    expect(valueText([2000, 2040])).toBe('2000, 2040');
    expect(valueText([{ from: '/a', to: '/b' }])).toBe('{"from":"/a","to":"/b"}');
  });
});

describe('configGroups', () => {
  it('groups like the sections of sofarr.toml', () => {
    const groups = configGroups({
      version_hint: 'x',
      server: { bind: '0.0.0.0:7676' },
      auth: { enabled: true, password_hash: '***', session_days: 30 },
      log: { level: 'info', nested: { deep: 1 } },
      indexer: [
        { name: 'A', url: 'https://a.example/api?***', api_key: '***' },
        { name: 'B', url: 'https://b.example/api', api_key: '***' },
      ],
      download: { path_map: [] },
      nzbget: null,
    });
    expect(groups.map((g) => g.title)).toEqual([
      GENERAL,
      'server',
      'auth',
      'log',
      'indexer 1',
      'indexer 2',
      'download',
    ]);
    expect(groups[0]?.rows).toEqual([
      { key: 'version_hint', value: 'x' },
      { key: 'nzbget', value: 'nicht gesetzt' },
    ]);
    expect(groups.find((g) => g.title === 'auth')?.rows).toEqual([
      { key: 'enabled', value: 'ja' },
      { key: 'password_hash', value: '***' },
      { key: 'session_days', value: '30' },
    ]);
    expect(groups.find((g) => g.title === 'log')?.rows).toContainEqual({
      key: 'nested.deep',
      value: '1',
    });
    expect(groups.find((g) => g.title === 'indexer 1')?.rows).toContainEqual({
      key: 'url',
      value: 'https://a.example/api?***',
    });
    expect(groups.find((g) => g.title === 'download')?.rows).toEqual([
      { key: 'path_map', value: 'keine' },
    ]);
  });

  it('has no general group when every value is a table', () => {
    expect(configGroups({ server: { bind: 'x' } }).map((g) => g.title)).toEqual(['server']);
  });
});
