import { describe, expect, it } from 'vitest';
import { formatCacheHitRate, formatTokenCount } from './tokenUsageFormatting';

const formatter = (locale: string) => (
  value: number,
  options?: Intl.NumberFormatOptions,
) => new Intl.NumberFormat(locale, options).format(value);

describe('token usage formatting', () => {
  it('uses fixed K/M/B token units with up to two decimal places', () => {
    expect(formatTokenCount(999, formatter('en-US'))).toBe('999');
    expect(formatTokenCount(1_234, formatter('en-US'))).toBe('1.23K');
    expect(formatTokenCount(999_999, formatter('en-US'))).toBe('1M');
    expect(formatTokenCount(5_396_217, formatter('en-US'))).toBe('5.4M');
    expect(formatTokenCount(1_000_000_000, formatter('en-US'))).toBe('1B');
    expect(formatTokenCount(12_345, formatter('zh-CN'))).toBe('12.35K');
    expect(formatTokenCount(1_000_000_000, formatter('zh-CN'))).toBe('1B');
  });

  it('rounds cache hit rates down to exactly two decimal places', () => {
    expect(formatCacheHitRate(0.95674, formatter('en-US'))).toBe('95.67%');
    expect(formatCacheHitRate(0.95679, formatter('en-US'))).toBe('95.67%');
    expect(formatCacheHitRate(1, formatter('en-US'))).toBe('100.00%');
  });
});
