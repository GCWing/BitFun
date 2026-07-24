import { describe, expect, it } from 'vitest';
import {
  TYPEWRITER_BASE_CHARS_PER_SEC,
  TYPEWRITER_FINISH_CHARS_PER_SEC,
  TYPEWRITER_FINISH_MAX_CHARS_PER_PAINT,
  TYPEWRITER_MAX_CHARS_PER_PAINT,
  TYPEWRITER_MAX_CHARS_PER_SEC,
  commitTypewriterReveal,
  computeTypewriterCharsPerSec,
  safeGraphemeRevealEnd,
} from './useTypewriter';

describe('computeTypewriterCharsPerSec', () => {
  it('starts near the base rate with an empty backlog', () => {
    expect(computeTypewriterCharsPerSec(0)).toBe(TYPEWRITER_BASE_CHARS_PER_SEC);
  });

  it('soft-accelerates as backlog grows', () => {
    const small = computeTypewriterCharsPerSec(10);
    const large = computeTypewriterCharsPerSec(100);
    expect(small).toBeGreaterThan(TYPEWRITER_BASE_CHARS_PER_SEC);
    expect(large).toBeGreaterThan(small);
  });

  it('never exceeds the live ceiling while streaming', () => {
    expect(computeTypewriterCharsPerSec(10_000)).toBe(TYPEWRITER_MAX_CHARS_PER_SEC);
  });

  it('uses the absolute finish rate once the model stream ends', () => {
    expect(computeTypewriterCharsPerSec({ backlog: 20, finishing: true }))
      .toBe(TYPEWRITER_FINISH_CHARS_PER_SEC);
    expect(computeTypewriterCharsPerSec({ backlog: 1, finishing: true }))
      .toBe(TYPEWRITER_FINISH_CHARS_PER_SEC);
    expect(TYPEWRITER_FINISH_CHARS_PER_SEC).toBeGreaterThan(TYPEWRITER_MAX_CHARS_PER_SEC);
  });
});

describe('commitTypewriterReveal', () => {
  it('does not reveal more than the per-paint cap', () => {
    const result = commitTypewriterReveal({
      backlog: 200,
      fractionalCarry: 50,
    });
    expect(result.chars).toBe(TYPEWRITER_MAX_CHARS_PER_PAINT);
    expect(result.fractionalCarry).toBe(50 - TYPEWRITER_MAX_CHARS_PER_PAINT);
  });

  it('honors a higher finish-mode paint cap', () => {
    const result = commitTypewriterReveal({
      backlog: 200,
      fractionalCarry: 80,
      maxCharsPerPaint: TYPEWRITER_FINISH_MAX_CHARS_PER_PAINT,
    });
    expect(result.chars).toBe(TYPEWRITER_FINISH_MAX_CHARS_PER_PAINT);
  });

  it('does not reveal more than the backlog', () => {
    const result = commitTypewriterReveal({
      backlog: 2,
      fractionalCarry: 10,
    });
    expect(result.chars).toBe(2);
    expect(result.fractionalCarry).toBe(8);
  });

  it('keeps fractional remainder below one character', () => {
    const result = commitTypewriterReveal({
      backlog: 20,
      fractionalCarry: 2.75,
    });
    expect(result.chars).toBe(2);
    expect(result.fractionalCarry).toBeCloseTo(0.75);
  });

  it('returns zero when there is nothing to reveal', () => {
    expect(commitTypewriterReveal({ backlog: 0, fractionalCarry: 4 })).toEqual({
      chars: 0,
      fractionalCarry: 0,
    });
  });
});

describe('safeGraphemeRevealEnd', () => {
  it('never exposes half a surrogate pair', () => {
    expect(safeGraphemeRevealEnd('A😀B', 2)).toBe(3);
  });

  it('keeps combining and ZWJ emoji sequences together', () => {
    expect(safeGraphemeRevealEnd('e\u0301!', 1)).toBe(2);
    const family = '👨‍👩‍👧‍👦';
    expect(safeGraphemeRevealEnd(`${family}!`, 1)).toBe(family.length);
  });

  it('clamps offsets to the text bounds', () => {
    expect(safeGraphemeRevealEnd('abc', -4)).toBe(0);
    expect(safeGraphemeRevealEnd('abc', 100)).toBe(3);
  });

  it('can segment from a previously known boundary for long streams', () => {
    const prefix = 'a'.repeat(10_000);
    expect(safeGraphemeRevealEnd(`${prefix}😀!`, prefix.length + 1, prefix.length))
      .toBe(prefix.length + 2);
  });
});
