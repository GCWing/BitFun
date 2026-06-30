// @vitest-environment jsdom

import React from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { VirtualMessageList } from './VirtualMessageList';
import { activeSessionHistoryProjectionHandoff } from './historyProjectionHandoff';
import type { Session } from '../../types/flow-chat';
import type { VirtualItem } from '../../store/modernFlowChatStore';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const stateMocks = vi.hoisted(() => ({
  activeSession: null as Session | null,
  virtualItems: [] as VirtualItem[],
  visibleTurnInfo: null as unknown,
  setVisibleTurnInfo: vi.fn(),
}));
const flowStoreMocks = vi.hoisted(() => ({
  hasPendingSessionHistoryCompletion: vi.fn(() => false),
  hasDeferredSessionHistoryProjection: vi.fn(() => false),
  requestSessionFullHistoryProjection: vi.fn(),
  revealPreviousSessionHistoryWindow: vi.fn(() => false),
  releaseSessionHistoryCompletionAfterInitialPaint: vi.fn(() => false),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const translations: Record<string, string> = {
        'historyState.preparingOlderHistory': 'Preparing older history...',
        'historyState.olderHistoryNotReady': 'Older history is not ready yet.',
      };
      return translations[key] ?? key;
    },
  }),
}));

vi.mock('react-virtuoso', () => ({
  Virtuoso: React.forwardRef((props: any, ref) => {
    const scrollerRef = React.useRef<HTMLDivElement | null>(null);
    React.useImperativeHandle(ref, () => ({
      scrollTo: vi.fn(),
      scrollToIndex: vi.fn(),
    }));

    React.useLayoutEffect(() => {
      if (!scrollerRef.current) {
        return;
      }

      props.scrollerRef?.(scrollerRef.current);
      return () => {
        props.scrollerRef?.(null);
      };
    }, [props]);

    React.useEffect(() => {
      if (props.data?.[0]?.turnId === 'turn-a') {
        props.atBottomStateChange?.(false);
      }
    }, [props]);

    return (
      <div
        ref={scrollerRef}
        data-testid="virtuoso"
        data-virtuoso-scroller="true"
        data-session-id={stateMocks.activeSession?.sessionId ?? ''}
        tabIndex={0}
      >
        {props.components?.Header ? <props.components.Header /> : null}
        {props.data?.map((item: VirtualItem, index: number) => (
          <div key={item.turnId} className="virtual-item-wrapper" data-turn-id={item.turnId} data-virtual-index={index} data-item-type={item.type}>
            {item.turnId}
          </div>
        ))}
        {props.components?.Footer ? <props.components.Footer /> : null}
      </div>
    );
  }),
}));

vi.mock('../../store/modernFlowChatStore', () => {
  const useModernFlowChatStore = (selector: (state: any) => unknown) => selector({
    visibleTurnInfo: stateMocks.visibleTurnInfo,
  });
  useModernFlowChatStore.getState = () => ({
    visibleTurnInfo: stateMocks.visibleTurnInfo,
    setVisibleTurnInfo: stateMocks.setVisibleTurnInfo,
  });

  return {
    useActiveSession: () => stateMocks.activeSession,
    useVirtualItems: () => stateMocks.virtualItems,
    useModernFlowChatStore,
  };
});

vi.mock('../../hooks/useActiveSessionState', () => ({
  useActiveSessionState: () => ({
    isProcessing: false,
    processingPhase: null,
  }),
}));

vi.mock('../../store/chatInputStateStore', () => ({
  useChatInputState: (selector: (state: any) => unknown) => selector({
    isActive: false,
    isExpanded: false,
    inputHeight: 0,
  }),
}));

vi.mock('../../store/FlowChatStore', () => ({
  flowChatStore: {
    hasPendingSessionHistoryCompletion: flowStoreMocks.hasPendingSessionHistoryCompletion,
    hasDeferredSessionHistoryProjection: flowStoreMocks.hasDeferredSessionHistoryProjection,
    requestSessionFullHistoryProjection: flowStoreMocks.requestSessionFullHistoryProjection,
    revealPreviousSessionHistoryWindow: flowStoreMocks.revealPreviousSessionHistoryWindow,
    releaseSessionHistoryCompletionAfterInitialPaint: flowStoreMocks.releaseSessionHistoryCompletionAfterInitialPaint,
  },
}));

vi.mock('@/shared/utils/startupTrace', () => ({
  startupTrace: { markPhase: vi.fn() },
}));

vi.mock('./VirtualItemRenderer', () => ({
  VirtualItemRenderer: ({ item, index }: { item: VirtualItem; index: number }) => (
    <div className="virtual-item-wrapper" data-turn-id={item.turnId} data-virtual-index={index} data-item-type={item.type}>
      {item.turnId}
    </div>
  ),
}));

vi.mock('../ScrollToLatestBar', () => ({
  ScrollToLatestBar: ({ visible }: { visible: boolean }) => (
    <div data-testid="scroll-to-latest" data-visible={visible ? 'true' : 'false'} />
  ),
}));

vi.mock('../ScrollToTurnHeaderButton', () => ({
  ScrollToTurnHeaderButton: () => null,
}));

vi.mock('../../hooks/useScrollToTurnHeader', () => ({
  useScrollToTurnHeader: () => ({
    shouldShowButton: false,
    handleClick: vi.fn(),
  }),
}));

vi.mock('../../hooks/useVisibleTaskInfo', () => ({
  useVisibleTaskInfo: () => ({
    visibleTaskInfo: null,
    scrollToTask: vi.fn(),
  }),
}));

vi.mock('../StickyTaskIndicator', () => ({
  StickyTaskIndicator: () => null,
}));

vi.mock('./ProcessingIndicator', () => ({
  ProcessingIndicator: () => null,
}));

vi.mock('./processingIndicatorVisibility', () => ({
  shouldReserveProcessingIndicatorSpace: () => false,
  shouldShowProcessingIndicator: () => false,
}));

vi.mock('./ScrollAnchor', () => ({
  ScrollAnchor: () => null,
}));

function createSession(sessionId: string, turnId: string, overrides: Partial<Session> = {}): Session {
  return {
    sessionId,
    title: sessionId,
    dialogTurns: [{
      id: turnId,
      sessionId,
      userMessage: { id: `user-${turnId}`, content: turnId, timestamp: 1 },
      modelRounds: [],
      status: 'completed',
      startTime: 1,
    }],
    status: 'idle',
    config: { agentType: 'agentic' },
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    isHistorical: false,
    todos: [],
    mode: 'agentic',
    sessionKind: 'normal',
    ...overrides,
  } as Session;
}

function createItem(turnId: string): VirtualItem {
  return {
    type: 'user-message',
    turnId,
    data: {
      id: `user-${turnId}`,
      content: turnId,
      timestamp: 1,
    },
  } as VirtualItem;
}

describe('VirtualMessageList session boundary', () => {
  let container: HTMLDivElement;
  let root: Root;
  let rafCallbacks: FrameRequestCallback[];

  const flushAnimationFrame = () => {
    const callbacks = rafCallbacks;
    rafCallbacks = [];
    act(() => {
      callbacks.forEach(callback => callback(performance.now()));
    });
  };

  beforeEach(() => {
    rafCallbacks = [];
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      rafCallbacks.push(callback);
      return rafCallbacks.length;
    }));
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
    vi.stubGlobal('ResizeObserver', class {
      observe = vi.fn();
      unobserve = vi.fn();
      disconnect = vi.fn();
    });
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    stateMocks.visibleTurnInfo = null;
    stateMocks.setVisibleTurnInfo.mockReset();
    flowStoreMocks.hasPendingSessionHistoryCompletion.mockReset();
    flowStoreMocks.hasPendingSessionHistoryCompletion.mockReturnValue(false);
    flowStoreMocks.hasDeferredSessionHistoryProjection.mockReset();
    flowStoreMocks.hasDeferredSessionHistoryProjection.mockReturnValue(false);
    flowStoreMocks.requestSessionFullHistoryProjection.mockReset();
    flowStoreMocks.revealPreviousSessionHistoryWindow.mockReset();
    flowStoreMocks.revealPreviousSessionHistoryWindow.mockReturnValue(false);
    flowStoreMocks.releaseSessionHistoryCompletionAfterInitialPaint.mockReset();
    flowStoreMocks.releaseSessionHistoryCompletionAfterInitialPaint.mockReturnValue(false);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.unstubAllGlobals();
  });

  it('resets viewport-local at-bottom state when the active session changes', () => {
    stateMocks.activeSession = createSession('session-a', 'turn-a');
    stateMocks.virtualItems = [createItem('turn-a')];

    act(() => {
      root.render(<VirtualMessageList />);
    });

    expect(container.querySelector('[data-testid="scroll-to-latest"]')?.getAttribute('data-visible')).toBe('true');

    stateMocks.activeSession = createSession('session-b', 'turn-b');
    stateMocks.virtualItems = [createItem('turn-b')];

    act(() => {
      root.render(<VirtualMessageList />);
    });

    expect(container.querySelector('[data-testid="scroll-to-latest"]')?.getAttribute('data-visible')).toBe('false');
  });

  it('does not expose stale history projection handoff snapshots across sessions', () => {
    const snapshot = {
      sessionId: 'session-a',
      reason: 'session-open',
      createdAtMs: 1,
      items: [createItem('turn-a')],
      mode: 'bottom-tail',
      targetTurnId: 'turn-a',
      footerHeightPx: 0,
    } as const;

    expect(activeSessionHistoryProjectionHandoff(snapshot, 'session-a')).toBe(snapshot);
    expect(activeSessionHistoryProjectionHandoff(snapshot, 'session-b')).toBeNull();
    expect(activeSessionHistoryProjectionHandoff(snapshot, null)).toBeNull();
    expect(activeSessionHistoryProjectionHandoff(null, 'session-a')).toBeNull();
  });

  it('does not request full history projection for ordinary upward reading scroll', () => {
    flowStoreMocks.hasDeferredSessionHistoryProjection.mockReturnValue(true);
    stateMocks.activeSession = createSession('session-a', 'turn-a', {
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'ready',
      isPartial: true,
      dialogTurns: [
        {
          id: 'turn-a',
          sessionId: 'session-a',
          userMessage: { id: 'user-turn-a', content: 'older loaded prompt', timestamp: 1 },
          modelRounds: [],
          status: 'completed',
          startTime: 1,
        },
        {
          id: 'turn-b',
          sessionId: 'session-a',
          userMessage: { id: 'user-turn-b', content: 'latest loaded prompt', timestamp: 2 },
          modelRounds: [],
          status: 'completed',
          startTime: 2,
        },
      ],
    });
    stateMocks.virtualItems = [createItem('turn-a'), createItem('turn-b')];

    act(() => {
      root.render(<VirtualMessageList />);
    });

    const scroller = container.querySelector('[data-virtuoso-scroller="true"]');
    expect(scroller).not.toBeNull();

    act(() => {
      scroller?.dispatchEvent(new WheelEvent('wheel', {
        deltaY: -120,
        bubbles: true,
      }));
    });
    flushAnimationFrame();
    flushAnimationFrame();

    expect(flowStoreMocks.requestSessionFullHistoryProjection).not.toHaveBeenCalled();
    expect(flowStoreMocks.revealPreviousSessionHistoryWindow).toHaveBeenCalledWith('session-a', 'wheel-up');
  });

  it('does not reveal previous history for upward scroll away from the history boundary', () => {
    flowStoreMocks.hasDeferredSessionHistoryProjection.mockReturnValue(true);
    stateMocks.activeSession = createSession('session-a', 'turn-a', {
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'ready',
      isPartial: true,
      dialogTurns: [
        {
          id: 'turn-a',
          sessionId: 'session-a',
          userMessage: { id: 'user-turn-a', content: 'older loaded prompt', timestamp: 1 },
          modelRounds: [],
          status: 'completed',
          startTime: 1,
        },
        {
          id: 'turn-b',
          sessionId: 'session-a',
          userMessage: { id: 'user-turn-b', content: 'latest loaded prompt', timestamp: 2 },
          modelRounds: [],
          status: 'completed',
          startTime: 2,
        },
      ],
    });
    stateMocks.virtualItems = [createItem('turn-a'), createItem('turn-b')];

    act(() => {
      root.render(<VirtualMessageList />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    expect(scroller).not.toBeNull();
    if (scroller) {
      scroller.scrollTop = 2000;
    }

    act(() => {
      scroller?.dispatchEvent(new WheelEvent('wheel', {
        deltaY: -120,
        bubbles: true,
      }));
    });
    flushAnimationFrame();
    flushAnimationFrame();

    expect(flowStoreMocks.requestSessionFullHistoryProjection).not.toHaveBeenCalled();
    expect(flowStoreMocks.revealPreviousSessionHistoryWindow).not.toHaveBeenCalled();
    expect(container.querySelector('[data-history-boundary-status]')).toBeNull();
  });

  it('surfaces a not-ready boundary state when a deferred history window cannot be revealed', () => {
    flowStoreMocks.hasDeferredSessionHistoryProjection.mockReturnValue(true);
    flowStoreMocks.revealPreviousSessionHistoryWindow.mockReturnValue(false);
    stateMocks.activeSession = createSession('session-a', 'turn-a', {
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'ready',
      isPartial: true,
      dialogTurns: [
        {
          id: 'turn-a',
          sessionId: 'session-a',
          userMessage: { id: 'user-turn-a', content: 'latest loaded prompt', timestamp: 1 },
          modelRounds: [],
          status: 'completed',
          startTime: 1,
        },
      ],
    });
    stateMocks.virtualItems = [createItem('turn-a')];

    act(() => {
      root.render(<VirtualMessageList />);
    });

    const scroller = container.querySelector('[data-virtuoso-scroller="true"]');
    expect(scroller).not.toBeNull();

    act(() => {
      scroller?.dispatchEvent(new WheelEvent('wheel', {
        deltaY: -120,
        bubbles: true,
      }));
    });
    flushAnimationFrame();
    flushAnimationFrame();

    expect(flowStoreMocks.requestSessionFullHistoryProjection).not.toHaveBeenCalled();
    expect(flowStoreMocks.revealPreviousSessionHistoryWindow).toHaveBeenCalledWith('session-a', 'wheel-up');
    expect(container.querySelector('[data-history-boundary-status="not-ready"]')?.textContent).toBe('Older history is not ready yet.');
  });

  it('starts background cache preparation for ordinary upward scroll before deferred cache is ready', () => {
    flowStoreMocks.hasPendingSessionHistoryCompletion.mockReturnValue(true);
    stateMocks.activeSession = createSession('session-a', 'turn-a', {
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'ready',
      isPartial: true,
      dialogTurns: [
        {
          id: 'turn-a',
          sessionId: 'session-a',
          userMessage: { id: 'user-turn-a', content: 'latest loaded prompt', timestamp: 1 },
          modelRounds: [],
          status: 'completed',
          startTime: 1,
        },
      ],
    });
    stateMocks.virtualItems = [createItem('turn-a')];

    act(() => {
      root.render(<VirtualMessageList />);
    });

    const scroller = container.querySelector('[data-virtuoso-scroller="true"]');
    expect(scroller).not.toBeNull();

    act(() => {
      scroller?.dispatchEvent(new WheelEvent('wheel', {
        deltaY: -120,
        bubbles: true,
      }));
    });
    flushAnimationFrame();
    flushAnimationFrame();

    expect(flowStoreMocks.requestSessionFullHistoryProjection).not.toHaveBeenCalled();
    expect(flowStoreMocks.revealPreviousSessionHistoryWindow).not.toHaveBeenCalled();
    expect(flowStoreMocks.releaseSessionHistoryCompletionAfterInitialPaint).toHaveBeenCalledWith('session-a', {
      immediate: true,
      reason: 'wheel-up',
    });
    expect(container.querySelector('[data-history-boundary-status="preparing"]')?.textContent).toBe('Preparing older history...');
  });

  it('surfaces a not-ready boundary state when older history work is unavailable', () => {
    stateMocks.activeSession = createSession('session-a', 'turn-a', {
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'ready',
      isPartial: true,
      dialogTurns: [
        {
          id: 'turn-a',
          sessionId: 'session-a',
          userMessage: { id: 'user-turn-a', content: 'latest loaded prompt', timestamp: 1 },
          modelRounds: [],
          status: 'completed',
          startTime: 1,
        },
      ],
    });
    stateMocks.virtualItems = [createItem('turn-a')];

    act(() => {
      root.render(<VirtualMessageList />);
    });

    const scroller = container.querySelector('[data-virtuoso-scroller="true"]');
    expect(scroller).not.toBeNull();

    act(() => {
      scroller?.dispatchEvent(new WheelEvent('wheel', {
        deltaY: -120,
        bubbles: true,
      }));
    });
    flushAnimationFrame();
    flushAnimationFrame();

    expect(flowStoreMocks.requestSessionFullHistoryProjection).not.toHaveBeenCalled();
    expect(flowStoreMocks.revealPreviousSessionHistoryWindow).not.toHaveBeenCalled();
    expect(flowStoreMocks.releaseSessionHistoryCompletionAfterInitialPaint).not.toHaveBeenCalled();
    expect(container.querySelector('[data-history-boundary-status="not-ready"]')?.textContent).toBe('Older history is not ready yet.');
  });
});
