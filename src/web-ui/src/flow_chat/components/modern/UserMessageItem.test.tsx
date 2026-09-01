import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { FlowChatContext } from './FlowChatContext';
import { UserMessageItem } from './UserMessageItem';
import { globalEventBus } from '@/infrastructure/event-bus';
import { useMessageEditStore } from '../../store/messageEditStore';
import {
  SessionExecutionEvent,
  stateMachineManager,
} from '../../state-machine';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const activeSessionRef: { current: any } = {
  current: null,
};
const rollbackServiceMock = vi.hoisted(() => ({
  rollbackSessionToTurn: vi.fn(async () => ({
    restoredFiles: [],
    fromTurnIndex: 0,
    composerText: 'restored prompt',
  })),
}));
const componentLibraryMock = vi.hoisted(() => ({
  confirmDanger: vi.fn(async () => true),
}));
const editServiceMock = vi.hoisted(() => ({
  describeUserMessageEditImpact: vi.fn(() => ({
    willStopRunningTask: false,
    willRestoreFiles: true,
    willDeleteTurns: true,
    willRerun: true,
  })),
  editAndRerunUserMessage: vi.fn(async () => undefined),
}));

function createPartialHistorySession(includeCatalog: boolean) {
  const session: any = {
    sessionId: 'partial-session',
    sessionKind: 'normal',
    isPartial: true,
    loadedTurnCount: 1,
    totalTurnCount: 20,
    dialogTurns: [{ id: 'turn-20', status: 'completed', backendTurnIndex: 19 }],
  };
  if (includeCatalog) {
    session.turnCatalog = {
      schemaVersion: 1,
      sessionId: 'partial-session',
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
    };
  }
  return session;
}

function createHydratedHistoryState(partialSession: any) {
  return {
    sessions: new Map([[
      'partial-session',
      {
        ...partialSession,
        isPartial: false,
        loadedTurnCount: 20,
        dialogTurns: Array.from({ length: 20 }, (_, index) => ({
          id: `turn-${index + 1}`,
          status: 'completed',
        })),
      },
    ]]),
    activeSessionId: 'partial-session',
  };
}

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
  useTranslation: () => ({
    t: (key: string) => {
      const labels: Record<string, string> = {
        'steering.statusPending': '等待触发',
        'steering.statusInjected': '已触发',
        'message.copy': '复制',
        'message.copyFailed': '复制失败',
      };
      return labels[key] ?? key;
    },
  }),
}));

vi.mock('../../store/modernFlowChatStore', () => ({
  useActiveSession: () => activeSessionRef.current,
}));

vi.mock('../../services/flow-chat-manager/PeerSessionRefreshModule', () => ({
  installPeerSessionRefresh: vi.fn(() => () => {}),
}));

const flowChatStoreMock = vi.hoisted(() => ({
  getState: vi.fn(() => ({
    sessions: new Map(),
    activeSessionId: null,
  })),
  loadSessionHistory: vi.fn(async () => undefined),
}));

vi.mock('../../store/FlowChatStore', () => ({
  FlowChatStore: {
    getInstance: () => flowChatStoreMock,
  },
  flowChatStore: flowChatStoreMock,
}));

vi.mock('../../services/SessionRollbackService', () => rollbackServiceMock);

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('@/infrastructure/event-bus', () => ({
  globalEventBus: {
    emit: vi.fn(),
  },
}));

vi.mock('@bitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@bitfun/ui')>(),
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('@/infrastructure/confirm-dialog', async importOriginal => ({
  ...await importOriginal<typeof import('@/infrastructure/confirm-dialog')>(),
  confirmDanger: componentLibraryMock.confirmDanger,
}));

vi.mock('../../services/UserMessageEditService', () => ({
  describeUserMessageEditImpact: editServiceMock.describeUserMessageEditImpact,
  editAndRerunUserMessage: editServiceMock.editAndRerunUserMessage,
}));

vi.mock('./UserMessageEditComposer', () => ({
  UserMessageEditComposer: ({ onSubmit }: { onSubmit: () => void }) => (
    <button
      type="button"
      className="user-message-edit-composer__icon-button--confirm"
      onClick={() => onSubmit()}
    >
      Submit edit
    </button>
  ),
}));

describe('UserMessageItem steering tag', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    flowChatStoreMock.getState.mockReturnValue({
      sessions: new Map(),
      activeSessionId: null,
    });
    flowChatStoreMock.loadSessionHistory.mockResolvedValue(undefined);
    componentLibraryMock.confirmDanger.mockResolvedValue(true);
    rollbackServiceMock.rollbackSessionToTurn.mockResolvedValue({
      restoredFiles: [],
      fromTurnIndex: 0,
      composerText: 'restored prompt',
    });
    editServiceMock.editAndRerunUserMessage.mockResolvedValue(undefined);
    stateMachineManager.clear();
    useMessageEditStore.getState().cancelEdit();
    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>', {
      pretendToBeVisual: true,
    });
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    vi.stubGlobal('HTMLElement', dom.window.HTMLElement);
    vi.stubGlobal('navigator', {
      clipboard: {
        writeText: vi.fn(),
      },
    });

    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
    activeSessionRef.current = null;
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    useMessageEditStore.getState().cancelEdit();
    vi.unstubAllGlobals();
  });

  it('renders pending steering tag on the right side of the message row', () => {
    act(() => {
      root.render(
        <FlowChatContext.Provider value={{ allowUserMessageRollback: false }}>
          <UserMessageItem
            message={{
              id: 'user-steering-1',
              content: 'Please adjust this now',
              timestamp: 1000,
            }}
            turnId="turn-1"
            steeringStatus="pending"
          />
        </FlowChatContext.Provider>,
      );
    });

    const main = container.querySelector('.user-message-item__main');
    const tag = main?.querySelector('.user-message-item__steering-tag');

    expect(tag?.textContent).toBe('等待触发');
  });

  it('does not render a steering tag after steering is triggered', () => {
    act(() => {
      root.render(
        <FlowChatContext.Provider value={{ allowUserMessageRollback: false }}>
          <UserMessageItem
            message={{
              id: 'user-steering-1',
              content: 'Please adjust this now',
              timestamp: 1000,
            }}
            turnId="turn-1"
            steeringStatus="completed"
          />
        </FlowChatContext.Provider>,
      );
    });

    expect(container.querySelector('.user-message-item__steering-tag')).toBeNull();
  });

  it('renders persisted reference metadata as capsules instead of raw prompt tags', () => {
    act(() => {
      root.render(
        <FlowChatContext.Provider value={{ allowUserMessageRollback: false }}>
          <UserMessageItem
            message={{
              id: 'user-reference-1',
              content: '[session: Delete all files] review this',
              timestamp: 1000,
              metadata: {
                composerPresentation: {
                  version: 1,
                  segments: [
                    {
                      kind: 'context',
                      context: {
                        id: 'session-reference-1',
                        type: 'session-reference',
                        sessionId: 'session-1',
                        sessionName: 'Delete all files',
                        workspacePath: '/workspace',
                        workspaceLabel: 'Workspace',
                        timestamp: 1,
                      },
                      tag: '[session: Delete all files]',
                      label: 'Delete all files',
                      title: 'Workspace · /workspace',
                    },
                    { kind: 'text', text: ' review this with ' },
                    {
                      kind: 'inline-token',
                      token: '[$pdf]',
                      tokenType: 'skill',
                      label: 'pdf',
                    },
                  ],
                },
              },
            }}
            turnId="turn-reference-1"
          />
        </FlowChatContext.Provider>,
      );
    });

    const content = container.querySelector('[data-testid="chat-user-message-content"]');
    expect(content?.textContent).toContain('Delete all files');
    expect(content?.textContent).toContain('review this with');
    expect(content?.textContent).toContain('pdf');
    expect(content?.textContent).not.toContain('[session:');
    expect(content?.querySelectorAll('.user-message-item__reference')).toHaveLength(2);
  });

  it('includes the persisted presentation when a failed message is restored to the input', () => {
    const composerPresentation = {
      version: 1,
      segments: [
        {
          kind: 'context',
          context: {
            id: 'session-reference-1',
            type: 'session-reference',
            sessionId: 'session-1',
            sessionName: 'Delete all files',
            workspacePath: '/workspace',
            workspaceLabel: 'Workspace',
            timestamp: 1,
          },
          tag: '[session: Delete all files]',
          label: 'Delete all files',
          title: 'Workspace · /workspace',
        },
      ],
    };
    activeSessionRef.current = {
      sessionId: 'failed-session',
      sessionKind: 'normal',
      dialogTurns: [{ id: 'turn-failed-1', status: 'error' }],
    };

    act(() => {
      root.render(
        <FlowChatContext.Provider value={{ sessionId: 'failed-session', allowUserMessageRollback: true }}>
          <UserMessageItem
            message={{
              id: 'user-failed-1',
              content: '[session: Delete all files]',
              timestamp: 1000,
              metadata: { composerPresentation },
            }}
            turnId="turn-failed-1"
          />
        </FlowChatContext.Provider>,
      );
    });

    const fillButton = container.querySelectorAll<HTMLButtonElement>('.user-message-item__copy-btn')[1];
    expect(fillButton).not.toBeNull();
    act(() => {
      fillButton?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(globalEventBus.emit).toHaveBeenCalledWith('fill-chat-input', {
      content: '[session: Delete all files]',
      composerPresentation,
    });
  });

  it('hides the rollback button for subagent sessions', () => {
    activeSessionRef.current = {
      sessionId: 'subagent-session',
      sessionKind: 'subagent',
      dialogTurns: [
        {
          id: 'turn-1',
          status: 'completed',
        },
      ],
    };

    act(() => {
      root.render(
        <FlowChatContext.Provider value={{ sessionId: 'subagent-session', allowUserMessageRollback: true }}>
          <UserMessageItem
            message={{
              id: 'user-subagent-1',
              content: 'subagent question',
              timestamp: 1000,
            }}
            turnId="turn-1"
          />
        </FlowChatContext.Provider>,
      );
    });

    expect(container.querySelector('.user-message-item__rollback-btn')).toBeNull();
  });

  it('renders the rollback button for normal sessions when rollback is allowed', () => {
    activeSessionRef.current = {
      sessionId: 'main-session',
      sessionKind: 'normal',
      dialogTurns: [
        {
          id: 'turn-1',
          status: 'completed',
        },
      ],
    };

    act(() => {
      root.render(
        <FlowChatContext.Provider value={{ sessionId: 'main-session', allowUserMessageRollback: true }}>
          <UserMessageItem
            message={{
              id: 'user-main-1',
              content: 'main session question',
              timestamp: 1000,
            }}
            turnId="turn-1"
          />
        </FlowChatContext.Provider>,
      );
    });

    expect(container.querySelector('.user-message-item__rollback-btn')).not.toBeNull();
  });

  it('disables file-consistent rollback and message editing for remote workspaces', () => {
    activeSessionRef.current = {
      sessionId: 'remote-session',
      sessionKind: 'normal',
      remoteConnectionId: 'ssh:user@example.com:22',
      remoteSshHost: 'example.com',
      config: {},
      dialogTurns: [
        {
          id: 'turn-1',
          status: 'completed',
        },
      ],
    };

    act(() => {
      root.render(
        <FlowChatContext.Provider
          value={{
            sessionId: 'remote-session',
            allowUserMessageRollback: true,
            allowUserMessageEdit: true,
          }}
        >
          <UserMessageItem
            message={{
              id: 'user-remote-1',
              content: 'remote session question',
              timestamp: 1000,
            }}
            turnId="turn-1"
          />
        </FlowChatContext.Provider>,
      );
    });

    const rollbackButton = container.querySelector<HTMLButtonElement>(
      '.user-message-item__rollback-btn',
    );
    const editButton = container.querySelector<HTMLButtonElement>('.user-message-item__edit-btn');

    expect(rollbackButton?.disabled).toBe(true);
    expect(rollbackButton?.title).toContain('message.rollbackDisabledRemote');
    expect(editButton?.disabled).toBe(true);
    expect(editButton?.title).toContain('message.editDisabledRemote');
  });

  it('hides the edit button when the panel context disables user message editing', () => {
    activeSessionRef.current = {
      sessionId: 'btw-session',
      sessionKind: 'btw',
      dialogTurns: [
        {
          id: 'turn-1',
          status: 'completed',
        },
      ],
    };

    act(() => {
      root.render(
        <FlowChatContext.Provider
          value={{
            sessionId: 'btw-session',
            allowUserMessageRollback: true,
            allowUserMessageEdit: false,
          }}
        >
          <UserMessageItem
            message={{
              id: 'user-btw-1',
              content: 'btw session question',
              timestamp: 1000,
            }}
            turnId="turn-1"
          />
        </FlowChatContext.Provider>,
      );
    });

    expect(container.querySelector('.user-message-item__edit-btn')).toBeNull();
  });

  it('keeps edit and rollback available for on-demand hydration in a partial history tail', () => {
    activeSessionRef.current = {
      sessionId: 'partial-session',
      sessionKind: 'normal',
      isPartial: true,
      loadedTurnCount: 1,
      totalTurnCount: 20,
      dialogTurns: [
        {
          id: 'turn-20',
          status: 'completed',
          backendTurnIndex: 19,
        },
      ],
    };

    act(() => {
      root.render(
        <FlowChatContext.Provider
          value={{
            sessionId: 'partial-session',
            allowUserMessageRollback: true,
            allowUserMessageEdit: true,
          }}
        >
          <UserMessageItem
            message={{
              id: 'user-partial-20',
              content: 'latest partial prompt',
              timestamp: 1000,
            }}
            turnId="turn-20"
          />
        </FlowChatContext.Provider>,
      );
    });

    expect(container.querySelector<HTMLButtonElement>('.user-message-item__edit-btn')?.disabled).toBe(false);
    expect(container.querySelector<HTMLButtonElement>('.user-message-item__rollback-btn')?.disabled).toBe(false);
  });

  it('rolls back partial history by stable Turn identity without full hydration', async () => {
    activeSessionRef.current = {
      sessionId: 'partial-session',
      sessionKind: 'normal',
      isPartial: true,
      loadedTurnCount: 1,
      totalTurnCount: 20,
      dialogTurns: [
        {
          id: 'turn-20',
          status: 'completed',
          backendTurnIndex: 19,
        },
      ],
    };
    flowChatStoreMock.getState.mockReturnValue({
      sessions: new Map([[
        'partial-session',
        {
          ...activeSessionRef.current,
          isPartial: false,
          loadedTurnCount: 20,
          dialogTurns: Array.from({ length: 20 }, (_, index) => ({
            id: `turn-${index + 1}`,
            status: 'completed',
          })),
        },
      ]]),
      activeSessionId: 'partial-session',
    });

    act(() => {
      root.render(
        <FlowChatContext.Provider
          value={{
            sessionId: 'partial-session',
            allowUserMessageRollback: true,
            allowUserMessageEdit: true,
          }}
        >
          <UserMessageItem
            message={{
              id: 'user-partial-20',
              content: 'latest partial prompt',
              timestamp: 1000,
            }}
            turnId="turn-20"
          />
        </FlowChatContext.Provider>,
      );
    });

    await act(async () => {
      container.querySelector<HTMLButtonElement>('.user-message-item__rollback-btn')?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(rollbackServiceMock.rollbackSessionToTurn).toHaveBeenCalledWith(expect.objectContaining({ sessionId: 'partial-session', targetTurnId: 'turn-20', kind: 'rollback' }));
    expect(flowChatStoreMock.loadSessionHistory).not.toHaveBeenCalled();
  });

  it('keeps edit and rollback available for a rendered Turn outside the canonical tail', () => {
    activeSessionRef.current = createPartialHistorySession(false);

    act(() => {
      root.render(
        <FlowChatContext.Provider
          value={{
            sessionId: 'partial-session',
            allowUserMessageRollback: true,
            allowUserMessageEdit: true,
          }}
        >
          <UserMessageItem
            message={{
              id: 'user-partial-5',
              content: 'older window prompt',
              timestamp: 1000,
            }}
            turnId="turn-5"
            absoluteTurnIndex={5}
          />
        </FlowChatContext.Provider>,
      );
    });

    expect(container.querySelector<HTMLButtonElement>('.user-message-item__edit-btn')?.disabled).toBe(false);
    expect(container.querySelector<HTMLButtonElement>('.user-message-item__rollback-btn')?.disabled).toBe(false);
  });

  it('disables edit and rollback while the Session is not idle', async () => {
    activeSessionRef.current = createPartialHistorySession(false);
    await stateMachineManager.transition('partial-session', SessionExecutionEvent.START);

    await act(async () => {
      root.render(
        <FlowChatContext.Provider
          value={{
            sessionId: 'partial-session',
            allowUserMessageRollback: true,
            allowUserMessageEdit: true,
          }}
        >
          <UserMessageItem
            message={{ id: 'user-partial-5', content: 'older window prompt', timestamp: 1000 }}
            turnId="turn-5"
            absoluteTurnIndex={5}
          />
        </FlowChatContext.Provider>,
      );
    });

    expect(container.querySelector<HTMLButtonElement>('.user-message-item__edit-btn')?.disabled).toBe(true);
    expect(container.querySelector<HTMLButtonElement>('.user-message-item__rollback-btn')?.disabled).toBe(true);
  });

  it('rolls back a cataloged history-window Turn by stable identity', async () => {
    activeSessionRef.current = createPartialHistorySession(true);
    flowChatStoreMock.getState.mockReturnValue(
      createHydratedHistoryState(activeSessionRef.current),
    );

    act(() => {
      root.render(
        <FlowChatContext.Provider
          value={{
            sessionId: 'partial-session',
            allowUserMessageRollback: true,
            allowUserMessageEdit: true,
          }}
        >
          <UserMessageItem
            message={{ id: 'user-partial-5', content: 'older window prompt', timestamp: 1000 }}
            turnId="turn-5"
          />
        </FlowChatContext.Provider>,
      );
    });

    await act(async () => {
      container.querySelector<HTMLButtonElement>('.user-message-item__rollback-btn')?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(rollbackServiceMock.rollbackSessionToTurn).toHaveBeenCalledWith(expect.objectContaining({ sessionId: 'partial-session', targetTurnId: 'turn-5', kind: 'rollback' }));
  });

  it('edits and reruns a cataloged history-window Turn without pre-hydration', async () => {
    activeSessionRef.current = createPartialHistorySession(true);
    flowChatStoreMock.getState.mockReturnValue(
      createHydratedHistoryState(activeSessionRef.current),
    );

    act(() => {
      root.render(
        <FlowChatContext.Provider
          value={{
            sessionId: 'partial-session',
            allowUserMessageRollback: true,
            allowUserMessageEdit: true,
          }}
        >
          <UserMessageItem
            message={{ id: 'user-partial-5', content: 'older window prompt', timestamp: 1000 }}
            turnId="turn-5"
          />
        </FlowChatContext.Provider>,
      );
    });

    await act(async () => {
      container.querySelector<HTMLButtonElement>('.user-message-item__edit-btn')?.click();
    });
    await act(async () => {
      useMessageEditStore.getState().setDraft('edited older window prompt');
    });
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>('.user-message-edit-composer__icon-button--confirm')
        ?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(editServiceMock.editAndRerunUserMessage).toHaveBeenCalledWith(expect.objectContaining({
      sessionId: 'partial-session',
      turnId: 'turn-5',
      originalContent: 'older window prompt',
      editedContent: 'edited older window prompt',
    }));
    expect(flowChatStoreMock.loadSessionHistory).not.toHaveBeenCalled();
  });
});
