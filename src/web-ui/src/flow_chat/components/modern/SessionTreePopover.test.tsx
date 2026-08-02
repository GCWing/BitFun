// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SessionTreePopover } from './SessionTreePopover';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  getSessionLineage: vi.fn(),
  sessions: new Map<string, Record<string, unknown>>(),
}));

vi.mock('@/infrastructure/api/service-api/SessionAPI', () => ({
  sessionAPI: {
    getSessionLineage: mocks.getSessionLineage,
  },
}));

vi.mock('../../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({ sessions: mocks.sessions }),
    subscribe: () => () => undefined,
  },
}));

vi.mock('@/component-library', async () => {
  const ReactModule = await import('react');

  return {
    DotMatrixLoader: () => <span data-testid="dot-matrix-loader" />,
    IconButton: ({
      children,
      tooltip,
      ...props
    }: React.ButtonHTMLAttributes<HTMLButtonElement> & { tooltip?: string }) => (
      <button type="button" title={tooltip} {...props}>{children}</button>
    ),
  };
});

function createSession(
  sessionId: string,
  sessionKind: 'main' | 'subagent',
  parentSessionId?: string,
): Record<string, unknown> {
  return {
    sessionId,
    sessionKind,
    parentSessionId,
    parentToolCallId: parentSessionId ? 'tool-1' : undefined,
    title: sessionId === 'root' ? 'Root session' : 'Running child',
    createdAt: sessionId === 'root' ? 1 : 2,
    workspacePath: '/workspace',
    mode: 'code',
    config: { agentType: 'worker' },
    subagentType: sessionKind === 'subagent' ? 'worker' : undefined,
    dialogTurns: sessionKind === 'subagent' ? [{ status: 'processing' }] : [],
  };
}

describe('SessionTreePopover', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    mocks.getSessionLineage.mockReset();
    mocks.getSessionLineage.mockResolvedValue(null);
    mocks.sessions.clear();
    mocks.sessions.set('root', createSession('root', 'main'));
    mocks.sessions.set('child', createSession('child', 'subagent', 'root'));
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.querySelector('[data-testid="flowchat-header-session-tree-menu"]')?.remove();
    vi.restoreAllMocks();
  });

  it('offers non-cascading cancellation for running child sessions', async () => {
    const onCancelSession = vi.fn().mockResolvedValue(true);
    const t = (key: string) => key;

    await act(async () => {
      root.render(
        <SessionTreePopover
          sessionId="root"
          fallbackWorkspacePath="/workspace"
          onCancelSession={onCancelSession}
          t={t}
        />,
      );
    });

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="flowchat-header-session-tree"]')?.click();
      await Promise.resolve();
    });

    const actionButton = container.querySelector<HTMLButtonElement>(
      '[aria-label="flowChatHeader.agentTreeActions"]',
    );
    expect(actionButton).not.toBeNull();
    const childNode = Array.from(container.querySelectorAll('[role="treeitem"]'))
      .find(node => node.textContent?.includes('Running child'));
    const status = childNode?.querySelector('.session-tree-popover__status');
    expect(childNode).not.toBeUndefined();
    expect(status).not.toBeNull();
    expect(Boolean(actionButton?.compareDocumentPosition(status!) & Node.DOCUMENT_POSITION_FOLLOWING)).toBe(true);

    await act(async () => {
      actionButton?.click();
    });

    const cancelButton = document.querySelector<HTMLButtonElement>(
      '[data-testid="flowchat-header-session-tree-menu"] [role="menuitem"]',
    );
    expect(cancelButton).not.toBeNull();

    await act(async () => {
      cancelButton?.click();
      await Promise.resolve();
    });

    expect(onCancelSession).toHaveBeenCalledWith(expect.objectContaining({
      sessionId: 'child',
      isRoot: false,
    }));
  });
});
