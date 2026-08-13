import { describe, expect, it } from 'vitest';
import {
  contentEndScrollTop,
  FLOWCHAT_ANIMATED_JUMP_MAX_VIEWPORTS,
  FLOWCHAT_AT_CONTENT_END_THRESHOLD_PX,
  isViewportAtTail,
  memorylessFollowState,
  nextTailFollowState,
  resolveAnimatedJumpBehavior,
  tailHoldMaxGapPx,
  tailSnapBackScrollTop,
  tailSpacerPxForViewport,
  turnTopAlignmentEntersReservedBlank,
  type TailFollowState,
} from './flowChatTailFollow';

const VIEWPORT = 800;
const BOTTOM_INSET = 168;
const SPACER = tailSpacerPxForViewport(VIEWPORT, BOTTOM_INSET);
const MAX_GAP = tailHoldMaxGapPx(VIEWPORT);
const THRESHOLD = FLOWCHAT_AT_CONTENT_END_THRESHOLD_PX;

function holding(target: number): TailFollowState {
  return { mode: 'hold-tail', target };
}

function pinning(target: number): TailFollowState {
  return { mode: 'pin-turn-top', target };
}

describe('tailSpacerPxForViewport', () => {
  it('reserves exactly enough to put a bare new Turn on the top edge', () => {
    // Worst case for the pin: the user message is the newest item and nothing
    // has answered it, so the message, the input inset and the spacer are all
    // that lie below its top. One viewport of them is what the pin needs.
    const spacer = tailSpacerPxForViewport(VIEWPORT, BOTTOM_INSET);
    const smallestUserMessagePx = VIEWPORT - BOTTOM_INSET - spacer;
    expect(smallestUserMessagePx).toBeGreaterThan(0);
    expect(smallestUserMessagePx).toBeLessThan(64);
  });

  it('never reserves less than the gap the hold rule may be holding', () => {
    // A composer expanded most of the way up the viewport leaves the pin almost
    // nothing to reserve, but `hold-tail` still parks up to `tailHoldMaxGapPx`
    // past the content end — and an offset the browser clamps is an offset the
    // hold rule does not get to hold.
    expect(tailSpacerPxForViewport(VIEWPORT, VIEWPORT - 80))
      .toBe(tailHoldMaxGapPx(VIEWPORT));
  });

  it('stays well under a full viewport, so the scroll range does not end in blank', () => {
    expect(tailSpacerPxForViewport(VIEWPORT, BOTTOM_INSET)).toBeLessThan(VIEWPORT);
  });

  it('reserves nothing before the scroller has been measured', () => {
    expect(tailSpacerPxForViewport(0, BOTTOM_INSET)).toBe(0);
  });
});

describe('turnTopAlignmentEntersReservedBlank', () => {
  it('leaves a Turn with content below it top-aligned', () => {
    expect(turnTopAlignmentEntersReservedBlank({
      turnTopScrollTop: 400,
      contentEndScrollTop: 3000,
    })).toBe(false);
  });

  it('clamps a Turn whose top lies past the end of real content', () => {
    // Everything below it is the reserved blank, which follow-output holds for
    // output that is arriving. Nothing arrives under a navigated Turn.
    expect(turnTopAlignmentEntersReservedBlank({
      turnTopScrollTop: 3200,
      contentEndScrollTop: 3000,
    })).toBe(true);
  });

  it('asks nothing about which Turn it is', () => {
    // The last Turn of a long transcript top-aligns like any other; short and
    // long is the result of the comparison, not an input to it.
    expect(turnTopAlignmentEntersReservedBlank({
      turnTopScrollTop: 3000 - VIEWPORT,
      contentEndScrollTop: 3000,
    })).toBe(false);
  });

  it('treats the content end itself as still on the transcript', () => {
    expect(turnTopAlignmentEntersReservedBlank({
      turnTopScrollTop: 3000,
      contentEndScrollTop: 3000,
    })).toBe(false);
  });
});

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

describe('memorylessFollowState', () => {
  it('drops a held collapse gap the user has already scrolled away from', () => {
    // The hold rule's refusal to move backwards protects a viewport it has been
    // holding continuously. After a takeover there is nothing left to protect,
    // and reinstating the old offset would land on a position nobody chose.
    expect(memorylessFollowState('hold-tail', {
      desiredScrollTop: 3700,
      pinScrollTop: null,
      maxGapPx: MAX_GAP,
    }).target).toBe(3700);
  });

  it('still prefers a pinned Turn over the content end', () => {
    expect(memorylessFollowState('pin-turn-top', {
      desiredScrollTop: 4300,
      pinScrollTop: 5000,
      maxGapPx: MAX_GAP,
    })).toEqual({ mode: 'pin-turn-top', target: 5000 });
  });

  it('reports the crossover so a suspended pin can be retired', () => {
    // No frame loop runs while the user owns the viewport, so this is the only
    // place a pin whose answer outgrew the viewport can be noticed.
    expect(memorylessFollowState('pin-turn-top', {
      desiredScrollTop: 5200,
      pinScrollTop: 5000,
      maxGapPx: MAX_GAP,
    })).toEqual({ mode: 'hold-tail', target: 5200 });
  });
});

describe('tailSnapBackScrollTop', () => {
  it('returns the follow target when the viewport rests below it', () => {
    expect(tailSnapBackScrollTop({
      scrollTop: 4000 + SPACER,
      followTargetScrollTop: 4000,
      thresholdPx: THRESHOLD,
    })).toBe(4000);
  });

  it('ignores an overshoot inside the tolerance', () => {
    expect(tailSnapBackScrollTop({
      scrollTop: 4000 + THRESHOLD,
      followTargetScrollTop: 4000,
      thresholdPx: THRESHOLD,
    })).toBeNull();
  });

  it('never fires while the user reads history above the target', () => {
    // Reading upwards is the case the whole rule must stay away from: the
    // region below the target carries no content, the region above is the
    // transcript.
    expect(tailSnapBackScrollTop({
      scrollTop: 100,
      followTargetScrollTop: 4000,
      thresholdPx: THRESHOLD,
    })).toBeNull();
  });

  it('leaves a pinned Turn alone even though it sits past the content end', () => {
    // The pin *is* the target here, so the blank below it is not an overshoot.
    expect(tailSnapBackScrollTop({
      scrollTop: 5000,
      followTargetScrollTop: 5000,
      thresholdPx: THRESHOLD,
    })).toBeNull();
  });
});

describe('isViewportAtTail', () => {
  const contentEnd = 4000;

  it('counts the content end itself', () => {
    expect(isViewportAtTail({
      scrollTop: contentEnd,
      contentEndScrollTop: contentEnd,
      followTargetScrollTop: contentEnd,
      thresholdPx: THRESHOLD,
    })).toBe(true);
  });

  it('counts a pinned Turn, which is at the tail by its own rule', () => {
    expect(isViewportAtTail({
      scrollTop: 5000,
      contentEndScrollTop: contentEnd,
      followTargetScrollTop: 5000,
      thresholdPx: THRESHOLD,
    })).toBe(true);
  });

  it('excludes a viewport parked in the reserved blank', () => {
    // The one-sided test this replaced reported "at the bottom" here, which hid
    // the jump-to-latest affordance on a screen with nothing on it.
    expect(isViewportAtTail({
      scrollTop: contentEnd + SPACER,
      contentEndScrollTop: contentEnd,
      followTargetScrollTop: contentEnd,
      thresholdPx: THRESHOLD,
    })).toBe(false);
  });

  it('excludes a viewport scrolled up into the transcript', () => {
    expect(isViewportAtTail({
      scrollTop: 100,
      contentEndScrollTop: contentEnd,
      followTargetScrollTop: contentEnd,
      thresholdPx: THRESHOLD,
    })).toBe(false);
  });
});

describe('resolveAnimatedJumpBehavior', () => {
  const budget = VIEWPORT * FLOWCHAT_ANIMATED_JUMP_MAX_VIEWPORTS;

  it('animates a jump the reader can follow', () => {
    expect(resolveAnimatedJumpBehavior({
      fromPx: 4000,
      targetPx: 4000 + VIEWPORT,
      clientHeight: VIEWPORT,
    })).toBe('smooth');
  });

  it('animates a pinned Turn coming back into place, which is under a screen', () => {
    // The `pin-turn-top` branch of a jump to latest, where the newest Turn's
    // answer is shorter than the viewport by construction.
    expect(resolveAnimatedJumpBehavior({
      fromPx: 12_000,
      targetPx: 12_180,
      clientHeight: VIEWPORT,
    })).toBe('smooth');
  });

  it('lands a jump from the head of a long transcript outright', () => {
    /*
     * The measurement the budget comes from: a jump issued for 8717px animated
     * 5480 of them inside the yield and was finished by the follow loop in one
     * 3290px write. Reading it as an animation is the mistake — what the reader
     * saw was two thirds of a scroll and then a jump.
     */
    expect(resolveAnimatedJumpBehavior({
      fromPx: 0,
      targetPx: 8717,
      clientHeight: VIEWPORT,
    })).toBe('auto');
  });

  it('measures the distance in viewports, not pixels', () => {
    // The same travel, on a display tall enough to still show where it went.
    const travelPx = VIEWPORT * FLOWCHAT_ANIMATED_JUMP_MAX_VIEWPORTS + 400;
    expect(resolveAnimatedJumpBehavior({
      fromPx: 0,
      targetPx: travelPx,
      clientHeight: VIEWPORT,
    })).toBe('auto');
    expect(resolveAnimatedJumpBehavior({
      fromPx: 0,
      targetPx: travelPx,
      clientHeight: travelPx,
    })).toBe('smooth');
  });

  it('takes the budget itself as near enough', () => {
    expect(resolveAnimatedJumpBehavior({
      fromPx: 0,
      targetPx: budget,
      clientHeight: VIEWPORT,
    })).toBe('smooth');
    expect(resolveAnimatedJumpBehavior({
      fromPx: 0,
      targetPx: budget + 1,
      clientHeight: VIEWPORT,
    })).toBe('auto');
  });

  it('judges the distance travelled, whichever way it goes', () => {
    // A jump to latest normally scrolls down, but `pin-turn-top` can aim above
    // the viewport when a restored tail presentation arrives under a pin.
    expect(resolveAnimatedJumpBehavior({
      fromPx: 9000,
      targetPx: 9000 - budget - 1,
      clientHeight: VIEWPORT,
    })).toBe('auto');
  });

  it('does not animate against a scroller that has not been measured', () => {
    // No budget to scale and nothing on screen to follow. Every other reading
    // of a zero height makes this a *short* jump, which is the wrong one.
    expect(resolveAnimatedJumpBehavior({
      fromPx: 0,
      targetPx: 0,
      clientHeight: 0,
    })).toBe('auto');
  });
});
