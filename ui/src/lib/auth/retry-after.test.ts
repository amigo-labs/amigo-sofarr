import { describe, expect, it } from 'vitest';
import { DEFAULT_RETRY_SECONDS, parseRetryAfter } from './retry-after';

const NOW = Date.parse('2026-11-12T12:00:00Z');

describe('parseRetryAfter', () => {
  it('reads delay seconds', () => {
    expect(parseRetryAfter('42', NOW)).toBe(42);
    expect(parseRetryAfter(' 0 ', NOW)).toBe(0);
  });

  it('reads an HTTP date relative to now, rounded up', () => {
    expect(parseRetryAfter('Thu, 12 Nov 2026 12:01:30 GMT', NOW)).toBe(90);
    expect(parseRetryAfter(new Date(NOW + 1500).toUTCString(), NOW)).toBe(1);
  });

  it('never waits a negative time for a date in the past', () => {
    expect(parseRetryAfter('Thu, 12 Nov 2026 11:00:00 GMT', NOW)).toBe(0);
  });

  it('waits 60 seconds without a usable value', () => {
    expect(DEFAULT_RETRY_SECONDS).toBe(60);
    expect(parseRetryAfter(null, NOW)).toBe(60);
    expect(parseRetryAfter('', NOW)).toBe(60);
    expect(parseRetryAfter('bald', NOW)).toBe(60);
    expect(parseRetryAfter('-5', NOW)).toBe(60);
    expect(parseRetryAfter('1.5', NOW)).toBe(60);
    expect(parseRetryAfter('99999999999999999999', NOW)).toBe(60);
  });
});
