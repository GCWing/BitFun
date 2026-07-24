// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { ModernFlowChatContainer } from './ModernFlowChatContainer';
import type { Session } from '../../types/flow-chat';
import { flowChatStore } from '../../store/FlowChatStore';
import {
  clearHistorySessionOpenTransition,
  dispatchHistorySessionOpenIntent,
  HISTORY_SESSION_OPEN_INTENT_EVENT,
} from '../../services/sessionOpenIntent';

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
  useFlowChatNavigation: vi.fn(),
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

describe('ModernFlowChatContainer historical empty state', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
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

  it('keeps the loading shell while historical sessions are hydrating', () => {
    stateMocks.activeSession = createSession({ historyState: 'hydrating' } as Partial<Session>);

    act(() => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(container.textContent).toContain('Loading saved session');
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

  it('keeps partial-session search scoped to the loaded window', async () => {
    const pendingSpy = vi
      .spyOn(flowChatStore, 'hasPendingSessionHistoryCompletion')
      .mockReturnValue(true);
    const projectionSpy = vi
      .spyOn(flowChatStore, 'requestSessionFullHistoryProjection')
      .mockReturnValue(true);

    searchStateMock.searchQuery = 'older prompt';
    stateMocks.activeSession = createSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'ready',
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

    expect(projectionSpy).not.toHaveBeenCalled();
    expect(startupTraceMock.markPhase).not.toHaveBeenCalledWith(
      'historical_session_full_hydrate_released_for_search',
      expect.anything(),
    );

    projectionSpy.mockRestore();
    pendingSpy.mockRestore();
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
    };

    await act(async () => {
      root.render(<ModernFlowChatContainer />);
    });

    expect(headerPropsMock.latest).toMatchObject({
      currentTurn: 100,
      totalTurns: 100,
      canJumpToPreviousTurn: true,
      canJumpToNextTurn: false,
    });
    expect(headerPropsMock.latest?.turns).toMatchObject([
      { turnId: 'turn-99', turnIndex: 99 },
      { turnId: 'turn-100', turnIndex: 100 },
    ]);

    act(() => {
      (headerPropsMock.latest?.onJumpToPreviousTurn as (() => void) | undefined)?.();
    });

    expect(virtualListMock.pinTurnToTopWithStatus).toHaveBeenLastCalledWith('turn-99', {
      behavior: 'smooth',
      pinMode: 'transient',
    });
  });

  it('retries header turn selection without advancing header state until the virtual list accepts it', async () => {
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

    let initialAccepted = true;
    await act(async () => {
      initialAccepted = (headerPropsMock.latest?.onJumpToTurn as ((turnId: string) => boolean) | undefined)?.('turn-1') ?? true;
    });

    expect(initialAccepted).toBe(false);
    expect(virtualListMock.pinTurnToTopWithStatus).toHaveBeenLastCalledWith('turn-1', {
      behavior: 'smooth',
      pinMode: 'transient',
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

  it('delegates accepted header turn selections to the list without container-level retry', async () => {
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

    let accepted = false;
    await act(async () => {
      accepted = (headerPropsMock.latest?.onJumpToTurn as ((turnId: string) => boolean) | undefined)?.('turn-1') ?? false;
    });

    expect(accepted).toBe(true);
    expect(virtualListMock.pinTurnToTopWithStatus).toHaveBeenLastCalledWith('turn-1', {
      behavior: 'smooth',
      pinMode: 'transient',
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

    let accepted = false;
    await act(async () => {
      accepted = (headerPropsMock.latest?.onJumpToTurn as ((turnId: string) => boolean) | undefined)?.('turn-1') ?? false;
    });

    expect(accepted).toBe(true);
    expect(virtualListMock.pinTurnToTopWithStatus).toHaveBeenLastCalledWith('turn-1', {
      behavior: 'smooth',
      pinMode: 'transient',
    });
    const pendingCallCount = virtualListMock.pinTurnToTopWithStatus.mock.calls.length;
    flushAnimationFrame();
    flushAnimationFrame();
    expect(virtualListMock.pinTurnToTopWithStatus.mock.calls.length).toBe(pendingCallCount);
  });

  it('rejects stale header turn selections without issuing a pin request', async () => {
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
    let accepted = true;
    await act(async () => {
      accepted = (headerPropsMock.latest?.onJumpToTurn as ((turnId: string) => boolean) | undefined)?.('turn-missing') ?? true;
    });

    expect(accepted).toBe(false);
    expect(virtualListMock.pinTurnToTopWithStatus.mock.calls.length).toBe(beforeSelectionCallCount);
  });

  it('keeps long-session header turn selections single-shot after the list accepts the pin', async () => {
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
    expect(headerPropsMock.latest?.turns).toHaveLength(25);

    const beforeSelectionCallCount = virtualListMock.pinTurnToTopWithStatus.mock.calls.length;
    let accepted = false;
    await act(async () => {
      accepted = (headerPropsMock.latest?.onJumpToTurn as ((turnId: string) => boolean) | undefined)?.('turn-7') ?? false;
    });

    expect(accepted).toBe(true);
    expect(virtualListMock.pinTurnToTopWithStatus.mock.calls.length).toBe(beforeSelectionCallCount + 1);
    expect(virtualListMock.pinTurnToTopWithStatus).toHaveBeenLastCalledWith('turn-7', {
      behavior: 'smooth',
      pinMode: 'transient',
    });

    flushAnimationFrame();
    flushAnimationFrame();
    flushAnimationFrame();
    expect(virtualListMock.pinTurnToTopWithStatus.mock.calls.length).toBe(beforeSelectionCallCount + 1);
  });

  it('cancels not-yet-accepted header turn retry when the user scrolls manually', async () => {
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

    let accepted = true;
    await act(async () => {
      accepted = (headerPropsMock.latest?.onJumpToTurn as ((turnId: string) => boolean) | undefined)?.('turn-1') ?? true;
    });

    expect(accepted).toBe(false);
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

  it('does not expose previous navigation before the loaded tail range in partial history', async () => {
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
      canJumpToPreviousTurn: false,
      canJumpToNextTurn: true,
    });

    act(() => {
      (headerPropsMock.latest?.onJumpToPreviousTurn as (() => void) | undefined)?.();
    });

    expect(virtualListMock.pinTurnToTop).not.toHaveBeenCalled();
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
