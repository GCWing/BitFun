// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  nextEasedScrollTopPx,
  shouldEaseTailFollow,
  TAIL_EASE_ALPHA,
  TAIL_EASE_LANDED_PX,
  TAIL_EASE_LINE_PX,
  TAIL_EASE_SNAP_ABOVE_PX,
} from './flowChatTailEase';

describe('nextEasedScrollTopPx', () => {
  it('covers a fixed fraction of what is left', () => {
    const step = nextEasedScrollTopPx(100, 180);

    expect(step.outcome).toBe('eased');
    expect(step.offsetPx).toBeCloseTo(100 + 80 * TAIL_EASE_ALPHA, 6);
  });

  it('converges rather than overshooting', () => {
    let offsetPx = 0;
    for (let frame = 0; frame < 60; frame += 1) {
      const step = nextEasedScrollTopPx(offsetPx, 100);
      expect(step.offsetPx).toBeLessThanOrEqual(100);
      offsetPx = step.offsetPx;
    }

    expect(offsetPx).toBe(100);
  });

  it('writes out the last fraction of a pixel instead of halving it forever', () => {
    const step = nextEasedScrollTopPx(100 - TAIL_EASE_LANDED_PX, 100);

    expect(step).toEqual({ offsetPx: 100, outcome: 'landed' });
  });

  it('reports having landed when there is nowhere to go', () => {
    expect(nextEasedScrollTopPx(100, 100)).toEqual({ offsetPx: 100, outcome: 'landed' });
  });

  it('jumps rather than animating through content nobody saw', () => {
    const step = nextEasedScrollTopPx(0, TAIL_EASE_SNAP_ABOVE_PX + 1);

    expect(step).toEqual({ offsetPx: TAIL_EASE_SNAP_ABOVE_PX + 1, outcome: 'snapped' });
  });

  it('eases right up to the snap threshold', () => {
    expect(nextEasedScrollTopPx(0, TAIL_EASE_SNAP_ABOVE_PX).outcome).toBe('eased');
  });

  it('never takes an eased step bigger than the line it exists to break up', () => {
    // The threshold is derived from this: past it the opening step of an ease
    // is larger than the jump it replaces, so the follow jumps instead.
    const worstEasedStepPx = nextEasedScrollTopPx(0, TAIL_EASE_SNAP_ABOVE_PX).offsetPx;

    expect(worstEasedStepPx).toBeLessThanOrEqual(TAIL_EASE_LINE_PX);
  });

  it('goes straight to a target above it, because a shrink eased reads as a claw-back', () => {
    const step = nextEasedScrollTopPx(400, 320);

    expect(step).toEqual({ offsetPx: 320, outcome: 'landed' });
  });

  it('settles on the growth per frame under steady growth, whatever alpha is', () => {
    // The point of the ease is even motion, not slower motion: at equilibrium
    // it moves exactly as fast as the content grows.
    const growthPerFramePx = 8;
    let offsetPx = 0;
    let targetPx = 0;
    let lastStepPx = 0;
    for (let frame = 0; frame < 200; frame += 1) {
      targetPx += growthPerFramePx;
      const step = nextEasedScrollTopPx(offsetPx, targetPx);
      lastStepPx = step.offsetPx - offsetPx;
      offsetPx = step.offsetPx;
    }

    expect(lastStepPx).toBeCloseTo(growthPerFramePx, 3);
    // And the price of that evenness is riding a fixed distance behind: the
    // frame's own growth is already in the target when the ease acts on it.
    expect(targetPx - offsetPx).toBeCloseTo(
      growthPerFramePx * (1 - TAIL_EASE_ALPHA) / TAIL_EASE_ALPHA,
      3,
    );
  });
});

describe('shouldEaseTailFollow', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  function stubReducedMotion(matches: boolean): void {
    vi.stubGlobal('matchMedia', vi.fn(() => ({ matches })));
  }

  it('refuses while the box is still growing, where a smooth follow would cost a re-measure', () => {
    stubReducedMotion(false);

    expect(shouldEaseTailFollow({ scrollHeightPx: 200, clientHeightPx: 300 })).toBe(false);
    expect(shouldEaseTailFollow({ scrollHeightPx: 300, clientHeightPx: 300 })).toBe(false);
  });

  it('eases once the box overflows and there is an offset to move', () => {
    stubReducedMotion(false);

    expect(shouldEaseTailFollow({ scrollHeightPx: 301, clientHeightPx: 300 })).toBe(true);
  });

  it('refuses when the reader asked for less motion', () => {
    stubReducedMotion(true);

    expect(shouldEaseTailFollow({ scrollHeightPx: 900, clientHeightPx: 300 })).toBe(false);
  });

  it('eases where matchMedia is unavailable rather than treating that as a preference', () => {
    vi.stubGlobal('matchMedia', undefined);

    expect(shouldEaseTailFollow({ scrollHeightPx: 900, clientHeightPx: 300 })).toBe(true);
  });
});
