import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TaskToolDisplay } from './TaskToolDisplay';
import { taskCollapseStateManager } from '../store/TaskCollapseStateManager';
import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, options?: Record<string, unknown> & { defaultValue?: string }) =>
      options?.defaultValue ?? String(options?.agentType ?? _key),
  }),
}));

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
});
