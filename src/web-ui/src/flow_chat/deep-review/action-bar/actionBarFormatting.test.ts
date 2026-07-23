import { describe, expect, it } from 'vitest';
import { formatElapsedTime } from './actionBarFormatting';

describe('action bar formatting', () => {
  it('formats elapsed milliseconds without changing existing labels', () => {
    expect(formatElapsedTime(999)).toBe('0s');
    expect(formatElapsedTime(12_000)).toBe('12s');
    expect(formatElapsedTime(60_000)).toBe('1m 0s');
    expect(formatElapsedTime(125_000)).toBe('2m 5s');
  });
});
