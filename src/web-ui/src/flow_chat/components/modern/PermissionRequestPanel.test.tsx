// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequest } from '@/infrastructure/api/service-api/AgentAPI';
import { PermissionRequestPanel } from './PermissionRequestPanel';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const TRANSLATIONS: Record<string, string> = {
  'permission.actions.read': 'Read files',
  'permission.actions.edit': 'Edit files',
  'permission.actions.bash': 'Run command',
  'permission.actions.git': 'Git action',
  'permission.actions.computerUse': 'Control device',
  'permission.actions.webSearch': 'Search web',
  'permission.actions.webFetch': 'Access web',
  'permission.actions.mcp': 'MCP tool',
  'permission.actions.task': 'Run task',
  'permission.actions.skill': 'Run skill',
  'permission.actions.pagePublish': 'Save page version',
  'permission.actions.pageDeploy': 'Deploy page version',
  'permission.actions.customTool': 'External tool',
  'permission.actions.externalDirectory': 'Access external directory',
  'permission.actions.other': 'Other action',
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: Record<string, string>) => {
      if (key === 'permission.subagentOwner') {
        return `${values?.subagent} subagent`;
      }
      if (key === 'permission.allowAlwaysTooltip') {
        return `Always allow saves matching access for ${values?.projectPath}`;
      }
      if (key === 'permission.risks.pageSave') {
        return `Save ${values?.slug} as ${values?.visibility} without deploying.`;
      }
      if (key === 'permission.collapsePanel') {
        return 'Collapse permission requests';
      }
      if (key === 'permission.expandPanel') {
        return `Expand ${values?.count} pending permission requests`;
      }
      return TRANSLATIONS[key] ?? key;
    },
  }),
}));

vi.mock('@/component-library', () => ({
  Tooltip: ({ content, children }: { content: string; children: React.ReactElement }) => (
    <span data-tooltip={content}>{children}</span>
  ),
}));

vi.mock('../../store/chatInputStateStore', () => ({
  useChatInputState: () => 0,
}));

function request(delegated: boolean): PermissionRequest {
  return {
    requestId: delegated ? 'child-request' : 'direct-request',
    roundId: delegated ? 'round-child' : 'round-parent',
    order: 0,
    sessionId: delegated ? 'child-session' : 'parent-session',
    toolCallId: delegated ? 'child-tool' : 'direct-tool',
    projectPath: '/workspace/BitFun',
    projectId: 'project-1',
    agentId: delegated ? 'Explore' : 'agentic',
    action: 'edit',
    resources: ['src/main.rs'],
    saveResources: ['src/main.rs'],
    source: { kind: 'tool_call', identity: 'Write' },
    delegation: delegated
      ? {
          parentSessionId: 'parent-session',
          parentDialogTurnId: 'parent-turn',
          parentToolCallId: 'parent-task',
          subagentType: 'Explore',
        }
      : undefined,
  };
}

describe('PermissionRequestPanel', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('names the subagent that owns a delegated permission request', () => {
    act(() => {
      root.render(
        <PermissionRequestPanel
          requests={[request(true)]}
          onRespond={vi.fn()}
          onRespondBatch={vi.fn()}
        />,
      );
    });

    expect(container.textContent).toContain('Explore subagent');
    expect(container.querySelector('.permission-request-panel__heading h2')?.textContent)
      .toBe('permission.title');
  });

  it('keeps direct request details in the request row and scopes always allow to the project path', () => {
    act(() => {
      root.render(
        <PermissionRequestPanel
          requests={[request(false)]}
          onRespond={vi.fn()}
          onRespondBatch={vi.fn()}
        />,
      );
    });

    expect(container.textContent).toContain('Edit files');
    expect(container.textContent).toContain('Write');
    expect(container.textContent).not.toContain('edit');
    expect(container.textContent).not.toContain('subagent');
    const tooltips = [...container.querySelectorAll('[data-tooltip]')]
      .map((node) => node.getAttribute('data-tooltip'));
    expect(tooltips).toContain('Always allow saves matching access for /workspace/BitFun');
    expect(tooltips).not.toContain('project-1');
  });

  it('keeps resources to one ellipsized summary with the complete value in a tooltip', () => {
    const longResource = 'src/a-very-long-directory-name/another-long-directory/file-with-a-long-name.ts';
    const bashRequest = {
      ...request(false),
      action: 'bash',
      resources: [longResource, 'pnpm run type-check:web'],
    };
    act(() => {
      root.render(
        <PermissionRequestPanel
          requests={[bashRequest]}
          onRespond={vi.fn()}
          onRespondBatch={vi.fn()}
        />,
      );
    });

    const resourceSummary = container.querySelector('.permission-request-panel__resource-summary');
    expect(resourceSummary?.textContent).toBe(`${longResource}, pnpm run type-check:web`);
    expect(resourceSummary?.parentElement?.getAttribute('data-tooltip'))
      .toBe(`${longResource}, pnpm run type-check:web`);
    expect(container.textContent).toContain('Run command');
  });

  it('localizes structured Page risk details and hides persistent approval', () => {
    const pageRequest = {
      ...request(false),
      action: 'page_publish',
      resources: ['page:demo; visibility=private; deploy=saved-version-only'],
      saveResources: [],
      displayMetadata: {
        pageOperation: 'save',
        pageSlug: 'demo',
        pageVisibility: 'private',
        requiresFreshApproval: true,
      },
    };
    act(() => {
      root.render(
        <PermissionRequestPanel
          requests={[pageRequest]}
          onRespond={vi.fn()}
          onRespondBatch={vi.fn()}
        />,
      );
    });

    expect(container.textContent).toContain('Save demo as permission.visibility.private without deploying.');
    expect([...container.querySelectorAll('button')]
      .some((button) => button.textContent?.includes('permission.allowAlways'))).toBe(false);
  });

  it('uses friendly labels for every recognized permission action', () => {
    const expectedActions = [
      ['read', 'Read files'],
      ['edit', 'Edit files'],
      ['bash', 'Run command'],
      ['git', 'Git action'],
      ['computer_use', 'Control device'],
      ['websearch', 'Search web'],
      ['webfetch', 'Access web'],
      ['mcp', 'MCP tool'],
      ['task', 'Run task'],
      ['skill', 'Run skill'],
      ['page_publish', 'Save page version'],
      ['page_deploy', 'Deploy page version'],
      ['custom_tool', 'External tool'],
      ['external_directory', 'Access external directory'],
      ['future_action', 'Other action'],
    ] as const;

    act(() => {
      root.render(
        <PermissionRequestPanel
          requests={expectedActions.map(([action], index) => ({
            ...request(false),
            requestId: `request-${index}`,
            action,
          }))}
          onRespond={vi.fn()}
          onRespondBatch={vi.fn()}
        />,
      );
    });

    expect([...container.querySelectorAll('.permission-request-panel__action')]
      .map((element) => element.textContent))
      .toEqual(expectedActions.map(([, label]) => label));
  });

  it('shows one ordered batch and responds to the current and following requests once', async () => {
    const first = request(false);
    const second = { ...request(false), requestId: 'second-request', order: 1 };
    const onRespondBatch = vi.fn(() => Promise.resolve());
    await act(async () => {
      root.render(
        <PermissionRequestPanel
          requests={[first, second]}
          onRespond={vi.fn()}
          onRespondBatch={onRespondBatch}
        />,
      );
    });

    const batchButton = [...container.querySelectorAll('button')].find(
      (button) => button.textContent?.includes('permission.allowCurrentAndFollowing'),
    );
    expect(batchButton).toBeDefined();
    await act(async () => {
      batchButton?.click();
      await Promise.resolve();
    });

    expect(onRespondBatch).toHaveBeenCalledWith(first.requestId, 'once', undefined);
    expect(container.querySelectorAll('[role="listitem"]')).toHaveLength(2);
  });

  it('collapses to an anchored permission indicator and reopens it with the session pending count', () => {
    act(() => {
      root.render(
        <PermissionRequestPanel
          requests={[request(false)]}
          totalPendingCount={3}
          onRespond={vi.fn()}
          onRespondBatch={vi.fn()}
        />,
      );
    });

    const collapseButton = container.querySelector<HTMLButtonElement>(
      '[data-testid="permission-request-panel-collapse"]',
    );
    expect(collapseButton?.getAttribute('aria-expanded')).toBe('true');

    act(() => collapseButton?.click());

    expect(container.querySelector('.permission-request-panel')).toBeNull();
    const expandButton = container.querySelector<HTMLButtonElement>(
      '[data-testid="permission-request-panel-expand"]',
    );
    expect(expandButton?.textContent).toContain('3');
    expect(expandButton?.getAttribute('aria-label')).toBe('Expand 3 pending permission requests');
    expect(expandButton?.getAttribute('aria-expanded')).toBe('false');

    act(() => expandButton?.click());

    expect(container.querySelector('.permission-request-panel')).not.toBeNull();
  });

  it('expands again when a newly active permission batch replaces a collapsed one', () => {
    const first = request(false);
    const nextBatch = { ...first, requestId: 'next-request', roundId: 'next-round' };
    act(() => {
      root.render(
        <PermissionRequestPanel
          key={`${first.sessionId}:${first.roundId}`}
          requests={[first]}
          onRespond={vi.fn()}
          onRespondBatch={vi.fn()}
        />,
      );
    });

    act(() => {
      container.querySelector<HTMLButtonElement>(
        '[data-testid="permission-request-panel-collapse"]',
      )?.click();
    });
    expect(container.querySelector('[data-testid="permission-request-panel-expand"]')).not.toBeNull();

    act(() => {
      root.render(
        <PermissionRequestPanel
          key={`${nextBatch.sessionId}:${nextBatch.roundId}`}
          requests={[nextBatch]}
          onRespond={vi.fn()}
          onRespondBatch={vi.fn()}
        />,
      );
    });

    expect(container.querySelector('.permission-request-panel')).not.toBeNull();
  });
});
