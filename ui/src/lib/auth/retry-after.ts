/** Seconds to wait when a throttled answer says nothing usable (spec web-ui-shell AC-05). */
export const DEFAULT_RETRY_SECONDS = 60;

/**
 * Seconds until the next attempt from a `Retry-After` header: delay seconds or an HTTP
 * date (RFC 9110 §10.2.3). A missing or invalid value means [`DEFAULT_RETRY_SECONDS`].
 */
export function parseRetryAfter(value: string | null, now: number): number {
  const text = value?.trim() ?? '';
  if (text === '') return DEFAULT_RETRY_SECONDS;
  if (/^\d+$/.test(text)) {
    const seconds = Number(text);
    return Number.isSafeInteger(seconds) ? seconds : DEFAULT_RETRY_SECONDS;
  }
  // Every HTTP date format starts with the day name ("Thu, 12 Nov 2026 …").
  const at = /^[A-Za-z]{3,9},? /.test(text) ? Date.parse(text) : Number.NaN;
  if (Number.isNaN(at)) return DEFAULT_RETRY_SECONDS;
  return Math.max(0, Math.ceil((at - now) / 1000));
}
