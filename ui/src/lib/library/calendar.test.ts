import { describe, expect, it } from 'vitest';
import {
  addDays,
  dayTitle,
  daysOf,
  isDay,
  monthGrid,
  rangeOf,
  rangeTitle,
  step,
  today,
  weekNumber,
  weekStart,
} from './calendar';

describe('calendar ranges (AC-07)', () => {
  it('weeks run from Monday to Sunday', () => {
    expect(weekStart('2026-11-12')).toBe('2026-11-09');
    expect(weekStart('2026-11-09')).toBe('2026-11-09');
    expect(weekStart('2026-11-15')).toBe('2026-11-09');
    expect(rangeOf('woche', '2026-11-12')).toEqual({ from: '2026-11-09', to: '2026-11-15' });
  });

  it('months run from the first to the last day', () => {
    expect(rangeOf('monat', '2026-11-12')).toEqual({ from: '2026-11-01', to: '2026-11-30' });
    expect(rangeOf('monat', '2028-02-10')).toEqual({ from: '2028-02-01', to: '2028-02-29' });
  });

  it('steps back and forth across years and daylight saving', () => {
    expect(step('woche', '2026-11-12', 1)).toBe('2026-11-16');
    expect(step('woche', '2026-11-12', -1)).toBe('2026-11-02');
    expect(step('woche', '2026-03-26', 1)).toBe('2026-03-30');
    expect(step('monat', '2026-12-31', 1)).toBe('2027-01-01');
    expect(step('monat', '2026-01-31', -1)).toBe('2025-12-01');
  });

  it('lists every day of a range', () => {
    expect(daysOf({ from: '2026-12-30', to: '2027-01-02' })).toEqual([
      '2026-12-30',
      '2026-12-31',
      '2027-01-01',
      '2027-01-02',
    ]);
    expect(addDays('2026-02-28', 1)).toBe('2026-03-01');
  });

  it('titles weeks and months in German', () => {
    expect(rangeTitle('monat', '2026-11-12')).toBe('November 2026');
    expect(rangeTitle('monat', '2026-03-01')).toBe('März 2026');
    expect(rangeTitle('woche', '2026-11-12')).toBe('KW 46 · 9.11. – 15.11.2026');
    expect(weekNumber('2027-01-01')).toBe(53);
    expect(dayTitle('2026-11-12')).toBe('Do, 12.11.');
  });

  it('accepts only real dates', () => {
    expect(isDay('2026-11-12')).toBe(true);
    expect(isDay('2026-02-30')).toBe(false);
    expect(isDay('12.11.2026')).toBe(false);
    expect(isDay(null)).toBe(false);
    expect(today(new Date(2026, 10, 12, 23, 30))).toBe('2026-11-12');
  });
});

/** Rows, first and last day, and the outside days of a month's grid. */
const shape = (day: string) => {
  const weeks = monthGrid(day);
  return {
    rows: weeks.length,
    first: weeks[0]?.[0]?.day,
    last: weeks.at(-1)?.at(-1)?.day,
    outside: weeks
      .flat()
      .filter((c) => c.outside)
      .map((c) => c.day),
  };
};

describe('month grid (ui-calendar AC-01)', () => {
  it('a month starting on Monday has no leading outside days', () => {
    // June 2026 starts on a Monday and ends on a Tuesday.
    expect(shape('2026-06-15')).toEqual({
      rows: 5,
      first: '2026-06-01',
      last: '2026-07-05',
      outside: ['2026-07-01', '2026-07-02', '2026-07-03', '2026-07-04', '2026-07-05'],
    });
  });

  it('February of a non-leap year starting on Monday has exactly four rows', () => {
    expect(shape('2027-02-10')).toEqual({
      rows: 4,
      first: '2027-02-01',
      last: '2027-02-28',
      outside: [],
    });
  });

  it('a leap February and a month needing six rows', () => {
    expect(shape('2028-02-29').rows).toBe(5);
    expect(
      monthGrid('2028-02-01')
        .flat()
        .filter((c) => !c.outside),
    ).toHaveLength(29);
    // November 2026 starts on a Sunday: six rows.
    const november = shape('2026-11-16');
    expect(november.rows).toBe(6);
    expect(november.first).toBe('2026-10-26');
    expect(november.last).toBe('2026-12-06');
  });

  it('every row has seven days, Monday first, in order', () => {
    for (const week of monthGrid('2026-08-01')) {
      expect(week).toHaveLength(7);
      expect(dayTitle(week[0]?.day ?? '')).toMatch(/^Mo, /);
    }
    const days = monthGrid('2026-08-01')
      .flat()
      .map((c) => c.day);
    expect(days).toEqual(daysOf({ from: days[0] ?? '', to: days.at(-1) ?? '' }));
  });
});
