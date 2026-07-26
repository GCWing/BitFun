import { describe, expect, it } from 'vitest';
import { computeFixedPopoverPositionInViewport } from './fixedPopoverViewport';

describe('computeFixedPopoverPositionInViewport', () => {
  it('uses the default bottom placement when it fits', () => {
    const position = computeFixedPopoverPositionInViewport(
      { left: 20, top: 100, bottom: 124 },
      200,
      180,
      { width: 800, height: 600 },
    );

    expect(position).toEqual({ left: 20, top: 130 });
  });

  it('flips above when the default bottom placement overflows', () => {
    const position = computeFixedPopoverPositionInViewport(
      { left: 700, top: 500, bottom: 524 },
      200,
      180,
      { width: 800, height: 600 },
    );

    expect(position).toEqual({ left: 592, top: 314 });
  });

  it('pins an oversized menu to the viewport padding', () => {
    const position = computeFixedPopoverPositionInViewport(
      { left: 140, top: 80, bottom: 104 },
      240,
      500,
      { width: 180, height: 400 },
    );

    expect(position).toEqual({ left: 8, top: 8 });
  });
});
