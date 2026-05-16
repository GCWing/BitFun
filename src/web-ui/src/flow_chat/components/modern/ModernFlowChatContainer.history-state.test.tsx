// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { ModernFlowChatContainer } from './ModernFlowChatContainer';
import type { Session } from '../../types/flow-chat';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const stateMocks = vi.hoisted(() => ({
  activeSession: null as Session | null,
  virtualItems: [] as unknown[],
  visibleTurnInfo: null as unknown,
}));

const switchChatSessionMock = vi.hoisted(() => vi.fn());

vi.mock('react-i18next', () => ({
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

vi.mock('../../utils/acpSession', () => ({
  isAcpFlowSession: () => false,
}));

vi.mock('../../store/modernFlowChatStore', () => ({
  useVirtualItems: () => stateMocks.virtualItems,
  useActiveSession: () => stateMocks.activeSession,
  useVisibleTurnInfo: () => stateMocks.visibleTurnInfo,
}));

vi.mock('./VirtualMessageList', () => ({
  VirtualMessageList: React.forwardRef(() => <div data-testid="virtual-list" />),
}));

vi.mock('./FlowChatHeader', () => ({
  FlowChatHeader: () => <div data-testid="flowchat-header" />,
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
  useFlowChatSearch: () => ({
    searchQuery: '',
    onSearchChange: vi.fn(),
    matches: [],
    matchIndices: [],
    currentMatchIndex: -1,
    currentMatchVirtualIndex: -1,
    goToNext: vi.fn(),
    goToPrev: vi.fn(),
    clearSearch: vi.fn(),
  }),
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

describe('ModernFlowChatContainer historical empty state', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    stateMocks.virtualItems = [];
    stateMocks.visibleTurnInfo = null;
    switchChatSessionMock.mockReset();
  });

  afterEach(() => {
    if (root) {
      act(() => {
        root.unmount();
      });
    }
    container?.remove();
    stateMocks.activeSession = null;
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
});
