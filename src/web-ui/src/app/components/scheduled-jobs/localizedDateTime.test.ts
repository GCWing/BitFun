/**
 * This control exists because a native datetime-local renders in the browser's
 * locale, not the app's. These cases pin the ordering it replaces that with,
 * and the text <-> value contract callers still rely on.
 */

import { describe, expect, it } from 'vitest';
import {
  dateTimeFormatHint,
  formatDateTimeText,
  parseDateTimeText,
  resolveDateFieldOrder,
} from './localizedDateTime';

describe('resolveDateFieldOrder', () => {
  it('reads year-first for CJK locales, regardless of the browser locale', () => {
    expect(resolveDateFieldOrder('zh-CN')).toBe('year-first');
    expect(resolveDateFieldOrder('zh-TW')).toBe('year-first');
    expect(resolveDateFieldOrder('ja-JP')).toBe('year-first');
  });

  it('reads month-first for English and unknown locales', () => {
    expect(resolveDateFieldOrder('en-US')).toBe('month-first');
    expect(resolveDateFieldOrder(undefined)).toBe('month-first');
    expect(resolveDateFieldOrder('')).toBe('month-first');
  });
});

describe('formatDateTimeText', () => {
  it('renders in the locale order', () => {
    expect(formatDateTimeText('2026-08-12T04:48', 'year-first')).toBe('2026/08/12 04:48');
    expect(formatDateTimeText('2026-08-12T04:48', 'month-first')).toBe('08/12/2026 04:48');
  });

  it('renders empty for a missing or unparseable value', () => {
    expect(formatDateTimeText('', 'year-first')).toBe('');
    expect(formatDateTimeText('nonsense', 'year-first')).toBe('');
  });
});

describe('parseDateTimeText', () => {
  it('round-trips its own rendering in both orders', () => {
    for (const order of ['year-first', 'month-first'] as const) {
      const text = formatDateTimeText('2026-08-12T04:48', order);
      expect(parseDateTimeText(text, order)).toBe('2026-08-12T04:48');
    }
  });

  it('accepts loose separators and unpadded numbers', () => {
    expect(parseDateTimeText('2026-8-12 4:8', 'year-first')).toBe('2026-08-12T04:08');
    expect(parseDateTimeText('2026.08.12 04:48', 'year-first')).toBe('2026-08-12T04:48');
    expect(parseDateTimeText('2026/08/12 04:48:30', 'year-first')).toBe('2026-08-12T04:48');
  });

  it('locates the year by its four digits, whichever order it was typed in', () => {
    // Someone pasting the other locale's order still lands on the right date.
    expect(parseDateTimeText('08/12/2026 04:48', 'year-first')).toBe('2026-08-12T04:48');
    expect(parseDateTimeText('2026/08/12 04:48', 'month-first')).toBe('2026-08-12T04:48');
  });

  it('falls back to the locale order when no group is four digits', () => {
    expect(parseDateTimeText('26/8/12 04:48', 'year-first')).toBeNull();
    expect(parseDateTimeText('8/12/26 04:48', 'month-first')).toBeNull();
  });

  it('returns null while the text is still incomplete', () => {
    expect(parseDateTimeText('', 'year-first')).toBeNull();
    expect(parseDateTimeText('2026', 'year-first')).toBeNull();
    expect(parseDateTimeText('2026/08/12', 'year-first')).toBeNull();
  });

  it('rejects out-of-range components', () => {
    expect(parseDateTimeText('2026/13/12 04:48', 'year-first')).toBeNull();
    expect(parseDateTimeText('2026/08/12 24:48', 'year-first')).toBeNull();
    expect(parseDateTimeText('2026/08/12 04:60', 'year-first')).toBeNull();
  });

  it('clamps a day past the end of its month instead of rolling over', () => {
    expect(parseDateTimeText('2026/02/31 09:00', 'year-first')).toBe('2026-02-28T09:00');
    expect(parseDateTimeText('2028/02/31 09:00', 'year-first')).toBe('2028-02-29T09:00');
    expect(parseDateTimeText('2026/04/31 09:00', 'year-first')).toBe('2026-04-30T09:00');
  });
});

describe('dateTimeFormatHint', () => {
  it('shows the placeholder in the same order the field reads', () => {
    expect(dateTimeFormatHint('year-first')).toBe('YYYY/MM/DD HH:mm');
    expect(dateTimeFormatHint('month-first')).toBe('MM/DD/YYYY HH:mm');
  });
});
