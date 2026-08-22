import { describe, expect, it } from 'vitest';
import {
  contentEndScrollTop,
  FLOWCHAT_ANIMATED_JUMP_MAX_VIEWPORTS,
  FLOWCHAT_AT_CONTENT_END_THRESHOLD_PX,
  isTailBlankMeasurable,
  isViewportAtTail,
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
const MAX_GAP = tailHoldMaxGapPx(VIEWPORT, SPACER);
const THRESHOLD = FLOWCHAT_AT_CONTENT_END_THRESHOLD_PX;

function holding(target: number): TailFollowState {
  return { target };
}

describe('tailSpacerPxForViewport', () => {
  it('caps the footer and spacer at three quarters of the viewport', () => {
    const spacer = tailSpacerPxForViewport(VIEWPORT, BOTTOM_INSET);
    expect(BOTTOM_INSET + spacer).toBe(Math.round(VIEWPORT * 0.75));
    expect(VIEWPORT - BOTTOM_INSET - spacer).toBe(200);
  });

  it('lets an expanded footer consume the reservation without making spacer negative', () => {
    expect(tailSpacerPxForViewport(VIEWPORT, VIEWPORT - 80)).toBe(0);
  });

  it('stays well under a full viewport, so the scroll range does not end in blank', () => {
    expect(tailSpacerPxForViewport(VIEWPORT, BOTTOM_INSET)).toBeLessThan(VIEWPORT);
  });

  it('reserves nothing before the scroller has been measured', () => {
    expect(tailSpacerPxForViewport(0, BOTTOM_INSET)).toBe(0);
  });

  it('never lets the hold gap exceed the physical spacer', () => {
    expect(tailHoldMaxGapPx(VIEWPORT, 120)).toBe(120);
    expect(tailHoldMaxGapPx(VIEWPORT, SPACER)).toBe(SPACER);
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
      maxGapPx: MAX_GAP,
    });
    expect(next).toEqual({ target: 4200 });
  });

  it('holds its offset when a collapse fits the tolerated gap', () => {
    // A card above the live output collapses by 300px: the tail rises, but the
    // viewport must not move or earlier content would visually drop by 300px.
    const next = nextTailFollowState(holding(4000), {
      desiredScrollTop: 3700,
      maxGapPx: MAX_GAP,
    });
    expect(next.target).toBe(4000);
  });

  it('gives ground only past the tolerated gap, and only by the excess', () => {
    const next = nextTailFollowState(holding(4000), {
      desiredScrollTop: 1000,
      maxGapPx: MAX_GAP,
    });
    expect(next.target).toBe(1000 + MAX_GAP);
  });

  it('never drops below the content-end target', () => {
    const next = nextTailFollowState(holding(100), {
      desiredScrollTop: 900,
      maxGapPx: MAX_GAP,
    });
    expect(next.target).toBe(900);
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

  it('counts a reveal position owned by follow-output', () => {
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

  it('animates a nearby follow target coming back into place', () => {
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
    // A follow target can be above the viewport. Distance is absolute either way.
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
