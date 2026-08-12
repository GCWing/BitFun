/**
 * The whole reason this control exists is that a native datetime-local renders
 * in the browser's locale, not the app's. These cases pin the ordering it
 * replaces that with, and the value contract callers still rely on.
 */

import { describe, expect, it } from 'vitest';
import {
  composeDateTimeValue,
  parseDateTimeValue,
  resolveDateSegmentOrder,
} from './localizedDateTime';

describe('resolveDateSegmentOrder', () => {
  it('reads year-first for Chinese, regardless of the browser locale', () => {
    expect(resolveDateSegmentOrder('zh-CN')).toEqual(['year', 'month', 'day']);
    expect(resolveDateSegmentOrder('zh-TW')).toEqual(['year', 'month', 'day']);
  });

  it('reads month-first for English', () => {
    expect(resolveDateSegmentOrder('en-US')).toEqual(['month', 'day', 'year']);
  });

  it('falls back to month-first for an unknown or missing locale', () => {
    expect(resolveDateSegmentOrder(undefined)).toEqual(['month', 'day', 'year']);
    expect(resolveDateSegmentOrder('')).toEqual(['month', 'day', 'year']);
  });
});

describe('parseDateTimeValue', () => {
  it('splits the native datetime-local value into segments', () => {
    expect(parseDateTimeValue('2026-08-12T04:48')).toEqual({
      year: '2026', month: '08', day: '12', hour: '04', minute: '48',
    });
  });

  it('tolerates a seconds suffix', () => {
    expect(parseDateTimeValue('2026-08-12T04:48:30').minute).toBe('48');
  });

  it('returns empty segments for anything unparseable', () => {
    expect(parseDateTimeValue('')).toEqual({
      year: '', month: '', day: '', hour: '', minute: '',
    });
    expect(parseDateTimeValue('not a date').year).toBe('');
  });
});

describe('composeDateTimeValue', () => {
  it('rebuilds the exact value the native input would have produced', () => {
    expect(composeDateTimeValue({
      year: '2026', month: '08', day: '12', hour: '04', minute: '48',
    })).toBe('2026-08-12T04:48');
  });

  it('round-trips through parse without drift', () => {
    const original = '2026-12-31T23:59';
    expect(composeDateTimeValue(parseDateTimeValue(original))).toBe(original);
  });

  it('holds back a partial date instead of emitting a broken timestamp', () => {
    expect(composeDateTimeValue({
      year: '2026', month: '08', day: '', hour: '04', minute: '48',
    })).toBeNull();
  });

  it('clamps a day past the end of its month', () => {
    // February 31 would otherwise roll into March.
    expect(composeDateTimeValue({
      year: '2026', month: '02', day: '31', hour: '09', minute: '00',
    })).toBe('2026-02-28T09:00');
  });

  it('respects a leap year', () => {
    expect(composeDateTimeValue({
      year: '2028', month: '02', day: '31', hour: '09', minute: '00',
    })).toBe('2028-02-29T09:00');
  });

  it('pads a short month or day back to two digits', () => {
    expect(composeDateTimeValue({
      year: '2026', month: '8', day: '2', hour: '04', minute: '48',
    })).toBe('2026-08-02T04:48');
  });
});
