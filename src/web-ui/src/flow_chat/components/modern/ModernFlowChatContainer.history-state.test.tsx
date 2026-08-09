// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { ModernFlowChatContainer } from './ModernFlowChatContainer';
import type { HistoryWindowBoundaryIntentResult } from './VirtualMessageList';
import type { Session } from '../../types/flow-chat';
import { flowChatStore } from '../../store/FlowChatStore';
import {
  clearHistorySessionOpenTransition,
  dispatchHistorySessionOpenIntent,
  HISTORY_SESSION_OPEN_INTENT_EVENT,
} from '../../services/sessionOpenIntent';
import { FLOWCHAT_TURN_RAIL_ROW_HEIGHT_PX } from './flowChatTurnRailWindow';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const stateMocks = vi.hoisted(() => ({
  activeSession: null as Session | null,
  virtualItems: [] as unknown[],
  visibleTurnInfo: null as unknown,
}));

const switchChatSessionMock = vi.hoisted(() => vi.fn());
const virtualListMock = vi.hoisted(() => ({
  scrollToTurn: vi.fn(),
  scrollToIndex: vi.fn(),
  scrollToSearchMatch: vi.fn(),
  clearSearchMatch: vi.fn(),
  scrollToPhysicalBottomAndClearPin: vi.fn(),
  scrollToTurnEndAndClearPin: vi.fn(() => true),
  scrollToLatestEndPosition: vi.fn(),
  isTurnRenderedInViewport: vi.fn(() => false),
  isTurnTextRenderedInViewport: vi.fn(() => false),
  pinTurnToTop: vi.fn(() => true),
  pinTurnToTopWithStatus: vi.fn(() => 'settled' as const),
  prepareTurnPinToTop: vi.fn(() => 'pending' as const),
}));
const virtualListActionClickMock = vi.hoisted(() => vi.fn());
const startupTraceMock = vi.hoisted(() => ({
  markPhase: vi.fn(),
}));
const historySessionDiagnosticsMock = vi.hoisted(() => ({
  beginHistorySessionDiagnostics: vi.fn(() => 'diag-1'),
  recordHistorySessionDiagnosticEvent: vi.fn(),
  warnHistorySessionLoadingLayerStalled: vi.fn(),
}));
const searchStateMock = vi.hoisted(() => ({
  searchQuery: '',
  onSearchChange: vi.fn(),
  matches: [] as unknown[],
  matchIndices: [] as number[],
  currentMatchIndex: -1,
  currentMatchVirtualIndex: -1,
  goToNext: vi.fn(),
  goToPrev: vi.fn(),
  clearSearch: vi.fn(),
}));
const headerPropsMock = vi.hoisted(() => ({
  latest: null as Record<string, unknown> | null,
}));
const virtualListPropsMock = vi.hoisted(() => ({
  latest: null as Record<string, unknown> | null,
}));
const navigationOptionsMock = vi.hoisted(() => ({
  latest: null as Record<string, unknown> | null,
}));
const agentApiMock = vi.hoisted(() => ({
  listBackgroundCommandActivities: vi.fn(() => Promise.resolve({ activities: [] })),
  onPermissionRequestEvent: vi.fn(() => vi.fn()),
  subscribePermissionRequests: vi.fn(() => Promise.resolve()),
  listPendingPermissionRequests: vi.fn(() => Promise.resolve([])),
}));

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
  useTranslation: () => ({
    t: (key: string) => {
      const labels: Record<string, string> = {
        'historyState.loadingTitle': 'Loading saved session',
        'historyState.loadingDescription': 'Preparing the conversation history.',
        'historyState.failedTitle': 'Session history did not load',
        'historyState.failedDescription': 'Retry loading the saved conversation.',
        'historyState.retry': 'Retry',
      };
      return labels[key] ?? key;
    },
  }),
}));

vi.mock('@/infrastructure/hooks/useShortcut', () => ({
  useShortcut: vi.fn(),
}));

vi.mock('@/flow_chat/services/FlowChatManager', () => ({
  FlowChatManager: {
    getInstance: () => ({
      cancelCurrentTask: vi.fn(),
      createChatSession: vi.fn(),
      switchChatSession: switchChatSessionMock,
    }),
  },
}));

vi.mock('@/app/stores/sessionModeStore', () => ({
  useSessionModeStore: {
    getState: () => ({
      setMode: vi.fn(),
    }),
  },
}));

vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useWorkspaceContext: () => ({
    workspacePath: 'D:/workspace/BitFun',
  }),
}));

vi.mock('@/infrastructure/api/service-api/AgentAPI', () => ({
  agentAPI: agentApiMock,
}));

vi.mock('../../utils/acpSession', () => ({
  isAcpFlowSession: () => false,
}));

vi.mock('../../store/modernFlowChatStore', () => ({
  sessionToVirtualItems: (session: Session | null) => (session?.dialogTurns ?? []).map(turn => ({
    type: 'user-message',
    turnId: turn.id,
    data: turn.userMessage,
  })),
  useVirtualItems: () => stateMocks.virtualItems,
  useActiveSession: () => stateMocks.activeSession,
  useVisibleTurnInfo: () => stateMocks.visibleTurnInfo,
}));

vi.mock('./VirtualMessageList', () => ({
  VirtualMessageList: React.forwardRef((props: Record<string, unknown>, ref) => {
    virtualListPropsMock.latest = props;
    React.useImperativeHandle(ref, () => virtualListMock);
    return (
      <div data-testid="virtual-list">
        <button type="button" data-testid="virtual-list-action" onClick={virtualListActionClickMock}>
          Hidden action
        </button>
      </div>
    );
  }),
}));

vi.mock('@/shared/utils/startupTrace', () => ({
  isRemoteTraceContext: () => false,
  startupTrace: startupTraceMock,
}));

vi.mock('../../services/historySessionDiagnostics', () => historySessionDiagnosticsMock);

vi.mock('./FlowChatHeader', () => ({
  FlowChatHeader: (props: Record<string, unknown>) => {
    headerPropsMock.latest = props;
    return <div data-testid="flowchat-header" />;
  },
}));

vi.mock('../WelcomePanel', () => ({
  WelcomePanel: () => <div data-testid="welcome-panel">Welcome panel</div>,
}));

vi.mock('./useExploreGroupState', () => ({
  useExploreGroupState: () => ({
    exploreGroupStates: {},
    onExploreGroupToggle: vi.fn(),
    onExpandGroup: vi.fn(),
    onExpandAllInTurn: vi.fn(),
    onCollapseGroup: vi.fn(),
  }),
}));

vi.mock('./useFlowChatFileActions', () => ({
  useFlowChatFileActions: () => ({
    handleFileViewRequest: vi.fn(),
  }),
}));

vi.mock('./useFlowChatNavigation', () => ({
  useFlowChatNavigation: (options: Record<string, unknown>) => {
    navigationOptionsMock.latest = options;
  },
}));

vi.mock('./useFlowChatCopyDialog', () => ({
  useFlowChatCopyDialog: vi.fn(),
}));

vi.mock('./useFlowChatSync', () => ({
  useFlowChatSync: vi.fn(),
}));

vi.mock('./useFlowChatToolActions', () => ({
  useFlowChatToolActions: () => ({
    handleToolConfirm: vi.fn(),
    handleToolReject: vi.fn(),
  }),
}));

vi.mock('./useFlowChatSearch', () => ({
  useFlowChatSearch: () => searchStateMock,
}));

function createSession(overrides: Partial<Session> = {}): Session {
  return {
    sessionId: 'session-1',
    title: 'Saved session',
    dialogTurns: [],
    status: 'idle',
    config: { agentType: 'agentic' },
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    isHistorical: true,
    todos: [],
    mode: 'agentic',
    workspacePath: 'D:/workspace/BitFun',
    sessionKind: 'normal',
    ...overrides,
  };
}

function createTurn(id: string, content: string, status: 'completed' | 'processing' = 'completed') {
  return {
    id,
    turnId: id,
    sessionId: 'session-1',
    timestamp: 1,
    userMessage: { id: `user-${id}`, content, timestamp: 1 },
    modelRounds: [],
    startTime: 1,
    status,
  };
}

let rafCallbacks: FrameRequestCallback[] = [];

function flushAnimationFrame() {
  const callbacks = rafCallbacks;
  rafCallbacks = [];
  act(() => {
    callbacks.forEach(callback => callback(performance.now()));
  });
}

function clickTurnRailItem(container: HTMLElement, turnId: string) {
  const item = container.querySelector<HTMLButtonElement>(`[data-turn-id="${turnId}"]`);
  expect(item).not.toBeNull();
  act(() => {
    item?.click();
  });
}

function scrollTurnRailToOrdinal(
  container: HTMLElement,
  ordinal: number,
  clientHeight = FLOWCHAT_TURN_RAIL_ROW_HEIGHT_PX * 5,
) {
  const list = container.querySelector<HTMLDivElement>('.flowchat-turn-rail__list');
  expect(list).not.toBeNull();
  if (!list) return;

  Object.defineProperty(list, 'clientHeight', { configurable: true, value: clientHeight });
  list.scrollTop = ordinal * FLOWCHAT_TURN_RAIL_ROW_HEIGHT_PX;
  act(() => {
    list.dispatchEvent(new Event('scroll', { bubbles: true }));
  });
}

describe('ModernFlowChatContainer historical empty state', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.restoreAllMocks();
    rafCallbacks = [];
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      rafCallbacks.push(callback);
      return rafCallbacks.length;
    }));
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
    // jsdom in vitest 4.x may expose window.localStorage without a callable
    // getItem; provide a minimal storage so shouldShowMockBackgroundActivities
    // does not crash during render.
    vi.stubGlobal('localStorage', {
      getItem: vi.fn(() => null),
      setItem: vi.fn(),
      removeItem: vi.fn(),
      clear: vi.fn(),
      key: vi.fn(() => null),
      length: 0,
    });
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    stateMocks.virtualItems = [];
    stateMocks.visibleTurnInfo = null;
    switchChatSessionMock.mockReset();
    virtualListMock.scrollToTurn.mockReset();
    virtualListMock.scrollToIndex.mockReset();
    virtualListMock.scrollToSearchMatch.mockReset();
    virtualListMock.clearSearchMatch.mockReset();
    virtualListMock.scrollToPhysicalBottomAndClearPin.mockReset();
    virtualListMock.scrollToTurnEndAndClearPin.mockReset();
    virtualListMock.scrollToTurnEndAndClearPin.mockReturnValue(true);
    virtualListMock.scrollToLatestEndPosition.mockReset();
    virtualListMock.isTurnRenderedInViewport.mockReset();
    virtualListMock.isTurnRenderedInViewport.mockReturnValue(false);
    virtualListMock.isTurnTextRenderedInViewport.mockReset();
    virtualListMock.isTurnTextRenderedInViewport.mockReturnValue(false);
    virtualListMock.pinTurnToTop.mockReset();
    virtualListMock.pinTurnToTop.mockReturnValue(true);
    virtualListMock.pinTurnToTopWithStatus.mockReset();
    virtualListMock.pinTurnToTopWithStatus.mockReturnValue('settled');
    virtualListMock.prepareTurnPinToTop.mockReset();
    virtualListMock.prepareTurnPinToTop.mockReturnValue('pending');
    virtualListActionClickMock.mockReset();
    startupTraceMock.markPhase.mockReset();
    historySessionDiagnosticsMock.beginHistorySessionDiagnostics.mockReset();
    historySessionDiagnosticsMock.beginHistorySessionDiagnostics.mockReturnValue('diag-1');
    historySessionDiagnosticsMock.recordHistorySessionDiagnosticEvent.mockReset();
    historySessionDiagnosticsMock.warnHistorySessionLoadingLayerStalled.mockReset();
    agentApiMock.listBackgroundCommandActivities.mockClear();
    agentApiMock.listBackgroundCommandActivities.mockResolvedValue({ activities: [] });
    searchStateMock.searchQuery = '';
    searchStateMock.onSearchChange.mockReset();
    searchStateMock.matches = [];
    searchStateMock.matchIndices = [];
    searchStateMock.currentMatchIndex = -1;
    searchStateMock.currentMatchVirtualIndex = -1;
    searchStateMock.goToNext.mockReset();
    searchStateMock.goToPrev.mockReset();
    searchStateMock.clearSearch.mockReset();
    headerPropsMock.latest = null;
    virtualListPropsMock.latest = null;
    navigationOptionsMock.latest = null;
    clearHistorySessionOpenTransition();
  });

  afterEach(() => {
    if (root) {
      act(() => {
        root.unmount();
      });
    }
    container?.remove();
    stateMocks.activeSession = null;
    clearHistorySessionOpenTransition();
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('shows a history loading shell for metadata-only sessions instead of the new-session welcome', () => {
    stateMocks.activeSession = createSession({ historyState: 'metadata-only' } as Partial<Session>);

    act(() => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(container.textContent).toContain('Loading saved session');
    expect(container.querySelector('[data-testid="welcome-panel"]')).toBeNull();
  });

  it('defers viewport anchoring while the host scene is inactive', () => {
    const turn = createTurn('turn-1', 'One');
    stateMocks.activeSession = createSession({
      dialogTurns: [turn],
      historyState: 'ready',
      contextRestoreState: 'ready',
    });
    stateMocks.virtualItems = [{
      type: 'user-message',
      turnId: turn.id,
      data: turn.userMessage,
    }];

    act(() => {
      root.render(<ModernFlowChatContainer isViewportActive={false} />);
    });

    expect(virtualListPropsMock.latest).toMatchObject({ isViewportActive: false });
    expect(virtualListMock.scrollToTurnEndAndClearPin).not.toHaveBeenCalled();

    act(() => {
      root.render(<ModernFlowChatContainer isViewportActive />);
    });

    expect(virtualListPropsMock.latest).toMatchObject({ isViewportActive: true });
    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenCalledWith(turn.id);
  });

  it('keeps the loading shell while historical sessions are hydrating', () => {
    stateMocks.activeSession = createSession({ historyState: 'hydrating' } as Partial<Session>);

    act(() => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(container.textContent).toContain('Loading saved session');
    expect(container.querySelector('[data-testid="welcome-panel"]')).toBeNull();
  });

  it('renders a host-provided empty state instead of the generic welcome panel', () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'new',
      dialogTurns: [],
    } as Partial<Session>);

    act(() => {
      root.render(
        <ModernFlowChatContainer
          emptyState={<div data-testid="miniapp-welcome">MiniApp welcome</div>}
        />
      );
    });

    expect(container.querySelector('[data-testid="miniapp-welcome"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="welcome-panel"]')).toBeNull();
  });

  it('reports a stalled history loading layer after the diagnostic threshold', async () => {
    vi.useFakeTimers();
    stateMocks.activeSession = createSession({
      sessionId: 'history-session',
      historyState: 'metadata-only',
      dialogTurns: [],
    } as Partial<Session>);

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(container.textContent).toContain('Loading saved session');
    expect(historySessionDiagnosticsMock.warnHistorySessionLoadingLayerStalled).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(799);
    });

    expect(historySessionDiagnosticsMock.warnHistorySessionLoadingLayerStalled).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });

    expect(historySessionDiagnosticsMock.warnHistorySessionLoadingLayerStalled).toHaveBeenCalledWith(
      'history-session',
      expect.objectContaining({
        durationMs: 800,
        historyState: 'metadata-only',
        isHistorical: true,
        isRemote: false,
        activeSessionIdMatches: true,
        hasRenderableContent: false,
        dialogTurnCount: 0,
      }),
    );
  });

  it('does not show the new-session welcome while a restored session is waiting for virtual items', () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [{
        id: 'turn-1',
        turnId: 'turn-1',
        sessionId: 'session-1',
        timestamp: 1,
        userMessage: { id: 'user-1', content: 'Saved prompt', timestamp: 1 },
        modelRounds: [],
        startTime: 1,
        status: 'completed',
      }],
    } as Partial<Session>);

    act(() => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(container.textContent).toContain('Loading saved session');
    expect(container.querySelector('[data-testid="welcome-panel"]')).toBeNull();
  });

  it('covers the current message list after a historical session open intent', async () => {
    stateMocks.activeSession = createSession({
      sessionId: 'current-session',
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [createTurn('turn-1', 'Current visible prompt')],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Current visible prompt' } },
    ];

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(container.querySelector('[data-testid="virtual-list"]')).not.toBeNull();
    expect(container.querySelector('.modern-flowchat-container__history-overlay')).toBeNull();

    act(() => {
      window.dispatchEvent(new CustomEvent(HISTORY_SESSION_OPEN_INTENT_EVENT, {
        detail: { sessionId: 'history-session', sessionTitle: 'Saved investigation' },
      }));
    });

    expect(container.textContent).not.toContain('Loading saved session');
    expect(container.querySelector('[data-testid="virtual-list"]')).not.toBeNull();
    expect(container.querySelector('.modern-flowchat-container__history-overlay')).toBeNull();
    expect(container.querySelector('.modern-flowchat-container__history-open-intent-shield')).not.toBeNull();
    expect(container.querySelector('.modern-flowchat-container__history-open-intent-spinner')).not.toBeNull();
    expect(container.textContent).toContain('Hidden action');
    expect(container.textContent).not.toContain('Saved investigation');
    expect(container.querySelector('.modern-flowchat-container__messages')?.getAttribute('data-show-history-open-intent-overlay'))
      .toBe('true');
    (container.querySelector('[data-testid="virtual-list-action"]') as HTMLButtonElement | null)?.click();
    expect(virtualListActionClickMock).not.toHaveBeenCalled();

    stateMocks.activeSession = createSession({
      sessionId: 'history-session',
      historyState: 'metadata-only',
    } as Partial<Session>);
    stateMocks.virtualItems = [];

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(container.textContent).not.toContain('Loading saved session');
    expect(container.querySelector('.modern-flowchat-container__history-overlay')).toBeNull();
    expect(container.querySelector('[data-testid="welcome-panel"]')).toBeNull();
    expect(container.querySelector('.modern-flowchat-container__history-open-intent-shield')).not.toBeNull();
    expect(container.querySelector('.modern-flowchat-container__messages')?.getAttribute('data-show-history-loading-layer'))
      .toBe('false');
    expect(container.querySelector('.modern-flowchat-container__messages')?.getAttribute('data-show-history-open-intent-overlay'))
      .toBe('true');

    stateMocks.activeSession = createSession({
      sessionId: 'history-session',
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [createTurn('turn-2', 'Restored latest prompt')],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Restored latest prompt' } },
    ];

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(container.querySelector('[data-testid="virtual-list"]')).not.toBeNull();
    expect(container.querySelector('.modern-flowchat-container__history-open-intent-shield')).toBeNull();
    expect(container.querySelector('.modern-flowchat-container__messages')?.getAttribute('data-show-history-open-intent-overlay'))
      .toBe('false');
  });

  it('removes the loading layer when a hydrating session receives its initial tail turns', async () => {
    stateMocks.activeSession = createSession({
      sessionId: 'history-session',
      historyState: 'hydrating',
      dialogTurns: [],
    } as Partial<Session>);
    stateMocks.virtualItems = [];

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    const initialOverlay = container.querySelector('.modern-flowchat-container__history-overlay');
    expect(initialOverlay).not.toBeNull();
    expect(container.querySelector('[data-testid="virtual-list"]')).toBeNull();

    stateMocks.activeSession = createSession({
      sessionId: 'history-session',
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      dialogTurns: [
        createTurn('turn-1', 'Older restored prompt'),
        createTurn('turn-2', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older restored prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest restored prompt' } },
    ];
    virtualListMock.isTurnTextRenderedInViewport.mockReturnValue(false);

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(container.querySelector('[data-testid="virtual-list"]')).not.toBeNull();
    expect(container.querySelector('.modern-flowchat-container__history-overlay')).not.toBe(initialOverlay);
    expect(container.querySelector('.modern-flowchat-container__history-overlay')).toBeNull();
    expect(container.textContent).not.toContain('Loading saved session');
    expect(container.querySelector('.modern-flowchat-container__messages')?.getAttribute('data-show-history-transition-overlay'))
      .toBe('true');
  });

  it('keeps restored content visible while restored latest text is not ready', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      dialogTurns: [
        createTurn('turn-1', 'Older restored prompt'),
        createTurn('turn-2', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older restored prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest restored prompt' } },
    ];
    virtualListMock.isTurnTextRenderedInViewport.mockReturnValue(false);

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(container.querySelector('[data-testid="virtual-list"]')).not.toBeNull();
    expect(container.textContent).not.toContain('Loading saved session');
    expect(container.querySelector('.modern-flowchat-container__history-overlay')).toBeNull();
    expect(container.querySelector('.modern-flowchat-container__messages')?.getAttribute('data-show-history-transition-overlay'))
      .toBe('true');

    flushAnimationFrame();
    expect(container.querySelector('[data-testid="virtual-list"]')).not.toBeNull();
    expect(container.textContent).not.toContain('Loading saved session');
    expect(container.querySelector('.modern-flowchat-container__history-overlay')).toBeNull();

    virtualListMock.isTurnTextRenderedInViewport.mockReturnValue(true);
    flushAnimationFrame();

    expect(container.textContent).not.toContain('Loading saved session');
    expect(container.querySelector('.modern-flowchat-container__history-overlay')).toBeNull();
  });

  it('does not show the initial history progress again when full hydration adds older turns', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      dialogTurns: [
        createTurn('turn-1', 'Older restored prompt'),
        createTurn('turn-2', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older restored prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest restored prompt' } },
    ];
    virtualListMock.isTurnTextRenderedInViewport.mockReturnValue(false);

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(container.querySelector('.modern-flowchat-container__history-overlay')).toBeNull();

    virtualListMock.isTurnTextRenderedInViewport.mockReturnValue(true);
    flushAnimationFrame();

    expect(container.querySelector('.modern-flowchat-container__history-overlay')).toBeNull();

    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      dialogTurns: [
        createTurn('turn-0', 'Restored older prompt'),
        createTurn('turn-1', 'Older restored prompt'),
        createTurn('turn-2', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-0', data: { id: 'user-turn-0', content: 'Restored older prompt' } },
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older restored prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest restored prompt' } },
    ];

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(container.querySelector('.modern-flowchat-container__history-overlay')).toBeNull();
  });

  it('blocks pointer activation until restored latest text is visible', async () => {
    const releaseSpy = vi
      .spyOn(flowChatStore, 'releaseSessionHistoryCompletionAfterInitialPaint')
      .mockReturnValue(true);

    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      dialogTurns: [
        createTurn('turn-1', 'Older restored prompt'),
        createTurn('turn-2', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older restored prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest restored prompt' } },
    ];
    virtualListMock.isTurnTextRenderedInViewport.mockReturnValue(false);

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    const hiddenAction = container.querySelector('[data-testid="virtual-list-action"]') as HTMLButtonElement;
    expect(hiddenAction).not.toBeNull();
    expect(container.textContent).not.toContain('Loading saved session');
    expect(container.querySelector('.modern-flowchat-container__history-overlay')).toBeNull();

    act(() => {
      hiddenAction.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    });

    expect(virtualListActionClickMock).not.toHaveBeenCalled();

    virtualListMock.isTurnTextRenderedInViewport.mockReturnValue(true);
    flushAnimationFrame();
    flushAnimationFrame();
    flushAnimationFrame();

    expect(container.querySelector('.modern-flowchat-container__history-overlay')).toBeNull();
    expect(releaseSpy).toHaveBeenCalledWith('session-1');
    expect(startupTraceMock.markPhase).toHaveBeenCalledWith(
      'historical_session_initial_content_painted',
      expect.objectContaining({
        sessionId: 'session-1',
        latestTurnId: 'turn-2',
        released: true,
      }),
    );

    act(() => {
      hiddenAction.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    });

    expect(virtualListActionClickMock).toHaveBeenCalledTimes(1);
    releaseSpy.mockRestore();
  });

  it('defers background command snapshot until restored latest text is visible and painted', async () => {
    const releaseSpy = vi
      .spyOn(flowChatStore, 'releaseSessionHistoryCompletionAfterInitialPaint')
      .mockReturnValue(true);

    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      dialogTurns: [
        createTurn('turn-1', 'Older restored prompt'),
        createTurn('turn-2', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older restored prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest restored prompt' } },
    ];
    virtualListMock.isTurnTextRenderedInViewport.mockReturnValue(false);

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(agentApiMock.listBackgroundCommandActivities).not.toHaveBeenCalled();

    virtualListMock.isTurnTextRenderedInViewport.mockReturnValue(true);
    flushAnimationFrame();
    expect(releaseSpy).not.toHaveBeenCalled();
    expect(agentApiMock.listBackgroundCommandActivities).not.toHaveBeenCalled();

    flushAnimationFrame();
    expect(releaseSpy).not.toHaveBeenCalled();
    expect(agentApiMock.listBackgroundCommandActivities).not.toHaveBeenCalled();

    flushAnimationFrame();
    expect(releaseSpy).toHaveBeenCalledWith('session-1');
    expect(agentApiMock.listBackgroundCommandActivities).not.toHaveBeenCalled();

    flushAnimationFrame();
    expect(agentApiMock.listBackgroundCommandActivities).not.toHaveBeenCalled();

    flushAnimationFrame();
    expect(agentApiMock.listBackgroundCommandActivities).toHaveBeenCalledTimes(1);
    expect(agentApiMock.listBackgroundCommandActivities).toHaveBeenCalledWith({
      agentSessionId: 'session-1',
    });

    releaseSpy.mockRestore();
  });

  it('skips stale background command snapshot when another history session open starts first', async () => {
    const releaseSpy = vi
      .spyOn(flowChatStore, 'releaseSessionHistoryCompletionAfterInitialPaint')
      .mockReturnValue(true);

    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      dialogTurns: [
        createTurn('turn-1', 'Older restored prompt'),
        createTurn('turn-2', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older restored prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest restored prompt' } },
    ];
    virtualListMock.isTurnTextRenderedInViewport.mockReturnValue(false);

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    virtualListMock.isTurnTextRenderedInViewport.mockReturnValue(true);
    flushAnimationFrame();
    flushAnimationFrame();

    act(() => {
      dispatchHistorySessionOpenIntent('session-2', 'Next saved session');
    });
    flushAnimationFrame();

    expect(releaseSpy).toHaveBeenCalledWith('session-1');
    flushAnimationFrame();
    flushAnimationFrame();
    expect(agentApiMock.listBackgroundCommandActivities).not.toHaveBeenCalled();

    releaseSpy.mockRestore();
  });

  it('keeps full history projection deferred when latest text visibility signal is missed', async () => {
    const releaseSpy = vi
      .spyOn(flowChatStore, 'releaseSessionHistoryCompletionAfterInitialPaint')
      .mockReturnValue(true);

    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      dialogTurns: [
        createTurn('turn-1', 'Older restored prompt'),
        createTurn('turn-2', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older restored prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest restored prompt' } },
    ];
    virtualListMock.isTurnTextRenderedInViewport.mockReturnValue(false);

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    for (let index = 0; index < 30; index += 1) {
      flushAnimationFrame();
    }

    expect(container.textContent).not.toContain('Loading saved session');
    expect(releaseSpy).not.toHaveBeenCalled();
    expect(startupTraceMock.markPhase).toHaveBeenCalledWith(
      'historical_session_initial_content_paint_signal_missed',
      expect.objectContaining({ attempts: 30 }),
    );

    releaseSpy.mockRestore();
  });

  it('requests full history when search starts from a partial session', async () => {
    const ensureSpy = vi
      .spyOn(flowChatStore, 'ensureSessionFullHistory')
      .mockResolvedValue(true);

    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'ready',
      isPartial: true,
      dialogTurns: [
        createTurn('turn-2', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest restored prompt' } },
    ];
    virtualListMock.isTurnTextRenderedInViewport.mockReturnValue(false);

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    await act(async () => {
      (headerPropsMock.latest?.onSearchChange as ((query: string) => void) | undefined)?.(
        'older prompt',
      );
      await Promise.resolve();
    });

    expect(searchStateMock.onSearchChange).toHaveBeenCalledWith('older prompt');
    expect(ensureSpy).toHaveBeenCalledWith('session-1', 'flowchat-search');
  });

  it('repositions an unchanged virtual match when the search query changes', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [createTurn('turn-1', 'Searchable prompt')],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Searchable prompt' } },
    ];
    searchStateMock.searchQuery = 'search';
    searchStateMock.matches = [{
      virtualItemIndex: 0,
      turnId: 'turn-1',
      type: 'user-message',
      occurrenceIndex: 0,
    }];
    searchStateMock.currentMatchIndex = 0;
    searchStateMock.currentMatchVirtualIndex = 0;

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });
    flushAnimationFrame();

    expect(virtualListMock.scrollToSearchMatch).toHaveBeenLastCalledWith({
      virtualItemIndex: 0,
      query: 'search',
      flowItemId: undefined,
      occurrenceIndex: 0,
      expandableIds: undefined,
    });

    searchStateMock.searchQuery = 'searchable';
    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });
    flushAnimationFrame();

    expect(virtualListMock.scrollToSearchMatch).toHaveBeenCalledTimes(2);
    expect(virtualListMock.scrollToSearchMatch).toHaveBeenLastCalledWith({
      virtualItemIndex: 0,
      query: 'searchable',
      flowItemId: undefined,
      occurrenceIndex: 0,
      expandableIds: undefined,
    });
  });

  it('keeps the new-session welcome for genuinely new empty sessions', () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'new',
    } as Partial<Session>);

    act(() => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(container.querySelector('[data-testid="welcome-panel"]')).not.toBeNull();
  });

  it('shows retry for failed history loads', () => {
    stateMocks.activeSession = createSession({ historyState: 'failed' } as Partial<Session>);

    act(() => {
      root.render(<ModernFlowChatContainer />);
    });

    const retryButton = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('Retry'));
    expect(container.textContent).toContain('Session history did not load');
    expect(retryButton).toBeTruthy();

    act(() => {
      retryButton?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(switchChatSessionMock).toHaveBeenCalledWith('session-1');
  });

  it('shows global turn numbers for partial tail history while navigation stays within loaded turns', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      isPartial: true,
      loadedTurnCount: 2,
      totalTurnCount: 100,
      dialogTurns: [
        createTurn('turn-99', 'Recent restored prompt'),
        createTurn('turn-100', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-99', data: { id: 'user-turn-99', content: 'Recent restored prompt' } },
      { type: 'user-message', turnId: 'turn-100', data: { id: 'user-turn-100', content: 'Latest restored prompt' } },
    ];
    stateMocks.visibleTurnInfo = {
      turnId: 'turn-100',
      turnIndex: 2,
      totalTurns: 2,
      userMessage: 'Latest restored prompt',
      visibleTurnIds: ['turn-99', 'turn-100'],
    };

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(headerPropsMock.latest).toMatchObject({
      currentTurn: 100,
      totalTurns: 100,
    });
    const previousTurnRailItem = container.querySelector<HTMLButtonElement>('[data-turn-id="turn-99"]');
    const currentTurnRailItem = container.querySelector<HTMLButtonElement>('[data-turn-id="turn-100"]');
    expect(previousTurnRailItem?.dataset.turnIndex).toBe('99');
    expect(currentTurnRailItem?.dataset.turnIndex).toBe('100');
    expect(currentTurnRailItem?.getAttribute('aria-current')).toBe('step');
    expect(previousTurnRailItem?.className).toContain('flowchat-turn-rail__item--visible');
    expect(currentTurnRailItem?.className).toContain('flowchat-turn-rail__item--visible');

    clickTurnRailItem(container, 'turn-99');

    expect(virtualListMock.pinTurnToTopWithStatus).toHaveBeenLastCalledWith('turn-99', {
      behavior: 'auto',
      pinMode: 'transient',
      alignmentPolicy: 'best-effort',
    });

  });

  it('treats the latest streaming Turn marker as transient immediate navigation', async () => {
    const streamingTurn = {
      ...createTurn('turn-2', 'Streaming prompt', 'processing'),
      modelRounds: [{
        id: 'round-2',
        index: 0,
        items: [{
          id: 'text-2',
          type: 'text' as const,
          content: 'Streaming output',
          isStreaming: true,
          timestamp: 1,
          status: 'streaming' as const,
        }],
        isStreaming: true,
        isComplete: false,
        status: 'streaming' as const,
        startTime: 1,
      }],
    } as Session['dialogTurns'][number];
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [
        createTurn('turn-1', 'Older prompt'),
        streamingTurn,
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Streaming prompt' } },
    ];
    stateMocks.visibleTurnInfo = {
      turnId: 'turn-1',
      turnIndex: 1,
      totalTurns: 2,
      userMessage: 'Older prompt',
    };

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    const restoreTailSpy = vi.spyOn(flowChatStore, 'restoreSessionTailPresentation');
    const latestEndCallCount = virtualListMock.scrollToLatestEndPosition.mock.calls.length;
    clickTurnRailItem(container, 'turn-2');

    expect(virtualListMock.pinTurnToTopWithStatus).toHaveBeenLastCalledWith('turn-2', {
      behavior: 'auto',
      pinMode: 'transient',
      alignmentPolicy: 'best-effort',
    });
    expect(restoreTailSpy).not.toHaveBeenCalled();
    expect(virtualListMock.scrollToLatestEndPosition.mock.calls.length).toBe(latestEndCallCount);

    restoreTailSpy.mockRestore();
  });

  it('keeps an active history presentation when its latest Turn marker is selected', async () => {
    const presentationTurns = Array.from(
      { length: 8 },
      (_, index) => createTurn(`turn-${index + 3}`, `Prompt ${index + 3}`),
    );
    const catalog = {
      schemaVersion: 1,
      sessionId: 'session-1',
      revision: 'catalog-v1',
      totalTurnCount: 10,
      complete: true,
      entries: Array.from({ length: 10 }, (_, ordinal) => ({
        ordinal,
        storageTurnIndex: ordinal,
        turnId: `turn-${ordinal + 1}`,
        preview: `Prompt ${ordinal + 1}`,
        previewTruncated: false,
      })),
    };
    const loadSpy = vi.spyOn(flowChatStore, 'loadSessionTurnWindow').mockResolvedValue({
      status: 'ready',
      sessionId: 'session-1',
      targetOrdinal: 4,
      targetTurnId: 'turn-5',
      navigationGeneration: 7,
      isCurrent: true,
      cacheHit: true,
      catalog,
      range: {
        startOrdinal: 2,
        endOrdinalExclusive: 10,
        turns: presentationTurns,
        lastAccessedAt: 1,
        source: 'target',
      },
    });
    const activateSpy = vi.spyOn(flowChatStore, 'activateSessionHistoryWindow').mockReturnValue({
      range: {
        startOrdinal: 2,
        endOrdinalExclusive: 10,
        targetTurnId: 'turn-5',
        mode: 'history-window',
      },
      turns: presentationTurns,
    });
    stateMocks.activeSession = createSession({
      historyState: 'ready',
      isPartial: true,
      totalTurnCount: 10,
      turnCatalog: catalog,
      dialogTurns: [
        createTurn('turn-9', 'Recent prompt'),
        createTurn('turn-10', 'Latest prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = stateMocks.activeSession.dialogTurns.map(turn => ({
      type: 'user-message',
      turnId: turn.id,
      data: turn.userMessage,
    }));

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });
    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-turn-id="turn-5"]')?.click();
      await Promise.resolve();
    });
    expect(virtualListPropsMock.latest).toMatchObject({ presentationMode: 'history-window' });

    const restoreTailSpy = vi.spyOn(flowChatStore, 'restoreSessionTailPresentation');
    const latestEndCallCount = virtualListMock.scrollToLatestEndPosition.mock.calls.length;
    scrollTurnRailToOrdinal(container, 9);
    clickTurnRailItem(container, 'turn-10');

    expect(virtualListMock.pinTurnToTopWithStatus).toHaveBeenLastCalledWith('turn-10', {
      behavior: 'auto',
      pinMode: 'transient',
      alignmentPolicy: 'best-effort',
    });
    expect(restoreTailSpy).not.toHaveBeenCalled();
    expect(virtualListPropsMock.latest).toMatchObject({ presentationMode: 'history-window' });
    expect(virtualListMock.scrollToLatestEndPosition.mock.calls.length).toBe(latestEndCallCount);

    stateMocks.activeSession = {
      ...stateMocks.activeSession,
      dialogTurns: [
        createTurn('turn-9', 'Recent prompt'),
        createTurn('turn-10', 'Latest live update', 'processing'),
      ],
    };
    stateMocks.virtualItems = stateMocks.activeSession.dialogTurns.map(turn => ({
      type: 'user-message',
      turnId: turn.id,
      data: turn.userMessage,
    }));
    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });
    const liveLatestItem = (virtualListPropsMock.latest?.items as Array<{
      type: string;
      turnId?: string;
      data?: { content?: string };
    }>).find(item => item.type === 'user-message' && item.turnId === 'turn-10');
    expect(liveLatestItem?.data?.content).toBe('Latest live update');
    expect(virtualListPropsMock.latest).toMatchObject({ presentationMode: 'history-window' });

    await act(async () => {
      (virtualListPropsMock.latest?.onRequestJumpToLatest as (() => void) | undefined)?.();
    });
    flushAnimationFrame();
    expect(restoreTailSpy).toHaveBeenCalledOnce();
    expect(restoreTailSpy).toHaveBeenLastCalledWith('session-1');
    expect(virtualListPropsMock.latest).toMatchObject({ presentationMode: 'tail' });
    expect(virtualListMock.scrollToLatestEndPosition.mock.calls.length).toBe(latestEndCallCount + 1);

    const tailAnchorCallCount = virtualListMock.scrollToTurnEndAndClearPin.mock.calls.length;
    stateMocks.activeSession = {
      ...stateMocks.activeSession,
      totalTurnCount: 11,
      dialogTurns: [
        ...stateMocks.activeSession.dialogTurns,
        createTurn('turn-11', 'New completed prompt'),
      ],
    };
    stateMocks.virtualItems = stateMocks.activeSession.dialogTurns.map(turn => ({
      type: 'user-message',
      turnId: turn.id,
      data: turn.userMessage,
    }));
    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });
    flushAnimationFrame();
    expect(virtualListPropsMock.latest).toMatchObject({ presentationMode: 'tail' });
    expect(virtualListMock.scrollToTurnEndAndClearPin.mock.calls.length).toBe(tailAnchorCallCount + 1);
    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenLastCalledWith('turn-11');

    const reactivateSpy = vi.spyOn(flowChatStore, 'reactivateSessionHistoryWindow').mockReturnValue({
      range: {
        startOrdinal: 2,
        endOrdinalExclusive: 10,
        targetTurnId: 'turn-5',
        mode: 'history-window',
      },
      turns: presentationTurns,
    });
    scrollTurnRailToOrdinal(container, 4);
    clickTurnRailItem(container, 'turn-5');
    expect(reactivateSpy).toHaveBeenCalledWith('session-1', {
      startOrdinal: 2,
      endOrdinalExclusive: 10,
      targetTurnId: 'turn-5',
      mode: 'history-window',
    });
    expect(loadSpy).toHaveBeenCalledTimes(1);
    expect(virtualListPropsMock.latest).toMatchObject({ presentationMode: 'history-window' });

    reactivateSpy.mockRestore();
    restoreTailSpy.mockRestore();
    loadSpy.mockRestore();
    activateSpy.mockRestore();
  });

  it('retains a complete small history projection when jumping to the latest Turn', async () => {
    const presentationTurns = Array.from(
      { length: 10 },
      (_, index) => createTurn(`turn-${index + 1}`, `Prompt ${index + 1}`),
    );
    const catalog = {
      schemaVersion: 1,
      sessionId: 'session-1',
      revision: 'catalog-complete-v1',
      totalTurnCount: 10,
      complete: true,
      entries: Array.from({ length: 10 }, (_, ordinal) => ({
        ordinal,
        storageTurnIndex: ordinal,
        turnId: `turn-${ordinal + 1}`,
        preview: `Prompt ${ordinal + 1}`,
        previewTruncated: false,
      })),
    };
    const loadSpy = vi.spyOn(flowChatStore, 'loadSessionTurnWindow').mockResolvedValue({
      status: 'ready',
      sessionId: 'session-1',
      targetOrdinal: 4,
      targetTurnId: 'turn-5',
      navigationGeneration: 8,
      isCurrent: true,
      cacheHit: true,
      catalog,
      range: {
        startOrdinal: 0,
        endOrdinalExclusive: 10,
        turns: presentationTurns,
        lastAccessedAt: 1,
        source: 'target',
      },
    });
    const activateSpy = vi.spyOn(flowChatStore, 'activateSessionHistoryWindow').mockReturnValue({
      range: {
        startOrdinal: 0,
        endOrdinalExclusive: 10,
        targetTurnId: 'turn-5',
        mode: 'history-window',
      },
      turns: presentationTurns,
    });
    stateMocks.activeSession = createSession({
      historyState: 'ready',
      isPartial: true,
      totalTurnCount: 10,
      turnCatalog: catalog,
      dialogTurns: [
        createTurn('turn-9', 'Recent prompt'),
        createTurn('turn-10', 'Latest prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = stateMocks.activeSession.dialogTurns.map(turn => ({
      type: 'user-message',
      turnId: turn.id,
      data: turn.userMessage,
    }));

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });
    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-turn-id="turn-5"]')?.click();
      await Promise.resolve();
    });

    expect(loadSpy).toHaveBeenCalledWith('session-1', 4, { source: 'target' });
    expect(virtualListPropsMock.latest).toMatchObject({
      presentationMode: 'history-window',
      viewportMode: 'history-reading',
    });
    expect((virtualListPropsMock.latest?.items as Array<{ turnId: string }>).map(item => item.turnId))
      .toEqual(presentationTurns.map(turn => turn.id));

    const restoreTailSpy = vi.spyOn(flowChatStore, 'restoreSessionTailPresentation');
    const initialItems = virtualListPropsMock.latest?.items;
    await act(async () => {
      (virtualListPropsMock.latest?.onRequestJumpToLatest as (() => void) | undefined)?.();
    });

    expect(restoreTailSpy).not.toHaveBeenCalled();
    expect(virtualListPropsMock.latest).toMatchObject({
      presentationMode: 'history-window',
      viewportMode: 'live-tail',
      historyWindow: null,
    });
    expect(virtualListPropsMock.latest?.items).toBe(initialItems);

    stateMocks.activeSession = {
      ...stateMocks.activeSession,
      totalTurnCount: 11,
      dialogTurns: [
        createTurn('turn-10', 'Latest live update', 'processing'),
        createTurn('turn-11', 'New live prompt', 'processing'),
      ],
    };
    stateMocks.virtualItems = stateMocks.activeSession.dialogTurns.map(turn => ({
      type: 'user-message',
      turnId: turn.id,
      data: turn.userMessage,
    }));
    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(virtualListPropsMock.latest).toMatchObject({
      presentationMode: 'history-window',
      viewportMode: 'live-tail',
    });
    expect((virtualListPropsMock.latest?.items as Array<{ turnId: string }>).map(item => item.turnId))
      .toEqual([...presentationTurns.map(turn => turn.id), 'turn-11']);
    const liveLatestItem = (virtualListPropsMock.latest?.items as Array<{
      turnId: string;
      data?: { content?: string };
    }>).find(item => item.turnId === 'turn-10');
    expect(liveLatestItem?.data?.content).toBe('Latest live update');

    restoreTailSpy.mockRestore();
    loadSpy.mockRestore();
    activateSpy.mockRestore();
  });

  it('retries turn-rail selection without advancing visible-turn state until the virtual list accepts it', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [
        createTurn('turn-1', 'Older prompt'),
        createTurn('turn-2', 'Latest prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest prompt' } },
    ];
    stateMocks.visibleTurnInfo = {
      turnId: 'turn-2',
      turnIndex: 2,
      totalTurns: 2,
      userMessage: 'Latest prompt',
    };
    virtualListMock.pinTurnToTopWithStatus.mockReturnValue('rejected');

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(headerPropsMock.latest).toMatchObject({
      currentTurn: 2,
      totalTurns: 2,
    });

    clickTurnRailItem(container, 'turn-1');
    expect(virtualListMock.pinTurnToTopWithStatus).toHaveBeenLastCalledWith('turn-1', {
      behavior: 'auto',
      pinMode: 'transient',
      alignmentPolicy: 'best-effort',
    });
    expect(headerPropsMock.latest).toMatchObject({
      currentTurn: 2,
      totalTurns: 2,
    });

    virtualListMock.pinTurnToTopWithStatus.mockReturnValue('settled');
    stateMocks.virtualItems = [
      ...stateMocks.virtualItems,
    ];

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });
    flushAnimationFrame();

    expect(virtualListMock.pinTurnToTopWithStatus).toHaveBeenLastCalledWith('turn-1', {
      behavior: 'auto',
      pinMode: 'transient',
      alignmentPolicy: 'best-effort',
    });
    expect(headerPropsMock.latest).toMatchObject({
      currentTurn: 2,
      totalTurns: 2,
    });

    stateMocks.visibleTurnInfo = {
      turnId: 'turn-1',
      turnIndex: 1,
      totalTurns: 2,
      userMessage: 'Older prompt',
    };

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(headerPropsMock.latest).toMatchObject({
      currentTurn: 1,
      totalTurns: 2,
    });
  });

  it('delegates accepted turn-rail selections to the list without container-level retry', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [
        createTurn('turn-1', 'Older prompt'),
        createTurn('turn-2', 'Latest prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest prompt' } },
    ];
    stateMocks.visibleTurnInfo = {
      turnId: 'turn-2',
      turnIndex: 2,
      totalTurns: 2,
      userMessage: 'Latest prompt',
    };
    virtualListMock.pinTurnToTopWithStatus.mockReturnValue('settled');

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    clickTurnRailItem(container, 'turn-1');
    expect(virtualListMock.pinTurnToTopWithStatus).toHaveBeenLastCalledWith('turn-1', {
      behavior: 'auto',
      pinMode: 'transient',
      alignmentPolicy: 'best-effort',
    });
    expect(headerPropsMock.latest).toMatchObject({
      currentTurn: 2,
      totalTurns: 2,
    });

    const acceptedCallCount = virtualListMock.pinTurnToTopWithStatus.mock.calls.length;
    flushAnimationFrame();
    expect(virtualListMock.pinTurnToTopWithStatus.mock.calls.length).toBe(acceptedCallCount);

    stateMocks.visibleTurnInfo = {
      turnId: 'turn-1',
      turnIndex: 1,
      totalTurns: 2,
      userMessage: 'Older prompt',
    };

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });
    flushAnimationFrame();
    expect(headerPropsMock.latest).toMatchObject({
      currentTurn: 1,
      totalTurns: 2,
    });
    const settledCallCount = virtualListMock.pinTurnToTopWithStatus.mock.calls.length;
    flushAnimationFrame();
    expect(virtualListMock.pinTurnToTopWithStatus.mock.calls.length).toBe(settledCallCount);
  });

  it('accepts list-owned pending turn pins without retrying from the container', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [
        createTurn('turn-1', 'Older prompt'),
        createTurn('turn-2', 'Latest prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest prompt' } },
    ];
    stateMocks.visibleTurnInfo = {
      turnId: 'turn-2',
      turnIndex: 2,
      totalTurns: 2,
      userMessage: 'Latest prompt',
    };
    virtualListMock.pinTurnToTopWithStatus.mockReturnValue('pending');

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    clickTurnRailItem(container, 'turn-1');
    expect(virtualListMock.pinTurnToTopWithStatus).toHaveBeenLastCalledWith('turn-1', {
      behavior: 'auto',
      pinMode: 'transient',
      alignmentPolicy: 'best-effort',
    });
    const pendingCallCount = virtualListMock.pinTurnToTopWithStatus.mock.calls.length;
    flushAnimationFrame();
    flushAnimationFrame();
    expect(virtualListMock.pinTurnToTopWithStatus.mock.calls.length).toBe(pendingCallCount);
  });

  it('does not render stale turn-rail targets', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [
        createTurn('turn-1', 'Older prompt'),
        createTurn('turn-2', 'Latest prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest prompt' } },
    ];
    stateMocks.visibleTurnInfo = {
      turnId: 'turn-2',
      turnIndex: 2,
      totalTurns: 2,
      userMessage: 'Latest prompt',
    };

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    const beforeSelectionCallCount = virtualListMock.pinTurnToTopWithStatus.mock.calls.length;
    expect(container.querySelector('[data-turn-id="turn-missing"]')).toBeNull();
    expect(virtualListMock.pinTurnToTopWithStatus.mock.calls.length).toBe(beforeSelectionCallCount);
  });

  it('keeps long-session turn-rail selections single-shot after the list accepts the pin', async () => {
    const turns = Array.from({ length: 25 }, (_, index) => {
      const turnNumber = index + 1;
      return createTurn(`turn-${turnNumber}`, `Prompt ${turnNumber}`);
    });
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: turns,
    } as Partial<Session>);
    stateMocks.virtualItems = turns.map(turn => ({
      type: 'user-message',
      turnId: turn.id,
      data: { id: `user-${turn.id}`, content: turn.userMessage.content },
    }));
    stateMocks.visibleTurnInfo = {
      turnId: 'turn-25',
      turnIndex: 25,
      totalTurns: 25,
      userMessage: 'Prompt 25',
    };
    virtualListMock.pinTurnToTopWithStatus.mockReturnValue('settled');

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(headerPropsMock.latest).toMatchObject({
      currentTurn: 25,
      totalTurns: 25,
    });
    expect(container.querySelector('[data-testid="flowchat-turn-rail"]')?.getAttribute(
      'data-total-turn-count',
    )).toBe('25');
    expect(container.querySelectorAll('.flowchat-turn-rail__item').length).toBeLessThan(25);

    scrollTurnRailToOrdinal(container, 6);

    const beforeSelectionCallCount = virtualListMock.pinTurnToTopWithStatus.mock.calls.length;
    clickTurnRailItem(container, 'turn-7');
    expect(virtualListMock.pinTurnToTopWithStatus.mock.calls.length).toBe(beforeSelectionCallCount + 1);
    expect(virtualListMock.pinTurnToTopWithStatus).toHaveBeenLastCalledWith('turn-7', {
      behavior: 'auto',
      pinMode: 'transient',
      alignmentPolicy: 'best-effort',
    });

    flushAnimationFrame();
    flushAnimationFrame();
    flushAnimationFrame();
    expect(virtualListMock.pinTurnToTopWithStatus.mock.calls.length).toBe(beforeSelectionCallCount + 1);
  });

  it('cancels a not-yet-accepted turn-navigation retry when the user scrolls manually', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [
        createTurn('turn-1', 'Older prompt'),
        createTurn('turn-2', 'Latest prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest prompt' } },
    ];
    stateMocks.visibleTurnInfo = {
      turnId: 'turn-2',
      turnIndex: 2,
      totalTurns: 2,
      userMessage: 'Latest prompt',
    };
    virtualListMock.pinTurnToTopWithStatus.mockReturnValue('rejected');

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    clickTurnRailItem(container, 'turn-1');
    expect(headerPropsMock.latest).toMatchObject({
      currentTurn: 2,
      totalTurns: 2,
    });

    flushAnimationFrame();
    const retryCallCount = virtualListMock.pinTurnToTopWithStatus.mock.calls.length;
    expect(retryCallCount).toBeGreaterThan(1);

    await act(async () => {
      (virtualListPropsMock.latest?.onUserScrollIntent as (() => void) | undefined)?.();
    });
    expect(headerPropsMock.latest).toMatchObject({
      currentTurn: 2,
      totalTurns: 2,
    });

    flushAnimationFrame();
    expect(virtualListMock.pinTurnToTopWithStatus.mock.calls.length).toBe(retryCallCount);
  });

  it('renders ordinal navigation placeholders for old hosts without a turn catalog', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      isPartial: true,
      loadedTurnCount: 2,
      totalTurnCount: 100,
      dialogTurns: [
        createTurn('turn-99', 'Recent restored prompt'),
        createTurn('turn-100', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-99', data: { id: 'user-turn-99', content: 'Recent restored prompt' } },
      { type: 'user-message', turnId: 'turn-100', data: { id: 'user-turn-100', content: 'Latest restored prompt' } },
    ];
    stateMocks.visibleTurnInfo = {
      turnId: 'turn-99',
      turnIndex: 1,
      totalTurns: 2,
      userMessage: 'Recent restored prompt',
    };

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(headerPropsMock.latest).toMatchObject({
      currentTurn: 99,
      totalTurns: 100,
    });
    expect(container.querySelector('[data-testid="flowchat-turn-rail"]')?.getAttribute(
      'data-total-turn-count',
    )).toBe('100');
    expect(container.querySelectorAll('.flowchat-turn-rail__item').length).toBeLessThan(100);
    expect(container.querySelector('[data-turn-key="storage:0"]')).toBeNull();
    expect(container.querySelector('[data-turn-id="turn-98"]')).toBeNull();
    expect(container.querySelector('[data-turn-id="turn-99"]')?.getAttribute('aria-disabled')).toBeNull();
    expect(container.querySelector('[data-turn-id="turn-100"]')?.getAttribute('aria-disabled')).toBeNull();

    scrollTurnRailToOrdinal(container, 0);

    expect(container.querySelector('[data-turn-key="storage:0"]')?.getAttribute('aria-disabled')).toBeNull();
    expect(container.querySelector('[data-turn-id="turn-99"]')).toBeNull();
    expect(virtualListMock.pinTurnToTopWithStatus).not.toHaveBeenCalled();
  });

  it('windows catalog markers while resolving loaded tail identities', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      isPartial: true,
      loadedTurnCount: 2,
      totalTurnCount: 100,
      turnCatalog: {
        schemaVersion: 1,
        sessionId: 'session-1',
        revision: 'catalog-1',
        totalTurnCount: 100,
        complete: false,
        entries: Array.from({ length: 100 }, (_, ordinal) => ({
          ordinal,
          storageTurnIndex: ordinal,
          ...(ordinal === 98
            ? { turnId: 'turn-99', preview: 'Stale catalog preview' }
            : ordinal === 99
              ? { turnId: 'turn-100', preview: 'Latest catalog preview' }
              : {}),
          previewTruncated: false,
        })),
      },
      dialogTurns: [
        createTurn('turn-99', 'Recent restored prompt'),
        createTurn('turn-100', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-99', data: { id: 'user-turn-99', content: 'Recent restored prompt' } },
      { type: 'user-message', turnId: 'turn-100', data: { id: 'user-turn-100', content: 'Latest restored prompt' } },
    ];
    stateMocks.visibleTurnInfo = {
      turnId: 'turn-100',
      turnIndex: 2,
      totalTurns: 2,
      userMessage: 'Latest restored prompt',
      visibleTurnIds: ['turn-99', 'turn-100'],
    };

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(container.querySelector('[data-testid="flowchat-turn-rail"]')?.getAttribute(
      'data-total-turn-count',
    )).toBe('100');
    expect(container.querySelectorAll('.flowchat-turn-rail__item').length).toBeLessThan(100);
    expect(container.querySelector('[data-turn-key="storage:0"]')).toBeNull();
    expect(container.querySelector('[data-turn-id="turn-99"]')?.getAttribute('aria-disabled')).toBeNull();
    expect(container.querySelector('[data-turn-id="turn-100"]')?.getAttribute('aria-disabled')).toBeNull();

    scrollTurnRailToOrdinal(container, 0);

    expect(container.querySelector('[data-turn-key="storage:0"]')?.getAttribute('aria-disabled')).toBeNull();
    expect(container.querySelector('[data-turn-id="turn-100"]')).toBeNull();
  });

  it('requests the unified full-history fallback for an unloaded catalog target', async () => {
    const ensureSpy = vi
      .spyOn(flowChatStore, 'ensureSessionFullHistory')
      .mockResolvedValue(true);
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      isPartial: true,
      loadedTurnCount: 2,
      totalTurnCount: 100,
      turnCatalog: {
        schemaVersion: 1,
        sessionId: 'session-1',
        revision: 'complete-catalog',
        totalTurnCount: 100,
        complete: true,
        entries: Array.from({ length: 100 }, (_, ordinal) => ({
          ordinal,
          storageTurnIndex: ordinal,
          turnId: `turn-${ordinal + 1}`,
          preview: `Prompt ${ordinal + 1}`,
          previewTruncated: false,
        })),
      },
      dialogTurns: [
        createTurn('turn-99', 'Recent restored prompt'),
        createTurn('turn-100', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-99', data: { id: 'user-turn-99', content: 'Recent restored prompt' } },
      { type: 'user-message', turnId: 'turn-100', data: { id: 'user-turn-100', content: 'Latest restored prompt' } },
    ];

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-turn-id="turn-1"]')?.click();
      await Promise.resolve();
    });

    expect(ensureSpy).toHaveBeenCalledWith('session-1', 'turn-rail-navigation');
    expect(virtualListMock.pinTurnToTopWithStatus).not.toHaveBeenCalledWith(
      'turn-1',
      expect.anything(),
    );
  });

  it('materializes a loaded Turn window for cross-feature focus before reusing the shared pin transaction', async () => {
    const targetTurn = createTurn('turn-5', 'Target prompt');
    const loadSpy = vi.spyOn(flowChatStore, 'loadSessionTurnWindow').mockResolvedValue({
      status: 'ready',
      sessionId: 'session-1',
      targetOrdinal: 4,
      targetTurnId: 'turn-5',
      navigationGeneration: 7,
      isCurrent: true,
      cacheHit: false,
      range: {
        startOrdinal: 2,
        endOrdinalExclusive: 7,
        turns: Array.from({ length: 5 }, (_, index) => createTurn(`turn-${index + 3}`, `Prompt ${index + 3}`)),
        lastAccessedAt: 1,
        source: 'target',
      },
    });
    const activateSpy = vi.spyOn(flowChatStore, 'activateSessionHistoryWindow').mockReturnValue({
      range: {
        startOrdinal: 2,
        endOrdinalExclusive: 7,
        targetTurnId: targetTurn.id,
        mode: 'history-window',
      },
      turns: Array.from({ length: 5 }, (_, index) => createTurn(`turn-${index + 3}`, `Prompt ${index + 3}`)),
    });
    stateMocks.activeSession = createSession({
      historyState: 'ready',
      isPartial: true,
      totalTurnCount: 10,
      turnCatalog: {
        schemaVersion: 1,
        sessionId: 'session-1',
        revision: 'catalog-v1',
        totalTurnCount: 10,
        complete: true,
        entries: Array.from({ length: 10 }, (_, ordinal) => ({
          ordinal,
          storageTurnIndex: ordinal,
          turnId: `turn-${ordinal + 1}`,
          preview: `Prompt ${ordinal + 1}`,
          previewTruncated: false,
        })),
      },
      dialogTurns: [
        createTurn('turn-9', 'Recent prompt'),
        createTurn('turn-10', 'Latest prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = stateMocks.activeSession.dialogTurns.map(turn => ({
      type: 'user-message',
      turnId: turn.id,
      data: turn.userMessage,
    }));

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });
    const target = container.querySelector<HTMLButtonElement>('[data-turn-id="turn-5"]');
    expect(target).not.toBeNull();
    await act(async () => {
      const onNavigateToFocusTurn = navigationOptionsMock.latest?.onNavigateToFocusTurn as (
        request: {
          sessionId: string;
          turnIndex: number;
          source: 'usage-report';
        },
      ) => Promise<boolean>;
      await expect(onNavigateToFocusTurn({
        sessionId: 'session-1',
        turnIndex: 5,
        source: 'usage-report',
      })).resolves.toBe(true);
    });

    expect(loadSpy).toHaveBeenCalledWith('session-1', 4, { source: 'target' });
    expect(virtualListMock.prepareTurnPinToTop).toHaveBeenCalledWith('turn-5', {
      behavior: 'auto',
      pinMode: 'transient',
      alignmentPolicy: 'best-effort',
    });
    expect(activateSpy).toHaveBeenCalledWith('session-1', 4, 7);
    expect(virtualListPropsMock.latest).toMatchObject({
      presentationMode: 'history-window',
      presentationRevision: 1,
    });
    expect((virtualListPropsMock.latest?.items as Array<{ turnId: string }>).map(item => item.turnId)).toEqual([
      'turn-3',
      'turn-4',
      'turn-5',
      'turn-6',
      'turn-7',
    ]);
    expect(stateMocks.activeSession.dialogTurns.map(turn => turn.id)).toEqual(['turn-9', 'turn-10']);
    expect(virtualListMock.prepareTurnPinToTop.mock.invocationCallOrder[0]).toBeLessThan(
      activateSpy.mock.invocationCallOrder[0],
    );

    const latestTailPinCallCount = virtualListMock.pinTurnToTop.mock.calls.length;
    const latestTailEndCallCount = virtualListMock.scrollToTurnEndAndClearPin.mock.calls.length;
    stateMocks.activeSession = {
      ...stateMocks.activeSession,
      totalTurnCount: 11,
      dialogTurns: [
        ...stateMocks.activeSession.dialogTurns,
        createTurn('turn-11', 'Streaming prompt', 'processing'),
      ],
    };
    stateMocks.virtualItems = stateMocks.activeSession.dialogTurns.map(turn => ({
      type: 'user-message',
      turnId: turn.id,
      data: turn.userMessage,
    }));
    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });
    flushAnimationFrame();
    expect(virtualListPropsMock.latest).toMatchObject({ presentationMode: 'history-window' });
    expect(virtualListMock.pinTurnToTop.mock.calls.length).toBe(latestTailPinCallCount);
    expect(virtualListMock.scrollToTurnEndAndClearPin.mock.calls.length).toBe(latestTailEndCallCount);

    const restoreTailSpy = vi.spyOn(flowChatStore, 'restoreSessionTailPresentation');
    const latestEndCallCountBeforeSend = virtualListMock.scrollToLatestEndPosition.mock.calls.length;
    await act(async () => {
      const onBeforeTurnPinRequest = navigationOptionsMock.latest?.onBeforeTurnPinRequest as (
        request: {
          sessionId: string;
          turnId: string;
          source: 'send-message';
          behavior: 'auto';
          pinMode: 'sticky-latest';
        },
      ) => void;
      onBeforeTurnPinRequest({
        sessionId: 'session-1',
        turnId: 'turn-11',
        source: 'send-message',
        behavior: 'auto',
        pinMode: 'sticky-latest',
      });
    });
    expect(restoreTailSpy).toHaveBeenCalledTimes(1);
    expect(restoreTailSpy).toHaveBeenLastCalledWith('session-1');
    expect(virtualListPropsMock.latest).toMatchObject({ presentationMode: 'tail' });
    expect((virtualListPropsMock.latest?.items as Array<{ turnId: string }>).map(item => item.turnId)).toEqual([
      'turn-9',
      'turn-10',
      'turn-11',
    ]);
    expect(virtualListMock.scrollToLatestEndPosition.mock.calls.length).toBe(latestEndCallCountBeforeSend);

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-turn-id="turn-5"]')?.click();
      await Promise.resolve();
    });
    expect(virtualListPropsMock.latest).toMatchObject({ presentationMode: 'history-window' });

    await act(async () => {
      (virtualListPropsMock.latest?.onRequestJumpToLatest as (() => void) | undefined)?.();
    });
    flushAnimationFrame();
    expect(restoreTailSpy).toHaveBeenCalledTimes(2);
    expect(restoreTailSpy).toHaveBeenLastCalledWith('session-1');
    expect(virtualListPropsMock.latest).toMatchObject({ presentationMode: 'tail' });
    expect(virtualListMock.scrollToLatestEndPosition.mock.calls.length).toBe(latestEndCallCountBeforeSend + 1);

    restoreTailSpy.mockRestore();
    loadSpy.mockRestore();
    activateSpy.mockRestore();
  });

  it('materializes an adjacent catalog window when tail history requests older turns', async () => {
    const catalog = {
      schemaVersion: 1,
      sessionId: 'session-1',
      revision: 'catalog-1',
      totalTurnCount: 10,
      complete: true,
      entries: Array.from({ length: 10 }, (_, ordinal) => ({
        ordinal,
        storageTurnIndex: ordinal,
        turnId: `turn-${ordinal + 1}`,
        preview: `Prompt ${ordinal + 1}`,
        previewTruncated: false,
      })),
    };
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      isPartial: true,
      loadedTurnCount: 2,
      totalTurnCount: 10,
      turnCatalog: catalog,
      dialogTurns: [
        createTurn('turn-9', 'Recent prompt'),
        createTurn('turn-10', 'Latest prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = stateMocks.activeSession.dialogTurns.map(turn => ({
      type: 'user-message',
      turnId: turn.id,
      data: turn.userMessage,
    }));
    const presentationTurns = Array.from(
      { length: 8 },
      (_, index) => createTurn(`turn-${index + 3}`, `Prompt ${index + 3}`),
    );
    const cachedTurns = Array.from(
      { length: 10 },
      (_, index) => createTurn(`turn-${index + 1}`, `Prompt ${index + 1}`),
    );
    vi.spyOn(flowChatStore, 'getState').mockReturnValue({
      sessions: new Map([['session-1', stateMocks.activeSession]]),
      activeSessionId: 'session-1',
    });
    vi.spyOn(flowChatStore, 'getSessionHistoryViewState').mockReturnValue({
      catalog,
      loadedRanges: [{
        startOrdinal: 0,
        endOrdinalExclusive: 10,
        turns: cachedTurns,
        lastAccessedAt: 1,
        source: 'prefetch',
      }],
      activeRange: null,
      pendingTargetOrdinal: null,
      navigationGeneration: 0,
    });
    vi.spyOn(flowChatStore, 'getSessionCanonicalTailRange').mockReturnValue({
      startOrdinal: 8,
      endOrdinalExclusive: 10,
    });
    const loadSpy = vi.spyOn(flowChatStore, 'loadSessionTurnWindow').mockResolvedValue({
      status: 'ready',
      sessionId: 'session-1',
      targetOrdinal: 7,
      targetTurnId: 'turn-8',
      navigationGeneration: 0,
      isCurrent: true,
      cacheHit: true,
      catalog,
    });
    const activateSpy = vi.spyOn(
      flowChatStore,
      'activateSessionHistoryWindowFromTail',
    ).mockReturnValue({
      range: {
        startOrdinal: 2,
        endOrdinalExclusive: 10,
        targetTurnId: null,
        mode: 'history-window',
      },
      turns: presentationTurns,
    });
    let resolveViewportPreparation: ((ready: boolean) => void) | undefined;
    const viewportPreparation = new Promise<boolean>(resolve => {
      resolveViewportPreparation = resolve;
    });
    const prepareViewportForPresentationCommit = vi.fn(() => viewportPreparation);
    const cancelViewportPresentationCommit = vi.fn();

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });
    let boundaryIntent: Promise<HistoryWindowBoundaryIntentResult> | undefined;
    await act(async () => {
      boundaryIntent = (
        virtualListPropsMock.latest?.onHistoryWindowBoundaryIntent as
          | ((
            direction: 'before' | 'after',
            options?: {
              prepareViewportForPresentationCommit?: () => (
                boolean | void | Promise<boolean | void>
              );
              cancelViewportPresentationCommit?: () => void;
            },
          ) => Promise<HistoryWindowBoundaryIntentResult>)
          | undefined
      )?.('before', {
        prepareViewportForPresentationCommit,
        cancelViewportPresentationCommit,
      });
      await Promise.resolve();
    });

    expect(prepareViewportForPresentationCommit).toHaveBeenCalledOnce();
    expect(cancelViewportPresentationCommit).not.toHaveBeenCalled();
    expect(activateSpy).not.toHaveBeenCalled();
    expect(virtualListPropsMock.latest).toMatchObject({ presentationMode: 'tail' });

    await act(async () => {
      resolveViewportPreparation?.(true);
      expect(await boundaryIntent).toBe('applied');
    });

    expect(loadSpy).toHaveBeenCalledWith('session-1', 7, {
      source: 'prefetch',
      before: 12,
      after: 1,
    });
    expect(activateSpy).toHaveBeenCalledWith('session-1', 7);
    expect(virtualListPropsMock.latest).toMatchObject({
      presentationMode: 'history-window',
      presentationRevision: 1,
    });
  });

  it('lets streaming restored sessions use follow-output instead of container sticky anchoring', async () => {
    stateMocks.activeSession = createSession({
      historyState: 'ready',
      dialogTurns: [
        createTurn('turn-1', 'Older restored prompt'),
        createTurn('turn-2', 'Latest restored prompt', 'processing'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older restored prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest restored prompt' } },
    ];

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(container.querySelector('[data-testid="virtual-list"]')).not.toBeNull();
    expect(virtualListMock.pinTurnToTop).not.toHaveBeenCalled();
    expect(startupTraceMock.markPhase).toHaveBeenCalledWith(
      'historical_session_latest_anchor_skipped',
      expect.objectContaining({ reason: 'streaming_follow_output', mode: 'follow-output' }),
    );
  });

  it('scrolls completed restored history to the tail after hydration clears isHistorical', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [
        createTurn('turn-1', 'Older restored prompt'),
        createTurn('turn-2', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older restored prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest restored prompt' } },
    ];

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    flushAnimationFrame();

    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenCalledTimes(1);
    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenCalledWith('turn-2');
    expect(virtualListMock.pinTurnToTop).not.toHaveBeenCalled();
    expect(startupTraceMock.markPhase).toHaveBeenCalledWith(
      'historical_session_latest_anchor_attempt',
      expect.objectContaining({ accepted: true, attempt: 1, mode: 'bottom' }),
    );
  });

  it('retries completed history tail anchoring when the virtual list is not ready on the first frame', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [
        createTurn('turn-1', 'Older restored prompt'),
        createTurn('turn-2', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older restored prompt' } },
      { type: 'user-message', turnId: 'turn-2', data: { id: 'user-turn-2', content: 'Latest restored prompt' } },
    ];
    virtualListMock.scrollToTurnEndAndClearPin
      .mockReturnValueOnce(false)
      .mockReturnValueOnce(true);

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    flushAnimationFrame();
    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenCalledTimes(1);
    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenLastCalledWith('turn-2');

    flushAnimationFrame();
    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenCalledTimes(2);
    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenLastCalledWith('turn-2');
    expect(startupTraceMock.markPhase).toHaveBeenCalledWith(
      'historical_session_latest_anchor_attempt',
      expect.objectContaining({ accepted: false, attempt: 1, mode: 'bottom' }),
    );
    expect(startupTraceMock.markPhase).toHaveBeenCalledWith(
      'historical_session_latest_anchor_attempt',
      expect.objectContaining({ accepted: true, attempt: 2, mode: 'bottom' }),
    );
  });

  it('does not re-anchor local restored history after full hydration expands the same latest turn', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [
        createTurn('turn-80', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-80', data: { id: 'user-turn-80', content: 'Latest restored prompt' } },
    ];

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    flushAnimationFrame();

    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenCalledTimes(1);
    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenLastCalledWith('turn-80');

    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [
        createTurn('turn-1', 'Older restored prompt'),
        createTurn('turn-80', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older restored prompt' } },
      { type: 'user-message', turnId: 'turn-80', data: { id: 'user-turn-80', content: 'Latest restored prompt' } },
    ];

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    flushAnimationFrame();

    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenCalledTimes(1);
    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenLastCalledWith('turn-80');
    expect(virtualListMock.pinTurnToTop).not.toHaveBeenCalled();
    expect(startupTraceMock.markPhase).toHaveBeenCalledWith(
      'historical_session_latest_anchor_skipped',
      expect.objectContaining({ reason: 'local_full_history_projection' }),
    );
  });

  it('does not re-anchor local full hydration when the latest restored turn is already visible', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [
        createTurn('turn-80', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-80', data: { id: 'user-turn-80', content: 'Latest restored prompt' } },
    ];

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    flushAnimationFrame();

    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenCalledTimes(1);
    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenLastCalledWith('turn-80');

    stateMocks.visibleTurnInfo = {
      turnId: 'turn-80',
      turnIndex: 1,
      totalTurns: 1,
      userMessage: 'Latest restored prompt',
    };
    virtualListMock.isTurnRenderedInViewport.mockReturnValue(true);
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [
        createTurn('turn-1', 'Older restored prompt'),
        createTurn('turn-80', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older restored prompt' } },
      { type: 'user-message', turnId: 'turn-80', data: { id: 'user-turn-80', content: 'Latest restored prompt' } },
    ];

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenCalledTimes(1);
    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenLastCalledWith('turn-80');

    flushAnimationFrame();

    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenCalledTimes(1);
    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenLastCalledWith('turn-80');
    expect(virtualListMock.pinTurnToTop).not.toHaveBeenCalled();
  });

  it('does not repeat immediate latest anchoring after visible turn info catches up', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: true,
      historyState: 'ready',
      dialogTurns: [
        createTurn('turn-80', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-80', data: { id: 'user-turn-80', content: 'Latest restored prompt' } },
    ];

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenCalledTimes(1);
    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenLastCalledWith('turn-80');

    stateMocks.visibleTurnInfo = {
      turnId: 'turn-80',
      turnIndex: 1,
      totalTurns: 1,
      userMessage: 'Latest restored prompt',
    };

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenCalledTimes(1);
    expect(virtualListMock.pinTurnToTop).not.toHaveBeenCalled();
  });

  it('does not re-anchor local full hydration when visible turn info is stale after prepending older turns', async () => {
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [
        createTurn('turn-80', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-80', data: { id: 'user-turn-80', content: 'Latest restored prompt' } },
    ];

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    flushAnimationFrame();

    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenCalledTimes(1);
    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenLastCalledWith('turn-80');

    stateMocks.visibleTurnInfo = {
      turnId: 'turn-80',
      turnIndex: 1,
      totalTurns: 1,
      userMessage: 'Latest restored prompt',
    };
    virtualListMock.isTurnRenderedInViewport.mockReturnValue(false);
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [
        createTurn('turn-1', 'Older restored prompt'),
        createTurn('turn-44', 'Middle restored prompt'),
        createTurn('turn-80', 'Latest restored prompt'),
      ],
    } as Partial<Session>);
    stateMocks.virtualItems = [
      { type: 'user-message', turnId: 'turn-1', data: { id: 'user-turn-1', content: 'Older restored prompt' } },
      { type: 'user-message', turnId: 'turn-44', data: { id: 'user-turn-44', content: 'Middle restored prompt' } },
      { type: 'user-message', turnId: 'turn-80', data: { id: 'user-turn-80', content: 'Latest restored prompt' } },
    ];

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    flushAnimationFrame();

    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenCalledTimes(1);
    expect(virtualListMock.scrollToTurnEndAndClearPin).toHaveBeenLastCalledWith('turn-80');
    expect(virtualListMock.pinTurnToTop).not.toHaveBeenCalled();
  });
});
