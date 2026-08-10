import { describe, expect, it } from 'vitest';
import {
  contentEndScrollTop,
  nextTailFollowState,
  tailHoldMaxGapPx,
  tailSpacerPxForViewport,
  type TailFollowState,
} from './flowChatTailFollow';

const VIEWPORT = 800;
const SPACER = tailSpacerPxForViewport(VIEWPORT);
const MAX_GAP = tailHoldMaxGapPx(VIEWPORT);

function holding(target: number): TailFollowState {
  return { mode: 'hold-tail', target };
}

function pinning(target: number): TailFollowState {
  return { mode: 'pin-turn-top', target };
}

describe('contentEndScrollTop', () => {
  it('excludes the resident tail spacer from the tail target', () => {
    expect(contentEndScrollTop({
      scrollHeight: 5000 + SPACER,
      clientHeight: VIEWPORT,
      tailSpacerPx: SPACER,
    })).toBe(5000 - VIEWPORT);
  });

  it('clamps to the top when the transcript is shorter than the viewport', () => {
    expect(contentEndScrollTop({
      scrollHeight: 300 + SPACER,
      clientHeight: VIEWPORT,
      tailSpacerPx: SPACER,
    })).toBe(0);
  });
});

describe('nextTailFollowState hold-tail', () => {
  it('follows content growth', () => {
    const next = nextTailFollowState(holding(4000), {
      desiredScrollTop: 4200,
      pinScrollTop: null,
      maxGapPx: MAX_GAP,
    });
    expect(next).toEqual({ mode: 'hold-tail', target: 4200 });
  });

  it('holds its offset when a collapse fits the tolerated gap', () => {
    // A card above the live output collapses by 300px: the tail rises, but the
    // viewport must not move or earlier content would visually drop by 300px.
    const next = nextTailFollowState(holding(4000), {
      desiredScrollTop: 3700,
      pinScrollTop: null,
      maxGapPx: MAX_GAP,
    });
    expect(next.target).toBe(4000);
  });

  it('gives ground only past the tolerated gap, and only by the excess', () => {
    const next = nextTailFollowState(holding(4000), {
      desiredScrollTop: 1000,
      pinScrollTop: null,
      maxGapPx: MAX_GAP,
    });
    expect(next.target).toBe(1000 + MAX_GAP);
  });

  it('never drops below the content-end target', () => {
    const next = nextTailFollowState(holding(100), {
      desiredScrollTop: 900,
      pinScrollTop: null,
      maxGapPx: MAX_GAP,
    });
    expect(next.target).toBe(900);
  });
});

describe('nextTailFollowState pin-turn-top', () => {
  it('holds a new Turn at the viewport top while its answer is short', () => {
    const next = nextTailFollowState(pinning(0), {
      desiredScrollTop: 4300,
      pinScrollTop: 5000,
      maxGapPx: MAX_GAP,
    });
    expect(next).toEqual({ mode: 'pin-turn-top', target: 5000 });
  });

  it('ignores the gap tolerance while pinned', () => {
    // The blank below a freshly submitted Turn is the point of the mode.
    const next = nextTailFollowState(pinning(5000), {
      desiredScrollTop: 4300,
      pinScrollTop: 5000,
      maxGapPx: 10,
    });
    expect(next.target).toBe(5000);
  });

  it('stays put while a collapse shrinks content under the pin', () => {
    const first = nextTailFollowState(pinning(0), {
      desiredScrollTop: 4400,
      pinScrollTop: 5000,
      maxGapPx: MAX_GAP,
    });
    const afterCollapse = nextTailFollowState(first, {
      desiredScrollTop: 4100,
      pinScrollTop: 5000,
      maxGapPx: MAX_GAP,
    });
    expect(afterCollapse.target).toBe(5000);
  });

  it('hands off to hold-tail once the answer overflows the viewport', () => {
    const next = nextTailFollowState(pinning(5000), {
      desiredScrollTop: 5000,
      pinScrollTop: 5000,
      maxGapPx: MAX_GAP,
    });
    expect(next).toEqual({ mode: 'hold-tail', target: 5000 });
  });

  it('does not regress after handing off', () => {
    const handoff = nextTailFollowState(pinning(5000), {
      desiredScrollTop: 5200,
      pinScrollTop: 5000,
      maxGapPx: MAX_GAP,
    });
    expect(handoff.mode).toBe('hold-tail');
    const afterCollapse = nextTailFollowState(handoff, {
      desiredScrollTop: 5000,
      pinScrollTop: 5000,
      maxGapPx: MAX_GAP,
    });
    expect(afterCollapse.target).toBe(5200);
  });

  it('falls back to the tail target until the Turn can be measured', () => {
    const next = nextTailFollowState(pinning(0), {
      desiredScrollTop: 4300,
      pinScrollTop: null,
      maxGapPx: MAX_GAP,
    });
    expect(next).toEqual({ mode: 'pin-turn-top', target: 4300 });
  });
});
