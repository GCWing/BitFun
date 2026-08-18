import { describe, expect, it } from 'vitest';
import {
  contentEndScrollTop,
  FLOWCHAT_ANIMATED_JUMP_MAX_VIEWPORTS,
  FLOWCHAT_AT_CONTENT_END_THRESHOLD_PX,
  isTailBlankMeasurable,
  isViewportAtTail,
  memorylessFollowState,
  nextTailFollowState,
  resolveAnimatedJumpBehavior,
  resolveTailDepartureCrossing,
  shouldResumeFollowAfterDeparture,
  tailHoldMaxGapPx,
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

describe('resolveTailDepartureCrossing', () => {
  it('keeps watching while the blank is still on screen', () => {
    expect(resolveTailDepartureCrossing({
      blankPx: 180,
      contentDeltaPx: 40,
      scrollDeltaPx: 0,
    })).toBe('watching');
  });

  it('reads content rising to meet a stationary reader as the tail catching up', () => {
    // The case the resume acts on: the reader has not moved, and the empty
    // space they were left looking at has just been filled.
    expect(resolveTailDepartureCrossing({
      blankPx: -12,
      contentDeltaPx: 190,
      scrollDeltaPx: 0,
    })).toBe('content-caught-up');
  });

  it('reads a reader climbing out past a still content end as reading history', () => {
    expect(resolveTailDepartureCrossing({
      blankPx: -240,
      contentDeltaPx: 0,
      scrollDeltaPx: -420,
    })).toBe('reader-left-blank');
  });

  it('gives a crossing where both moved to whichever moved further', () => {
    // Streaming does not stop because the reader scrolled, so both sides move
    // between samples and the tie-break decides. Callers record the two deltas
    // beside the verdict so this line can be revisited from the trail.
    expect(resolveTailDepartureCrossing({
      blankPx: -5,
      contentDeltaPx: 120,
      scrollDeltaPx: -40,
    })).toBe('content-caught-up');
    expect(resolveTailDepartureCrossing({
      blankPx: -5,
      contentDeltaPx: 40,
      scrollDeltaPx: -120,
    })).toBe('reader-left-blank');
  });

  it('counts the blank as gone the moment content reaches the bottom edge', () => {
    // `blankPx` is `scrollTop - contentEnd`, so zero is the content end exactly
    // on the viewport's bottom edge — no blank, and the departure is over.
    expect(resolveTailDepartureCrossing({
      blankPx: 0,
      contentDeltaPx: 30,
      scrollDeltaPx: 0,
    })).toBe('content-caught-up');
    expect(resolveTailDepartureCrossing({
      blankPx: 0.5,
      contentDeltaPx: 30,
      scrollDeltaPx: 0,
    })).toBe('watching');
  });

  it('does not read a shrinking transcript as the reader leaving', () => {
    // A tool card collapsing moves the content end *down*, which cannot end a
    // departure — and if it somehow coincides with a crossing, a negative
    // content delta must not out-vote the reader.
    expect(resolveTailDepartureCrossing({
      blankPx: -30,
      contentDeltaPx: -200,
      scrollDeltaPx: -60,
    })).toBe('reader-left-blank');
  });
});

describe('isTailBlankMeasurable', () => {
  it('accepts a blank the reserved spacer can account for', () => {
    expect(isTailBlankMeasurable({ blankPx: SPACER, tailSpacerPx: SPACER })).toBe(true);
    expect(isTailBlankMeasurable({ blankPx: -400, tailSpacerPx: SPACER })).toBe(true);
  });

  it('refuses a blank larger than the whole spacer, which cannot have been seen', () => {
    /*
     * `scrollTop` is clamped to `scrollHeight - clientHeight`, so a settled
     * viewport is never more than the spacer past the content end. Measured on
     * a session that paged its whole history on the first scroll up: the
     * prepend shifted the viewport 14252px to hold the reader's Turn still,
     * against a content end still reading 989 — a blank of 6239px in a
     * transcript reserving a few hundred.
     */
    expect(isTailBlankMeasurable({ blankPx: 6239, tailSpacerPx: SPACER })).toBe(false);
  });
});

describe('shouldResumeFollowAfterDeparture', () => {
  it('takes the viewport back when output caught up with a reader who had stopped', () => {
    expect(shouldResumeFollowAfterDeparture({
      crossing: 'content-caught-up',
      gestureLive: false,
    })).toBe(true);
  });

  it('leaves a reader who is still scrolling alone, whatever the geometry says', () => {
    /*
     * Measured, and the reason the gesture is an input at all: 320ms into a
     * live gesture the reader had climbed 200px while content grew 237, so the
     * tie-break called the crossing for the content — correctly — and acting on
     * it would have taken the viewport back mid-scroll.
     */
    expect(shouldResumeFollowAfterDeparture({
      crossing: 'content-caught-up',
      gestureLive: true,
    })).toBe(false);
  });

  it('never resumes for a reader who climbed out past the content end', () => {
    // Reading history is the case the whole departure exists to tell apart, and
    // a lapsed gesture claim does not make it something else.
    expect(shouldResumeFollowAfterDeparture({
      crossing: 'reader-left-blank',
      gestureLive: false,
    })).toBe(false);
  });

  it('decides nothing while the departure is still open', () => {
    expect(shouldResumeFollowAfterDeparture({
      crossing: 'watching',
      gestureLive: false,
    })).toBe(false);
  });
});
