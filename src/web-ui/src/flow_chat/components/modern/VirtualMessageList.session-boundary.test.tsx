// @vitest-environment jsdom

import React from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  VirtualMessageList,
  type VirtualMessageListRef,
} from './VirtualMessageList';
import { getLeadingVirtualItemIndexDelta } from './virtualMessageListLayout';
import { FlowChatViewportCoordinator } from './FlowChatViewportCoordinator';
import {
  clampPinReservationPxToViewport,
  consumeBottomReservationForContentGrowth,
  ensureCollapseReservationForScrollTop,
  getCanceledUnsettledStickyPinGrowthPx,
  isTurnPinRequestIdentityCurrent,
  protectCurrentCollapseReservation,
  reconcileUnsignaledShrinkReservation,
  releasePinReservationForUserNavigation,
  resolveAutoCollapseAnchorScrollTop,
  resolveCollapseIntentSettlementStrategy,
  resolveFollowingTailShrinkClampRecovery,
  resolveProvisionalStickyPinReservationPx,
  resolveStickyPinGrowthSettlementStrategy,
  settleRetainedCollapseReservationForAnchor,
  settleCollapseReservationForViewport,
  shouldBypassShrinkCompensationInTailFollow,
  shouldClearExpiredProvisionalStickyPin,
  shouldSyncPhysicalBottom,
  shouldSuppressFollowingTailNegativeScrollBy,
  transferCollapseReservationToPin,
  transferPinReservationToProtectedCollapse,
} from './flowChatScrollStability';
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
  scrollerScrollTo: vi.fn(),
  scrollToIndex: vi.fn(),
  initialTopMostItemIndex: null as unknown,
  initialTopMostItemIndexHistory: [] as unknown[],
  increaseViewportBy: null as unknown,
  rangeChanged: null as (() => void) | null,
}));
const flowStoreMocks = vi.hoisted(() => ({
  hasPendingSessionHistoryCompletion: vi.fn(() => false),
  hasDeferredSessionHistoryProjection: vi.fn(() => false),
  revealPreviousSessionHistoryWindow: vi.fn(() => false),
  releaseSessionHistoryCompletionAfterInitialPaint: vi.fn(() => false),
}));
const inputStateMocks = vi.hoisted(() => ({
  isActive: false,
  isExpanded: false,
  inputHeight: 0,
}));
const activeSessionStateMocks = vi.hoisted(() => ({
  isProcessing: false,
}));
const flowDiagnosticsMocks = vi.hoisted(() => ({
  enabled: false,
  trace: vi.fn(),
}));
const resizeObserverMocks = vi.hoisted(() => ({
  callbacks: [] as Array<() => void>,
}));

vi.mock('@/infrastructure/diagnostics/flowChatDiagnostics', () => ({
  flowChatDiagnostics: {
    isEnabled: () => flowDiagnosticsMocks.enabled,
    trace: flowDiagnosticsMocks.trace,
  },
}));

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
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
    virtuosoMocks.initialTopMostItemIndex = props.initialTopMostItemIndex;
    virtuosoMocks.initialTopMostItemIndexHistory.push(props.initialTopMostItemIndex);
    virtuosoMocks.increaseViewportBy = props.increaseViewportBy;
    virtuosoMocks.rangeChanged = props.rangeChanged ?? null;
    React.useImperativeHandle(ref, () => ({
      scrollTo: vi.fn(),
      scrollToIndex: vi.fn((options: { index: number }) => {
        virtuosoMocks.scrollToIndex(options);
        const localIndex = Math.max(0, options.index);
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

      if (typeof scrollerRef.current.scrollTo !== 'function') {
        Object.defineProperty(scrollerRef.current, 'scrollTo', {
          configurable: true,
          writable: true,
          value: (options?: ScrollToOptions) => {
            virtuosoMocks.scrollerScrollTo(options);
            if (typeof options?.top === 'number') {
              scrollerRef.current!.scrollTop = options.top;
            }
          },
        });
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
        {props.components?.Header ? <props.components.Header context={props.context} /> : null}
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
        {props.components?.Footer ? <props.components.Footer context={props.context} /> : null}
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
    isProcessing: activeSessionStateMocks.isProcessing,
    processingPhase: null,
  }),
}));

vi.mock('../../store/chatInputStateStore', () => ({
  useChatInputState: (selector: (state: any) => unknown) => selector(inputStateMocks),
}));

vi.mock('../../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({
      sessions: new Map(stateMocks.activeSession ? [[stateMocks.activeSession.sessionId, stateMocks.activeSession]] : []),
    }),
    hasPendingSessionHistoryCompletion: flowStoreMocks.hasPendingSessionHistoryCompletion,
    hasDeferredSessionHistoryProjection: flowStoreMocks.hasDeferredSessionHistoryProjection,
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
      constructor(callback: ResizeObserverCallback) {
        resizeObserverMocks.callbacks.push(() => {
          callback([], this as unknown as ResizeObserver);
        });
      }
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
    virtuosoMocks.scrollerScrollTo.mockReset();
    virtuosoMocks.scrollToIndex.mockReset();
    virtuosoMocks.initialTopMostItemIndex = null;
    virtuosoMocks.initialTopMostItemIndexHistory = [];
    virtuosoMocks.increaseViewportBy = null;
    virtuosoMocks.rangeChanged = null;
    flowStoreMocks.hasPendingSessionHistoryCompletion.mockReset();
    flowStoreMocks.hasPendingSessionHistoryCompletion.mockReturnValue(false);
    flowStoreMocks.hasDeferredSessionHistoryProjection.mockReset();
    flowStoreMocks.hasDeferredSessionHistoryProjection.mockReturnValue(false);
    flowStoreMocks.revealPreviousSessionHistoryWindow.mockReset();
    flowStoreMocks.revealPreviousSessionHistoryWindow.mockReturnValue(false);
    flowStoreMocks.releaseSessionHistoryCompletionAfterInitialPaint.mockReset();
    flowStoreMocks.releaseSessionHistoryCompletionAfterInitialPaint.mockReturnValue(false);
    inputStateMocks.isActive = false;
    inputStateMocks.isExpanded = false;
    inputStateMocks.inputHeight = 0;
    activeSessionStateMocks.isProcessing = false;
    flowDiagnosticsMocks.enabled = false;
    flowDiagnosticsMocks.trace.mockReset();
    resizeObserverMocks.callbacks = [];
  });

  it('keeps Virtuoso absolute indexes stable across prepend and remote-side trimming', () => {
    const previous = ['turn-3', 'turn-4', 'turn-5', 'turn-6'].map(createItem);
    const prepended = ['turn-1', 'turn-2', 'turn-3', 'turn-4', 'turn-5'].map(createItem);
    const trimmed = ['turn-4', 'turn-5', 'turn-6', 'turn-7'].map(createItem);

    const getStableKey = (item: VirtualItem) => `${item.type}:${item.turnId}`;
    expect(getLeadingVirtualItemIndexDelta(previous, prepended, getStableKey)).toBe(-2);
    expect(getLeadingVirtualItemIndexDelta(previous, trimmed, getStableKey)).toBe(1);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.unstubAllGlobals();
  });

  it('keeps the Virtuoso footer mounted across parent rerenders', () => {
    stateMocks.activeSession = createSession('session-a', 'turn-a');
    stateMocks.virtualItems = [createItem('turn-a')];

    act(() => {
      root.render(<VirtualMessageList />);
    });
    const firstFooter = container.querySelector('.message-list-footer');
    expect(firstFooter).not.toBeNull();
    if (!(firstFooter instanceof HTMLElement)) {
      return;
    }
    firstFooter.style.height = '900px';
    firstFooter.style.minHeight = '900px';

    act(() => {
      root.render(<VirtualMessageList />);
    });

    expect(container.querySelector('.message-list-footer')).toBe(firstFooter);
    expect(firstFooter.style.height).toBe('900px');
    expect(firstFooter.style.minHeight).toBe('900px');
  });

  it('rebases retained viewport geometry without converting inactive zero size into footer space', () => {
    stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-a', 'turn-b']);
    stateMocks.virtualItems = ['turn-a', 'turn-b'].flatMap(turnId => [
      createItem(turnId),
      createModelItem(turnId),
    ]);

    act(() => {
      root.render(<VirtualMessageList isViewportActive />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    const footer = container.querySelector<HTMLElement>('.message-list-footer');
    expect(scroller).not.toBeNull();
    expect(footer).not.toBeNull();
    if (!scroller || !footer) {
      return;
    }

    let hidden = false;
    let naturalContentHeight = 36_000;
    let scrollTop = 34_500;
    Object.defineProperties(scroller, {
      clientHeight: {
        configurable: true,
        get: () => hidden ? 0 : 1_000,
      },
      scrollHeight: {
        configurable: true,
        get: () => hidden
          ? 0
          : naturalContentHeight + (Number.parseFloat(footer.style.height) || 0),
      },
      scrollTop: {
        configurable: true,
        get: () => scrollTop,
        set: (value: number) => {
          scrollTop = value;
        },
      },
    });
    vi.spyOn(scroller, 'getBoundingClientRect').mockImplementation(() => createRect({
      top: 0,
      bottom: hidden ? 0 : 1_000,
      height: hidden ? 0 : 1_000,
    }));

    act(() => {
      root.render(<VirtualMessageList isViewportActive />);
    });
    const baselineFooterHeight = Number.parseFloat(footer.style.height);

    act(() => {
      root.render(<VirtualMessageList isViewportActive={false} />);
    });
    hidden = true;
    scrollTop = 0;
    act(() => {
      resizeObserverMocks.callbacks.at(-1)?.();
    });
    for (let frame = 0; frame < 4; frame += 1) {
      flushAnimationFrame();
    }

    expect(Number.parseFloat(footer.style.height)).toBe(baselineFooterHeight);

    naturalContentHeight += 600;
    hidden = false;
    act(() => {
      root.render(<VirtualMessageList isViewportActive />);
    });
    for (let frame = 0; frame < 4; frame += 1) {
      flushAnimationFrame();
    }

    expect(Number.parseFloat(footer.style.height)).toBeLessThanOrEqual(
      baselineFooterHeight + 1,
    );
    expect(scroller.scrollTop).toBeGreaterThan(0);
    expect(scroller.scrollHeight).toBeLessThan(40_000);
  });

  it('reconciles a stream that ends while its viewport is inactive after reactivation', () => {
    flowDiagnosticsMocks.enabled = true;
    const listRef = React.createRef<VirtualMessageListRef>();
    const session = createSession('session-a', 'turn-a');
    session.dialogTurns[0].status = 'processing';
    session.dialogTurns[0].modelRounds = [{
      id: 'round-turn-a',
      status: 'streaming',
      isStreaming: true,
      items: [],
      startTime: 1,
    } as typeof session.dialogTurns[number]['modelRounds'][number]];
    stateMocks.activeSession = session;
    stateMocks.virtualItems = [createItem('turn-a'), createModelItem('turn-a')];

    act(() => {
      root.render(<VirtualMessageList ref={listRef} isViewportActive />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    const footer = container.querySelector<HTMLElement>('.message-list-footer');
    const target = container.querySelector<HTMLElement>(
      '[data-turn-id="turn-a"][data-item-type="user-message"]',
    );
    expect(scroller).not.toBeNull();
    expect(footer).not.toBeNull();
    expect(target).not.toBeNull();
    if (!scroller || !footer || !target) {
      return;
    }

    let hidden = false;
    let scrollTop = 200;
    Object.defineProperties(scroller, {
      clientHeight: {
        configurable: true,
        get: () => hidden ? 0 : 1_000,
      },
      scrollHeight: {
        configurable: true,
        get: () => hidden
          ? 0
          : 1_200 + (Number.parseFloat(footer.style.height) || 0),
      },
      scrollTop: {
        configurable: true,
        get: () => scrollTop,
        set: (value: number) => {
          scrollTop = value;
        },
      },
    });
    vi.spyOn(scroller, 'getBoundingClientRect').mockImplementation(() => createRect({
      top: 0,
      bottom: hidden ? 0 : 1_000,
      height: hidden ? 0 : 1_000,
    }));
    vi.spyOn(target, 'getBoundingClientRect').mockImplementation(() => {
      const top = hidden ? 0 : 700 - scrollTop;
      return createRect({ top, bottom: top + 40, height: hidden ? 0 : 40 });
    });

    act(() => {
      listRef.current?.pinTurnToTopWithStatus('turn-a', {
        behavior: 'auto',
        pinMode: 'sticky-latest',
      });
    });
    expect(Number.parseFloat(footer.style.height)).toBeGreaterThan(0);

    act(() => {
      root.render(<VirtualMessageList ref={listRef} isViewportActive={false} />);
    });
    hidden = true;
    scrollTop = 0;
    stateMocks.activeSession = {
      ...session,
      dialogTurns: session.dialogTurns.map(turn => ({
        ...turn,
        status: 'completed',
        modelRounds: turn.modelRounds.map(round => ({
          ...round,
          status: 'completed',
          isStreaming: false,
        })),
      })),
    };
    flowDiagnosticsMocks.trace.mockClear();
    act(() => {
      root.render(<VirtualMessageList ref={listRef} isViewportActive={false} />);
      resizeObserverMocks.callbacks.at(-1)?.();
    });
    expect(flowDiagnosticsMocks.trace).not.toHaveBeenCalledWith(expect.objectContaining({
      location: 'VirtualMessageList.streamEndReconciliation',
    }));

    hidden = false;
    act(() => {
      root.render(<VirtualMessageList ref={listRef} isViewportActive />);
    });
    for (let frame = 0; frame < 4; frame += 1) {
      flushAnimationFrame();
    }

    expect(flowDiagnosticsMocks.trace).toHaveBeenCalledWith(expect.objectContaining({
      location: 'VirtualMessageList.streamEndReconciliation',
    }));
    expect(Number.parseFloat(footer.style.height)).toBeLessThanOrEqual(1_124);
    expect(scroller.scrollHeight).toBeLessThan(3_000);
  });

  it('routes jump-to-latest through the presentation owner while reading a history window', () => {
    const onRequestJumpToLatest = vi.fn();
    stateMocks.activeSession = createSession('session-a', 'turn-10');
    stateMocks.virtualItems = [createItem('turn-10')];

    act(() => {
      root.render(
        <VirtualMessageList
          presentationMode="history-window"
          historyWindow={{
            startOrdinal: 2,
            endOrdinalExclusive: 7,
            targetTurnId: 'turn-5',
            mode: 'history-window',
          }}
          presentationRevision={1}
          onRequestJumpToLatest={onRequestJumpToLatest}
        />,
      );
    });

    const jumpButton = container.querySelector<HTMLButtonElement>('[data-testid="scroll-to-latest"]');
    expect(jumpButton?.dataset.visible).toBe('true');
    act(() => jumpButton?.click());
    expect(onRequestJumpToLatest).toHaveBeenCalledOnce();
  });

  it('keeps canonical streaming output hidden from tail follow in history-window mode', () => {
    activeSessionStateMocks.isProcessing = true;
    stateMocks.activeSession = createSession('session-a', 'turn-10', {
      dialogTurns: [{
        ...createSession('session-a', 'turn-10').dialogTurns[0],
        status: 'processing',
      }],
    });
    const historyItems = ['turn-3', 'turn-4', 'turn-5'].map(createItem);
    stateMocks.virtualItems = historyItems;

    act(() => {
      root.render(
        <VirtualMessageList
          items={historyItems}
          presentationMode="history-window"
          historyWindow={{
            startOrdinal: 2,
            endOrdinalExclusive: 5,
            targetTurnId: 'turn-4',
            mode: 'history-window',
          }}
          presentationRevision={1}
        />,
      );
    });

    expect(container.querySelector('[data-testid="flowchat-message-list"]')?.getAttribute(
      'data-streaming-output',
    )).toBe('false');
  });

  it('allows live-tail follow while retaining a history-window projection', () => {
    activeSessionStateMocks.isProcessing = true;
    stateMocks.activeSession = createSession('session-a', 'turn-10', {
      dialogTurns: [{
        ...createSession('session-a', 'turn-10').dialogTurns[0],
        status: 'processing',
      }],
    });
    const historyItems = ['turn-3', 'turn-4', 'turn-5'].map(createItem);
    stateMocks.virtualItems = historyItems;
    const onRequestJumpToLatest = vi.fn();

    act(() => {
      root.render(
        <VirtualMessageList
          items={historyItems}
          presentationMode="history-window"
          viewportMode="live-tail"
          historyWindow={null}
          presentationRevision={1}
          onRequestJumpToLatest={onRequestJumpToLatest}
        />,
      );
    });

    expect(container.querySelector('[data-testid="flowchat-message-list"]')?.getAttribute(
      'data-viewport-mode',
    )).toBe('live-tail');
    expect(container.querySelector('[data-testid="flowchat-message-list"]')?.getAttribute(
      'data-streaming-output',
    )).toBe('true');

    const jumpButton = container.querySelector<HTMLButtonElement>('[data-testid="scroll-to-latest"]');
    expect(jumpButton).not.toBeNull();
    act(() => jumpButton?.click());
    expect(onRequestJumpToLatest).not.toHaveBeenCalled();
  });

  it('waits for quiet scroll input before capturing an adjacent history anchor', async () => {
    let prepareViewportForPresentationCommit:
      | (() => boolean | void | Promise<boolean | void>)
      | undefined;
    let resolveBoundaryIntent: ((handled: boolean) => void) | undefined;
    const boundaryIntent = new Promise<boolean>(resolve => {
      resolveBoundaryIntent = resolve;
    });
    const onBoundaryIntent = vi.fn((
      _direction: 'before' | 'after',
      options?: {
        prepareViewportForPresentationCommit?: () => boolean | void | Promise<boolean | void>;
      },
    ) => {
      prepareViewportForPresentationCommit = options?.prepareViewportForPresentationCommit;
      return boundaryIntent;
    });
    stateMocks.activeSession = createSession('session-a', 'turn-10');
    const initialItems = ['turn-3', 'turn-4', 'turn-5', 'turn-6', 'turn-7'].map(createItem);
    stateMocks.virtualItems = initialItems;

    act(() => {
      root.render(
        <VirtualMessageList
          items={initialItems}
          presentationMode="history-window"
          historyWindow={{
            startOrdinal: 2,
            endOrdinalExclusive: 7,
            targetTurnId: 'turn-5',
            mode: 'history-window',
          }}
          presentationRevision={1}
          historyBoundaryState={{ before: 'idle', after: 'idle' }}
          onHistoryWindowBoundaryIntent={onBoundaryIntent}
        />,
      );
    });

    const scroller = container.querySelector<HTMLElement>('[data-testid="virtuoso"]');
    const anchor = container.querySelector<HTMLElement>('[data-turn-id="turn-3"]');
    expect(scroller).not.toBeNull();
    expect(anchor).not.toBeNull();
    if (!scroller || !anchor) return;

    for (let frame = 0; frame < 8 && rafCallbacks.length > 0; frame += 1) {
      flushAnimationFrame();
    }

    setScrollerGeometry(scroller, {
      clientHeight: 100,
      scrollHeight: 500,
      scrollTop: 40,
    });
    scroller.getBoundingClientRect = () => createRect({ top: 0, bottom: 100, height: 100 });
    let anchorDocumentTop = 60;
    anchor.getBoundingClientRect = () => {
      const top = anchorDocumentTop - scroller.scrollTop;
      return createRect({ top, bottom: top + 20, height: 20 });
    };

    vi.useFakeTimers();
    try {
      act(() => {
        scroller.dispatchEvent(new WheelEvent('wheel', { deltaY: -20, bubbles: true }));
      });
      expect(onBoundaryIntent).toHaveBeenCalledWith('before', expect.objectContaining({
        prepareViewportForPresentationCommit: expect.any(Function),
      }));

      scroller.scrollTop = 80;
      let preparationSettled = false;
      const preparation = Promise.resolve(prepareViewportForPresentationCommit?.()).then(result => {
        preparationSettled = true;
        return result;
      });

      act(() => {
        vi.advanceTimersByTime(319);
        scroller.dispatchEvent(new WheelEvent('wheel', { deltaY: -20, bubbles: true }));
        vi.advanceTimersByTime(319);
      });
      await Promise.resolve();
      expect(preparationSettled).toBe(false);

      await act(async () => {
        vi.advanceTimersByTime(1);
        expect(await preparation).not.toBe(false);
      });

      const extendedItems = ['turn-1', 'turn-2', ...initialItems.map(item => item.turnId)].map(createItem);
      act(() => {
        anchorDocumentTop = 120;
        root.render(
          <VirtualMessageList
            items={extendedItems}
            presentationMode="history-window"
            historyWindow={{
              startOrdinal: 0,
              endOrdinalExclusive: 7,
              targetTurnId: 'turn-5',
              mode: 'history-window',
            }}
            presentationRevision={2}
            historyBoundaryState={{ before: 'loading', after: 'idle' }}
            onHistoryWindowBoundaryIntent={onBoundaryIntent}
          />,
        );
        resolveBoundaryIntent?.(true);
      });
      await act(async () => {
        await boundaryIntent;
      });
      flushAnimationFrame();
      flushAnimationFrame();

      expect(scroller.scrollTop).toBe(140);
      expect(container.querySelector('[data-history-paging-sentinel="loading"]')).not.toBeNull();

      act(() => {
        root.render(
          <VirtualMessageList
            items={extendedItems}
            presentationMode="history-window"
            historyWindow={{
              startOrdinal: 0,
              endOrdinalExclusive: 7,
              targetTurnId: 'turn-5',
              mode: 'history-window',
            }}
            presentationRevision={2}
            historyBoundaryState={{ before: 'idle', after: 'idle' }}
            onHistoryWindowBoundaryIntent={onBoundaryIntent}
          />,
        );
      });
      expect(container.querySelector('[data-history-paging-sentinel]')).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it('releases the visible element anchor when an adjacent history request is not handled', async () => {
    const onBoundaryIntent = vi.fn(async () => false);
    stateMocks.activeSession = createSession('session-a', 'turn-10');
    const initialItems = ['turn-3', 'turn-4', 'turn-5', 'turn-6', 'turn-7'].map(createItem);
    stateMocks.virtualItems = initialItems;

    act(() => {
      root.render(
        <VirtualMessageList
          items={initialItems}
          presentationMode="history-window"
          historyWindow={{
            startOrdinal: 2,
            endOrdinalExclusive: 7,
            targetTurnId: 'turn-5',
            mode: 'history-window',
          }}
          presentationRevision={1}
          historyBoundaryState={{ before: 'idle', after: 'idle' }}
          onHistoryWindowBoundaryIntent={onBoundaryIntent}
        />,
      );
    });

    const scroller = container.querySelector<HTMLElement>('[data-testid="virtuoso"]');
    const anchor = container.querySelector<HTMLElement>('[data-turn-id="turn-3"]');
    expect(scroller).not.toBeNull();
    expect(anchor).not.toBeNull();
    if (!scroller || !anchor) return;

    for (let frame = 0; frame < 8 && rafCallbacks.length > 0; frame += 1) {
      flushAnimationFrame();
    }

    setScrollerGeometry(scroller, {
      clientHeight: 100,
      scrollHeight: 500,
      scrollTop: 40,
    });
    scroller.getBoundingClientRect = () => createRect({ top: 0, bottom: 100, height: 100 });
    let anchorDocumentTop = 60;
    anchor.getBoundingClientRect = () => {
      const top = anchorDocumentTop - scroller.scrollTop;
      return createRect({ top, bottom: top + 20, height: 20 });
    };

    act(() => {
      scroller.dispatchEvent(new WheelEvent('wheel', { deltaY: -20, bubbles: true }));
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(onBoundaryIntent).toHaveBeenCalledWith('before', expect.objectContaining({
      prepareViewportForPresentationCommit: expect.any(Function),
    }));

    scroller.scrollTop = 80;
    anchorDocumentTop = 60;
    flushAnimationFrame();

    expect(scroller.scrollTop).toBe(80);
  });

  it('reports every turn intersecting the readable viewport in DOM order', () => {
    stateMocks.setVisibleTurnInfo.mockImplementation((info: unknown) => {
      stateMocks.visibleTurnInfo = info;
    });
    stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-a', 'turn-b']);
    stateMocks.virtualItems = [
      createItem('turn-a'),
      createModelItem('turn-a'),
      createItem('turn-b'),
      createModelItem('turn-b'),
    ];

    act(() => {
      root.render(<VirtualMessageList />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    const renderedItems = Array.from(
      container.querySelectorAll<HTMLElement>('.virtual-item-wrapper[data-turn-id]'),
    );
    expect(scroller).not.toBeNull();
    expect(renderedItems).toHaveLength(4);
    if (!scroller || renderedItems.length !== 4) return;

    vi.spyOn(scroller, 'getBoundingClientRect').mockReturnValue(createRect({
      top: 0,
      bottom: 300,
      height: 300,
    }));
    const itemRects = [
      { top: 0, bottom: 50 },
      { top: 50, bottom: 150 },
      { top: 150, bottom: 210 },
      { top: 210, bottom: 280 },
    ];
    renderedItems.forEach((item, index) => {
      const rect = itemRects[index];
      vi.spyOn(item, 'getBoundingClientRect').mockReturnValue(createRect({
        top: rect.top,
        bottom: rect.bottom,
        height: rect.bottom - rect.top,
      }));
    });

    act(() => {
      scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
    });
    for (let frame = 0; frame < 4; frame += 1) {
      flushAnimationFrame();
    }

    expect(stateMocks.setVisibleTurnInfo).toHaveBeenLastCalledWith(expect.objectContaining({
      turnId: 'turn-a',
      visibleTurnIds: ['turn-a', 'turn-b'],
    }));
    const publishCount = stateMocks.setVisibleTurnInfo.mock.calls.length;

    act(() => {
      scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
    });
    for (let frame = 0; frame < 4; frame += 1) {
      flushAnimationFrame();
    }

    expect(stateMocks.setVisibleTurnInfo).toHaveBeenCalledTimes(publishCount);
  });

  it('transfers collapse space to a sticky pin in one reservation state', () => {
    const currentState = {
      collapse: { kind: 'collapse' as const, px: 1_583, floorPx: 181 },
      pin: {
        kind: 'pin' as const,
        px: 0,
        floorPx: 0,
        mode: 'sticky-latest' as const,
        targetTurnId: 'turn-a',
      },
    };
    const nextPin = {
      ...currentState.pin,
      px: 559,
      floorPx: 559,
    };

    expect(transferCollapseReservationToPin(currentState, nextPin)).toEqual({
      collapse: { kind: 'collapse', px: 0, floorPx: 0 },
      pin: nextPin,
    });
  });

  it('transfers a released pin into a protected viewport range', () => {
    const currentState = {
      collapse: { kind: 'collapse' as const, px: 12, floorPx: 4 },
      pin: {
        kind: 'pin' as const,
        px: 100,
        floorPx: 100,
        mode: 'sticky-latest' as const,
        targetTurnId: 'turn-a',
      },
    };

    expect(transferPinReservationToProtectedCollapse(currentState)).toEqual({
      collapse: { kind: 'collapse', px: 112, floorPx: 104 },
      pin: {
        kind: 'pin',
        px: 0,
        floorPx: 0,
        mode: 'transient',
        targetTurnId: null,
      },
    });
  });

  it('invalidates pending turn pin work by generation, session, and target', () => {
    const request = {
      generation: 7,
      sessionId: 'session-a',
      turnId: 'turn-a',
    };

    expect(isTurnPinRequestIdentityCurrent(request, request)).toBe(true);
    expect(isTurnPinRequestIdentityCurrent(request, {
      ...request,
      generation: 8,
    })).toBe(false);
    expect(isTurnPinRequestIdentityCurrent(request, {
      ...request,
      sessionId: 'session-b',
    })).toBe(false);
    expect(isTurnPinRequestIdentityCurrent(request, {
      ...request,
      turnId: 'turn-b',
    })).toBe(false);
  });

  it('drops a provisional sticky pin on user intent without protecting its range', () => {
    const currentState = {
      collapse: { kind: 'collapse' as const, px: 12, floorPx: 4 },
      pin: {
        kind: 'pin' as const,
        px: 3_780,
        floorPx: 0,
        mode: 'sticky-latest' as const,
        targetTurnId: 'turn-a',
      },
    };

    expect(releasePinReservationForUserNavigation(currentState, {
      preserveCurrentRange: true,
      ownsElementAnchor: false,
    })).toEqual({
      collapse: currentState.collapse,
      pin: {
        kind: 'pin',
        px: 0,
        floorPx: 0,
        mode: 'transient',
        targetTurnId: null,
      },
    });
  });

  it('protects an established sticky pin range when user intent exits it', () => {
    const currentState = {
      collapse: { kind: 'collapse' as const, px: 12, floorPx: 4 },
      pin: {
        kind: 'pin' as const,
        px: 100,
        floorPx: 100,
        mode: 'sticky-latest' as const,
        targetTurnId: 'turn-a',
      },
    };

    expect(releasePinReservationForUserNavigation(currentState, {
      preserveCurrentRange: true,
      ownsElementAnchor: true,
    })).toEqual(transferPinReservationToProtectedCollapse(currentState));
  });

  it('keeps unresolved sticky pin fallback reservation idempotent and viewport-bounded', () => {
    expect(resolveProvisionalStickyPinReservationPx({
      scrollHeight: 3_803,
      clientHeight: 1_023,
      currentPinPx: 0,
    })).toBe(1_023);
    expect(resolveProvisionalStickyPinReservationPx({
      scrollHeight: 4_826,
      clientHeight: 1_023,
      currentPinPx: 1_023,
    })).toBe(1_023);
    expect(resolveProvisionalStickyPinReservationPx({
      scrollHeight: 6_583,
      clientHeight: 1_023,
      currentPinPx: 5_560,
    })).toBe(1_023);

    expect(clampPinReservationPxToViewport(640, 1_023)).toBe(640);
    expect(clampPinReservationPxToViewport(2_780, 1_023)).toBe(1_023);
  });

  it('clears only expired provisional sticky pins without an element anchor', () => {
    const pinReservation = {
      kind: 'pin' as const,
      px: 2_780,
      floorPx: 0,
      mode: 'sticky-latest' as const,
      targetTurnId: 'turn-a',
    };
    const baseOptions = {
      requestTurnId: 'turn-a',
      requestPinMode: 'sticky-latest' as const,
      pinReservation,
      ownsElementAnchor: false,
    };

    expect(shouldClearExpiredProvisionalStickyPin(baseOptions)).toBe(true);
    expect(shouldClearExpiredProvisionalStickyPin({
      ...baseOptions,
      ownsElementAnchor: true,
    })).toBe(false);
    expect(shouldClearExpiredProvisionalStickyPin({
      ...baseOptions,
      pinReservation: { ...pinReservation, floorPx: 200 },
    })).toBe(false);
    expect(shouldClearExpiredProvisionalStickyPin({
      ...baseOptions,
      requestTurnId: 'turn-b',
    })).toBe(false);
  });

  it('protects a settled element range from later unsignaled shrink reconciliation', () => {
    const settledState = settleCollapseReservationForViewport({
      collapse: { kind: 'collapse', px: 1_022, floorPx: 670 },
      pin: {
        kind: 'pin',
        px: 0,
        floorPx: 0,
        mode: 'transient',
        targetTurnId: null,
      },
    }, {
      scrollTop: 686,
      scrollHeight: 2_030,
      clientHeight: 1_023,
    });
    expect(settledState.collapse).toEqual({
      kind: 'collapse',
      px: 702,
      floorPx: 702,
    });

    const settledPinnedRange = settleCollapseReservationForViewport({
      collapse: { kind: 'collapse', px: 152.7, floorPx: 67.7 },
      pin: {
        kind: 'pin',
        px: 674.6,
        floorPx: 674.6,
        mode: 'sticky-latest',
        targetTurnId: 'turn-a',
      },
    }, {
      scrollTop: 5_122,
      scrollHeight: 6_235,
      clientHeight: 1_027,
    });
    expect(settledPinnedRange.collapse.px).toBeCloseTo(67.7, 1);
    expect(settledPinnedRange.collapse.floorPx).toBeCloseTo(67.7, 1);

    const protectedState = protectCurrentCollapseReservation({
      collapse: { kind: 'collapse', px: 784, floorPx: 670 },
      pin: {
        kind: 'pin',
        px: 0,
        floorPx: 0,
        mode: 'transient',
        targetTurnId: null,
      },
    });

    expect(protectedState.collapse).toEqual({
      kind: 'collapse',
      px: 784,
      floorPx: 784,
    });
    expect(reconcileUnsignaledShrinkReservation(protectedState, 2).collapse).toEqual({
      kind: 'collapse',
      px: 784,
      floorPx: 784,
    });
    expect(reconcileUnsignaledShrinkReservation({
      ...protectedState,
      collapse: { kind: 'collapse', px: 784, floorPx: 0 },
    }, 2).collapse).toEqual({
      kind: 'collapse',
      px: 2,
      floorPx: 0,
    });
  });

  it('extends and protects collapse range needed to restore a captured scroll position', () => {
    const currentState = {
      collapse: { kind: 'collapse' as const, px: 111.5, floorPx: 0 },
      pin: {
        kind: 'pin' as const,
        px: 0,
        floorPx: 0,
        mode: 'transient' as const,
        targetTurnId: null,
      },
    };

    const ensured = ensureCollapseReservationForScrollTop(currentState, {
      targetScrollTop: 933.33,
      scrollHeight: 1_897,
      clientHeight: 1_023,
    });

    expect(ensured.collapse.px).toBeCloseTo(171.83, 2);
    expect(ensured.collapse.floorPx).toBeCloseTo(171.83, 2);
  });

  it('does not synchronously shrink surplus collapse range while retaining an anchor', () => {
    const retained = ensureCollapseReservationForScrollTop({
      collapse: { kind: 'collapse', px: 205.85, floorPx: 0 },
      pin: {
        kind: 'pin',
        px: 0,
        floorPx: 0,
        mode: 'transient',
        targetTurnId: null,
      },
    }, {
      targetScrollTop: 1_003.33,
      scrollHeight: 2_242,
      clientHeight: 1_023,
    });

    expect(retained).toEqual({
      collapse: { kind: 'collapse', px: 205.85, floorPx: 0 },
      pin: {
        kind: 'pin',
        px: 0,
        floorPx: 0,
        mode: 'transient',
        targetTurnId: null,
      },
    });
  });

  it('does not reserve a viewport merely to keep scrollTop zero reachable', () => {
    const retained = ensureCollapseReservationForScrollTop({
      collapse: { kind: 'collapse', px: 559.49, floorPx: 559.49 },
      pin: {
        kind: 'pin',
        px: 0,
        floorPx: 0,
        mode: 'transient',
        targetTurnId: null,
      },
    }, {
      targetScrollTop: 0,
      scrollHeight: 1_023,
      clientHeight: 1_023,
    });

    expect(retained.collapse).toEqual({
      kind: 'collapse',
      px: 559.49,
      floorPx: 559.49,
    });
    expect(settleRetainedCollapseReservationForAnchor(retained, {
      targetScrollTop: 0,
      scrollHeight: 1_023,
      clientHeight: 1_023,
    }).collapse).toEqual({
      kind: 'collapse',
      px: 0,
      floorPx: 0,
    });
  });

  it('settles retained collapse estimates to the physical range required by the anchor', () => {
    const settled = settleRetainedCollapseReservationForAnchor({
      collapse: { kind: 'collapse', px: 1_669.04813, floorPx: 559.48962 },
      pin: {
        kind: 'pin',
        px: 0,
        floorPx: 0,
        mode: 'transient',
        targetTurnId: null,
      },
    }, {
      targetScrollTop: 662,
      scrollHeight: 3_354,
      clientHeight: 1_023,
    });

    expect(settled.collapse.px).toBeCloseTo(1.04813, 4);
    expect(settled.collapse.floorPx).toBeCloseTo(1.04813, 4);
  });

  it('drains a sticky pin floor only from measured content growth', () => {
    const currentState = {
      collapse: { kind: 'collapse' as const, px: 20, floorPx: 0 },
      pin: {
        kind: 'pin' as const,
        px: 100,
        floorPx: 100,
        mode: 'sticky-latest' as const,
        targetTurnId: 'turn-a',
      },
    };

    expect(consumeBottomReservationForContentGrowth(currentState, 35, true)).toEqual({
      collapse: { kind: 'collapse', px: 0, floorPx: 0 },
      pin: {
        ...currentState.pin,
        px: 85,
        floorPx: 85,
      },
    });
    expect(consumeBottomReservationForContentGrowth(currentState, 35, false)).toEqual({
      collapse: { kind: 'collapse', px: 0, floorPx: 0 },
      pin: currentState.pin,
    });
    expect(consumeBottomReservationForContentGrowth(currentState, 35, true, true)).toEqual({
      collapse: currentState.collapse,
      pin: {
        ...currentState.pin,
        px: 65,
        floorPx: 65,
      },
    });

    expect(consumeBottomReservationForContentGrowth({
      ...currentState,
      collapse: { kind: 'collapse', px: 30, floorPx: 20 },
    }, 25, false)).toEqual({
      collapse: { kind: 'collapse', px: 5, floorPx: 5 },
      pin: currentState.pin,
    });
  });

  it('syncs the physical bottom only for a real viewport-height change', () => {
    expect(shouldSyncPhysicalBottom({
      viewportHeightChanged: true,
      collapseProtectionActive: false,
      wasAtPhysicalBottom: true,
      ownsElementAnchor: true,
      isFollowingTail: false,
    })).toBe(false);
    expect(shouldSyncPhysicalBottom({
      viewportHeightChanged: true,
      collapseProtectionActive: false,
      wasAtPhysicalBottom: true,
      ownsElementAnchor: false,
      isFollowingTail: false,
    })).toBe(true);
    expect(shouldSyncPhysicalBottom({
      viewportHeightChanged: true,
      collapseProtectionActive: false,
      wasAtPhysicalBottom: true,
      ownsElementAnchor: false,
      isFollowingTail: true,
    })).toBe(false);
    expect(shouldSyncPhysicalBottom({
      viewportHeightChanged: false,
      collapseProtectionActive: false,
      wasAtPhysicalBottom: true,
      ownsElementAnchor: false,
      isFollowingTail: false,
    })).toBe(false);
  });

  it('suppresses only negative virtualizer compensation while following the streaming tail', () => {
    expect(shouldSuppressFollowingTailNegativeScrollBy({
      requestedTop: -242,
      isFollowingOutput: true,
      isStreamingOutput: true,
      wasAtPhysicalBottom: true,
    })).toBe(true);
    expect(shouldSuppressFollowingTailNegativeScrollBy({
      requestedTop: 36,
      isFollowingOutput: true,
      isStreamingOutput: true,
      wasAtPhysicalBottom: true,
    })).toBe(false);
    expect(shouldSuppressFollowingTailNegativeScrollBy({
      requestedTop: -242,
      isFollowingOutput: true,
      isStreamingOutput: true,
      wasAtPhysicalBottom: false,
    })).toBe(false);
    expect(shouldSuppressFollowingTailNegativeScrollBy({
      requestedTop: -242,
      isFollowingOutput: false,
      isStreamingOutput: true,
      wasAtPhysicalBottom: true,
    })).toBe(false);
  });

  it('recognizes only a non-user physical-bottom clamp from a shrinking scroll range', () => {
    const clampGeometry = {
      previousGeometry: {
        scrollTop: 645.3333129882812,
        scrollHeight: 1_673,
        clientHeight: 1_027,
      },
      currentGeometry: {
        scrollTop: 402.6666564941406,
        scrollHeight: 1_430,
        clientHeight: 1_027,
      },
      isFollowingOutput: true,
      isStreamingOutput: true,
      hasRecentUserUpwardIntent: false,
      scrollbarPointerInteractionActive: false,
      collapseProtectionActive: false,
    };

    const recovery = resolveFollowingTailShrinkClampRecovery(clampGeometry);
    expect(recovery?.targetScrollTop).toBe(645.3333129882812);
    expect(recovery?.rangeShrinkPx).toBe(243);
    expect(recovery?.scrollClampPx).toBeCloseTo(242.6666564941406, 10);
    expect(resolveFollowingTailShrinkClampRecovery({
      ...clampGeometry,
      hasRecentUserUpwardIntent: true,
    })).toBeNull();
    expect(resolveFollowingTailShrinkClampRecovery({
      ...clampGeometry,
      isFollowingOutput: false,
    })).toBeNull();
    expect(resolveFollowingTailShrinkClampRecovery({
      ...clampGeometry,
      currentGeometry: {
        ...clampGeometry.currentGeometry,
        scrollTop: 500,
      },
    })).toBeNull();
    expect(resolveFollowingTailShrinkClampRecovery({
      ...clampGeometry,
      currentGeometry: {
        ...clampGeometry.currentGeometry,
        clientHeight: 900,
      },
    })).toBeNull();
  });

  it('cancels unsettled sticky pin growth only for unsignaled height corrections', () => {
    expect(getCanceledUnsettledStickyPinGrowthPx({
      pendingGrowthPx: 207,
      shrinkPx: 207,
      hasActiveCollapseIntent: false,
    })).toBe(207);
    expect(getCanceledUnsettledStickyPinGrowthPx({
      pendingGrowthPx: 207,
      shrinkPx: 55,
      hasActiveCollapseIntent: false,
    })).toBe(55);
    expect(getCanceledUnsettledStickyPinGrowthPx({
      pendingGrowthPx: 207,
      shrinkPx: 207,
      hasActiveCollapseIntent: true,
    })).toBe(0);
  });

  it('settles sticky pin growth as soon as it exhausts the remaining pin floor', () => {
    expect(resolveStickyPinGrowthSettlementStrategy({
      pendingGrowthPx: 0,
      pinFloorPx: 11.77,
      hasActiveCollapseIntent: false,
    })).toBe('none');
    expect(resolveStickyPinGrowthSettlementStrategy({
      pendingGrowthPx: 10,
      pinFloorPx: 11.77,
      hasActiveCollapseIntent: false,
    })).toBe('wait-for-quiet');
    expect(resolveStickyPinGrowthSettlementStrategy({
      pendingGrowthPx: 126,
      pinFloorPx: 11.77,
      hasActiveCollapseIntent: true,
    })).toBe('wait-for-collapse');
    expect(resolveStickyPinGrowthSettlementStrategy({
      pendingGrowthPx: 126,
      pinFloorPx: 11.77,
      hasActiveCollapseIntent: false,
    })).toBe('settle-now');
  });

  it('selects an explicit settlement strategy for each collapse viewport owner', () => {
    const stickyReservation = {
      collapse: { kind: 'collapse' as const, px: 152.7, floorPx: 67.7 },
      pin: {
        kind: 'pin' as const,
        px: 674.6,
        floorPx: 674.6,
        mode: 'sticky-latest' as const,
        targetTurnId: 'turn-a',
      },
    };

    expect(resolveCollapseIntentSettlementStrategy({
      coordinatorMode: 'pinned-item',
      isFollowingOutput: false,
      isStreamingOutput: true,
      reservation: stickyReservation,
    })).toBe('reconcile-sticky-pin');
    expect(resolveCollapseIntentSettlementStrategy({
      coordinatorMode: 'following-tail',
      isFollowingOutput: true,
      isStreamingOutput: true,
      reservation: stickyReservation,
    })).toBe('retain-following-tail');
    expect(resolveCollapseIntentSettlementStrategy({
      coordinatorMode: 'preserving-element',
      isFollowingOutput: false,
      isStreamingOutput: true,
      reservation: stickyReservation,
    })).toBe('settle-preserved-element');
    expect(resolveCollapseIntentSettlementStrategy({
      coordinatorMode: 'idle',
      isFollowingOutput: false,
      isStreamingOutput: true,
      reservation: stickyReservation,
    })).toBe('settle-protected-viewport');
    expect(resolveCollapseIntentSettlementStrategy({
      coordinatorMode: 'idle',
      isFollowingOutput: false,
      isStreamingOutput: true,
      reservation: {
        ...stickyReservation,
        collapse: { ...stickyReservation.collapse, floorPx: 0 },
      },
    })).toBe('drain');
  });

  it('lets known streaming collapses reconcile without bypassing active intents', () => {
    expect(shouldBypassShrinkCompensationInTailFollow({
      isFollowingOutput: true,
      isStreamingOutput: true,
      hasActiveCollapseIntent: false,
    })).toBe(true);
    expect(shouldBypassShrinkCompensationInTailFollow({
      isFollowingOutput: true,
      isStreamingOutput: true,
      hasActiveCollapseIntent: true,
    })).toBe(false);
  });

  it('recovers the last stable scroll position when an auto collapse arrives after clamp', () => {
    expect(resolveAutoCollapseAnchorScrollTop({
      currentScrollTop: 1127.33,
      previousStableScrollTop: 1302.67,
      reason: 'auto',
      isFollowingOutput: true,
      isStreamingOutput: true,
    })).toBe(1302.67);
    expect(resolveAutoCollapseAnchorScrollTop({
      currentScrollTop: 1127.33,
      previousStableScrollTop: 1302.67,
      reason: 'manual',
      isFollowingOutput: true,
      isStreamingOutput: true,
    })).toBe(1127.33);
    expect(resolveAutoCollapseAnchorScrollTop({
      currentScrollTop: 1127.33,
      previousStableScrollTop: 1302.67,
      reason: 'auto',
      isFollowingOutput: false,
      isStreamingOutput: true,
    })).toBe(1127.33);
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

  it('resumes tail follow when a mounted streaming session receives its initial items', () => {
    const session = createSession('session-a', 'turn-a');
    session.dialogTurns[0].status = 'processing';
    session.dialogTurns[0].modelRounds = [{
      id: 'round-turn-a',
      status: 'streaming',
      isStreaming: true,
      items: [],
      startTime: 1,
    } as typeof session.dialogTurns[number]['modelRounds'][number]];
    stateMocks.activeSession = session;
    stateMocks.virtualItems = [];

    act(() => {
      root.render(<VirtualMessageList />);
    });
    virtuosoMocks.scrollerScrollTo.mockClear();

    stateMocks.virtualItems = [createItem('turn-a'), createModelItem('turn-a')];
    act(() => {
      root.render(<VirtualMessageList />);
    });

    expect(virtuosoMocks.scrollerScrollTo).toHaveBeenCalledWith(expect.objectContaining({
      behavior: 'auto',
    }));
  });

  it('restores a following-tail collapse anchor when delayed measurement clamps scrollTop', () => {
    const session = createSession('session-a', 'turn-a');
    session.dialogTurns[0].status = 'processing';
    session.dialogTurns[0].modelRounds = [{
      id: 'round-turn-a',
      status: 'streaming',
      isStreaming: true,
      items: [],
      startTime: 1,
    } as typeof session.dialogTurns[number]['modelRounds'][number]];
    stateMocks.activeSession = session;
    stateMocks.virtualItems = [createItem('turn-a'), createModelItem('turn-a')];

    act(() => {
      root.render(<VirtualMessageList />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    const footer = container.querySelector<HTMLElement>('.message-list-footer');
    const anchor = container.querySelector<HTMLElement>('[data-turn-id="turn-a"]');
    expect(scroller).not.toBeNull();
    expect(footer).not.toBeNull();
    expect(anchor).not.toBeNull();
    if (!scroller || !footer || !anchor) {
      return;
    }

    let contentHeight = 2_076;
    Object.defineProperties(scroller, {
      clientHeight: { configurable: true, value: 1_000 },
      scrollHeight: {
        configurable: true,
        get: () => contentHeight + (Number.parseFloat(footer.style.height) || 0),
      },
      scrollTop: { configurable: true, writable: true, value: 1_100 },
    });
    const footerHeightBeforeCollapse = Number.parseFloat(footer.style.height);
    act(() => {
      scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
    });

    act(() => {
      window.dispatchEvent(new CustomEvent('flowchat:tool-card-collapse-intent', {
        detail: {
          toolId: 'tool-a',
          toolName: 'Write',
          cardHeight: 200,
          anchorElement: anchor,
          reason: 'auto',
        },
      }));
    });
    expect(Number.parseFloat(footer.style.height)).toBeCloseTo(
      footerHeightBeforeCollapse + 100,
      2,
    );

    contentHeight -= 250;
    scroller.scrollTop = 1_050;
    act(() => {
      scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
    });

    expect(scroller.scrollTop).toBe(1_100);
    expect(Number.parseFloat(footer.style.height)).toBeCloseTo(
      footerHeightBeforeCollapse + 151,
      2,
    );
  });

  it('recovers a late following-tail shrink clamp after collapse protection was released', () => {
    flowDiagnosticsMocks.enabled = true;
    const session = createSession('session-a', 'turn-a');
    session.dialogTurns[0].status = 'processing';
    session.dialogTurns[0].modelRounds = [{
      id: 'round-turn-a',
      status: 'streaming',
      isStreaming: true,
      items: [],
      startTime: 1,
    } as typeof session.dialogTurns[number]['modelRounds'][number]];
    stateMocks.activeSession = session;
    stateMocks.virtualItems = [createItem('turn-a'), createModelItem('turn-a')];

    act(() => {
      root.render(<VirtualMessageList />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    const footer = container.querySelector<HTMLElement>('.message-list-footer');
    expect(scroller).not.toBeNull();
    expect(footer).not.toBeNull();
    if (!scroller || !footer) {
      return;
    }

    let contentHeight = 2_076;
    Object.defineProperties(scroller, {
      clientHeight: { configurable: true, value: 1_000 },
      scrollHeight: {
        configurable: true,
        get: () => contentHeight + (Number.parseFloat(footer.style.height) || 0),
      },
      scrollTop: { configurable: true, writable: true, value: 0 },
    });
    const footerHeightBeforeClamp = Number.parseFloat(footer.style.height);
    const stableScrollTop = scroller.scrollHeight - scroller.clientHeight;
    scroller.scrollTop = stableScrollTop;
    act(() => {
      window.dispatchEvent(new Event('resize'));
    });

    contentHeight -= 250;
    scroller.scrollTop = scroller.scrollHeight - scroller.clientHeight;
    act(() => {
      scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
    });

    expect(scroller.scrollTop).toBe(stableScrollTop);
    expect(Number.parseFloat(footer.style.height)).toBeCloseTo(
      footerHeightBeforeClamp + 251,
      2,
    );
    expect(flowDiagnosticsMocks.trace).toHaveBeenCalledWith(expect.objectContaining({
      location: 'VirtualMessageList.handleScroll',
      message: 'Following-tail shrink clamp recovered as a viewport transaction',
    }));
  });

  it('retains following-tail collapse protection after the animation finalizer', () => {
    flowDiagnosticsMocks.enabled = true;
    const session = createSession('session-a', 'turn-a');
    session.dialogTurns[0].status = 'processing';
    session.dialogTurns[0].modelRounds = [{
      id: 'round-turn-a',
      status: 'streaming',
      isStreaming: true,
      items: [],
      startTime: 1,
    } as typeof session.dialogTurns[number]['modelRounds'][number]];
    stateMocks.activeSession = session;
    stateMocks.virtualItems = [createItem('turn-a'), createModelItem('turn-a')];

    act(() => {
      root.render(<VirtualMessageList />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    const footer = container.querySelector<HTMLElement>('.message-list-footer');
    const anchor = container.querySelector<HTMLElement>('[data-turn-id="turn-a"]');
    expect(scroller).not.toBeNull();
    expect(footer).not.toBeNull();
    expect(anchor).not.toBeNull();
    if (!scroller || !footer || !anchor) {
      return;
    }

    let contentHeight = 2_076;
    Object.defineProperties(scroller, {
      clientHeight: { configurable: true, value: 1_000 },
      scrollHeight: {
        configurable: true,
        get: () => contentHeight + (Number.parseFloat(footer.style.height) || 0),
      },
      scrollTop: { configurable: true, writable: true, value: 1_100 },
    });
    const footerHeightBeforeCollapse = Number.parseFloat(footer.style.height);
    act(() => {
      scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
    });

    vi.useFakeTimers();
    try {
      act(() => {
        window.dispatchEvent(new CustomEvent('flowchat:tool-card-collapse-intent', {
          detail: {
            toolId: 'tool-a',
            toolName: 'Write',
            cardHeight: 200,
            anchorElement: anchor,
            reason: 'auto',
          },
        }));
      });
      expect(Number.parseFloat(footer.style.height)).toBeCloseTo(
        footerHeightBeforeCollapse + 100,
        2,
      );

      rafCallbacks = [];
      act(() => {
        vi.advanceTimersByTime(300);
      });
      for (let frame = 0; frame < 4; frame += 1) {
        flushAnimationFrame();
      }
      const provisionalFooterHeight = Number.parseFloat(footer.style.height);
      expect(provisionalFooterHeight).toBeCloseTo(
        footerHeightBeforeCollapse + 100,
        2,
      );
      const findRetainTrace = () => flowDiagnosticsMocks.trace.mock.calls.find(([event]) => (
        event.location === 'VirtualMessageList.retainCollapseRangeForQuietSettlement'
      ));
      if (!findRetainTrace()) {
        act(() => {
          vi.advanceTimersByTime(1_000);
        });
      }
      expect(findRetainTrace()).toBeDefined();

      act(() => {
        vi.advanceTimersByTime(120);
      });
      for (let frame = 0; frame < 4; frame += 1) {
        flushAnimationFrame();
      }
      const settledFooterHeight = Number.parseFloat(footer.style.height);
      const settleTrace = flowDiagnosticsMocks.trace.mock.calls.find(([event]) => (
        event.location === 'VirtualMessageList.settleRetainedCollapseRange'
      ));
      expect(settleTrace).toBeDefined();
      expect(settledFooterHeight).toBeLessThan(provisionalFooterHeight);
      expect(scroller.scrollTop).toBe(1_100);

      stateMocks.activeSession = {
        ...session,
        dialogTurns: session.dialogTurns.map(turn => ({
          ...turn,
          status: 'completed',
          modelRounds: turn.modelRounds.map(round => ({
            ...round,
            status: 'completed',
            isStreaming: false,
          })),
        })),
      };
      act(() => {
        root.render(<VirtualMessageList />);
      });
      expect(Number.parseFloat(footer.style.height)).toBeCloseTo(settledFooterHeight, 2);

      contentHeight -= 250;
      scroller.scrollTop = 1_050;
      act(() => {
        scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
      });

      expect(scroller.scrollTop).toBe(1_100);
      expect(Number.parseFloat(footer.style.height)).toBeGreaterThan(settledFooterHeight);

      act(() => {
        vi.advanceTimersByTime(120);
      });
      for (let frame = 0; frame < 4; frame += 1) {
        flushAnimationFrame();
      }
      const wasSettledAnchorReleased = () => flowDiagnosticsMocks.trace.mock.calls.some(([event]) => (
        event.location === 'VirtualMessageList.releaseSettledCollapseAnchor'
      ));
      for (let attempt = 0; attempt < 3 && !wasSettledAnchorReleased(); attempt += 1) {
        act(() => {
          vi.runOnlyPendingTimers();
        });
        for (let frame = 0; frame < 4; frame += 1) {
          flushAnimationFrame();
        }
      }
      expect(wasSettledAnchorReleased()).toBe(true);
    } finally {
      vi.useRealTimers();
    }
  });

  it('keeps the initial Virtuoso position when background updates arrive after an upward scroll', () => {
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

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    expect(scroller).not.toBeNull();
    if (!scroller) {
      return;
    }
    setScrollerGeometry(scroller, {
      scrollHeight: 6_000,
      clientHeight: 1_000,
      scrollTop: 4_000,
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
      index: 2,
      align: 'start',
      behavior: 'auto',
    }));
    expect(virtuosoMocks.increaseViewportBy).toEqual({
      top: 2_000,
      bottom: 2_000,
    });

    const target = container.querySelector<HTMLElement>(
      `[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`,
    );
    expect(target).not.toBeNull();
    if (!target) {
      return;
    }

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
    flushAnimationFrame();
    expect(scroller.scrollTop).toBe(4_443);
    expect(target.getBoundingClientRect().top).toBe(57);
    expect(virtuosoMocks.scrollToIndex).toHaveBeenCalledTimes(1);
    expect(virtuosoMocks.increaseViewportBy).toEqual({
      top: 600,
      bottom: 600,
    });
  });

  it('materializes the first turn before pinning when it is outside the rendered range', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    const turnIds = Array.from(
      { length: 9 },
      (_, index) => `turn-${String(index + 1).padStart(2, '0')}`,
    );

    stateMocks.activeSession = createSessionWithTurns('session-a', turnIds);
    stateMocks.virtualItems = turnIds.flatMap(turnId => [
      createItem(turnId),
      createModelItem(turnId),
    ]);
    virtuosoMocks.renderedRange = { start: 12, end: 18 };

    act(() => {
      root.render(<VirtualMessageList ref={listRef} />);
    });

    expect(virtuosoMocks.initialTopMostItemIndexHistory).toContainEqual({
      index: 17,
      align: 'end',
    });
    expect(virtuosoMocks.initialTopMostItemIndex).toBeUndefined();
    expect(container.querySelector(
      '[data-turn-id="turn-01"][data-item-type="user-message"]',
    )).toBeNull();
    virtuosoMocks.scrollToIndex.mockClear();
    const navigationRenderStart = virtuosoMocks.initialTopMostItemIndexHistory.length;

    let status: ReturnType<VirtualMessageListRef['pinTurnToTopWithStatus']> = 'rejected';
    act(() => {
      status = listRef.current?.pinTurnToTopWithStatus('turn-01', {
        behavior: 'auto',
        pinMode: 'transient',
      }) ?? 'rejected';
    });

    expect(status).toBe('pending');
    expect(virtuosoMocks.scrollToIndex).toHaveBeenCalledWith(expect.objectContaining({
      index: 0,
      align: 'start',
      behavior: 'auto',
    }));
    expect(virtuosoMocks.initialTopMostItemIndex).toBeUndefined();
    const navigationInitialPositionProps =
      virtuosoMocks.initialTopMostItemIndexHistory.slice(navigationRenderStart);
    expect(navigationInitialPositionProps.length).toBeGreaterThan(0);
    expect(navigationInitialPositionProps.every(value => value === undefined)).toBe(true);
    expect(container.querySelector(
      '[data-turn-id="turn-01"][data-item-type="user-message"]',
    )).not.toBeNull();
  });

  it('settles best-effort navigation at the natural maximum without pin footer space', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-a', 'turn-b']);
    stateMocks.virtualItems = [createItem('turn-a'), createItem('turn-b')];

    act(() => {
      root.render(<VirtualMessageList ref={listRef} />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    const footer = container.querySelector<HTMLElement>('.message-list-footer');
    const target = container.querySelector<HTMLElement>(
      '[data-turn-id="turn-b"][data-item-type="user-message"]',
    );
    expect(scroller).not.toBeNull();
    expect(footer).not.toBeNull();
    expect(target).not.toBeNull();
    if (!scroller || !footer || !target) {
      return;
    }

    setScrollerGeometry(scroller, {
      scrollHeight: 1_200,
      clientHeight: 1_000,
      scrollTop: 0,
    });
    vi.spyOn(scroller, 'getBoundingClientRect').mockReturnValue(createRect({
      top: 0,
      bottom: 1_000,
      height: 1_000,
    }));
    vi.spyOn(target, 'getBoundingClientRect').mockImplementation(() => {
      const top = 900 - scroller.scrollTop;
      return createRect({ top, bottom: top + 40, height: 40 });
    });
    const footerHeightBefore = footer.style.height;

    let status: ReturnType<VirtualMessageListRef['pinTurnToTopWithStatus']> = 'rejected';
    act(() => {
      status = listRef.current?.pinTurnToTopWithStatus('turn-b', {
        behavior: 'auto',
        pinMode: 'transient',
        alignmentPolicy: 'best-effort',
      }) ?? 'rejected';
    });

    expect(status).toBe('settled');
    expect(scroller.scrollTop).toBe(200);
    expect(footer.style.height).toBe(footerHeightBefore);
    expect(target.getBoundingClientRect().top).toBeGreaterThan(57);
  });

  it('keeps sticky-latest exact even when a best-effort policy is supplied', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-a', 'turn-b']);
    const latestTurn = stateMocks.activeSession.dialogTurns[1];
    latestTurn.status = 'processing';
    latestTurn.modelRounds = [{
      id: 'round-turn-b',
      status: 'streaming',
      isStreaming: true,
      items: [],
      startTime: 1,
    } as typeof latestTurn.modelRounds[number]];
    stateMocks.virtualItems = [createItem('turn-a'), createItem('turn-b')];

    act(() => {
      root.render(<VirtualMessageList ref={listRef} />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    const footer = container.querySelector<HTMLElement>('.message-list-footer');
    const target = container.querySelector<HTMLElement>(
      '[data-turn-id="turn-b"][data-item-type="user-message"]',
    );
    expect(scroller).not.toBeNull();
    expect(footer).not.toBeNull();
    expect(target).not.toBeNull();
    if (!scroller || !footer || !target) {
      return;
    }

    Object.defineProperties(scroller, {
      clientHeight: { configurable: true, value: 1_000 },
      scrollHeight: {
        configurable: true,
        get: () => 1_200 + (Number.parseFloat(footer.style.height) || 0),
      },
      scrollTop: { configurable: true, writable: true, value: 200 },
    });
    vi.spyOn(scroller, 'getBoundingClientRect').mockReturnValue(createRect({
      top: 0,
      bottom: 1_000,
      height: 1_000,
    }));
    vi.spyOn(target, 'getBoundingClientRect').mockImplementation(() => {
      const top = 700 - scroller.scrollTop;
      return createRect({ top, bottom: top + 40, height: 40 });
    });

    let status: ReturnType<VirtualMessageListRef['pinTurnToTopWithStatus']> = 'rejected';
    act(() => {
      status = listRef.current?.pinTurnToTopWithStatus('turn-b', {
        behavior: 'auto',
        pinMode: 'sticky-latest',
        alignmentPolicy: 'best-effort',
      }) ?? 'rejected';
    });

    expect(status).toBe('settled');
    expect(Number.parseFloat(footer.style.height)).toBeGreaterThan(0);
    expect(target.getBoundingClientRect().top).toBe(57);
  });

  it('uses a prepared turn only for the initial Virtuoso mount', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    const turnIds = ['turn-01', 'turn-02', 'turn-03'];

    stateMocks.activeSession = createSessionWithTurns('session-a', turnIds);
    stateMocks.virtualItems = [];

    act(() => {
      root.render(<VirtualMessageList ref={listRef} />);
    });
    act(() => {
      listRef.current?.prepareTurnPinToTop('turn-02', {
        behavior: 'auto',
        pinMode: 'transient',
      });
    });

    virtuosoMocks.initialTopMostItemIndexHistory = [];
    stateMocks.virtualItems = turnIds.flatMap(turnId => [
      createItem(turnId),
      createModelItem(turnId),
    ]);
    act(() => {
      root.render(<VirtualMessageList ref={listRef} />);
    });

    expect(virtuosoMocks.initialTopMostItemIndexHistory).toContainEqual({
      index: 2,
      align: 'start',
    });
    expect(virtuosoMocks.initialTopMostItemIndex).toBeUndefined();
  });

  it('keeps the existing sticky range while a distant turn is materialized', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    const turnIds = Array.from({ length: 24 }, (_, index) => `turn-${String(index + 1).padStart(2, '0')}`);
    const latestTurnId = turnIds[turnIds.length - 1];
    const targetTurnId = turnIds[1];
    const session = createSessionWithTurns('session-a', turnIds);
    const latestTurn = session.dialogTurns[session.dialogTurns.length - 1];
    latestTurn.status = 'processing';
    latestTurn.modelRounds = [{
      id: `round-${latestTurnId}`,
      status: 'streaming',
      isStreaming: true,
      items: [],
      startTime: 1,
    } as typeof latestTurn.modelRounds[number]];
    stateMocks.activeSession = session;
    stateMocks.virtualItems = turnIds.flatMap(turnId => [
      createItem(turnId),
      createModelItem(turnId),
    ]);
    virtuosoMocks.renderedRange = { start: 40, end: 48 };

    act(() => {
      root.render(<VirtualMessageList ref={listRef} />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    const footer = container.querySelector<HTMLElement>('.message-list-footer');
    const latestTarget = container.querySelector<HTMLElement>(
      `[data-turn-id="${latestTurnId}"][data-item-type="user-message"]`,
    );
    expect(scroller).not.toBeNull();
    expect(footer).not.toBeNull();
    expect(latestTarget).not.toBeNull();
    if (!scroller || !footer || !latestTarget) {
      return;
    }

    Object.defineProperties(scroller, {
      clientHeight: { configurable: true, value: 1_000 },
      scrollHeight: {
        configurable: true,
        get: () => 1_200 + (Number.parseFloat(footer.style.height) || 0),
      },
      scrollTop: { configurable: true, writable: true, value: 200 },
    });
    vi.spyOn(scroller, 'getBoundingClientRect').mockReturnValue(createRect({
      top: 0,
      bottom: 1_000,
      height: 1_000,
    }));
    const latestTargetDocumentTop = 700;
    vi.spyOn(latestTarget, 'getBoundingClientRect').mockImplementation(() => {
      const top = latestTargetDocumentTop - scroller.scrollTop;
      return createRect({ top, bottom: top + 40, height: 40 });
    });

    let latestPinStatus: ReturnType<VirtualMessageListRef['pinTurnToTopWithStatus']> = 'rejected';
    act(() => {
      latestPinStatus = listRef.current?.pinTurnToTopWithStatus(latestTurnId, {
        behavior: 'auto',
        pinMode: 'sticky-latest',
      }) ?? 'rejected';
    });
    expect(latestPinStatus).toBe('settled');
    const stickyFooterHeight = Number.parseFloat(footer.style.height);
    expect(stickyFooterHeight).toBeGreaterThan(0);

    virtuosoMocks.scrollToIndex.mockClear();
    let targetPinStatus: ReturnType<VirtualMessageListRef['pinTurnToTopWithStatus']> = 'rejected';
    act(() => {
      targetPinStatus = listRef.current?.pinTurnToTopWithStatus(targetTurnId, {
        behavior: 'auto',
        pinMode: 'transient',
      }) ?? 'rejected';
    });

    expect(targetPinStatus).toBe('pending');
    expect(Number.parseFloat(footer.style.height)).toBe(stickyFooterHeight);
    expect(virtuosoMocks.scrollToIndex).toHaveBeenCalledWith(expect.objectContaining({
      align: 'start',
      behavior: 'auto',
    }));

    const target = container.querySelector<HTMLElement>(
      `[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`,
    );
    expect(target).not.toBeNull();
    if (!target) {
      return;
    }
    const targetDocumentTop = 800;
    vi.spyOn(target, 'getBoundingClientRect').mockImplementation(() => {
      const top = targetDocumentTop - scroller.scrollTop;
      return createRect({ top, bottom: top + 40, height: 40 });
    });

    act(() => {
      virtuosoMocks.rangeChanged?.();
    });

    expect(target.getBoundingClientRect().top).toBe(57);
    expect(Number.parseFloat(footer.style.height)).toBeGreaterThan(0);
  });

  it('routes turn navigation through Virtuoso while the initial snapshot is visible', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    const turnIds = Array.from({ length: 8 }, (_, index) => `turn-${index}`);
    const targetTurnId = 'turn-1';
    stateMocks.activeSession = createSessionWithTurns('session-a', turnIds, {
      contextRestoreState: 'pending',
      isPartial: true,
      historyState: 'ready',
    });
    stateMocks.virtualItems = turnIds.flatMap(turnId => [
      createItem(turnId),
      createModelItem(turnId),
    ]);
    virtuosoMocks.renderedRange = { start: 12, end: 16 };

    act(() => {
      root.render(<VirtualMessageList ref={listRef} />);
    });
    expect(container.querySelector('[data-testid="virtuoso"]')).not.toBeNull();
    expect(container.querySelector('[data-initial-history-snapshot="true"]')).not.toBeNull();
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
      index: 2,
      align: 'start',
      behavior: 'auto',
    }));
    expect(container.querySelector('[data-history-projection-handoff="true"]')?.getAttribute(
      'data-target-turn-id',
    )).toBe(targetTurnId);
  });

  it('releases the initial snapshot after Virtuoso renders readable target content', () => {
    stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-a', 'turn-b'], {
      contextRestoreState: 'pending',
      isPartial: true,
      historyState: 'ready',
    });
    stateMocks.virtualItems = [createItem('turn-a'), createItem('turn-b')];

    act(() => {
      root.render(<VirtualMessageList />);
    });

    expect(container.querySelector('[data-testid="virtuoso"]')).not.toBeNull();
    expect(container.querySelector('[data-initial-history-snapshot="true"]')).not.toBeNull();

    const scroller = container.querySelector<HTMLElement>('[data-testid="virtuoso"]');
    const target = scroller?.querySelector<HTMLElement>(
      '[data-turn-id="turn-b"][data-item-type="user-message"]',
    );
    expect(scroller).not.toBeNull();
    expect(target).not.toBeNull();
    if (!scroller || !target) {
      return;
    }
    vi.spyOn(scroller, 'getBoundingClientRect').mockReturnValue(createRect({
      top: 0,
      bottom: 1_000,
      height: 1_000,
    }));
    vi.spyOn(target, 'getBoundingClientRect').mockReturnValue(createRect({
      top: 200,
      bottom: 240,
      width: 400,
      height: 40,
    }));
    Object.defineProperty(target, 'innerText', {
      configurable: true,
      value: 'turn-b',
    });

    for (let frame = 0; frame < 5; frame += 1) {
      flushAnimationFrame();
    }

    expect(container.querySelector('[data-initial-history-snapshot="true"]')).toBeNull();
    expect(container.querySelector('[data-testid="virtuoso"]')).not.toBeNull();
  });

  it('does not render an initial projection handoff over a history-window presentation', () => {
    stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-a', 'turn-b'], {
      contextRestoreState: 'pending',
      isPartial: true,
      historyState: 'ready',
    });
    const tailItems = [createItem('turn-a'), createItem('turn-b')];
    stateMocks.virtualItems = tailItems;

    act(() => {
      root.render(<VirtualMessageList items={tailItems} />);
    });
    expect(container.querySelector('[data-history-projection-handoff="true"]')).not.toBeNull();

    const historyItems = [
      createItem('turn-a'),
      createModelItem('turn-a'),
      createItem('turn-b'),
      createModelItem('turn-b'),
    ];
    act(() => {
      root.render(
        <VirtualMessageList
          items={historyItems}
          presentationMode="history-window"
          historyWindow={{
            startOrdinal: 0,
            endOrdinalExclusive: 2,
            targetTurnId: 'turn-a',
            mode: 'history-window',
          }}
          presentationRevision={1}
        />,
      );
    });

    expect(container.querySelector('[data-history-projection-handoff="true"]')).toBeNull();
    expect(container.querySelector('[data-testid="virtuoso"]')).not.toBeNull();
  });

  it('does not let a canceled pending sticky pin RAF restore provisional footer space', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    const turnIds = Array.from(
      { length: 24 },
      (_, index) => `turn-${String(index + 1).padStart(2, '0')}`,
    );
    const latestTurnId = turnIds[turnIds.length - 1];
    const session = createSessionWithTurns('session-a', turnIds);
    const latestTurn = session.dialogTurns[session.dialogTurns.length - 1];
    latestTurn.status = 'processing';
    latestTurn.modelRounds = [{
      id: `round-${latestTurnId}`,
      status: 'streaming',
      isStreaming: true,
      items: [],
      startTime: 1,
    } as typeof latestTurn.modelRounds[number]];
    stateMocks.activeSession = session;
    stateMocks.virtualItems = turnIds.flatMap(turnId => [
      createItem(turnId),
      createModelItem(turnId),
    ]);
    virtuosoMocks.renderedRange = { start: 0, end: 4 };

    act(() => {
      root.render(<VirtualMessageList ref={listRef} />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    const footer = container.querySelector<HTMLElement>('.message-list-footer');
    expect(scroller).not.toBeNull();
    expect(footer).not.toBeNull();
    if (!scroller || !footer) {
      return;
    }

    setScrollerGeometry(scroller, {
      scrollHeight: 5_000,
      clientHeight: 1_000,
      scrollTop: 0,
    });
    const baselineFooterHeight = Number.parseFloat(footer.style.height);

    let status: ReturnType<VirtualMessageListRef['pinTurnToTopWithStatus']> = 'rejected';
    act(() => {
      status = listRef.current?.pinTurnToTopWithStatus(latestTurnId, {
        behavior: 'auto',
        pinMode: 'sticky-latest',
      }) ?? 'rejected';
    });
    expect(status).toBe('pending');
    expect(Number.parseFloat(footer.style.height)).toBeGreaterThan(baselineFooterHeight);
    expect(Number.parseFloat(footer.style.height) - baselineFooterHeight)
      .toBeLessThanOrEqual(scroller.clientHeight);

    const target = container.querySelector<HTMLElement>(
      `[data-turn-id="${latestTurnId}"][data-item-type="user-message"]`,
    );
    expect(target).not.toBeNull();
    if (!target) {
      return;
    }
    vi.spyOn(scroller, 'getBoundingClientRect').mockReturnValue(createRect({
      top: 0,
      bottom: 1_000,
      height: 1_000,
    }));
    vi.spyOn(target, 'getBoundingClientRect').mockReturnValue(createRect({
      top: 4_500,
      bottom: 4_540,
      height: 40,
    }));

    act(() => {
      scroller.dispatchEvent(new WheelEvent('wheel', {
        deltaY: -120,
        bubbles: true,
      }));
    });
    expect(Number.parseFloat(footer.style.height)).toBe(baselineFooterHeight);

    flushAnimationFrame();
    expect(Number.parseFloat(footer.style.height)).toBe(baselineFooterHeight);
  });

  it('releases a settled sticky pin at stream end even when its target is no longer rendered', () => {
    const listRef = React.createRef<VirtualMessageListRef>();
    const session = createSessionWithTurns('session-a', ['turn-a', 'turn-b']);
    const latestTurn = session.dialogTurns[session.dialogTurns.length - 1];
    latestTurn.status = 'processing';
    latestTurn.modelRounds = [{
      id: 'round-turn-b',
      status: 'streaming',
      isStreaming: true,
      items: [],
      startTime: 1,
    } as typeof latestTurn.modelRounds[number]];
    stateMocks.activeSession = session;
    stateMocks.virtualItems = ['turn-a', 'turn-b'].flatMap(turnId => [
      createItem(turnId),
      createModelItem(turnId),
    ]);

    act(() => {
      root.render(<VirtualMessageList ref={listRef} />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    const footer = container.querySelector<HTMLElement>('.message-list-footer');
    const target = container.querySelector<HTMLElement>(
      '[data-turn-id="turn-b"][data-item-type="user-message"]',
    );
    expect(scroller).not.toBeNull();
    expect(footer).not.toBeNull();
    expect(target).not.toBeNull();
    if (!scroller || !footer || !target) {
      return;
    }

    Object.defineProperties(scroller, {
      clientHeight: { configurable: true, value: 1_000 },
      scrollHeight: {
        configurable: true,
        get: () => 1_200 + (Number.parseFloat(footer.style.height) || 0),
      },
      scrollTop: { configurable: true, writable: true, value: 200 },
    });
    vi.spyOn(scroller, 'getBoundingClientRect').mockReturnValue(createRect({
      top: 0,
      bottom: 1_000,
      height: 1_000,
    }));
    const targetDocumentTop = 700;
    vi.spyOn(target, 'getBoundingClientRect').mockImplementation(() => {
      const top = targetDocumentTop - scroller.scrollTop;
      return createRect({ top, bottom: top + 40, height: 40 });
    });

    let status: ReturnType<VirtualMessageListRef['pinTurnToTopWithStatus']> = 'rejected';
    act(() => {
      status = listRef.current?.pinTurnToTopWithStatus('turn-b', {
        behavior: 'auto',
        pinMode: 'sticky-latest',
      }) ?? 'rejected';
    });
    expect(status).toBe('settled');
    expect(scroller.scrollTop).toBe(643);
    const footerHeightBeforeStreamEnd = Number.parseFloat(footer.style.height);
    expect(footerHeightBeforeStreamEnd).toBe(443);

    const release = vi.spyOn(FlowChatViewportCoordinator.prototype, 'release');
    virtuosoMocks.renderedRange = { start: 0, end: 2 };
    stateMocks.activeSession = {
      ...session,
      dialogTurns: session.dialogTurns.map(turn => turn.id === latestTurn.id
        ? {
          ...turn,
          status: 'completed',
          modelRounds: turn.modelRounds.map(round => ({
            ...round,
            status: 'completed',
            isStreaming: false,
          })),
        }
        : turn),
    };
    act(() => {
      root.render(<VirtualMessageList ref={listRef} />);
    });

    expect(container.querySelector(
      '[data-turn-id="turn-b"][data-item-type="user-message"]',
    )).toBeNull();
    expect(release).toHaveBeenCalledWith('stream-end-pinned-item');
    expect(Number.parseFloat(footer.style.height)).toBe(footerHeightBeforeStreamEnd);
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

  it('keeps the initial snapshot bounded while search navigation uses Virtuoso', () => {
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
    virtuosoMocks.renderedRange = { start: 12, end: 16 };

    act(() => {
      root.render(<VirtualMessageList ref={listRef} />);
    });

    const initialSnapshot = container.querySelector<HTMLElement>('[data-initial-history-snapshot="true"]');
    expect(initialSnapshot).not.toBeNull();
    expect(container.querySelector('[data-testid="virtuoso"]')).not.toBeNull();
    expect(initialSnapshot?.querySelectorAll('.virtual-item-wrapper').length)
      .toBeLessThan(stateMocks.virtualItems.length);
    expect(initialSnapshot?.querySelector(
      `[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`,
    )).toBeNull();
    expect(initialSnapshot?.querySelector(
      `[data-turn-id="${latestTurnId}"][data-item-type="user-message"]`,
    )).not.toBeNull();
    expect(container.querySelector(
      `[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`,
    )).toBeNull();
    virtuosoMocks.scrollToIndex.mockClear();

    act(() => {
      listRef.current?.scrollToSearchMatch({
        virtualItemIndex: 2,
        query: targetTurnId,
      });
    });

    expect(virtuosoMocks.scrollToIndex).toHaveBeenCalledWith(expect.objectContaining({
      index: 2,
      align: 'center',
      behavior: 'auto',
    }));
    expect(container.querySelector(
      `[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`,
    )).not.toBeNull();
  });

  it('keeps Virtuoso navigation stable while an initial snapshot handoff is active', () => {
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
      const targetDocumentTop = 2_800;
      vi.spyOn(target, 'getBoundingClientRect').mockImplementation(() => {
        const top = targetDocumentTop - scroller.scrollTop;
        return createRect({
          top,
          bottom: top + 40,
          height: 40,
        });
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
        scrollHeight: 5_200,
        clientHeight: 1_000,
      });

      act(() => {
        root.render(<VirtualMessageList ref={listRef} />);
      });

      expect(scroller.scrollTop).toBe(4_000);

      let didPin = false;
      act(() => {
        didPin = listRef.current?.pinTurnToTop('turn-a', { behavior: 'auto' }) ?? false;
      });

      expect(didPin).toBe(true);
      const pinnedScrollTop = scroller.scrollTop;
      expect(pinnedScrollTop).toBe(2_703);
      expect(target.getBoundingClientRect().top).toBe(97);

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

  it('keeps latest reachable after Virtuoso materializes an older turn during snapshot handoff', () => {
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
    virtuosoMocks.renderedRange = { start: 12, end: 16 };

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
    expect(scroller.scrollTop).toBe(11_000);
    expect(container.querySelector(`[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`)).not.toBeNull();
    expect(container.querySelector(
      `[data-testid="virtuoso"] [data-turn-id="${latestTurnId}"][data-item-type="user-message"]`,
    )).toBeNull();

    act(() => {
      container.querySelector<HTMLElement>('[data-testid="scroll-to-latest"]')?.dispatchEvent(
        new MouseEvent('click', { bubbles: true }),
      );
    });

    expect(onUserScrollIntent).toHaveBeenCalledTimes(1);
    expect(scroller.scrollTop).toBe(11_000);
  });

  it('keeps a Virtuoso history pin when materialization reports the old bottom first', () => {
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
    virtuosoMocks.renderedRange = { start: 12, end: 16 };

    act(() => {
      root.render(<VirtualMessageList ref={listRef} />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    expect(scroller).not.toBeNull();
    if (!scroller) {
      return;
    }

    setScrollerGeometry(scroller, {
      scrollHeight: 6_275,
      clientHeight: 1_027,
      scrollTop: 5_248,
    });
    const scrollTo = vi.fn((options?: ScrollToOptions) => {
      if (options?.behavior !== 'smooth' && typeof options?.top === 'number') {
        scroller.scrollTop = options.top;
      }
    });
    Object.defineProperty(scroller, 'scrollTo', {
      configurable: true,
      value: scrollTo,
    });

    expect(container.querySelector(
      `[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`,
    )).toBeNull();
    expect(container.querySelector(
      `[data-turn-id="${latestTurnId}"][data-item-type="user-message"]`,
    )).not.toBeNull();

    let pinStatus: ReturnType<VirtualMessageListRef['pinTurnToTopWithStatus']> = 'rejected';
    act(() => {
      pinStatus = listRef.current?.pinTurnToTopWithStatus(targetTurnId, {
        behavior: 'smooth',
      }) ?? 'rejected';
    });

    expect(pinStatus).toBe('pending');
    expect(virtuosoMocks.scrollToIndex).toHaveBeenCalledWith(expect.objectContaining({
      index: 2,
      align: 'start',
      behavior: 'auto',
    }));
    expect(container.querySelector(
      `[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`,
    )).not.toBeNull();

    // The browser clamps the old bottom position when the target window is
    // materialized, before the smooth scroll animation has moved the pane.
    setScrollerGeometry(scroller, {
      scrollHeight: 2_693,
      clientHeight: 1_027,
      scrollTop: 1_666,
    });
    act(() => {
      scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
    });

    expect(container.querySelector(
      `[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`,
    )).not.toBeNull();

    // A real downward user gesture is still allowed to return to the latest
    // window once the pane reaches its physical bottom.
    act(() => {
      scroller.dispatchEvent(new WheelEvent('wheel', { deltaY: 120, bubbles: true }));
      scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
    });

    expect(container.querySelector(
      `[data-turn-id="${targetTurnId}"][data-item-type="user-message"]`,
    )).not.toBeNull();
  });

  it('keeps the initial Virtuoso position when footer height changes after an upward scroll', () => {
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

  it('does not let the initial snapshot become a collapse scroll writer', () => {
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

    expect(scroller.scrollTop).toBe(4_000);
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
    stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-a'], {
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

    expect(flowStoreMocks.revealPreviousSessionHistoryWindow).toHaveBeenCalledWith('session-a', 'wheel-up');
  });

  it('paginates partial history through Virtuoso after upward intent', () => {
    flowStoreMocks.hasDeferredSessionHistoryProjection.mockReturnValue(true);
    flowStoreMocks.revealPreviousSessionHistoryWindow.mockReturnValue(true);
    stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-3', 'turn-4', 'turn-5'], {
      isHistorical: true,
      historyState: 'ready',
      contextRestoreState: 'pending',
      isPartial: true,
      loadedTurnCount: 3,
      totalTurnCount: 6,
    });
    stateMocks.virtualItems = ['turn-3', 'turn-4', 'turn-5'].flatMap(turnId => [
      createItem(turnId),
      createModelItem(turnId),
    ]);

    act(() => {
      root.render(<VirtualMessageList />);
    });

    const scroller = container.querySelector<HTMLElement>('[data-virtuoso-scroller="true"]');
    expect(scroller).not.toBeNull();
    expect(container.querySelector('[data-testid="virtuoso"]')).not.toBeNull();
    expect(container.querySelector('[data-initial-history-snapshot="true"]')).not.toBeNull();

    act(() => {
      scroller?.dispatchEvent(new WheelEvent('wheel', {
        deltaY: -120,
        bubbles: true,
      }));
    });

    expect(container.querySelector('[data-initial-history-snapshot="true"]')).toBeNull();
    expect(container.querySelector('[data-turn-id="turn-3"]')).not.toBeNull();
    expect(flowStoreMocks.revealPreviousSessionHistoryWindow).not.toHaveBeenCalled();

    flushAnimationFrame();
    flushAnimationFrame();

    expect(flowStoreMocks.revealPreviousSessionHistoryWindow).toHaveBeenCalledWith('session-a', 'wheel-up');

    stateMocks.activeSession = createSessionWithTurns(
      'session-a',
      ['turn-0', 'turn-1', 'turn-2', 'turn-3', 'turn-4', 'turn-5'],
      {
        isHistorical: true,
        historyState: 'ready',
        contextRestoreState: 'pending',
        isPartial: false,
        loadedTurnCount: 6,
        totalTurnCount: 6,
      },
    );
    stateMocks.virtualItems = ['turn-0', 'turn-1', 'turn-2', 'turn-3', 'turn-4', 'turn-5'].flatMap(turnId => [
      createItem(turnId),
      createModelItem(turnId),
    ]);

    act(() => {
      root.render(<VirtualMessageList />);
    });

    expect(container.querySelector('[data-turn-id="turn-3"]')).not.toBeNull();
    expect(container.querySelector('[data-history-paging-sentinel]')).toBeNull();
  });

  it('waits until the Virtuoso history boundary is near before starting pagination', () => {
    flowStoreMocks.hasDeferredSessionHistoryProjection.mockReturnValue(true);
    stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-3', 'turn-4', 'turn-5'], {
      isHistorical: true,
      historyState: 'ready',
      contextRestoreState: 'pending',
      isPartial: true,
      loadedTurnCount: 3,
      totalTurnCount: 6,
    });
    stateMocks.virtualItems = ['turn-3', 'turn-4', 'turn-5'].flatMap(turnId => [
      createItem(turnId),
      createModelItem(turnId),
    ]);

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
      scrollTop: 2_000,
    });

    act(() => {
      scroller.dispatchEvent(new WheelEvent('wheel', { deltaY: -120, bubbles: true }));
      scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
    });
    flushAnimationFrame();
    flushAnimationFrame();

    expect(flowStoreMocks.revealPreviousSessionHistoryWindow).not.toHaveBeenCalled();

    scroller.scrollTop = 1_000;
    act(() => {
      scroller.dispatchEvent(new Event('scroll', { bubbles: true }));
    });
    flushAnimationFrame();
    flushAnimationFrame();

    expect(flowStoreMocks.revealPreviousSessionHistoryWindow).toHaveBeenCalledWith('session-a', 'scroll-near-partial-history-boundary');
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

    expect(flowStoreMocks.revealPreviousSessionHistoryWindow).not.toHaveBeenCalled();
    expect(container.querySelector('[data-history-boundary-status]')).toBeNull();
  });

  it('requests an adjacent Turn window for catalog-backed tail history', async () => {
    const onHistoryWindowBoundaryIntent = vi.fn(async (
      _direction: 'before' | 'after',
      _options?: {
        prepareViewportForPresentationCommit?: () => boolean | void | Promise<boolean | void>;
      },
    ) => {
      return true;
    });
    stateMocks.activeSession = createSession('session-a', 'turn-a', {
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'ready',
      isPartial: true,
      loadedTurnCount: 1,
      totalTurnCount: 20,
      turnCatalog: {
        schemaVersion: 1,
        sessionId: 'session-a',
        revision: 'catalog-1',
        totalTurnCount: 20,
        complete: true,
        entries: Array.from({ length: 20 }, (_, ordinal) => ({
          ordinal,
          storageTurnIndex: ordinal,
          turnId: `turn-${ordinal + 1}`,
          preview: `Prompt ${ordinal + 1}`,
          previewTruncated: false,
        })),
      },
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
      root.render(
        <VirtualMessageList onHistoryWindowBoundaryIntent={onHistoryWindowBoundaryIntent} />,
      );
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
    await act(async () => {
      await Promise.resolve();
    });

    expect(onHistoryWindowBoundaryIntent).toHaveBeenCalledWith('before', expect.objectContaining({
      prepareViewportForPresentationCommit: expect.any(Function),
    }));
    expect(flowStoreMocks.revealPreviousSessionHistoryWindow).not.toHaveBeenCalled();
    expect(flowStoreMocks.releaseSessionHistoryCompletionAfterInitialPaint).not.toHaveBeenCalled();
  });

  it('treats an exhausted catalog boundary as complete without showing a not-ready error', async () => {
    const onHistoryWindowBoundaryIntent = vi.fn(async () => 'exhausted' as const);
    stateMocks.activeSession = createSession('session-a', 'turn-a', {
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'ready',
      isPartial: true,
      loadedTurnCount: 1,
      totalTurnCount: 1,
      turnCatalog: {
        schemaVersion: 1,
        sessionId: 'session-a',
        revision: 'catalog-1',
        totalTurnCount: 1,
        complete: true,
        entries: [{
          ordinal: 0,
          storageTurnIndex: 0,
          turnId: 'turn-a',
          preview: 'Prompt 1',
          previewTruncated: false,
        }],
      },
    });
    stateMocks.virtualItems = [createItem('turn-a')];

    act(() => {
      root.render(
        <VirtualMessageList onHistoryWindowBoundaryIntent={onHistoryWindowBoundaryIntent} />,
      );
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
    await act(async () => {
      await Promise.resolve();
    });

    expect(onHistoryWindowBoundaryIntent).toHaveBeenCalledOnce();
    expect(container.querySelector('[data-history-boundary-status]')).toBeNull();
  });

  it('releases a pending catalog boundary transaction when returning from history to tail', () => {
    const pendingBoundary = new Promise<'applied'>(() => {});
    const onHistoryWindowBoundaryIntent = vi.fn(() => pendingBoundary);
    stateMocks.activeSession = createSessionWithTurns('session-a', ['turn-a'], {
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'ready',
      isPartial: true,
      loadedTurnCount: 1,
      totalTurnCount: 2,
      turnCatalog: {
        schemaVersion: 1,
        sessionId: 'session-a',
        revision: 'catalog-1',
        totalTurnCount: 2,
        complete: true,
        entries: Array.from({ length: 2 }, (_, ordinal) => ({
          ordinal,
          storageTurnIndex: ordinal,
          turnId: `turn-${ordinal + 1}`,
          preview: `Prompt ${ordinal + 1}`,
          previewTruncated: false,
        })),
      },
    });
    stateMocks.virtualItems = [createItem('turn-a')];

    act(() => {
      root.render(
        <VirtualMessageList
          presentationMode="tail"
          onHistoryWindowBoundaryIntent={onHistoryWindowBoundaryIntent}
        />,
      );
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
    expect(onHistoryWindowBoundaryIntent).toHaveBeenCalledOnce();

    act(() => {
      root.render(
        <VirtualMessageList
          presentationMode="history-window"
          historyWindow={{
            startOrdinal: 0,
            endOrdinalExclusive: 2,
            targetTurnId: null,
            mode: 'history-window',
          }}
          onHistoryWindowBoundaryIntent={onHistoryWindowBoundaryIntent}
        />,
      );
    });
    act(() => {
      root.render(
        <VirtualMessageList
          presentationMode="tail"
          onHistoryWindowBoundaryIntent={onHistoryWindowBoundaryIntent}
        />,
      );
    });

    act(() => {
      scroller?.dispatchEvent(new WheelEvent('wheel', {
        deltaY: -120,
        bubbles: true,
      }));
    });
    flushAnimationFrame();
    flushAnimationFrame();
    expect(onHistoryWindowBoundaryIntent).toHaveBeenCalledTimes(2);
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

    expect(flowStoreMocks.revealPreviousSessionHistoryWindow).not.toHaveBeenCalled();
    expect(flowStoreMocks.releaseSessionHistoryCompletionAfterInitialPaint).not.toHaveBeenCalled();
    expect(container.querySelector('[data-history-boundary-status="not-ready"]')?.textContent).toBe('Older history is not ready yet.');
  });
});
