// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@/infrastructure/api/service-api/ToolAPI', () => ({
  toolAPI: { executeTool: vi.fn() },
}));

vi.mock('@/infrastructure/i18n/hooks/useI18n', async () => {
  const { createTestI18nT } = await import('@/test/i18nTestUtils');
  return { useI18n: () => ({ t: createTestI18nT('common') }) };
});

// SessionScene renders GroupLogView for group sessions and ChatPane otherwise
// (R-WF-14 routing). Stub the two leaves so the routing decision is observable.
vi.mock('./GroupLogView', () => ({
  default: ({
    groupId,
    workspacePath,
    isSceneActive,
  }: {
    groupId: string;
    workspacePath: string;
    isSceneActive?: boolean;
  }) => (
    <div
      data-testid="group-log-view"
      data-group-id={groupId}
      data-workspace-path={workspacePath}
      data-scene-active={isSceneActive === true ? 'true' : 'false'}
    />
  ),
}));

vi.mock('./ChatPane', () => ({
  default: ({ workspacePath, isSceneActive }: { workspacePath?: string; isSceneActive?: boolean }) => (
    <div
      data-testid="chat-pane"
      data-workspace-path={workspacePath ?? ''}
      data-scene-active={isSceneActive === true ? 'true' : 'false'}
    />
  ),
}));

// Layout scaffolding stubs (panels are not part of the R-WF-14 routing contract).
vi.mock('./AuxPane', () => ({
  default: () => <div data-testid="aux-pane" />,
}));

vi.mock('./BottomTerminalPane', () => ({
  default: () => <div data-testid="bottom-terminal-pane" />,
}));

vi.mock('@/flow_chat/store/modernFlowChatStore', async () => {
  const actual = await vi.importActual<typeof import('@/flow_chat/store/modernFlowChatStore')>(
    '@/flow_chat/store/modernFlowChatStore',
  );
  return {
    ...actual,
    useActiveSession: vi.fn(() => ({
      sessionId: 'group-1',
      isGroupChat: true,
      title: '项目群',
      workspacePath: '/workspace-a',
    })),
  };
});

vi.mock('@/app/hooks/useApp', () => ({
  useApp: () => ({
    state: {
      layout: {
        rightPanelWidth: 600,
        bottomTerminalPanelHeight: 240,
        rightPanelCollapsed: false,
        bottomTerminalPanelCollapsed: true,
        chatCollapsed: false,
        centerPanelCollapsed: false,
        chatFullWidth: false,
      },
    },
    updateRightPanelWidth: vi.fn(),
    toggleRightPanel: vi.fn(),
    toggleChatFullWidth: vi.fn(),
    updateBottomTerminalPanelHeight: vi.fn(),
    toggleBottomTerminalPanel: vi.fn(),
  }),
}));

vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useWorkspaceContext: () => ({ assistantWorkspacesList: [] }),
}));

vi.mock('@/tools/terminal/services/terminalPanelPreferenceService', () => ({
  getCachedTerminalPanelPosition: () => 'right',
  onTerminalPanelPositionChange: () => () => {},
  refreshTerminalPanelPosition: () => Promise.resolve(),
}));

import SessionScene from './SessionScene';
import { useActiveSession } from '@/flow_chat/store/modernFlowChatStore';

const mockUseActiveSession = vi.mocked(useActiveSession);

let container: HTMLDivElement;
let root: Root;

const renderScene = (props?: React.ComponentProps<typeof SessionScene>) => {
  act(() => {
    root.render(<SessionScene isActive {...props} />);
  });
};

describe('SessionScene group routing (R-WF-14, RO-3)', () => {
  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mockUseActiveSession.mockReset();
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('RO-3: routes a group-chat session to GroupLogView (not ChatPane)', () => {
    mockUseActiveSession.mockReturnValue({
      sessionId: 'group-1',
      isGroupChat: true,
      title: '项目群',
      workspacePath: '/workspace-a',
    });
    renderScene({ workspacePath: '/workspace-a' });

    expect(document.querySelector('[data-testid="group-log-view"]')).not.toBeNull();
    expect(document.querySelector('[data-testid="chat-pane"]')).toBeNull();
    const log = document.querySelector('[data-testid="group-log-view"]');
    expect(log!.getAttribute('data-group-id')).toBe('group-1');
    expect(log!.getAttribute('data-workspace-path')).toBe('/workspace-a');
  });

  it('RO-3: leaves ordinary sessions on ChatPane (routing does not misfire)', () => {
    mockUseActiveSession.mockReturnValue({
      sessionId: 'session-1',
      isGroupChat: undefined,
      title: '普通会话',
      workspacePath: '/workspace-a',
    });
    renderScene({ workspacePath: '/workspace-a' });

    expect(document.querySelector('[data-testid="chat-pane"]')).not.toBeNull();
    expect(document.querySelector('[data-testid="group-log-view"]')).toBeNull();
  });

  it('RO-3: group routing honors isGroupChat restored from metadata (false = not a group)', () => {
    mockUseActiveSession.mockReturnValue({
      sessionId: 'session-2',
      isGroupChat: false,
      title: '普通会话',
      workspacePath: '/workspace-a',
    });
    renderScene({ workspacePath: '/workspace-a' });

    expect(document.querySelector('[data-testid="chat-pane"]')).not.toBeNull();
    expect(document.querySelector('[data-testid="group-log-view"]')).toBeNull();
  });
});
