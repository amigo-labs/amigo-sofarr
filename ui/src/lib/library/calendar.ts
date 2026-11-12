// Weeks and months of the calendar page (spec web-ui-library AC-07). Dates are plain
// `YYYY-MM-DD` strings computed in UTC, so time zones and daylight saving never shift them.

export type Span = 'woche' | 'monat';

export type Range = { readonly from: string; readonly to: string };

const MONTHS = [
  'Januar',
  'Februar',
  'März',
  'April',
  'Mai',
  'Juni',
  'Juli',
  'August',
  'September',
  'Oktober',
  'November',
  'Dezember',
] as const;

export const WEEKDAYS = ['Mo', 'Di', 'Mi', 'Do', 'Fr', 'Sa', 'So'] as const;

const toDate = (day: string): Date => new Date(`${day}T00:00:00Z`);
const toDay = (date: Date): string => date.toISOString().slice(0, 10);

/** Whether a text is a valid `YYYY-MM-DD` date. */
export function isDay(text: string | null): text is string {
  return text !== null && /^\d{4}-\d{2}-\d{2}$/.test(text) && toDay(toDate(text)) === text;
}

export function addDays(day: string, days: number): string {
  const date = toDate(day);
  date.setUTCDate(date.getUTCDate() + days);
  return toDay(date);
}

/** Monday of the week of `day`. */
export function weekStart(day: string): string {
  const weekday = (toDate(day).getUTCDay() + 6) % 7;
  return addDays(day, -weekday);
}

/** The week (Monday to Sunday) or the month of `day`. */
export function rangeOf(span: Span, day: string): Range {
  if (span === 'woche') {
    const from = weekStart(day);
    return { from, to: addDays(from, 6) };
  }
  const date = toDate(day);
  const from = new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), 1));
  const to = new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth() + 1, 0));
  return { from: toDay(from), to: toDay(to) };
}

/** The day `steps` weeks or months away (back with negative steps). */
export function step(span: Span, day: string, steps: number): string {
  if (span === 'woche') return addDays(weekStart(day), steps * 7);
  const date = toDate(rangeOf('monat', day).from);
  date.setUTCMonth(date.getUTCMonth() + steps);
  return toDay(date);
}

/** ISO week number of `day`. */
export function weekNumber(day: string): number {
  const date = toDate(addDays(weekStart(day), 3));
  const yearStart = Date.UTC(date.getUTCFullYear(), 0, 1);
  return Math.floor((date.getTime() - yearStart) / 86_400_000 / 7) + 1;
}

/** The heading of a range, e.g. "November 2026" or "KW 46 · 9.11. – 15.11.2026". */
export function rangeTitle(span: Span, day: string): string {
  const { from, to } = rangeOf(span, day);
  const [fy, fm, fd] = from.split('-');
  const [ty, tm, td] = to.split('-');
  if (span === 'monat') return `${MONTHS[Number(fm) - 1]} ${fy}`;
  return `KW ${weekNumber(day)} · ${Number(fd)}.${Number(fm)}. – ${Number(td)}.${Number(tm)}.${ty}`;
}

/** Every day of a range, in order. */
export function daysOf(range: Range): string[] {
  const days: string[] = [];
  for (let day = range.from; day <= range.to; day = addDays(day, 1)) days.push(day);
  return days;
}

/** The weekday and date of a day, e.g. "Do, 12.11.". */
export function dayTitle(day: string): string {
  const date = toDate(day);
  const weekday = WEEKDAYS[(date.getUTCDay() + 6) % 7];
  return `${weekday}, ${date.getUTCDate()}.${date.getUTCMonth() + 1}.`;
}

/** Today in the browser's time zone as `YYYY-MM-DD`. */
export function today(now: Date = new Date()): string {
  const month = String(now.getMonth() + 1).padStart(2, '0');
  const date = String(now.getDate()).padStart(2, '0');
  return `${now.getFullYear()}-${month}-${date}`;
}
