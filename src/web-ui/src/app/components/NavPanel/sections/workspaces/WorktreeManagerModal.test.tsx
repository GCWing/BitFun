/**
 * @vitest-environment jsdom
 */

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { WorktreeSummary } from '@/infrastructure/api/service-api/WorktreeAPI';
import { WorktreeManagerModal } from './WorktreeManagerModal';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  getConfig: vi.fn(),
  remove: vi.fn(),
  refresh: vi.fn(async () => undefined),
  revealInExplorer: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
}));

vi.mock('@/infrastructure/api', () => ({
  configAPI: { getConfig: mocks.getConfig },
  workspaceAPI: { revealInExplorer: mocks.revealInExplorer },
  worktreeAPI: {
    createBranch: vi.fn(),
    promote: vi.fn(),
    recreate: vi.fn(),
    remove: mocks.remove,
  },
}));

vi.mock('@/infrastructure/api/service-api/WorktreeAPI', () => ({
  WorktreeCommandError: class WorktreeCommandError extends Error {
    constructor(
      public readonly code: string,
      message: string,
      public readonly recoveryPath?: string,
    ) {
      super(message);
    }
  },
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string, values?: Record<string, unknown>) =>
      values ? `${key}:${JSON.stringify(values)}` : key,
  }),
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    success: mocks.success,
    error: mocks.error,
  },
}));

vi.mock('@/component-library', () => ({
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
  ConfirmDialog: ({
    isOpen,
    title,
    message,
    preview,
    confirmText,
    onConfirm,
  }: {
    isOpen: boolean;
    title: React.ReactNode;
    message: React.ReactNode;
    preview?: React.ReactNode;
    confirmText: React.ReactNode;
    onConfirm: () => void;
  }) => isOpen ? (
    <section data-testid="confirm-dialog">
      <h2>{title}</h2>
      {message}
      <code>{preview}</code>
      <button data-testid="confirm-remove" type="button" onClick={onConfirm}>
        {confirmText}
      </button>
    </section>
  ) : null,
  InputDialog: () => null,
  Modal: ({
    children,
    isOpen,
    title,
  }: {
    children: React.ReactNode;
    isOpen: boolean;
    title: React.ReactNode;
  }) => isOpen ? <section><h1>{title}</h1>{children}</section> : null,
}));

function summary(overrides: Partial<WorktreeSummary> = {}): WorktreeSummary {
  return {
    worktreeId: 'wt-1',
    projectWorkspacePath: '/repo',
    path: '/managed/wt-1',
    head: '0123456789abcdef',
    lifecycle: 'managed',
    isMain: false,
    dirty: false,
    locked: false,
    missing: false,
    hasUnpublishedCommits: false,
    associatedSessionCount: 0,
    runningSessionCount: 0,
    sessions: [],
    ...overrides,
  };
}

describe('WorktreeManagerModal removal safety', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mocks.getConfig.mockResolvedValue({ branchPrefix: 'bitfun/' });
    mocks.remove.mockResolvedValue({ worktreeId: 'wt-1', removed: true });
    mocks.refresh.mockClear();
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.clearAllMocks();
  });

  async function openRemoveDialog(worktree: WorktreeSummary): Promise<void> {
    await act(async () => {
      root.render(
        <WorktreeManagerModal
          isOpen
          projectWorkspacePath="/repo"
          worktrees={[worktree]}
          loading={false}
          onClose={vi.fn()}
          onRefresh={mocks.refresh}
          onCreateWorktree={vi.fn()}
          onCreateSession={vi.fn(async () => undefined)}
        />
      );
      await Promise.resolve();
    });
    const item = container.querySelector('[data-worktree-id="wt-1"]');
    const removeButton = Array.from(item?.querySelectorAll('button') ?? [])
      .find(button => button.textContent?.includes('manager.remove'));
    await act(async () => {
      removeButton?.click();
    });
  }

  it('lists each loss risk and requires a second confirmation before force removal', async () => {
    await openRemoveDialog(summary({
      dirty: true,
      hasUnpublishedCommits: true,
      associatedSessionCount: 2,
    }));

    expect(container.textContent).toContain('manager.risks.dirty');
    expect(container.textContent).toContain('manager.risks.unpublished');
    expect(container.textContent).toContain('manager.risks.sessions');

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="confirm-remove"]')?.click();
    });
    expect(mocks.remove).not.toHaveBeenCalled();
    expect(container.textContent).toContain('manager.removeDialog.forceTitle');

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="confirm-remove"]')?.click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(mocks.remove).toHaveBeenCalledWith(
      '/repo',
      'wt-1',
      expect.any(String),
      true,
    );
    expect(mocks.refresh).toHaveBeenCalledOnce();
  });

  it('never offers force removal while a session remains unarchived', async () => {
    await openRemoveDialog(summary({
      associatedSessionCount: 1,
      runningSessionCount: 1,
    }));

    expect(container.textContent).toContain('manager.removeDialog.blocked');
    expect(container.textContent).toContain('manager.risks.running');
    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="confirm-remove"]')?.click();
    });
    expect(mocks.remove).not.toHaveBeenCalled();
    expect(container.querySelector('[data-testid="confirm-dialog"]')).toBeNull();
  });
});
