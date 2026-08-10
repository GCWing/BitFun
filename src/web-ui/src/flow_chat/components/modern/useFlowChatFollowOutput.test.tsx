// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
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
    getTailSpacerPx: () => TAIL_SPACER,
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

  function runNextFrame() {
    const frame = frames.shift();
    expect(frame).toBeDefined();
    act(() => frame?.(16));
  }

  beforeEach(() => {
    container = document.createElement('div');
    scroller = document.createElement('div');
    document.body.append(container, scroller);
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
});
