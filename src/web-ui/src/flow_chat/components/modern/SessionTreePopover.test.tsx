// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SessionTreePopover } from './SessionTreePopover';
import { useSubagentIdentityStore } from '../../subagent-identity';

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
    IconButton: ReactModule.forwardRef<HTMLButtonElement, React.ButtonHTMLAttributes<HTMLButtonElement> & {
      tooltip?: string;
    }>(({
      children,
      tooltip,
      ...props
    }, ref) => (
      <button ref={ref} type="button" title={tooltip} {...props}>{children}</button>
    )),
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
    useSubagentIdentityStore.getState().clear();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.querySelector('[data-bf-overlay-host="true"]')?.remove();
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

    const panel = document.querySelector<HTMLElement>('.session-tree-popover__panel');
    expect(panel?.parentElement?.getAttribute('data-bf-overlay-host')).toBe('true');
    expect(panel?.style.visibility).toBe('visible');
    const actionButton = panel?.querySelector<HTMLButtonElement>(
      '[aria-label="flowChatHeader.agentTreeActions"]',
    );
    expect(actionButton).not.toBeNull();
    const childNode = Array.from(panel?.querySelectorAll('[role="treeitem"]') ?? [])
      .find(node => node.textContent?.includes('Running child'));
    const status = childNode?.querySelector('.subagent-avatar__status');
    expect(childNode).not.toBeUndefined();
    expect(status).not.toBeNull();
    expect(childNode?.contains(status!)).toBe(true);

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

  it('assigns distinct avatar and name identities to sibling subagents', async () => {
    mocks.sessions.set('child-2', createSession('child-2', 'subagent', 'root'));
    mocks.sessions.set('child-3', createSession('child-3', 'subagent', 'root'));
    const t = (key: string) => key;

    await act(async () => {
      root.render(
        <SessionTreePopover
          sessionId="root"
          fallbackWorkspacePath="/workspace"
          t={t}
        />,
      );
    });
    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="flowchat-header-session-tree"]')?.click();
      await Promise.resolve();
    });

    const avatars = Array.from(document.querySelectorAll<HTMLElement>(
      '.session-tree-popover__panel [data-bf-component="subagent-avatar"]',
    ));
    expect(avatars).toHaveLength(3);
    expect(new Set(avatars.map(avatar => avatar.dataset.bfAvatarId)).size).toBe(3);
    expect(new Set(avatars.map(avatar => avatar.dataset.bfNameId)).size).toBe(3);
  });

  it('closes a sibling action-menu portal and restores focus with the parent', async () => {
    const t = (key: string) => key;
    await act(async () => {
      root.render(
        <SessionTreePopover
          sessionId="root"
          fallbackWorkspacePath="/workspace"
          onCancelSession={vi.fn().mockResolvedValue(true)}
          t={t}
        />,
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="flowchat-header-session-tree"]',
    );
    await act(async () => {
      trigger?.click();
      await Promise.resolve();
    });

    const panel = document.querySelector<HTMLElement>('.session-tree-popover__panel');
    const actionButton = panel?.querySelector<HTMLButtonElement>(
      '[aria-label="flowChatHeader.agentTreeActions"]',
    );
    await act(async () => {
      actionButton?.click();
    });

    const actionMenuItem = document.querySelector<HTMLButtonElement>(
      '[data-testid="flowchat-header-session-tree-menu"] [role="menuitem"]',
    );
    actionMenuItem?.focus();
    expect(document.activeElement).toBe(actionMenuItem);

    await act(async () => {
      trigger?.click();
    });

    expect(document.querySelector('[data-testid="flowchat-header-session-tree-menu"]')).toBeNull();
    expect(document.activeElement).toBe(trigger);
    expect(panel?.getAttribute('aria-hidden')).toBe('true');
  });
});
