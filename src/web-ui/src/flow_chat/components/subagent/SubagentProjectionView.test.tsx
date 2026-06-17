// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { SubagentProjectionView } from './SubagentProjectionView';
import type { FlowChatState, Session } from '../../types/flow-chat';

let flowChatState: FlowChatState;
const ensureBtwSessionAvailableMock = vi.fn();

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string; shown?: number; total?: number }) =>
      options?.defaultValue ?? key,
  }),
}));

vi.mock('../FlowTextBlock', () => ({
  FlowTextBlock: () => <div data-testid="flow-text-block" />,
}));

vi.mock('../../tool-cards/ModelThinkingDisplay', () => ({
  ModelThinkingDisplay: () => <div data-testid="thinking-display" />,
}));

vi.mock('../FlowToolCard', () => ({
  FlowToolCard: () => <div data-testid="flow-tool-card" />,
}));

vi.mock('../modern/SmoothHeightCollapse', () => ({
  SmoothHeightCollapse: ({
    children,
    isOpen,
  }: {
    children: React.ReactNode;
    isOpen: boolean;
  }) => (isOpen ? <>{children}</> : null),
}));

vi.mock('../../store/FlowChatStore', () => ({
  FlowChatStore: {
    getInstance: () => ({
      getState: () => flowChatState,
      subscribe: () => () => {},
    }),
  },
}));

vi.mock('../../services/openBtwSession', () => ({
  ensureBtwSessionAvailable: (...args: unknown[]) => ensureBtwSessionAvailableMock(...args),
}));

function createSession(overrides: Partial<Session>): Session {
  return {
    sessionId: 'subagent-1',
    title: 'Subagent',
    dialogTurns: [],
    status: 'idle',
    config: {},
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    sessionKind: 'subagent',
    ...overrides,
  } as Session;
}

describe('SubagentProjectionView', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    ensureBtwSessionAvailableMock.mockReset();

    flowChatState = {
      sessions: new Map([
        ['parent-1', createSession({
          sessionId: 'parent-1',
          sessionKind: 'normal',
          workspacePath: 'D:/workspace/project',
          remoteConnectionId: 'remote-1',
          remoteSshHost: 'host-1',
        })],
      ]),
      activeSessionId: 'parent-1',
    };
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it('hydrates a metadata-only historical subagent session for collapsed transcript projection', async () => {
    flowChatState.sessions.set('subagent-1', createSession({
      isHistorical: true,
      historyState: 'metadata-only',
      workspacePath: 'D:/workspace/project',
      remoteConnectionId: 'remote-1',
      remoteSshHost: 'host-1',
    }));

    await act(async () => {
      root.render(
        <SubagentProjectionView
          parentTaskToolId="task-1"
          parentSessionId="parent-1"
          subagentSessionId="subagent-1"
        />,
      );
      await Promise.resolve();
    });

    expect(ensureBtwSessionAvailableMock).toHaveBeenCalledWith(
      expect.objectContaining({
        childSessionId: 'subagent-1',
        parentSessionId: 'parent-1',
        workspacePath: 'D:/workspace/project',
        sessionKind: 'subagent',
        remoteConnectionId: 'remote-1',
        remoteSshHost: 'host-1',
        includeInternal: true,
      }),
    );
  });

  it('does not hydrate when the caller already supplies projected items', async () => {
    flowChatState.sessions.set('subagent-1', createSession({
      isHistorical: true,
      historyState: 'metadata-only',
      workspacePath: 'D:/workspace/project',
    }));

    await act(async () => {
      root.render(
        <SubagentProjectionView
          parentTaskToolId="task-1"
          parentSessionId="parent-1"
          subagentSessionId="subagent-1"
          items={[]}
        />,
      );
      await Promise.resolve();
    });

    expect(ensureBtwSessionAvailableMock).not.toHaveBeenCalled();
  });
});
