import { describe, expect, it } from 'vitest';
import {
  addDays,
  dayTitle,
  daysOf,
  isDay,
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
