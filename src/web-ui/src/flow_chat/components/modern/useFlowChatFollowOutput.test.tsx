// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useFlowChatFollowOutput } from './useFlowChatFollowOutput';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

type FollowOutputController = ReturnType<typeof useFlowChatFollowOutput>;

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
}: {
  scroller: HTMLElement;
  onController: (controller: FollowOutputController) => void;
  performAutoFollowScroll: () => void;
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
    performLatestTurnStickyPin: vi.fn(),
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
});
