import { describe, expect, it, vi } from 'vitest';
import {
  differenceInLocalCalendarDays,
  formatRecentWorkspaceDate,
} from './recentWorkspaceDate';

const t = vi.fn((key: string, options?: Record<string, unknown>) => (
  options?.count === undefined ? key : `${key}:${options.count}`
));
const formatLocaleDate = vi.fn((date: Date) => `absolute:${date.toISOString()}`);

describe('formatRecentWorkspaceDate', () => {
  it('labels an earlier time on the same calendar day as today', () => {
    const date = new Date(2026, 7, 6, 0, 1);

    expect(formatRecentWorkspaceDate(
      date.toISOString(),
      new Date(2026, 7, 6, 23, 59),
      t,
      formatLocaleDate,
    )).toBe('time.today');
  });

  it('labels the previous calendar day as yesterday even when less than 24 hours ago', () => {
    const date = new Date(2026, 7, 5, 23, 59);

    expect(formatRecentWorkspaceDate(
      date.toISOString(),
      new Date(2026, 7, 6, 0, 1),
      t,
      formatLocaleDate,
    )).toBe('time.yesterday');
  });

  it.each([
    ['spring-forward', 23, new Date('2026-03-09T04:30:00Z'), new Date('2026-03-08T05:30:00Z')],
    ['fall-back', 25, new Date('2026-11-02T05:30:00Z'), new Date('2026-11-01T04:30:00Z')],
  ])('uses calendar dates across a %s boundary', (_name, elapsedHours, now, date) => {
    vi.spyOn(now, 'getFullYear').mockReturnValue(2026);
    vi.spyOn(now, 'getMonth').mockReturnValue(_name === 'spring-forward' ? 2 : 10);
    vi.spyOn(now, 'getDate').mockReturnValue(_name === 'spring-forward' ? 9 : 2);
    vi.spyOn(date, 'getFullYear').mockReturnValue(2026);
    vi.spyOn(date, 'getMonth').mockReturnValue(_name === 'spring-forward' ? 2 : 10);
    vi.spyOn(date, 'getDate').mockReturnValue(_name === 'spring-forward' ? 8 : 1);

    expect((now.getTime() - date.getTime()) / (60 * 60 * 1000)).toBe(elapsedHours);
    expect(differenceInLocalCalendarDays(now, date)).toBe(1);
  });

  it('formats a future calendar date as an absolute date', () => {
    const date = new Date(2026, 7, 7, 0, 20);

    expect(formatRecentWorkspaceDate(
      date.toISOString(),
      new Date(2026, 7, 6, 23, 50),
      t,
      formatLocaleDate,
    )).toBe(`absolute:${date.toISOString()}`);
  });
});
