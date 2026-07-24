// @vitest-environment jsdom

import React from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { VirtualMessageList, type VirtualMessageListRef } from './VirtualMessageList';
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
const virtuosoMocks = vi.hoisted(() => ({
  renderedRange: null as { start: number; end: number } | null,
  scrollToIndex: vi.fn(),
}));
const flowStoreMocks = vi.hoisted(() => ({
  hasPendingSessionHistoryCompletion: vi.fn(() => false),
  hasDeferredSessionHistoryProjection: vi.fn(() => false),
  requestSessionFullHistoryProjection: vi.fn(),
  revealPreviousSessionHistoryWindow: vi.fn(() => false),
  releaseSessionHistoryCompletionAfterInitialPaint: vi.fn(() => false),
}));
const inputStateMocks = vi.hoisted(() => ({
  isActive: false,
  isExpanded: false,
  inputHeight: 0,
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
    const [, rerender] = React.useReducer((value: number) => value + 1, 0);
    React.useImperativeHandle(ref, () => ({
      scrollTo: vi.fn(),
      scrollToIndex: vi.fn((options: { index: number }) => {
        virtuosoMocks.scrollToIndex(options);
        const localIndex = Math.max(0, options.index - (props.firstItemIndex ?? 0));
        virtuosoMocks.renderedRange = {
          start: localIndex,
          end: Math.min(props.data?.length ?? 0, localIndex + 4),
        };
        rerender();
      }),
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
        {props.data
          ?.map((item: VirtualItem, index: number) => ({ item, index }))
          .filter(({ index }: { index: number }) => {
            const range = virtuosoMocks.renderedRange;
            return !range || (index >= range.start && index < range.end);
          })
          .map(({ item, index }: { item: VirtualItem; index: number }) => (
            <div
              key={`${item.type}:${item.turnId}`}
              className="virtual-item-wrapper"
              data-turn-id={item.turnId}
              data-virtual-index={index}
              data-item-type={item.type}
            >
              {item.type === 'user-message' ? item.data.content : item.turnId}
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
  useChatInputState: (selector: (state: any) => unknown) => selector(inputStateMocks),
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
  ScrollToLatestBar: ({ visible, onClick }: { visible: boolean; onClick?: () => void }) => (
    <button type="button" data-testid="scroll-to-latest" data-visible={visible ? 'true' : 'false'} onClick={onClick} />
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

function createModelItem(turnId: string): VirtualItem {
  return {
    type: 'model-round',
    turnId,
    isLastRound: true,
    isTurnComplete: true,
    data: {
      id: `round-${turnId}`,
      status: 'completed',
      isStreaming: false,
      items: [{
        id: `text-${turnId}`,
        type: 'text',
        content: 'x'.repeat(2_000),
        status: 'completed',
        timestamp: 1,
      }],
    },
  } as VirtualItem;
}

function createSessionWithTurns(sessionId: string, turnIds: string[], overrides: Partial<Session> = {}): Session {
  return createSession(sessionId, turnIds[0] ?? 'turn-a', {
    dialogTurns: turnIds.map((turnId, index) => ({
      id: turnId,
      sessionId,
      userMessage: { id: `user-${turnId}`, content: turnId, timestamp: index + 1 },
      modelRounds: [],
      status: 'completed',
      startTime: index + 1,
    })),
    ...overrides,
  });
}

function setScrollerGeometry(scroller: HTMLElement, metrics: {
  scrollHeight: number;
  clientHeight: number;
  scrollTop?: number;
}): void {
  Object.defineProperty(scroller, 'scrollHeight', {
    configurable: true,
    value: metrics.scrollHeight,
  });
  Object.defineProperty(scroller, 'clientHeight', {
    configurable: true,
    value: metrics.clientHeight,
  });
  if (metrics.scrollTop !== undefined) {
    scroller.scrollTop = metrics.scrollTop;
  }
}

function createRect(overrides: Partial<DOMRect>): DOMRect {
  const left = overrides.left ?? 0;
  const top = overrides.top ?? 0;
  const width = overrides.width ?? 0;
  const height = overrides.height ?? 0;
  const right = overrides.right ?? left + width;
  const bottom = overrides.bottom ?? top + height;

  return {
    x: overrides.x ?? left,
    y: overrides.y ?? top,
    left,
    top,
    width,
    height,
    right,
    bottom,
    toJSON: () => ({}),
  } as DOMRect;
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
    virtuosoMocks.renderedRange = null;
    virtuosoMocks.scrollToIndex.mockReset();
    flowStoreMocks.hasPendingSessionHistoryCompletion.mockReset();
    flowStoreMocks.hasPendingSessionHistoryCompletion.mockReturnValue(false);
    flowStoreMocks.hasDeferredSessionHistoryProjection.mockReset();
    flowStoreMocks.hasDeferredSessionHistoryProjection.mockReturnValue(false);
    flowStoreMocks.requestSessionFullHistoryProjection.mockReset();
    flowStoreMocks.revealPreviousSessionHistoryWindow.mockReset();
    flowStoreMocks.revealPreviousSessionHistoryWindow.mockReturnValue(false);
    flowStoreMocks.releaseSessionHistoryCompletionAfterInitialPaint.mockReset();
    flowStoreMocks.releaseSessionHistoryCompletionAfterInitialPaint.mockReturnValue(false);
    inputStateMocks.isActive = false;
    inputStateMocks.isExpanded = false;
    inputStateMocks.inputHeight = 0;
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

  it('keeps static initial history position when background updates arrive after an upward scroll', () => {
    let nowMs = 1_000;
    const nowSpy = vi.spyOn(performance, 'now').mockImplementation(() => nowMs);

    try {
      stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-a', 'turn-b'], {
        isHistorical: false,
        historyState: 'ready',
        contextRestoreState: 'pending',
        isPartial: true,
      });
      stateMocks.virtualItems = [createItem('turn-a'), createItem('turn-b')];

      act(() => {
        root.render(<VirtualMessageList />);
      });

      const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
      expect(scroller).not.toBeNull();
      if (!scroller) {
        return;
      }

      setScrollerGeometry(scroller, {
        scrollHeight: 5_000,
        clientHeight: 1_000,
        scrollTop: 4_000,
      });

      act(() => {
        scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
      });

      act(() => {
        scroller.dispatchEvent(new WheelEvent('wheel', {
          deltaY: -720,
          bubbles: true,
        }));
        scroller.scrollTop = 1_800;
        scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
      });
      flushAnimationFrame();
      expect(scroller.scrollTop).toBe(1_800);

      nowMs = 2_000;
      stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-a', 'turn-b', 'turn-c'], {
        isHistorical: false,
        historyState: 'ready',
        contextRestoreState: 'pending',
        isPartial: true,
      });
      stateMocks.virtualItems = [createItem('turn-a'), createItem('turn-b'), createItem('turn-c')];

      act(() => {
        root.render(<VirtualMessageList />);
      });

      expect(scroller.scrollTop).toBe(1_800);
    } finally {
      nowSpy.mockRestore();
    }
  });

  it('materializes an unrendered turn with an immediate auto index jump', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    const turnIds = Array.from(
      { length: 24 },
      (_, index) => `turn-${String(index + 1).padStart(2, '0')}`,
    );
    const targetTurnId = 'turn-02';

    stateMocks.activeSession = createSessionWithTurns('session-a', turnIds);
    stateMocks.virtualItems = turnIds.flatMap(turnId => [
      createItem(turnId),
      createModelItem(turnId),
    ]);
    virtuosoMocks.renderedRange = { start: 40, end: 48 };

    act(() => {
      root.render(<VirtualMessageList ref={listRef} />);
    });

    expect(
      container.querySelector(
        `[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`,
      ),
    ).toBeNull();
    virtuosoMocks.scrollToIndex.mockClear();

    let status: ReturnType<VirtualMessageListRef['pinTurnToTopWithStatus']> = 'rejected';
    act(() => {
      status = listRef.current?.pinTurnToTopWithStatus(targetTurnId, {
        behavior: 'smooth',
        pinMode: 'transient',
      }) ?? 'rejected';
    });

    expect(status).toBe('pending');
    expect(virtuosoMocks.scrollToIndex).toHaveBeenCalledWith(expect.objectContaining({
      align: 'start',
      behavior: 'auto',
    }));

    const target = container.querySelector<HTMLElement>(
      `[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`,
    );
    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    expect(target).not.toBeNull();
    expect(scroller).not.toBeNull();
    if (!target || !scroller) {
      return;
    }

    setScrollerGeometry(scroller, {
      scrollHeight: 6_000,
      clientHeight: 1_000,
      scrollTop: 4_000,
    });
    vi.spyOn(scroller, 'getBoundingClientRect').mockReturnValue(createRect({
      top: 0,
      bottom: 1_000,
      height: 1_000,
    }));
    const targetDocumentTop = 4_500;
    vi.spyOn(target, 'getBoundingClientRect').mockImplementation(() => {
      const top = targetDocumentTop - scroller.scrollTop;
      return createRect({
        top,
        bottom: top + 40,
        height: 40,
      });
    });

    flushAnimationFrame();
    flushAnimationFrame();
    flushAnimationFrame();
    expect(scroller.scrollTop).toBe(4_443);
    expect(target.getBoundingClientRect().top).toBe(57);
    expect(virtuosoMocks.scrollToIndex).toHaveBeenCalledTimes(1);
  });

  it('centers the exact text range when navigating to a search match', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-a', 'turn-b']);
    stateMocks.virtualItems = [
      createItem('turn-a'),
      {
        ...createItem('turn-b'),
        data: {
          ...createItem('turn-b').data,
          content: 'prefix needle suffix',
        },
      },
    ];

    act(() => {
      root.render(<VirtualMessageList ref={listRef} />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    expect(scroller).not.toBeNull();
    if (!scroller) {
      return;
    }

    setScrollerGeometry(scroller, {
      scrollHeight: 2_000,
      clientHeight: 500,
      scrollTop: 0,
    });
    vi.spyOn(scroller, 'getBoundingClientRect').mockReturnValue(createRect({
      top: 0,
      bottom: 500,
      height: 500,
    }));
    Object.defineProperty(Range.prototype, 'getBoundingClientRect', {
      configurable: true,
      value: vi.fn(() => createRect({
        top: 800,
        bottom: 820,
        height: 20,
      })),
    });
    virtuosoMocks.scrollToIndex.mockClear();

    act(() => {
      listRef.current?.scrollToSearchMatch({
        virtualItemIndex: 1,
        query: 'needle',
      });
    });
    flushAnimationFrame();

    expect(virtuosoMocks.scrollToIndex).toHaveBeenCalledWith(expect.objectContaining({
      align: 'center',
      behavior: 'auto',
    }));
    expect(scroller.scrollTop).toBe(560);
    delete (Range.prototype as Range & {
      getBoundingClientRect?: () => DOMRect;
    }).getBoundingClientRect;
  });

  it('renders only a bounded static-history window around an omitted search result', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    const turnIds = Array.from({ length: 8 }, (_, index) => `turn-${index}`);
    const targetTurnId = 'turn-1';
    const latestTurnId = 'turn-7';

    stateMocks.activeSession = createSessionWithTurns('session-a', turnIds, {
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      isPartial: true,
    });
    stateMocks.virtualItems = turnIds.flatMap(turnId => [
      createItem(turnId),
      createModelItem(turnId),
    ]);

    act(() => {
      root.render(<VirtualMessageList ref={listRef} />);
    });

    expect(container.querySelector(
      `[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`,
    )).toBeNull();
    expect(container.querySelector(
      `[data-turn-id="${latestTurnId}"][data-item-type="user-message"]`,
    )).not.toBeNull();

    act(() => {
      listRef.current?.scrollToSearchMatch({
        virtualItemIndex: 2,
        query: targetTurnId,
      });
    });
    flushAnimationFrame();

    expect(container.querySelector(
      `[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`,
    )).not.toBeNull();
    expect(container.querySelector(
      `[data-turn-id="${latestTurnId}"][data-item-type="user-message"]`,
    )).toBeNull();
    expect(container.querySelector(
      '[data-history-initial-render-tail-spacer="true"]',
    )).not.toBeNull();
    expect(container.querySelectorAll('.virtual-item-wrapper').length)
      .toBeLessThan(stateMocks.virtualItems.length);
  });

  it('keeps a static initial history turn pin from being pulled back to bottom by the initial guard', () => {
    let nowMs = 1_000;
    const nowSpy = vi.spyOn(performance, 'now').mockImplementation(() => nowMs);
    const listRef = React.createRef<VirtualMessageListRef>();

    try {
      stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-a', 'turn-b'], {
        isHistorical: false,
        historyState: 'ready',
        contextRestoreState: 'pending',
        isPartial: true,
      });
      stateMocks.virtualItems = [createItem('turn-a'), createItem('turn-b')];

      act(() => {
        root.render(<VirtualMessageList ref={listRef} />);
      });

      const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
      const target = container.querySelector<HTMLElement>('[data-turn-id="turn-a"][data-item-type="user-message"]');
      expect(scroller).not.toBeNull();
      expect(target).not.toBeNull();
      if (!scroller || !target) {
        return;
      }

      setScrollerGeometry(scroller, {
        scrollHeight: 5_000,
        clientHeight: 1_000,
        scrollTop: 4_000,
      });
      Object.defineProperty(scroller, 'scrollTo', {
        configurable: true,
        value: vi.fn((options?: ScrollToOptions) => {
          if (typeof options?.top === 'number') {
            scroller.scrollTop = options.top;
          }
        }),
      });
      vi.spyOn(scroller, 'getBoundingClientRect').mockReturnValue(createRect({
        top: 40,
        bottom: 1_040,
        height: 1_000,
      }));
      vi.spyOn(target, 'getBoundingClientRect').mockReturnValue(createRect({
        top: -1_200,
        bottom: -1_160,
        height: 40,
      }));

      act(() => {
        scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
      });

      stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-a', 'turn-b', 'turn-c'], {
        isHistorical: false,
        historyState: 'ready',
        contextRestoreState: 'pending',
        isPartial: true,
      });
      stateMocks.virtualItems = [createItem('turn-a'), createItem('turn-b'), createItem('turn-c')];
      setScrollerGeometry(scroller, {
        scrollHeight: 5_200,
        clientHeight: 1_000,
      });

      act(() => {
        root.render(<VirtualMessageList ref={listRef} />);
      });

      expect(scroller.scrollTop).toBe(4_200);

      let didPin = false;
      act(() => {
        didPin = listRef.current?.pinTurnToTop('turn-a', { behavior: 'auto' }) ?? false;
      });

      expect(didPin).toBe(true);
      const pinnedScrollTop = scroller.scrollTop;
      expect(pinnedScrollTop).toBeLessThan(4_200);

      expect(rafCallbacks.length).toBeGreaterThan(0);
      for (let frame = 0; frame < 4; frame += 1) {
        nowMs += 16;
        flushAnimationFrame();
        expect(scroller.scrollTop).toBe(pinnedScrollTop);
      }
    } finally {
      nowSpy.mockRestore();
    }
  });

  it('keeps latest reachable after pinning an older turn outside the static initial tail', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    const onUserScrollIntent = vi.fn();
    const turnIds = Array.from({ length: 8 }, (_, index) => `turn-${index}`);
    const targetTurnId = 'turn-1';
    const latestTurnId = 'turn-7';

    stateMocks.activeSession = createSessionWithTurns('session-a', turnIds, {
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      isPartial: true,
    });
    stateMocks.virtualItems = turnIds.flatMap(turnId => [
      createItem(turnId),
      createModelItem(turnId),
    ]);

    act(() => {
      root.render(<VirtualMessageList ref={listRef} onUserScrollIntent={onUserScrollIntent} />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    expect(scroller).not.toBeNull();
    if (!scroller) {
      return;
    }

    Object.defineProperty(scroller, 'clientHeight', {
      configurable: true,
      value: 1_000,
    });
    Object.defineProperty(scroller, 'scrollHeight', {
      configurable: true,
      get: () => (
        container.querySelector(`[data-turn-id="${latestTurnId}"][data-item-type="user-message"]`)
          ? 12_000
          : 9_000
      ),
    });
    scroller.scrollTop = 11_000;
    Object.defineProperty(scroller, 'scrollTo', {
      configurable: true,
      value: vi.fn((options?: ScrollToOptions) => {
        if (typeof options?.top === 'number') {
          scroller.scrollTop = options.top;
        }
      }),
    });

    expect(container.querySelector(`[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`)).toBeNull();
    expect(container.querySelector(`[data-turn-id="${latestTurnId}"][data-item-type="user-message"]`)).not.toBeNull();

    let pinStatus: ReturnType<VirtualMessageListRef['pinTurnToTopWithStatus']> = 'rejected';
    act(() => {
      pinStatus = listRef.current?.pinTurnToTopWithStatus(targetTurnId, {
        behavior: 'auto',
      }) ?? 'rejected';
    });

    expect(pinStatus).toBe('pending');
    expect(scroller.scrollTop).toBeLessThan(11_000);
    expect(container.querySelector(`[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`)).not.toBeNull();
    expect(container.querySelector(`[data-turn-id="${latestTurnId}"][data-item-type="user-message"]`)).toBeNull();
    expect(container.querySelector('[data-history-initial-render-tail-spacer="true"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="scroll-to-latest"]')?.getAttribute('data-visible')).toBe('true');

    act(() => {
      container.querySelector<HTMLElement>('[data-testid="scroll-to-latest"]')?.dispatchEvent(
        new MouseEvent('click', { bubbles: true }),
      );
    });

    expect(onUserScrollIntent).toHaveBeenCalledTimes(1);
    expect(container.querySelector(`[data-turn-id="${latestTurnId}"][data-item-type="user-message"]`)).not.toBeNull();
    expect(scroller.scrollTop).toBe(11_000);
  });

  it('keeps static initial history position when footer height changes after an upward scroll', () => {
    let nowMs = 1_000;
    const nowSpy = vi.spyOn(performance, 'now').mockImplementation(() => nowMs);

    try {
      stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-a', 'turn-b'], {
        isHistorical: false,
        historyState: 'ready',
        contextRestoreState: 'pending',
        isPartial: true,
      });
      stateMocks.virtualItems = [createItem('turn-a'), createItem('turn-b')];

      act(() => {
        root.render(<VirtualMessageList />);
      });

      const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
      expect(scroller).not.toBeNull();
      if (!scroller) {
        return;
      }

      setScrollerGeometry(scroller, {
        scrollHeight: 5_000,
        clientHeight: 1_000,
        scrollTop: 4_000,
      });

      act(() => {
        scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
      });

      act(() => {
        scroller.dispatchEvent(new WheelEvent('wheel', {
          deltaY: -720,
          bubbles: true,
        }));
        scroller.scrollTop = 1_800;
        scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
      });
      flushAnimationFrame();
      expect(scroller.scrollTop).toBe(1_800);

      nowMs = 2_000;
      inputStateMocks.isActive = true;
      inputStateMocks.inputHeight = 320;
      setScrollerGeometry(scroller, {
        scrollHeight: 5_320,
        clientHeight: 1_000,
      });

      act(() => {
        root.render(<VirtualMessageList />);
      });

      expect(scroller.scrollTop).toBe(1_800);
    } finally {
      nowSpy.mockRestore();
    }
  });

  it('does not treat a collapse-compensated bottom as user-left-bottom', () => {
    stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-a', 'turn-b'], {
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      isPartial: true,
    });
    stateMocks.virtualItems = [createItem('turn-a'), createItem('turn-b')];

    act(() => {
      root.render(<VirtualMessageList />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    expect(scroller).not.toBeNull();
    if (!scroller) {
      return;
    }

    setScrollerGeometry(scroller, {
      scrollHeight: 5_000,
      clientHeight: 1_000,
      scrollTop: 4_000,
    });

    act(() => {
      scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
      window.dispatchEvent(new CustomEvent('flowchat:tool-card-collapse-intent', {
        detail: {
          toolId: 'tool-a',
          cardHeight: 300,
          reason: 'test-collapse',
        },
      }));
    });
    setScrollerGeometry(scroller, {
      scrollHeight: 5_300,
      clientHeight: 1_000,
      scrollTop: 4_000,
    });

    act(() => {
      scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
    });

    stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-a', 'turn-b', 'turn-c'], {
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      isPartial: true,
    });
    stateMocks.virtualItems = [createItem('turn-a'), createItem('turn-b'), createItem('turn-c')];
    setScrollerGeometry(scroller, {
      scrollHeight: 5_600,
      clientHeight: 1_000,
    });

    act(() => {
      root.render(<VirtualMessageList />);
    });

    expect(scroller.scrollTop).toBeGreaterThan(4_000);
    expect(scroller.scrollTop).toBeLessThanOrEqual(4_600);
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
