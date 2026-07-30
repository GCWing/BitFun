// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  computeContinuousFollowStep,
  useFlowChatFollowOutput,
} from './useFlowChatFollowOutput';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

type FollowOutputController = ReturnType<typeof useFlowChatFollowOutput>;

describe('computeContinuousFollowStep', () => {
  it('spreads a line-height-sized tail growth across multiple frames', () => {
    const firstStep = computeContinuousFollowStep(24, 1000 / 60);

    expect(firstStep).toBeGreaterThan(0);
    expect(firstStep).toBeLessThan(24);
  });

  it('retargets proportionally while capping large catch-up jumps', () => {
    const smallStep = computeContinuousFollowStep(24, 1000 / 60);
    const largeStep = computeContinuousFollowStep(240, 1000 / 60);

    expect(largeStep).toBeGreaterThan(smallStep);
    expect(largeStep).toBeLessThanOrEqual(32);
  });

  it('snaps only the final subpixel remainder', () => {
    expect(computeContinuousFollowStep(0.4, 1000 / 60)).toBe(0.4);
    expect(computeContinuousFollowStep(Number.NaN, 1000 / 60)).toBe(0);
  });
});

function setScrollerMetrics(
  scroller: HTMLElement,
  metrics: { scrollHeight: number; clientHeight: number; scrollTop: number },
): void {
  Object.defineProperties(scroller, {
    scrollHeight: { configurable: true, value: metrics.scrollHeight },
    clientHeight: { configurable: true, value: metrics.clientHeight },
    scrollTop: { configurable: true, writable: true, value: metrics.scrollTop },
  });
}

function Harness({
  scroller,
  onController,
  performAutoFollowScroll,
  performLatestTurnStickyPin = vi.fn(),
}: {
  scroller: HTMLElement;
  onController: (controller: FollowOutputController) => void;
  performAutoFollowScroll: () => void;
  performLatestTurnStickyPin?: () => void;
}) {
  const scrollerRef = React.useRef<HTMLElement | null>(scroller);
  scrollerRef.current = scroller;

  const controller = useFlowChatFollowOutput({
    activeSessionId: 'session-1',
    latestTurnId: 'turn-2',
    virtualItemCount: 20,
    isStreaming: true,
    scrollerRef,
    performUserFollowScroll: vi.fn(),
    performAutoFollowScroll,
    performLatestTurnStickyPin,
  });

  onController(controller);
  return <div data-following-output={String(controller.isFollowingOutput)} />;
}

describe('useFlowChatFollowOutput', () => {
  let container: HTMLDivElement;
  let root: Root;
  let controller: FollowOutputController | null;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    controller = null;
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      void callback;
      return 1;
    }));
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.unstubAllGlobals();
  });

  it('exits output follow immediately when explicit user scroll intent is already away from bottom', () => {
    const scroller = document.createElement('div');
    setScrollerMetrics(scroller, {
      scrollHeight: 1500,
      clientHeight: 500,
      scrollTop: 1000,
    });
    const performAutoFollowScroll = vi.fn(() => {
      scroller.scrollTop = 1000;
    });

    act(() => {
      root.render(
        <Harness
          scroller={scroller}
          onController={nextController => {
            controller = nextController;
          }}
          performAutoFollowScroll={performAutoFollowScroll}
        />,
      );
    });

    act(() => {
      controller?.enterFollowOutput('auto-follow');
    });

    expect(controller?.isFollowingOutput).toBe(true);

    setScrollerMetrics(scroller, {
      scrollHeight: 1500,
      clientHeight: 500,
      scrollTop: 600,
    });

    act(() => {
      controller?.handleUserScrollIntent();
    });

    expect(controller?.isFollowingOutput).toBe(false);
  });

  it('exits output follow for explicit upward intent before browser scroll metrics move', () => {
    const scroller = document.createElement('div');
    setScrollerMetrics(scroller, {
      scrollHeight: 1500,
      clientHeight: 500,
      scrollTop: 1000,
    });
    const performAutoFollowScroll = vi.fn(() => {
      scroller.scrollTop = 1000;
    });

    act(() => {
      root.render(
        <Harness
          scroller={scroller}
          onController={nextController => {
            controller = nextController;
          }}
          performAutoFollowScroll={performAutoFollowScroll}
        />,
      );
    });

    act(() => {
      controller?.enterFollowOutput('auto-follow');
    });

    expect(controller?.isFollowingOutput).toBe(true);

    act(() => {
      controller?.handleUserScrollIntent();
    });

    expect(controller?.isFollowingOutput).toBe(false);
  });

  it('cancels armed auto-follow when upward intent arrives during the programmatic guard', () => {
    const scroller = document.createElement('div');
    setScrollerMetrics(scroller, {
      scrollHeight: 1500,
      clientHeight: 500,
      scrollTop: 1000,
    });
    const performAutoFollowScroll = vi.fn(() => {
      scroller.scrollTop = 1000;
    });

    act(() => {
      root.render(
        <Harness
          scroller={scroller}
          onController={nextController => {
            controller = nextController;
          }}
          performAutoFollowScroll={performAutoFollowScroll}
        />,
      );
    });

    act(() => {
      controller?.armFollowOutputForNewTurn();
    });

    expect(controller?.isFollowingOutput).toBe(false);

    act(() => {
      controller?.handleUserScrollIntent();
    });

    let activated = true;
    act(() => {
      activated = controller?.activateArmedFollowOutput() ?? true;
    });

    expect(activated).toBe(false);
    expect(controller?.isFollowingOutput).toBe(false);
  });

  it('resumes a mounted streaming session at the tail without replaying sticky pin', () => {
    const scroller = document.createElement('div');
    setScrollerMetrics(scroller, {
      scrollHeight: 1500,
      clientHeight: 500,
      scrollTop: 0,
    });
    const performAutoFollowScroll = vi.fn(() => {
      scroller.scrollTop = 1000;
    });
    const performLatestTurnStickyPin = vi.fn();

    act(() => {
      root.render(
        <Harness
          scroller={scroller}
          onController={nextController => {
            controller = nextController;
          }}
          performAutoFollowScroll={performAutoFollowScroll}
          performLatestTurnStickyPin={performLatestTurnStickyPin}
        />,
      );
    });

    let resumed = false;
    act(() => {
      resumed = controller?.resumeFollowOutputForMountedStream() ?? false;
    });

    expect(resumed).toBe(true);
    expect(controller?.isFollowingOutput).toBe(true);
    expect(performAutoFollowScroll).toHaveBeenCalledTimes(1);
    expect(performLatestTurnStickyPin).not.toHaveBeenCalled();
  });

  it('eases line-height growth without issuing another bottom snap', () => {
    const queuedFrames: FrameRequestCallback[] = [];
    let nextFrameId = 0;
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      queuedFrames.push(callback);
      nextFrameId += 1;
      return nextFrameId;
    }));

    const scroller = document.createElement('div');
    setScrollerMetrics(scroller, {
      scrollHeight: 1500,
      clientHeight: 500,
      scrollTop: 1000,
    });
    const performAutoFollowScroll = vi.fn(() => {
      scroller.scrollTop = scroller.scrollHeight - scroller.clientHeight;
    });

    act(() => {
      root.render(
        <Harness
          scroller={scroller}
          onController={nextController => {
            controller = nextController;
          }}
          performAutoFollowScroll={performAutoFollowScroll}
        />,
      );
    });

    act(() => {
      controller?.enterFollowOutput('auto-follow');
    });
    expect(performAutoFollowScroll).toHaveBeenCalledTimes(1);

    setScrollerMetrics(scroller, {
      scrollHeight: 1524,
      clientHeight: 500,
      scrollTop: 1000,
    });
    const firstFollowFrame = queuedFrames.shift();
    expect(firstFollowFrame).toBeDefined();

    act(() => {
      firstFollowFrame?.(1000 / 60);
    });

    expect(performAutoFollowScroll).toHaveBeenCalledTimes(1);
    expect(scroller.scrollTop).toBeGreaterThan(1000);
    expect(scroller.scrollTop).toBeLessThan(1024);
  });
});
