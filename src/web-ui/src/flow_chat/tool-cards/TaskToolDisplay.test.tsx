import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TaskToolDisplay } from './TaskToolDisplay';
import { taskCollapseStateManager } from '../store/TaskCollapseStateManager';
import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';

const mocks = vi.hoisted(() => ({
  openBtwSessionInAuxPane: vi.fn(),
  cancelSession: vi.fn(),
  notificationError: vi.fn(),
}));

vi.mock('react-i18next', () => {
  const t = (key: string, options?: Record<string, unknown>) => {
    if (key === 'toolCards.taskTool.headerLine') {
      return `${options?.agentType} agent: ${options?.description}`;
    }
    if (key === 'toolCards.taskTool.headerLinePrefix') {
      return `${options?.agentType} agent`;
    }
    if (key === 'toolCards.taskTool.headerLineSuffix') {
      return `: ${options?.description}`;
    }
    if (key === 'toolCards.taskTool.defaultAgentKind') {
      return 'Sub-agent';
    }
    if (key === 'toolCards.taskTool.cancelSession') {
      return `Cancel session: ${options?.sessionId}`;
    }
    if (key.startsWith('reviewTeams.') && typeof options?.defaultValue === 'string') {
      return options.defaultValue;
    }
    return key;
  };
  return {
    useTranslation: () => ({ t }),
  };
});

vi.mock('../../component-library', () => ({
  Button: ({
    children,
    disabled,
    onClick,
  }: {
    children: React.ReactNode;
    disabled?: boolean;
    onClick?: () => void;
  }) => (
    <button type="button" disabled={disabled} onClick={onClick}>
      {children}
    </button>
  ),
  CubeLoading: () => <span data-testid="cube-loading" />,
}));

vi.mock('@/component-library/components/Markdown/Markdown', () => ({
  Markdown: ({ content }: { content: string }) => <div>{content}</div>,
}));

vi.mock('@/shared/services/reviewTeamService', () => ({
  getReviewerContextBySubagentId: () => null,
}));

vi.mock('./ToolTimeoutIndicator', () => ({
  ToolTimeoutIndicator: () => <span data-testid="tool-timeout-indicator" />,
}));

vi.mock('../services/btwSessionPane', () => ({
  openBtwSessionInAuxPane: (...args: unknown[]) => mocks.openBtwSessionInAuxPane(...args),
}));

vi.mock('@/infrastructure/api/service-api/AgentAPI', () => ({
  agentAPI: {
    cancelSession: (...args: unknown[]) => mocks.cancelSession(...args),
  },
}));

vi.mock('@/shared/notification-system/services/NotificationService', () => ({
  notificationService: {
    error: (...args: unknown[]) => mocks.notificationError(...args),
  },
}));

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    subscribe: () => () => {},
    getState: () => ({
      sessions: new Map([
        ['parent-session', {
          sessionId: 'parent-session',
          workspacePath: 'D:\\workspace\\repo',
          remoteConnectionId: 'remote-1',
          remoteSshHost: 'host-1',
          config: { agentType: 'agentic' },
        }],
        ['subagent-session-1', {
          sessionId: 'subagent-session-1',
          mode: 'Explore',
          config: { agentType: 'Explore', modelName: 'fast' },
        }],
      ]),
    }),
  },
}));

let JSDOMCtor: (new (
  html?: string,
  options?: { pretendToBeVisual?: boolean; url?: string }
) => { window: Window & typeof globalThis }) | null = null;

try {
  const jsdom = await import('jsdom');
  JSDOMCtor = jsdom.JSDOM as typeof JSDOMCtor;
} catch {
  JSDOMCtor = null;
}

const describeWithJsdom = JSDOMCtor ? describe : describe.skip;

const config: ToolCardConfig = {
  toolName: 'Task',
  displayName: 'Task',
  icon: 'task',
  requiresConfirmation: false,
  resultDisplayType: 'summary',
};

function failedTaskItem(): FlowToolItem {
  return {
    id: 'task-tool-1',
    type: 'tool',
    toolName: 'Task',
    timestamp: Date.now(),
    status: 'error',
    toolCall: {
      id: 'task-call-1',
      input: {
        description: 'Review frontend',
        prompt: 'Review frontend code',
        subagent_type: 'ReviewFrontend',
      },
    },
    toolResult: {
      success: false,
      result: null,
      error: 'Subagent failed before finishing.',
    },
  };
}

function reviewTaskItem(
  status: FlowToolItem['status'],
  subagentType = 'ReviewFrontend',
  description = `Review frontend [packet reviewer:${subagentType}:group-1-of-1]`,
): FlowToolItem {
  return {
    id: 'task-tool-1',
    type: 'tool',
    toolName: 'Task',
    timestamp: Date.now(),
    status,
    toolCall: {
      id: 'task-call-1',
      input: {
        description,
        prompt: 'Review frontend code',
        subagent_type: subagentType,
      },
    },
    toolResult:
      status === 'completed'
        ? {
            success: true,
            result: {
              duration: 1000,
            },
          }
        : undefined,
  };
}

describeWithJsdom('TaskToolDisplay', () => {
  let dom: { window: Window & typeof globalThis };
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    dom = new JSDOMCtor!('<!doctype html><html><body></body></html>', {
      pretendToBeVisual: true,
      url: 'http://localhost',
    });

    const { window } = dom;
    vi.stubGlobal('window', window);
    vi.stubGlobal('document', window.document);
    vi.stubGlobal('navigator', window.navigator);
    vi.stubGlobal('HTMLElement', window.HTMLElement);
    vi.stubGlobal('CustomEvent', window.CustomEvent);
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);

    taskCollapseStateManager.clearAll();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    dom.window.close();
    vi.unstubAllGlobals();
    vi.clearAllMocks();
    taskCollapseStateManager.clearAll();
  });

  it('allows a failed subagent task card to collapse after it was expanded', async () => {
    taskCollapseStateManager.setCollapsed('task-tool-1', false);

    await act(async () => {
      root.render(
        <TaskToolDisplay
          toolItem={failedTaskItem()}
          config={config}
          sessionId="parent-session"
        />,
      );
    });

    expect(taskCollapseStateManager.isCollapsed('task-tool-1')).toBe(false);

    const card = container.querySelector<HTMLElement>('.base-tool-card');
    expect(card).toBeTruthy();

    await act(async () => {
      card!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(taskCollapseStateManager.isCollapsed('task-tool-1')).toBe(true);
  });

  it('keeps Deep Review reviewer task cards collapsed when they start running', async () => {
    await act(async () => {
      root.render(
        <TaskToolDisplay
          toolItem={reviewTaskItem('completed')}
          config={config}
          sessionId="parent-session"
        />,
      );
    });

    expect(taskCollapseStateManager.isCollapsed('task-tool-1')).toBe(true);

    await act(async () => {
      root.render(
        <TaskToolDisplay
          toolItem={reviewTaskItem('streaming')}
          config={config}
          sessionId="parent-session"
        />,
      );
    });

    expect(taskCollapseStateManager.isCollapsed('task-tool-1')).toBe(true);
  });

  it('keeps extra Deep Review reviewer task cards collapsed from packet metadata', async () => {
    await act(async () => {
      root.render(
        <TaskToolDisplay
          toolItem={reviewTaskItem('completed', 'ExtraReadonlyReview')}
          config={config}
          sessionId="parent-session"
        />,
      );
    });

    expect(taskCollapseStateManager.isCollapsed('task-tool-1')).toBe(true);

    await act(async () => {
      root.render(
        <TaskToolDisplay
          toolItem={reviewTaskItem('running', 'ExtraReadonlyReview')}
          config={config}
          sessionId="parent-session"
        />,
      );
    });

    expect(taskCollapseStateManager.isCollapsed('task-tool-1')).toBe(true);
  });

  it('opens the real subagent session in the aux pane when the task card rail is clicked', async () => {
    const toolItem: FlowToolItem = {
      ...reviewTaskItem('completed', 'Explore', 'Investigate task card behavior'),
      subagentSessionId: 'subagent-session-1',
    };

    await act(async () => {
      root.render(
        <TaskToolDisplay
          toolItem={toolItem}
          config={config}
          sessionId="parent-session"
        />,
      );
    });

    const openButton = container.querySelector<HTMLButtonElement>('.task-header-rail__hit');
    expect(openButton).toBeTruthy();

    await act(async () => {
      openButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(mocks.openBtwSessionInAuxPane).toHaveBeenCalledWith({
      childSessionId: 'subagent-session-1',
      parentSessionId: 'parent-session',
      workspacePath: 'D:\\workspace\\repo',
      sessionKind: 'subagent',
      sessionTitle: expect.any(String),
      agentType: 'Explore',
      parentToolCallId: 'task-call-1',
      subagentType: 'Explore',
      remoteConnectionId: 'remote-1',
      remoteSshHost: 'host-1',
      includeInternal: true,
    });
  });

  it('renders spawn task cards from the result subagent session metadata', async () => {
    const toolItem: FlowToolItem = {
      id: 'task-tool-spawn',
      type: 'tool',
      toolName: 'Task',
      timestamp: Date.now(),
      status: 'completed',
      toolCall: {
        id: 'task-call-spawn',
        input: {
          action: 'spawn',
          fork_context: true,
          description: 'Explore isolated context',
          prompt: 'Investigate the isolated path',
        },
      },
      toolResult: {
        success: true,
        result: {
          action: 'spawn',
          session_id: 'subagent-session-1',
        },
      },
    };

    await act(async () => {
      root.render(
        <TaskToolDisplay
          toolItem={toolItem}
          config={config}
          sessionId="parent-session"
        />,
      );
    });

    const header = container.querySelector<HTMLElement>('.task-action');
    expect(header?.textContent).toContain('Explore');
    expect(header?.textContent).toContain('fast');
    expect(header?.textContent).toContain('Explore isolated context');
  });

  it('renders send_input task cards from the target subagent session metadata', async () => {
    const toolItem: FlowToolItem = {
      id: 'task-tool-send-input',
      type: 'tool',
      toolName: 'Task',
      timestamp: Date.now(),
      status: 'running',
      toolCall: {
        id: 'task-call-send-input',
        input: {
          action: 'send_input',
          session_id: 'subagent-session-1',
          description: 'Continue investigation',
          prompt: 'Keep checking the failing path',
        },
      },
    };

    await act(async () => {
      root.render(
        <TaskToolDisplay
          toolItem={toolItem}
          config={config}
          sessionId="parent-session"
        />,
      );
    });

    const header = container.querySelector<HTMLElement>('.task-action');
    expect(header?.textContent).toContain('Explore');
    expect(header?.textContent).toContain('fast');
    expect(header?.textContent).toContain('Continue investigation');
  });

  it('stops a running foreground subagent from the task header', async () => {
    mocks.cancelSession.mockResolvedValueOnce(undefined);

    const toolItem: FlowToolItem = {
      ...reviewTaskItem('running', 'Explore', 'Investigate task card behavior'),
      subagentSessionId: 'subagent-session-1',
    };

    await act(async () => {
      root.render(
        <TaskToolDisplay
          toolItem={toolItem}
          config={config}
          sessionId="parent-session"
        />,
      );
    });

    const stopButton = container.querySelector<HTMLButtonElement>('.task-subagent-stop-button');
    expect(stopButton).toBeTruthy();

    await act(async () => {
      stopButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(mocks.cancelSession).toHaveBeenCalledWith('subagent-session-1');
  });

  it('does not show the foreground stop button for background subagents', async () => {
    const toolItem: FlowToolItem = {
      ...reviewTaskItem('running', 'Explore', 'Investigate background behavior'),
      subagentSessionId: 'subagent-session-1',
      toolCall: {
        id: 'task-call-1',
        input: {
          description: 'Investigate background behavior',
          prompt: 'Keep checking the background path',
          subagent_type: 'Explore',
          run_in_background: true,
        },
      },
    };

    await act(async () => {
      root.render(
        <TaskToolDisplay
          toolItem={toolItem}
          config={config}
          sessionId="parent-session"
        />,
      );
    });

    expect(container.querySelector('.task-subagent-stop-button')).toBeNull();
  });

  it('renders cancelled foreground subagent results as cancelled instead of failed', async () => {
    const toolItem: FlowToolItem = {
      ...reviewTaskItem('error', 'Explore', 'Investigate cancellable task'),
      subagentSessionId: 'subagent-session-1',
      toolResult: {
        success: false,
        result: null,
        error: 'Subagent task has been cancelled',
        duration_ms: 1200,
      },
    };

    await act(async () => {
      root.render(
        <TaskToolDisplay
          toolItem={toolItem}
          config={config}
          sessionId="parent-session"
        />,
      );
    });

    expect(container.querySelector('.task-failed-badge')).toBeNull();
    expect(container.querySelector('.status-cancelled')).toBeTruthy();
    expect(container.textContent).not.toContain('Failed');
  });

  it('keeps cancel task cards collapsed and disables opening the subagent session', async () => {
    taskCollapseStateManager.setCollapsed('task-tool-cancel', false);
    const toolItem: FlowToolItem = {
      id: 'task-tool-cancel',
      type: 'tool',
      toolName: 'Task',
      timestamp: Date.now(),
      status: 'completed',
      toolCall: {
        id: 'task-call-cancel',
        input: {
          action: 'cancel',
          session_id: 'subagent-session-1',
          description: 'Cancel investigation',
        },
      },
      toolResult: {
        success: true,
        result: {
          action: 'cancel',
          status: 'cancelled',
          session_id: 'subagent-session-1',
          cancelled_background_tasks: 1,
        },
      },
    };

    await act(async () => {
      root.render(
        <TaskToolDisplay
          toolItem={toolItem}
          config={config}
          sessionId="parent-session"
        />,
      );
    });

    expect(container.querySelector('.task-header-rail__hit')).toBeNull();
    expect(container.querySelector('.compact-tool-card')).toBeTruthy();
    expect(container.textContent).toContain('Cancel session: subagent-session-1');
    expect(container.querySelector('.base-tool-card.expanded')).toBeNull();
    expect(taskCollapseStateManager.isCollapsed('task-tool-cancel')).toBe(true);
  });
});
