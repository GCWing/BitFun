// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { tailSpacerPxForViewport } from './flowChatTailFollow';
import { useFlowChatFollowOutput } from './useFlowChatFollowOutput';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

type Controller = ReturnType<typeof useFlowChatFollowOutput>;

const VIEWPORT = 500;
/** Matches `tailSpacerPxForViewport(VIEWPORT)`. */
const TAIL_SPACER = 500;
/** Matches `tailHoldMaxGapPx(VIEWPORT)`. */
const MAX_GAP = 300;

function setScrollerMetrics(
  scroller: HTMLElement,
  metrics: { scrollHeight: number; clientHeight: number; scrollTop: number },
) {
  Object.defineProperties(scroller, {
    scrollHeight: { configurable: true, value: metrics.scrollHeight },
    clientHeight: { configurable: true, value: metrics.clientHeight },
    scrollTop: { configurable: true, writable: true, value: metrics.scrollTop },
  });
}

interface HarnessProps {
  latestTurnId: string;
  isStreaming?: boolean;
  scroller: HTMLElement;
  scrollToContentEnd?: (behavior: ScrollBehavior) => void;
  scrollTurnToTop?: (turnId: string) => boolean;
  resolveTurnTopScrollTop?: (turnId: string) => number | null;
  isOpeningViewport?: boolean;
  onController: (controller: Controller) => void;
}

function Harness({
  latestTurnId,
  isStreaming = true,
  scroller,
  scrollToContentEnd = () => {},
  scrollTurnToTop = () => false,
  resolveTurnTopScrollTop = () => null,
  isOpeningViewport = false,
  onController,
}: HarnessProps) {
  const scrollerRef = React.useRef<HTMLElement | null>(scroller);
  const controller = useFlowChatFollowOutput({
    activeSessionId: 'session-1',
    latestTurnId,
    virtualItemCount: 2,
    isStreaming,
    isViewportActive: true,
    scrollerRef,
    // The spacer tracks the viewport, exactly as the component's state does.
    getTailSpacerPx: () => tailSpacerPxForViewport(scroller.clientHeight),
    scrollToContentEnd,
    scrollTurnToTop,
    resolveTurnTopScrollTop,
    isOpeningViewport: () => isOpeningViewport,
  });
  onController(controller);
  return <div data-following={String(controller.isFollowingOutput)} />;
}

describe('useFlowChatFollowOutput', () => {
  let container: HTMLDivElement;
  let root: Root;
  let scroller: HTMLDivElement;
  let controller: Controller | null;
  let frames: FrameRequestCallback[];
  let scrollTo: ReturnType<typeof vi.fn>;

  function runNextFrame() {
    const frame = frames.shift();
    expect(frame).toBeDefined();
    act(() => frame?.(16));
  }

  beforeEach(() => {
    container = document.createElement('div');
    scroller = document.createElement('div');
    document.body.append(container, scroller);
    // jsdom has no layout engine, so the animated scroll is a no-op here and
    // the tests place the viewport where the animation would have left it.
    scrollTo = vi.fn();
    scroller.scrollTo = scrollTo as unknown as HTMLElement['scrollTo'];
    root = createRoot(container);
    controller = null;
    frames = [];
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      frames.push(callback);
      return frames.length;
    }));
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    scroller.remove();
    vi.unstubAllGlobals();
  });

  it('opens a newly submitted Turn at the viewport top instead of the tail', () => {
    const scrollTurnToTop = vi.fn(() => true);
    const scrollToContentEnd = vi.fn();
    const props = {
      scroller,
      scrollToContentEnd,
      scrollTurnToTop,
      onController: (next: Controller) => { controller = next; },
    };

    act(() => {
      root.render(<Harness {...props} latestTurnId="turn-1" isStreaming={false} />);
    });
    // Mount settles on the content end; only the *new Turn* must not.
    expect(scrollToContentEnd).toHaveBeenCalledWith('auto');
    scrollToContentEnd.mockClear();

    act(() => {
      root.render(<Harness {...props} latestTurnId="turn-2" isStreaming={false} />);
    });

    expect(scrollTurnToTop).toHaveBeenCalledWith('turn-2');
    expect(scrollToContentEnd).not.toHaveBeenCalled();
    expect(controller?.isFollowingOutput).toBe(true);
  });

  it('settles on the content end when a session opens without streaming', () => {
    const scrollToContentEnd = vi.fn();
    act(() => {
      root.render(
        <Harness
          latestTurnId="turn-1"
          isStreaming={false}
          scroller={scroller}
          scrollToContentEnd={scrollToContentEnd}
          onController={next => { controller = next; }}
        />,
      );
    });

    expect(scrollToContentEnd).toHaveBeenCalledWith('auto');
    expect(controller?.isFollowingOutput).toBe(true);
  });

  it('tracks the content end exactly while the transcript is still opening', () => {
    // Virtuoso compensates a history prepend by writing scrollTop before the
    // prepended heights reach the DOM. While opening, the transcript is hidden
    // and we are authoritative — accommodating that write would be invisible
    // now and permanent once paging stops.
    setScrollerMetrics(scroller, {
      scrollHeight: 1500 + TAIL_SPACER,
      clientHeight: VIEWPORT,
      scrollTop: 0,
    });
    act(() => {
      root.render(
        <Harness
          latestTurnId="turn-1"
          scroller={scroller}
          isOpeningViewport
          onController={next => { controller = next; }}
        />,
      );
    });
    runNextFrame();
    expect(scroller.scrollTop).toBe(1000);

    scroller.scrollTop = 1000 + 380;
    runNextFrame();
    expect(scroller.scrollTop).toBe(1000);
  });

  it('does not strand the viewport inside the tail spacer after opening', () => {
    // Regression: the gap tolerance is a streaming allowance. Applied to a
    // foreign forward move it parked the content end mid-viewport forever,
    // because nothing pulls the target back down once paging stops.
    setScrollerMetrics(scroller, {
      scrollHeight: 1500 + TAIL_SPACER,
      clientHeight: VIEWPORT,
      scrollTop: 0,
    });
    act(() => {
      root.render(
        <Harness
          latestTurnId="turn-1"
          scroller={scroller}
          isOpeningViewport
          onController={next => { controller = next; }}
        />,
      );
    });
    runNextFrame();

    scroller.scrollTop = 1000 + 380;
    runNextFrame();
    expect(scroller.scrollTop).toBe(1000);
    expect(1000).toBeLessThan(1000 + MAX_GAP);
  });

  it('falls back to the content end when the new Turn cannot be targeted', () => {
    const scrollToContentEnd = vi.fn();
    const props = {
      scroller,
      scrollToContentEnd,
      scrollTurnToTop: () => false,
      onController: (next: Controller) => { controller = next; },
    };

    act(() => {
      root.render(<Harness {...props} latestTurnId="turn-1" isStreaming={false} />);
    });
    act(() => {
      root.render(<Harness {...props} latestTurnId="turn-2" isStreaming={false} />);
    });

    expect(scrollToContentEnd).toHaveBeenCalledWith('auto');
    expect(controller?.isFollowingOutput).toBe(true);
  });

  it('holds the pinned Turn at the top while its answer is shorter than the viewport', () => {
    // Real content ends at 1200, so the tail target is 700 — well above the
    // pinned Turn at 900. The pin must win until the answer overflows.
    setScrollerMetrics(scroller, {
      scrollHeight: 1200 + TAIL_SPACER,
      clientHeight: VIEWPORT,
      scrollTop: 900,
    });
    const props = {
      scroller,
      scrollTurnToTop: () => true,
      resolveTurnTopScrollTop: () => 900,
      onController: (next: Controller) => { controller = next; },
    };

    act(() => {
      root.render(<Harness {...props} latestTurnId="turn-1" />);
    });
    act(() => {
      root.render(<Harness {...props} latestTurnId="turn-2" />);
    });

    scroller.scrollTop = 0;
    runNextFrame();
    expect(scroller.scrollTop).toBe(900);
  });

  it('hands the pinned Turn off to tail follow once the answer overflows', () => {
    setScrollerMetrics(scroller, {
      scrollHeight: 2000 + TAIL_SPACER,
      clientHeight: VIEWPORT,
      scrollTop: 900,
    });
    const props = {
      scroller,
      scrollTurnToTop: () => true,
      resolveTurnTopScrollTop: () => 900,
      onController: (next: Controller) => { controller = next; },
    };

    act(() => {
      root.render(<Harness {...props} latestTurnId="turn-1" />);
    });
    act(() => {
      root.render(<Harness {...props} latestTurnId="turn-2" />);
    });

    runNextFrame();
    // Content end (2000 - 500) has overtaken the pin, so the tail owns it.
    expect(scroller.scrollTop).toBe(1500);
  });

  it('follows content growth against the content end, not the tail spacer', () => {
    setScrollerMetrics(scroller, {
      scrollHeight: 1500 + TAIL_SPACER,
      clientHeight: VIEWPORT,
      scrollTop: 1000,
    });

    act(() => {
      root.render(
        <Harness
          latestTurnId="turn-1"
          scroller={scroller}
          onController={next => { controller = next; }}
        />,
      );
    });
    expect(controller?.isFollowingOutput).toBe(true);

    setScrollerMetrics(scroller, {
      scrollHeight: 1800 + TAIL_SPACER,
      clientHeight: VIEWPORT,
      scrollTop: 1000,
    });
    runNextFrame();

    expect(scroller.scrollTop).toBe(1300);
  });

  it('holds its offset when a collapse shrinks content under the viewport', () => {
    // The regression this whole mechanism exists for: a tool card collapsing
    // above the live output must not drag earlier content down.
    setScrollerMetrics(scroller, {
      scrollHeight: 1500 + TAIL_SPACER,
      clientHeight: VIEWPORT,
      scrollTop: 0,
    });
    act(() => {
      root.render(
        <Harness
          latestTurnId="turn-1"
          scroller={scroller}
          onController={next => { controller = next; }}
        />,
      );
    });
    runNextFrame();
    expect(scroller.scrollTop).toBe(1000);

    setScrollerMetrics(scroller, {
      scrollHeight: 1200 + TAIL_SPACER,
      clientHeight: VIEWPORT,
      scrollTop: 1000,
    });
    runNextFrame();

    expect(scroller.scrollTop).toBe(1000);
  });

  it('gives ground only past the tolerated gap after a very large collapse', () => {
    setScrollerMetrics(scroller, {
      scrollHeight: 1500 + TAIL_SPACER,
      clientHeight: VIEWPORT,
      scrollTop: 0,
    });
    act(() => {
      root.render(
        <Harness
          latestTurnId="turn-1"
          scroller={scroller}
          onController={next => { controller = next; }}
        />,
      );
    });
    runNextFrame();

    setScrollerMetrics(scroller, {
      scrollHeight: 700 + TAIL_SPACER,
      clientHeight: VIEWPORT,
      scrollTop: 1000,
    });
    runNextFrame();

    expect(scroller.scrollTop).toBe(200 + MAX_GAP);
  });

  it('does not force the content end when a resize re-asserts follow', () => {
    const scrollToContentEnd = vi.fn();
    setScrollerMetrics(scroller, {
      scrollHeight: 1500 + TAIL_SPACER,
      clientHeight: VIEWPORT,
      scrollTop: 0,
    });
    act(() => {
      root.render(
        <Harness
          latestTurnId="turn-1"
          scroller={scroller}
          scrollToContentEnd={scrollToContentEnd}
          onController={next => { controller = next; }}
        />,
      );
    });
    runNextFrame();
    scrollToContentEnd.mockClear();

    setScrollerMetrics(scroller, {
      scrollHeight: 1200 + TAIL_SPACER,
      clientHeight: VIEWPORT,
      scrollTop: 1000,
    });
    act(() => controller?.scheduleFollowToLatest());

    expect(scrollToContentEnd).not.toHaveBeenCalled();
    expect(scroller.scrollTop).toBe(1000);
  });

  it('settles the held blank once streaming stops', () => {
    const scrollToContentEnd = vi.fn();
    const props = {
      scroller,
      scrollToContentEnd,
      onController: (next: Controller) => { controller = next; },
    };
    setScrollerMetrics(scroller, {
      scrollHeight: 1500 + TAIL_SPACER,
      clientHeight: VIEWPORT,
      scrollTop: 0,
    });
    act(() => {
      root.render(<Harness {...props} latestTurnId="turn-1" />);
    });
    runNextFrame();

    setScrollerMetrics(scroller, {
      scrollHeight: 1200 + TAIL_SPACER,
      clientHeight: VIEWPORT,
      scrollTop: 1000,
    });
    runNextFrame();
    scrollToContentEnd.mockClear();

    act(() => {
      root.render(<Harness {...props} latestTurnId="turn-1" isStreaming={false} />);
    });

    expect(scrollToContentEnd).toHaveBeenCalledWith('smooth');
  });

  it('lets explicit user scroll intent exit follow immediately', () => {
    act(() => {
      root.render(
        <Harness
          latestTurnId="turn-1"
          scroller={scroller}
          onController={next => { controller = next; }}
        />,
      );
    });
    act(() => controller?.enterFollowOutput('jump-to-latest'));
    expect(controller?.isFollowingOutput).toBe(true);

    act(() => controller?.handleUserScrollIntent());
    expect(controller?.isFollowingOutput).toBe(false);
    expect(cancelAnimationFrame).toHaveBeenCalled();
  });

  it('uses smooth behavior only for an explicit jump to latest', () => {
    const scrollToContentEnd = vi.fn();
    act(() => {
      root.render(
        <Harness
          latestTurnId="turn-1"
          scroller={scroller}
          scrollToContentEnd={scrollToContentEnd}
          onController={next => { controller = next; }}
        />,
      );
    });
    act(() => controller?.enterFollowOutput('jump-to-latest'));
    expect(scrollToContentEnd).toHaveBeenCalledWith('smooth');
  });

  it('yields the frame loop to its own animated scroll instead of overwriting it', () => {
    // The loop assigns scrollTop outright, which cancels an in-flight smooth
    // scroll on the very next frame: `jump-to-latest` asked for an animation
    // and got a jump.
    setScrollerMetrics(scroller, {
      scrollHeight: 1500 + TAIL_SPACER,
      clientHeight: VIEWPORT,
      scrollTop: 0,
    });
    act(() => {
      root.render(
        <Harness
          latestTurnId="turn-1"
          scroller={scroller}
          onController={next => { controller = next; }}
        />,
      );
    });
    runNextFrame();
    expect(scroller.scrollTop).toBe(1000);

    scroller.scrollTop = 0;
    act(() => controller?.enterFollowOutput('jump-to-latest'));
    runNextFrame();

    expect(scroller.scrollTop).toBe(0);
  });

  describe('jumping to latest while the newest Turn is pinned', () => {
    /** Pins `turn-2` at 900 with real content ending at 1200 (tail target 700). */
    function pinLatestTurn(overrides?: { resolveTurnTopScrollTop?: () => number | null }) {
      setScrollerMetrics(scroller, {
        scrollHeight: 1200 + TAIL_SPACER,
        clientHeight: VIEWPORT,
        scrollTop: 900,
      });
      const props = {
        scroller,
        scrollTurnToTop: () => true,
        resolveTurnTopScrollTop: overrides?.resolveTurnTopScrollTop ?? (() => 900),
        onController: (next: Controller) => { controller = next; },
      };
      act(() => {
        root.render(<Harness {...props} latestTurnId="turn-1" isStreaming={false} />);
      });
      act(() => {
        root.render(<Harness {...props} latestTurnId="turn-2" isStreaming={false} />);
      });
    }

    it('returns to the pin rather than the end of content', () => {
      // Restoring the tail presentation asks for a jump to latest one frame
      // after the Turn that caused it got pinned, which used to overwrite the
      // pin. Aiming at the content end here also scrolls *up*, shoving the
      // message the user just sent into the middle of the viewport.
      const scrollToContentEnd = vi.fn();
      setScrollerMetrics(scroller, {
        scrollHeight: 1200 + TAIL_SPACER,
        clientHeight: VIEWPORT,
        scrollTop: 900,
      });
      const props = {
        scroller,
        scrollToContentEnd,
        scrollTurnToTop: () => true,
        resolveTurnTopScrollTop: () => 900,
        onController: (next: Controller) => { controller = next; },
      };
      act(() => {
        root.render(<Harness {...props} latestTurnId="turn-1" isStreaming={false} />);
      });
      scrollToContentEnd.mockClear();
      act(() => {
        root.render(<Harness {...props} latestTurnId="turn-2" isStreaming={false} />);
      });

      act(() => controller?.enterFollowOutput('jump-to-latest'));

      expect(scrollToContentEnd).not.toHaveBeenCalled();
      scroller.scrollTop = 0;
      runNextFrame();
      expect(scroller.scrollTop).toBe(900);
    });

    it('animates back to the pin instead of jumping', () => {
      // The frame loop assigns scrollTop outright, so without the yield budget
      // this branch was an instant move where every other jump to latest is
      // animated.
      pinLatestTurn();
      scroller.scrollTop = 0;
      scrollTo.mockClear();

      act(() => controller?.enterFollowOutput('jump-to-latest'));

      expect(scrollTo).toHaveBeenCalledWith({ top: 900, behavior: 'smooth' });
      runNextFrame();
      // jsdom does not animate, so the loop must have left the viewport alone.
      expect(scroller.scrollTop).toBe(0);
    });

    it('resumes at the content end once the pin has been retired', () => {
      // The exemption is only for a Turn whose answer still fits one viewport.
      // Past the crossover the pin is gone and the ordinary rule applies.
      const scrollToContentEnd = vi.fn();
      setScrollerMetrics(scroller, {
        scrollHeight: 2000 + TAIL_SPACER,
        clientHeight: VIEWPORT,
        scrollTop: 900,
      });
      const props = {
        scroller,
        scrollToContentEnd,
        scrollTurnToTop: () => true,
        resolveTurnTopScrollTop: () => 900,
        onController: (next: Controller) => { controller = next; },
      };
      act(() => {
        root.render(<Harness {...props} latestTurnId="turn-1" isStreaming={false} />);
      });
      act(() => {
        root.render(<Harness {...props} latestTurnId="turn-2" isStreaming={false} />);
      });
      // Content end (1500) has overtaken the pin, which retires it.
      runNextFrame();
      scrollToContentEnd.mockClear();

      act(() => controller?.enterFollowOutput('jump-to-latest'));

      expect(scrollToContentEnd).toHaveBeenCalledWith('smooth');
    });
  });

  describe('snapping back out of the reserved blank', () => {
    /** Places the viewport where a gesture into the tail spacer would leave it. */
    function restInBlank(scrollTop: number) {
      scroller.scrollTop = scrollTop;
      act(() => controller?.handleScrollSettled());
    }

    it('returns an idle transcript to the end of real content', () => {
      setScrollerMetrics(scroller, {
        scrollHeight: 1500 + TAIL_SPACER,
        clientHeight: VIEWPORT,
        scrollTop: 0,
      });
      act(() => {
        root.render(
          <Harness
            latestTurnId="turn-1"
            isStreaming={false}
            scroller={scroller}
            onController={next => { controller = next; }}
          />,
        );
      });
      act(() => controller?.handleUserScrollIntent());

      restInBlank(1000 + TAIL_SPACER);

      expect(scrollTo).toHaveBeenCalledWith({ top: 1000, behavior: 'smooth' });
    });

    it('returns a short new Turn to the viewport top, not to the content end', () => {
      // The pin outlives the user takeover on purpose. Snapping to the content
      // end here would scroll *up* and shove the message the user just sent
      // into the middle of the viewport.
      setScrollerMetrics(scroller, {
        scrollHeight: 1200 + TAIL_SPACER,
        clientHeight: VIEWPORT,
        scrollTop: 900,
      });
      const props = {
        scroller,
        scrollTurnToTop: () => true,
        resolveTurnTopScrollTop: () => 900,
        onController: (next: Controller) => { controller = next; },
      };
      act(() => {
        root.render(<Harness {...props} latestTurnId="turn-1" />);
      });
      act(() => {
        root.render(<Harness {...props} latestTurnId="turn-2" />);
      });
      act(() => controller?.handleUserScrollIntent());

      restInBlank(1400);

      expect(scrollTo).toHaveBeenCalledWith({ top: 900, behavior: 'smooth' });
    });

    it('leaves a viewport resting above the target alone', () => {
      setScrollerMetrics(scroller, {
        scrollHeight: 1500 + TAIL_SPACER,
        clientHeight: VIEWPORT,
        scrollTop: 0,
      });
      act(() => {
        root.render(
          <Harness
            latestTurnId="turn-1"
            isStreaming={false}
            scroller={scroller}
            onController={next => { controller = next; }}
          />,
        );
      });
      act(() => controller?.handleUserScrollIntent());
      scrollTo.mockClear();

      restInBlank(200);

      expect(scrollTo).not.toHaveBeenCalled();
      expect(controller?.isFollowingOutput).toBe(false);
    });

    it('hands the viewport back to follow once the snap arrives', () => {
      setScrollerMetrics(scroller, {
        scrollHeight: 1500 + TAIL_SPACER,
        clientHeight: VIEWPORT,
        scrollTop: 0,
      });
      act(() => {
        root.render(
          <Harness
            latestTurnId="turn-1"
            isStreaming={false}
            scroller={scroller}
            onController={next => { controller = next; }}
          />,
        );
      });
      act(() => controller?.handleUserScrollIntent());
      restInBlank(1000 + TAIL_SPACER);

      restInBlank(1000);

      expect(controller?.isFollowingOutput).toBe(true);
    });

    it('does not take the viewport back when a gesture overrode the snap', () => {
      setScrollerMetrics(scroller, {
        scrollHeight: 1500 + TAIL_SPACER,
        clientHeight: VIEWPORT,
        scrollTop: 0,
      });
      act(() => {
        root.render(
          <Harness
            latestTurnId="turn-1"
            isStreaming={false}
            scroller={scroller}
            onController={next => { controller = next; }}
          />,
        );
      });
      act(() => controller?.handleUserScrollIntent());
      restInBlank(1000 + TAIL_SPACER);

      restInBlank(200);

      expect(controller?.isFollowingOutput).toBe(false);
    });
  });

  describe('anchoring the viewport bottom across a resize', () => {
    /** Mounts at the content end, then hands the viewport to the user. */
    function restAtContentEnd() {
      setScrollerMetrics(scroller, {
        scrollHeight: 1500 + VIEWPORT,
        clientHeight: VIEWPORT,
        scrollTop: 0,
      });
      act(() => {
        root.render(
          <Harness
            latestTurnId="turn-1"
            isStreaming={false}
            scroller={scroller}
            onController={next => { controller = next; }}
          />,
        );
      });
      runNextFrame();
      expect(scroller.scrollTop).toBe(1000);
      act(() => controller?.handleUserScrollIntent());
    }

    /** Content and footer stay at 1500; only the viewport, and so the spacer, change. */
    function resizeViewportTo(clientHeight: number, scrollTop: number) {
      setScrollerMetrics(scroller, {
        scrollHeight: 1500 + clientHeight,
        clientHeight,
        scrollTop,
      });
    }

    it('keeps content against the viewport bottom when the viewport grows', () => {
      // A viewport 200px taller lowers the content end by 200px. Without the
      // clamp the spacer removed, the resting viewport is left that far into
      // the blank.
      restAtContentEnd();
      resizeViewportTo(VIEWPORT + 200, 1000);

      act(() => controller?.handleViewportResize({
        viewportHeightDeltaPx: 200,
        wasAtTail: true,
      }));

      expect(scroller.scrollTop).toBe(800);
    });

    it('keeps content against the viewport bottom when the viewport shrinks', () => {
      // The opposite direction: the content end moves down past the viewport,
      // cutting off the bottom of what was on screen.
      restAtContentEnd();
      resizeViewportTo(VIEWPORT - 200, 1000);

      act(() => controller?.handleViewportResize({
        viewportHeightDeltaPx: -200,
        wasAtTail: true,
      }));

      expect(scroller.scrollTop).toBe(1200);
    });

    it('anchors the bottom for a viewport reading history too', () => {
      // The rule is not about the tail. A plain scroller preserves scrollTop
      // and reveals or swallows content at the bottom edge; for a transcript
      // the bottom edge is the one worth holding still.
      restAtContentEnd();
      resizeViewportTo(VIEWPORT + 200, 600);

      act(() => controller?.handleViewportResize({
        viewportHeightDeltaPx: 200,
        wasAtTail: false,
      }));

      expect(scroller.scrollTop).toBe(400);
    });

    it('does not drag a viewport reading history to the tail', () => {
      restAtContentEnd();
      resizeViewportTo(VIEWPORT - 200, 600);

      act(() => controller?.handleViewportResize({
        viewportHeightDeltaPx: -200,
        wasAtTail: false,
      }));

      // Bottom-anchored, and nowhere near the content end at 1200.
      expect(scroller.scrollTop).toBe(800);
    });

    it('corrects instantly, since the content does not appear to move', () => {
      restAtContentEnd();
      scrollTo.mockClear();
      resizeViewportTo(VIEWPORT + 200, 1000);

      act(() => controller?.handleViewportResize({
        viewportHeightDeltaPx: 200,
        wasAtTail: true,
      }));

      expect(scrollTo).not.toHaveBeenCalled();
    });

    it('does not hand the viewport back to follow', () => {
      // A gesture ending in the blank means "take me to the end"; a layout
      // change means nothing at all.
      restAtContentEnd();
      resizeViewportTo(VIEWPORT + 200, 1000);

      act(() => controller?.handleViewportResize({
        viewportHeightDeltaPx: 200,
        wasAtTail: true,
      }));

      expect(controller?.isFollowingOutput).toBe(false);
    });

    it('follows the content end while a reflow keeps moving it', () => {
      // A width change reflows every item, and arithmetic cannot track where
      // the bottom line went. The end of the transcript is the one position
      // that can be recomputed, so a viewport resting on it is put back on it —
      // repeatedly, because the virtualizer re-estimates over several passes.
      restAtContentEnd();

      // Narrower: the same text wraps into 300px more content.
      setScrollerMetrics(scroller, {
        scrollHeight: 1800 + VIEWPORT,
        clientHeight: VIEWPORT,
        scrollTop: 1000,
      });
      act(() => controller?.handleViewportResize({
        viewportHeightDeltaPx: 0,
        wasAtTail: true,
      }));
      expect(scroller.scrollTop).toBe(1300);

      // Re-measurement adds another 200px above the viewport.
      setScrollerMetrics(scroller, {
        scrollHeight: 2000 + VIEWPORT,
        clientHeight: VIEWPORT,
        scrollTop: 1300,
      });
      act(() => controller?.handleViewportResize({
        viewportHeightDeltaPx: 0,
        wasAtTail: true,
      }));
      expect(scroller.scrollTop).toBe(1500);
    });

    it('leaves a reflow alone for a viewport that was not at the end', () => {
      restAtContentEnd();
      setScrollerMetrics(scroller, {
        scrollHeight: 1800 + VIEWPORT,
        clientHeight: VIEWPORT,
        scrollTop: 200,
      });

      act(() => controller?.handleViewportResize({
        viewportHeightDeltaPx: 0,
        wasAtTail: false,
      }));

      expect(scroller.scrollTop).toBe(200);
    });
  });
});
