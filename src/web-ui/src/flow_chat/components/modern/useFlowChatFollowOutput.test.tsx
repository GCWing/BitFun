// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { flushSync } from 'react-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { tailSpacerPxForViewport } from './flowChatTailFollow';
import {
  TAIL_EASE_ALPHA,
  TAIL_EASE_LINE_PX,
  TAIL_EASE_SNAP_ABOVE_PX,
} from '../../utils/flowChatTailEase';
import { SMOOTH_SCROLL_STALL_MS, useFlowChatFollowOutput } from './useFlowChatFollowOutput';
import {
  useFlowChatViewportOwner,
  type FlowChatViewportOwnerApi,
} from './useFlowChatViewportOwner';
import { USER_DRIVEN_SCROLL_WINDOW_MS } from './flowChatViewportAnchor';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

type Controller = ReturnType<typeof useFlowChatFollowOutput>;

const VIEWPORT = 500;
/** Input-stack footer the harness renders below the transcript. */
const BOTTOM_INSET = 100;
/** The spacer the component would render for a given viewport. */
const spacerFor = (clientHeight: number) => tailSpacerPxForViewport(clientHeight, BOTTOM_INSET);
const TAIL_SPACER = spacerFor(VIEWPORT);
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

/**
 * `turn-N` is the Nth Turn of the session, so the ledger holds N of them.
 *
 * A Turn arriving grows the ledger, which is what every test walking `turn-1`
 * to `turn-2` means. A rollback is the one case where the identity moves and
 * the ledger does not grow, and those tests state the count themselves.
 */
const turnCountFor = (turnId: string) => Number(turnId.replace('turn-', '')) || 1;

interface HarnessProps {
  latestTurnId: string;
  /** Turns in the session ledger. Defaults to what `latestTurnId` implies. */
  dialogTurnCount?: number;
  isStreaming?: boolean;
  scroller: HTMLElement;
  scrollToContentEnd?: (behavior: ScrollBehavior) => void;
  scrollTurnToTop?: (turnId: string) => boolean;
  resolveTurnTopScrollTop?: (turnId: string) => number | null;
  isOpeningViewport?: boolean;
  onController: (controller: Controller) => void;
  /** The register the hook writes through, for a test that has to hold it. */
  onViewportOwner?: (owner: FlowChatViewportOwnerApi) => void;
}

function Harness({
  latestTurnId,
  dialogTurnCount = turnCountFor(latestTurnId),
  isStreaming = true,
  scroller,
  scrollToContentEnd = () => {},
  scrollTurnToTop = () => false,
  resolveTurnTopScrollTop = () => null,
  isOpeningViewport = false,
  onController,
  onViewportOwner,
}: HarnessProps) {
  const scrollerRef = React.useRef<HTMLElement | null>(scroller);
  // The real register, not a stand-in: what this hook is allowed to write is
  // now part of its behaviour, so a permissive fake would test the wrong thing.
  const viewportOwner = useFlowChatViewportOwner(scrollerRef);
  onViewportOwner?.(viewportOwner);
  const controller = useFlowChatFollowOutput({
    activeSessionId: 'session-1',
    latestTurnId,
    dialogTurnCount,
    virtualItemCount: 2,
    isStreaming,
    isViewportActive: true,
    scrollerRef,
    // Sized from live layout, exactly as the component's state does.
    getTailSpacerPx: () => tailSpacerPxForViewport(scroller.clientHeight, BOTTOM_INSET),
    scrollToContentEnd,
    scrollTurnToTop,
    resolveTurnTopScrollTop,
    isOpeningViewport: () => isOpeningViewport,
    viewportOwner,
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

  it('does not read a rolled-back Turn as a newly arrived one', () => {
    /*
     * Measured as a report: send a message, watch it pin, roll it back, and the
     * Turn *before* it takes the viewport top. `latestTurnId` is right — it is
     * the ledger's last Turn — but the detector asked whether the identity had
     * changed, and a truncation changes it without anything having arrived.
     */
    const scrollTurnToTop = vi.fn(() => true);
    const props = {
      scroller,
      scrollTurnToTop,
      onController: (next: Controller) => { controller = next; },
    };

    act(() => {
      root.render(<Harness {...props} latestTurnId="turn-2" isStreaming={false} />);
    });
    scrollTurnToTop.mockClear();

    // The rollback removes turn-2, so the ledger's last Turn is turn-1 again.
    act(() => {
      root.render(
        <Harness {...props} latestTurnId="turn-1" dialogTurnCount={1} isStreaming={false} />,
      );
    });

    expect(scrollTurnToTop).not.toHaveBeenCalled();
  });

  it('returns to the end of the transcript when a rollback takes the Turn it was following', () => {
    const scrollToContentEnd = vi.fn();
    const props = {
      scroller,
      scrollToContentEnd,
      scrollTurnToTop: () => true,
      onController: (next: Controller) => { controller = next; },
    };

    act(() => {
      root.render(<Harness {...props} latestTurnId="turn-2" isStreaming={false} />);
    });
    scrollToContentEnd.mockClear();

    act(() => {
      root.render(
        <Harness {...props} latestTurnId="turn-1" dialogTurnCount={1} isStreaming={false} />,
      );
    });
    // Nothing on its own: a shorter ledger is not a reason to move, because
    // hydration and window merges write it too.
    expect(scrollToContentEnd).not.toHaveBeenCalled();

    // The rollback says so itself, and the answer is the tail.
    act(() => { controller?.handleTurnsRolledBack(); });
    expect(scrollToContentEnd).toHaveBeenCalledWith('auto');
  });

  it('settles a rollback on the new tail even after the reader took the viewport', () => {
    /*
     * Gating this on ownership made it dead code in the case it was written
     * for. Reaching a Turn far enough up to want it gone means scrolling, and
     * scrolling is what hands the viewport back to the reader — so the answer
     * never ran, the anchor held the reader's Turn at its offset from the
     * viewport top, and an 8-Turn session rolled back at Turn 7 came to rest on
     * Turns 2..6 with the new last Turn's answer below the fold.
     *
     * Taking it is safe for the reason the snap back's is: a rollback at Turn N
     * removes N and everything after it, and N was on screen. The new tail is
     * within a Turn of where the reader already is.
     */
    const scrollToContentEnd = vi.fn();
    act(() => {
      root.render(
        <Harness
          latestTurnId="turn-2"
          isStreaming={false}
          scroller={scroller}
          scrollToContentEnd={scrollToContentEnd}
          onController={next => { controller = next; }}
        />,
      );
    });
    act(() => { controller?.handleUserScrollIntent(); });
    expect(controller?.isFollowingOutput).toBe(false);
    scrollToContentEnd.mockClear();

    act(() => { controller?.handleTurnsRolledBack(); });
    expect(scrollToContentEnd).toHaveBeenCalledWith('auto');
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
    // The virtualizer compensates a history prepend by writing scrollTop before the
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

  describe('standing down for its own animated scroll', () => {
    /**
     * A clock the test moves by hand.
     *
     * The stand-down ends on a *duration* without travel, so a test that runs
     * its frames in microseconds of real time proves nothing about it either
     * way. Every frame below states how much time it took.
     */
    let nowMs = 0;
    let nowSpy: ReturnType<typeof vi.spyOn>;

    beforeEach(() => {
      nowMs = 1_000;
      nowSpy = vi.spyOn(performance, 'now').mockImplementation(() => nowMs);
    });

    afterEach(() => {
      nowSpy.mockRestore();
    });

    /**
     * Mounts a transcript whose content ends at 4500, settles there, and then
     * issues an animated jump to latest from the top.
     *
     * jsdom does not animate, so the animation is played by hand below. That is
     * the point of these tests: what ends the stand-down is the viewport having
     * stopped moving, and only a test that moves it can tell that apart from a
     * budget running out.
     */
    function jumpToLatestFromTheTop() {
      setScrollerMetrics(scroller, {
        scrollHeight: 5000 + TAIL_SPACER,
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
      expect(scroller.scrollTop).toBe(4500);
      scroller.scrollTop = 0;
      act(() => controller?.enterFollowOutput('jump-to-latest'));
    }

    /** One frame, `forMs` after the last, in which the animation moved `byPx`. */
    function animateFrame(byPx: number, forMs = 5) {
      nowMs += forMs;
      scroller.scrollTop += byPx;
      runNextFrame();
    }

    it('stays out of the way for as long as the animation is still travelling', () => {
      // The stand-down used to be 45 frames, which is 0.75s at 60Hz and 0.52s
      // on a busy 200Hz display — measured, a jump issued for 8717px animated
      // 5480 of them and was finished by the loop in one 3290px write.
      jumpToLatestFromTheTop();

      for (let frame = 0; frame < 60; frame += 1) {
        animateFrame(50);
        expect(scroller.scrollTop).toBe(50 * (frame + 1));
      }
    });

    it('sits through the frames a smooth scroll takes to visibly start', () => {
      /*
       * The regression this pair of constants exists for. A programmatic smooth
       * scroll eases in — measured on WebView2, 2px in its first 50ms against
       * 9734px to travel — and with scroll offsets quantised to 0.8px the early
       * frames genuinely do not move. Counting two still *frames* took the
       * viewport back 21ms after asking for the animation, and the reader saw
       * an instant jump.
       */
      jumpToLatestFromTheTop();

      for (let frame = 0; frame < 20; frame += 1) {
        animateFrame(0);
      }
      expect(scroller.scrollTop).toBe(0);

      // 100ms in, still inside the window, and the animation finally shows.
      animateFrame(0.8);
      expect(scroller.scrollTop).toBe(0.8);
    });

    it('resumes once the animation stops moving the viewport', () => {
      jumpToLatestFromTheTop();
      animateFrame(50);

      // Still inside the window a stalling animation is given.
      animateFrame(0, SMOOTH_SCROLL_STALL_MS - 10);
      expect(scroller.scrollTop).toBe(50);

      animateFrame(0, 20);
      expect(scroller.scrollTop).toBe(4500);
    });

    it('takes the viewport back on the backstop when the animation never ends', () => {
      jumpToLatestFromTheTop();
      // Travelling the whole time, so nothing but the backstop can end this.
      for (let frame = 0; frame < 12; frame += 1) {
        animateFrame(1, 100);
      }

      expect(scroller.scrollTop).toBe(4500);
    });
  });

  describe('easing the follow across the frames it has', () => {
    /** Mounts a streaming transcript resting exactly on its content end. */
    function followFromContentEnd() {
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
    }

    /** Grows real content by `byPx`, which is what moves the follow target. */
    function growContentBy(byPx: number) {
      setScrollerMetrics(scroller, {
        scrollHeight: 1500 + byPx + TAIL_SPACER,
        clientHeight: VIEWPORT,
        scrollTop: scroller.scrollTop,
      });
    }

    it('spends a line of growth over the frames that were empty', () => {
      // Markdown reflows a line at a time, so an outright write puts all 24px
      // on one frame out of seven and none on the rest.
      followFromContentEnd();
      growContentBy(TAIL_EASE_LINE_PX);

      runNextFrame();

      expect(scroller.scrollTop).toBeCloseTo(1000 + TAIL_EASE_LINE_PX * TAIL_EASE_ALPHA, 5);
      expect(scroller.scrollTop - 1000).toBeLessThan(TAIL_EASE_LINE_PX);
    });

    it('keeps stepping until it has landed on the target', () => {
      followFromContentEnd();
      growContentBy(TAIL_EASE_LINE_PX);

      let previousPx = 1000;
      for (let frame = 0; frame < 20; frame += 1) {
        runNextFrame();
        // Every step is under a line: that is the bar the ease has to clear to
        // be worth having, since a line is what it set out to replace.
        expect(scroller.scrollTop - previousPx).toBeLessThan(TAIL_EASE_LINE_PX);
        previousPx = scroller.scrollTop;
      }

      // Inside the loop's own `BOTTOM_EPSILON_PX`, where it stops writing at
      // all — the ease converges on the target rather than stalling short of
      // it or ringing around it.
      expect(1000 + TAIL_EASE_LINE_PX - scroller.scrollTop).toBeLessThan(2);
      expect(scroller.scrollTop).toBeLessThanOrEqual(1000 + TAIL_EASE_LINE_PX);
    });

    it('jumps when an ease would open with a bigger step than the jump it replaces', () => {
      // A burst arriving further behind than four lines: a quarter of that is
      // already worse than having simply gone the whole way.
      followFromContentEnd();
      growContentBy(TAIL_EASE_SNAP_ABOVE_PX + 100);

      runNextFrame();

      expect(scroller.scrollTop).toBe(1000 + TAIL_EASE_SNAP_ABOVE_PX + 100);
    });

    it('does not ease while the transcript is still opening', () => {
      // The reveal waits for the viewport to reach the content end, and while
      // it waits nothing is painted — so an ease there is travel nobody sees,
      // holding open the one phase whose whole point is to be over.
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
            isOpeningViewport
            onController={next => { controller = next; }}
          />,
        );
      });
      growContentBy(TAIL_EASE_LINE_PX);

      runNextFrame();

      expect(scroller.scrollTop).toBe(1000 + TAIL_EASE_LINE_PX);
    });

    it('goes the whole way at once when content shrinks under the viewport', () => {
      // Easing down through a shrink reads as the transcript being clawed
      // backwards, which is worse than the jump it would replace.
      followFromContentEnd();
      setScrollerMetrics(scroller, {
        scrollHeight: 700 + TAIL_SPACER,
        clientHeight: VIEWPORT,
        scrollTop: 1000,
      });

      runNextFrame();

      // Past the tolerated gap, so the hold rule gives ground — in one step.
      expect(scroller.scrollTop).toBe(200 + MAX_GAP);
    });
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

    it('returns a viewport follow owns but is no longer correcting', () => {
      /*
       * Ownership outlives the frame loop by design, so "follow owns this" does
       * not mean "something is bringing it back". A scrollbar drag reaches this
       * state on every platform whose scrollbar leaves no gutter to recognise
       * the press by; before this, the viewport simply stayed in the blank.
       */
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
      // Run the settle budget out: the loop stops, ownership does not.
      while (frames.length > 0) runNextFrame();
      expect(controller?.isFollowingOutput).toBe(true);

      restInBlank(1000 + TAIL_SPACER);

      expect(scrollTo).toHaveBeenCalledWith({ top: 1000, behavior: 'smooth' });
    });

    it('leaves a viewport the frame loop is still correcting to the loop', () => {
      // A live loop reaches its target in one frame; snapping back would only
      // race it, and with an animation that the next frame would cancel.
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
      scrollTo.mockClear();

      restInBlank(1000 + TAIL_SPACER);

      expect(scrollTo).not.toHaveBeenCalled();
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

    it('asks again once the gesture that refused it has lapsed', () => {
      /*
       * The gesture's claim is a lapse timer, because a wheel has no end of its
       * own, and this correction is the one writer that asks from inside that
       * window — coming to rest is what triggers it. So the first ask is
       * refused by construction, and the answer is to ask again rather than to
       * release the hold: a settle also lands *between* two notches of a
       * gesture still in progress, and forcing the snap through there drags the
       * reader out of the blank they are scrolling into, every 800ms.
       *
       * Measured, the case the retry is for: a wheel at the head of a
       * three-Turn tail paged in 13 Turns, the compensation moved the reader
       * 2727px on the virtualizer's estimates, the real heights landed ~1670px
       * shorter and the browser clamped the offset to the end of the shrunken
       * range — 841px past the content end, the whole reserved spacer, blank.
       * The snap back was refused `heldBy: user-gesture` 57ms into a 200ms
       * hold, and nothing moved for the four minutes that followed.
       */
      let owner: FlowChatViewportOwnerApi | null = null;
      setScrollerMetrics(scroller, {
        scrollHeight: 1500 + TAIL_SPACER,
        clientHeight: VIEWPORT,
        scrollTop: 0,
      });
      vi.useFakeTimers();
      try {
        act(() => {
          root.render(
            <Harness
              latestTurnId="turn-1"
              isStreaming={false}
              scroller={scroller}
              onController={next => { controller = next; }}
              onViewportOwner={next => { owner = next; }}
            />,
          );
        });
        act(() => controller?.handleUserScrollIntent());
        // The wheel, as the list takes it: the reader holds the viewport for
        // the window the anchor uses, and the settle below lands inside it.
        act(() => {
          owner?.claim('user-gesture', { holdForMs: USER_DRIVEN_SCROLL_WINDOW_MS });
        });
        scrollTo.mockClear();

        restInBlank(1000 + TAIL_SPACER);
        // Refused, and rightly: as far as the register knows the reader still
        // has the viewport.
        expect(scrollTo).not.toHaveBeenCalled();

        act(() => { vi.advanceTimersByTime(USER_DRIVEN_SCROLL_WINDOW_MS + 20); });

        expect(scrollTo).toHaveBeenCalledWith({ top: 1000, behavior: 'smooth' });
      } finally {
        vi.useRealTimers();
      }
    });

    it('stops asking rather than chase a reader who keeps scrolling', () => {
      // Every retry is refused while the reader keeps re-taking the viewport,
      // and none of them move it. Their own next settle is what asks again.
      let owner: FlowChatViewportOwnerApi | null = null;
      setScrollerMetrics(scroller, {
        scrollHeight: 1500 + TAIL_SPACER,
        clientHeight: VIEWPORT,
        scrollTop: 0,
      });
      vi.useFakeTimers();
      try {
        act(() => {
          root.render(
            <Harness
              latestTurnId="turn-1"
              isStreaming={false}
              scroller={scroller}
              onController={next => { controller = next; }}
              onViewportOwner={next => { owner = next; }}
            />,
          );
        });
        act(() => controller?.handleUserScrollIntent());
        act(() => {
          owner?.claim('user-gesture', { holdForMs: USER_DRIVEN_SCROLL_WINDOW_MS });
        });
        scrollTo.mockClear();
        restInBlank(1000 + TAIL_SPACER);

        // Notches inside the window they renew, for longer than the retry
        // budget covers. This is what a wheel actually looks like: the register
        // never stops answering with the reader.
        for (let notch = 0; notch < 16; notch += 1) {
          act(() => {
            owner?.claim('user-gesture', { holdForMs: USER_DRIVEN_SCROLL_WINDOW_MS });
            vi.advanceTimersByTime(USER_DRIVEN_SCROLL_WINDOW_MS / 2);
          });
        }

        expect(scrollTo).not.toHaveBeenCalled();
      } finally {
        vi.useRealTimers();
      }
    });

    it('resumes on the end the content has now, not the one the snap aimed at', () => {
      /*
       * The snap is animated, and what it was aiming at can move while it
       * travels — a history page whose items are still measuring is exactly
       * that, and is also the reason the snap was needed. Resuming on
       * `scrollTop` adopts the stale offset as the hold rule's memory, and the
       * hold rule defends a gap rather than closing it.
       *
       * Measured on a 43-Turn session paged from a three-Turn tail: issued for
       * 12336, landed 608ms later against a content end of 11972, and the first
       * frame after it read `desired 11972, target 12336, onTarget true`. The
       * reader kept 364px of reserved blank and a Turn cut off at the top.
       */
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

      // The items measured shorter while the snap travelled, so the end of
      // content is 300px above where it was aimed.
      setScrollerMetrics(scroller, {
        scrollHeight: 1200 + TAIL_SPACER,
        clientHeight: VIEWPORT,
        scrollTop: 1000,
      });
      restInBlank(1000);

      expect(controller?.isFollowingOutput).toBe(true);
      // Not 1000, which is inside the gap the hold rule tolerates and would
      // therefore have been kept for good.
      expect(controller?.getFollowTargetScrollTop()).toBe(700);
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
        scrollHeight: 1500 + spacerFor(VIEWPORT),
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
        scrollHeight: 1500 + spacerFor(clientHeight),
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
        scrollHeight: 1800 + spacerFor(VIEWPORT),
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
        scrollHeight: 2000 + spacerFor(VIEWPORT),
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
        scrollHeight: 1800 + spacerFor(VIEWPORT),
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

  describe('ownership across renders', () => {
    /*
     * Ownership is a ref because the loop reads it between renders, and it was
     * *also* assigned from the `isFollowingOutput` state on every render. Those
     * two writers disagree for exactly as long as a state update is queued, and
     * React is free to render in that window: an update scheduled from a
     * passive effect sits at a lower priority than a synchronous render, which
     * then renders the value from before it and put the ref back to `false`.
     *
     * Measured on session open, which is where this always happens — the entry
     * comes from a mount effect: `followOutput.enter` recorded, then the very
     * first frame of the loop stood down with `not-following` and its settle
     * budget untouched, and no `followOutput.exit` anywhere in the trail
     * because nothing had exited. The register still held `follow-output`, so
     * the prepend compensation for a history window that arrived 90ms later was
     * left to a writer that was no longer running, and the transcript stayed at
     * offset 0 — the top of the window, eight Turns above the tail.
     */
    it('survives a render that has not seen the entry yet', () => {
      act(() => {
        root.render(
          <Harness
            scroller={scroller}
            latestTurnId="turn-1"
            isStreaming={false}
            onController={(next) => { controller = next; }}
          />,
        );
      });
      act(() => controller?.handleUserScrollIntent());
      expect(controller?.isFollowingOutputNow()).toBe(false);

      // Outside `act`, so the state update this schedules is still queued.
      controller?.enterFollowOutput('jump-to-latest');
      expect(controller?.isFollowingOutputNow()).toBe(true);

      // A synchronous render, which does not carry the queued update.
      flushSync(() => {
        root.render(
          <Harness
            scroller={scroller}
            latestTurnId="turn-1"
            isStreaming={false}
            onController={(next) => { controller = next; }}
          />,
        );
      });

      expect(controller?.isFollowingOutputNow()).toBe(true);
    });
  });
});
