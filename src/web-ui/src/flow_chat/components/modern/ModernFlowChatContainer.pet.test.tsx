// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { ModernFlowChatContainer } from './ModernFlowChatContainer';
import {
  aiExperienceConfigService,
  type AIExperienceSettings,
} from '@/infrastructure/config/services/AIExperienceConfigService';
import type { Session } from '../../types/flow-chat';
import { flowChatStore } from '../../store/FlowChatStore';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const stateMocks = vi.hoisted(() => ({
  activeSession: null as Session | null,
  virtualItems: [] as unknown[],
  visibleTurnInfo: null as unknown,
}));

const virtualListMock = vi.hoisted(() => ({
  scrollToTurn: vi.fn(),
  scrollToIndex: vi.fn(),
  scrollToPhysicalBottomAndClearPin: vi.fn(),
  scrollToTurnEndAndClearPin: vi.fn(() => true),
  scrollToLatestEndPosition: vi.fn(),
  isTurnRenderedInViewport: vi.fn(() => false),
  isTurnTextRenderedInViewport: vi.fn(() => false),
  pinTurnToTop: vi.fn(() => true),
}));

const startupTraceMock = vi.hoisted(() => ({
  markPhase: vi.fn(),
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

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: () => undefined },
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/infrastructure/hooks/useShortcut', () => ({ useShortcut: vi.fn() }));
vi.mock('@/flow_chat/services/FlowChatManager', () => ({
  FlowChatManager: {
    getInstance: () => ({
      cancelCurrentTask: vi.fn(),
      createChatSession: vi.fn(),
      switchChatSession: vi.fn(),
    }),
  },
}));
vi.mock('@/app/stores/sessionModeStore', () => ({
  useSessionModeStore: { getState: () => ({ setMode: vi.fn() }) },
}));
vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useWorkspaceContext: () => ({ workspacePath: 'D:/workspace/BitFun' }),
}));
vi.mock('../../utils/acpSession', () => ({ isAcpFlowSession: () => false }));
vi.mock('../../store/modernFlowChatStore', () => ({
  useVirtualItems: () => stateMocks.virtualItems,
  useActiveSession: () => stateMocks.activeSession,
  useVisibleTurnInfo: () => stateMocks.visibleTurnInfo,
}));
vi.mock('./VirtualMessageList', () => ({
  VirtualMessageList: React.forwardRef((_, ref) => {
    React.useImperativeHandle(ref, () => virtualListMock);
    return <div data-testid="virtual-list" />;
  }),
}));
vi.mock('@/shared/utils/startupTrace', () => ({ startupTrace: startupTraceMock }));
vi.mock('./FlowChatHeader', () => ({
  FlowChatHeader: (props: Record<string, unknown>) => {
    headerPropsMock.latest = props;
    return <div data-testid="flowchat-header" />;
  },
}));
vi.mock('../WelcomePanel', () => ({
  WelcomePanel: () => <div data-testid="welcome-panel">Welcome</div>,
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
  useFlowChatFileActions: () => ({ handleFileViewRequest: vi.fn() }),
}));
vi.mock('./useFlowChatNavigation', () => ({ useFlowChatNavigation: vi.fn() }));
vi.mock('./useFlowChatCopyDialog', () => ({ useFlowChatCopyDialog: vi.fn() }));
vi.mock('./useFlowChatSync', () => ({ useFlowChatSync: vi.fn() }));
vi.mock('./useFlowChatToolActions', () => ({
  useFlowChatToolActions: () => ({
    handleToolConfirm: vi.fn(),
    handleToolReject: vi.fn(),
  }),
}));
vi.mock('./useFlowChatSearch', () => ({
  useFlowChatSearch: () => searchStateMock,
}));

vi.mock('../ChatInputPixelPet', () => ({
  ChatInputPixelPet: (props: { mood: string; pet: unknown; className?: string }) => (
    <div
      data-testid="chat-input-pet"
      data-mood={props.mood}
      className={props.className}
    />
  ),
}));

const pet = {
  id: 'preset:panda',
  displayName: 'Panda',
  source: 'preset' as const,
  packagePath: '/pet',
  spritesheetPath: '/sprite.png',
  spritesheetMimeType: 'image/png',
};

function setServiceSettings(overrides: Partial<AIExperienceSettings>): void {
  const current = aiExperienceConfigService.getSettings();
  const next: AIExperienceSettings = { ...current, ...overrides };
  vi.spyOn(aiExperienceConfigService, 'getSettings').mockReturnValue(next);
  vi.spyOn(aiExperienceConfigService, 'getSettingsAsync').mockResolvedValue(next);
}

function createSession(overrides: Partial<Session> = {}): Session {
  return {
    sessionId: 'session-1',
    title: 'Test session',
    dialogTurns: [],
    status: 'idle',
    config: { agentType: 'agentic' },
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    isHistorical: false,
    todos: [],
    mode: 'agentic',
    workspacePath: 'D:/workspace/BitFun',
    sessionKind: 'normal',
    ...overrides,
  };
}

describe('ModernFlowChatContainer agent companion pet', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    stateMocks.activeSession = null;
    stateMocks.virtualItems = [];
    stateMocks.visibleTurnInfo = null;
    setServiceSettings({
      enable_agent_companion: true,
      agent_companion_display_mode: 'input',
      agent_companion_pet: pet,
    });
  });

  afterEach(() => {
    if (root) {
      act(() => { root.unmount(); });
    }
    container?.remove();
    vi.restoreAllMocks();
  });

  it('renders the pet at the bottom-left of the canvas when input mode is active', () => {
    stateMocks.activeSession = createSession();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    act(() => { root.render(<ModernFlowChatContainer />); });

    const petElement = container.querySelector('[data-testid="chat-input-pet"]');
    expect(petElement).not.toBeNull();
    const wrapper = petElement?.parentElement;
    expect(wrapper?.className).toContain('modern-flowchat-container__companion-pet');
  });

  it('does not render the pet when display mode is desktop', () => {
    setServiceSettings({ agent_companion_display_mode: 'desktop' });
    stateMocks.activeSession = createSession();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    act(() => { root.render(<ModernFlowChatContainer />); });

    expect(container.querySelector('[data-testid="chat-input-pet"]')).toBeNull();
  });

  it('does not render the pet when the agent companion toggle is off', () => {
    setServiceSettings({ enable_agent_companion: false });
    stateMocks.activeSession = createSession();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    act(() => { root.render(<ModernFlowChatContainer />); });

    expect(container.querySelector('[data-testid="chat-input-pet"]')).toBeNull();
  });
});
