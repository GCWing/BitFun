import { describe, expect, it } from 'vitest';
import {
  ANCHOR_SETTLE_FRAMES,
  USER_DRIVEN_SCROLL_WINDOW_MS,
  selectViewportAnchor,
  shouldCaptureViewportAnchorOnScroll,
  viewportAnchorCorrectionPx,
  type ViewportAnchorCandidate,
} from './flowChatViewportAnchor';

function candidate(
  turnId: string | null,
  top: number,
  height = 40,
): ViewportAnchorCandidate {
  return {
    turnId,
    offsetFromScrollerTop: top,
    bottomOffsetFromScrollerTop: top + height,
  };
}

const VIEWPORT_HEIGHT = 800;

describe('selectViewportAnchor', () => {
  it('anchors to the topmost Turn the viewport still shows', () => {
    expect(selectViewportAnchor([
      candidate('turn-1', -900),
      // Scrolled past the top edge, but its Turn still fills the screen.
      candidate('turn-2', -120, 300),
      candidate('turn-3', 260),
    ], VIEWPORT_HEIGHT)).toEqual({ turnId: 'turn-2', offsetFromScrollerTop: -120 });
  });

  it('keeps a Turn that is scrolled past the top edge but still on screen', () => {
    // Its top is above the viewport and its bottom is not: this is what the
    // reader is looking at, and the offset that describes it is negative.
    expect(selectViewportAnchor([candidate('turn-1', -30)], VIEWPORT_HEIGHT))
      .toEqual({ turnId: 'turn-1', offsetFromScrollerTop: -30 });
  });

  it('has no anchor when every Turn is above the viewport', () => {
    expect(selectViewportAnchor([
      candidate('turn-1', -900),
      candidate('turn-2', -400, 300),
    ], VIEWPORT_HEIGHT)).toBeNull();
  });

  it('has no anchor when every Turn is below the viewport', () => {
    /*
     * The reader is inside an answer taller than the screen, so the Turn they
     * are reading has no marker on it and the next one is far past the bottom
     * edge. Measured: an anchor taken at an offset of 1695.5px in a scroller at
     * most 1325.7px tall — at least 370px out of sight.
     *
     * Anchoring there inverts what the anchor is for. Content below the
     * viewport re-measuring moves nothing the reader can see, so every
     * correction against a marker down there is a displacement being created
     * rather than repaired.
     */
    expect(selectViewportAnchor([
      candidate('turn-1', -1_700),
      candidate('turn-2', 1_695.5),
    ], VIEWPORT_HEIGHT)).toBeNull();
  });

  it('has no anchor when the topmost visible candidate carries no Turn', () => {
    // Falling through to the next one would anchor to a Turn further down the
    // screen than the reading position belongs to.
    expect(selectViewportAnchor([
      candidate(null, -30),
      candidate('turn-2', 260),
    ], VIEWPORT_HEIGHT)).toBeNull();
  });

  it('anchors to the last Turn on screen when it is the only one', () => {
    // The bottom edge is a bound, not a preference: a marker just inside it is
    // still something the reader can see.
    expect(selectViewportAnchor(
      [candidate('turn-9', VIEWPORT_HEIGHT - 1)],
      VIEWPORT_HEIGHT,
    )).toEqual({ turnId: 'turn-9', offsetFromScrollerTop: VIEWPORT_HEIGHT - 1 });
  });

  it('has no anchor when nothing is rendered', () => {
    expect(selectViewportAnchor([], VIEWPORT_HEIGHT)).toBeNull();
  });
});

describe('viewportAnchorCorrectionPx', () => {
  it('reports the drift as the amount scrollTop has to grow', () => {
    const anchor = { turnId: 'turn-1', offsetFromScrollerTop: -120 };
    // The Turn ended up 880px further down the screen than it was.
    expect(viewportAnchorCorrectionPx(anchor, 760)).toBe(880);
  });

  it('is negative when the transcript moved up under the reader', () => {
    const anchor = { turnId: 'turn-1', offsetFromScrollerTop: 200 };
    expect(viewportAnchorCorrectionPx(anchor, -60)).toBe(-260);
  });

  it('is zero when the Turn is where it was', () => {
    const anchor = { turnId: 'turn-1', offsetFromScrollerTop: 200 };
    expect(viewportAnchorCorrectionPx(anchor, 200)).toBe(0);
  });
});

describe('shouldCaptureViewportAnchorOnScroll', () => {
  it('captures a scroll the user drove', () => {
    expect(shouldCaptureViewportAnchorOnScroll(true, 40)).toBe(true);
  });

  it('ignores a scroll nobody asked for', () => {
    // The transcript moved under the reader. Re-anchoring here would adopt the
    // displacement as the new reading position.
    expect(shouldCaptureViewportAnchorOnScroll(true, USER_DRIVEN_SCROLL_WINDOW_MS + 1))
      .toBe(false);
  });

  it('captures without intent when there is no anchor yet', () => {
    expect(shouldCaptureViewportAnchorOnScroll(false, 10_000)).toBe(true);
  });
});

describe('the anchor constants', () => {
  it('holds the anchor over more than a couple of frames', () => {
    // A prepend settles through a margin, then padding, then a margin release,
    // and no single callback covers all of it.
    expect(ANCHOR_SETTLE_FRAMES).toBeGreaterThan(2);
  });

  it('keeps the intent window shorter than a deliberate pause', () => {
    expect(USER_DRIVEN_SCROLL_WINDOW_MS).toBeLessThan(500);
  });
});
