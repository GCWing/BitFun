// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DispatchResultDialog } from './DispatchResultDialog';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  syncResult: vi.fn(),
}));

vi.mock('./dispatchApi', () => ({
  dispatchApi: {
    syncResult: mocks.syncResult,
  },
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock('@bitfun/ui', () => ({
  Alert: ({ message }: { message: string }) => <div role="alert">{message}</div>,
  ScrollArea: ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>,
  Button: ({
    children,
    disabled,
    onClick,
  }: React.PropsWithChildren<{ disabled?: boolean; onClick?: () => void }>) => (
    <button type="button" disabled={disabled} onClick={onClick}>
      {children}
    </button>
  ),
  Dialog: ({ children, open }: React.PropsWithChildren<{ open: boolean }>) => (
    open ? <div role="dialog">{children}</div> : null
  ),
  DialogBody: ({ children }: React.PropsWithChildren) => <div>{children}</div>,
  DialogClose: (props: React.ButtonHTMLAttributes<HTMLButtonElement>) => <button type="button" {...props} />,
  DialogHeader: ({ children }: React.PropsWithChildren) => <header>{children}</header>,
}));

const SYNCED = {
  changed: true,
  branch: 'bitfun/dispatch/job-1',
  baseCommit: '0'.repeat(40),
  headCommit: '1'.repeat(40),
  commitCount: 2,
  changes: [
    { status: 'A', path: 'new.txt' },
    { status: 'M', path: 'src/main.ts' },
  ],
  truncatedChanges: false,
  baselineWorktreePath: '/home/me/.bitfun/worktrees/repo/dispatch-job-1',
  syncedHeadCommit: '1'.repeat(40),
};

describe('DispatchResultDialog Git sync', () => {
  let container: HTMLDivElement;
  let root: Root;

  const buttonWith = (text: string) =>
    Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes(text));

  async function render(
    props: Partial<React.ComponentProps<typeof DispatchResultDialog>> = {},
  ): Promise<void> {
    await act(async () => {
      root.render(
        <DispatchResultDialog
          open
          jobId="job-1"
          branch="bitfun/dispatch/job-1"
          baselineWorktreePath="/home/me/.bitfun/worktrees/repo/dispatch-job-1"
          targetLabel="build-host"
          onClose={vi.fn()}
          {...props}
        />,
      );
      await Promise.resolve();
    });
  }

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.syncResult.mockResolvedValue(SYNCED);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('keeps branch and save location in collapsed details before syncing', async () => {
    await render();

    expect(container.textContent).toContain('bitfun/dispatch/job-1');
    expect(container.textContent).toContain('/home/me/.bitfun/worktrees/repo/dispatch-job-1');
    expect(container.querySelector('details')?.open).toBe(false);
    expect(mocks.syncResult).not.toHaveBeenCalled();

    await act(async () => {
      buttonWith('dispatch.syncAction')?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.syncResult).toHaveBeenCalledWith('job-1');
    expect(container.textContent).toContain('dispatch.syncSucceeded');
    expect(container.textContent).toContain('new.txt');
    expect(container.textContent).toContain('src/main.ts');
    expect(container.textContent).toContain('1'.repeat(40));
  });

  it('reports a clean target branch without inventing an apply step', async () => {
    mocks.syncResult.mockResolvedValue({
      ...SYNCED,
      changed: false,
      headCommit: SYNCED.baseCommit,
      commitCount: 0,
      changes: [],
    });
    await render();

    await act(async () => {
      buttonWith('dispatch.syncAction')?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('dispatch.syncNoChanges');
    expect(container.textContent).not.toContain('dispatch.resultApply');
  });

  it('fails closed before transport when the managed baseline is missing', async () => {
    await render({ baselineMissing: true });

    expect(container.textContent).toContain('dispatch.syncBaselineMissing');
    expect(buttonWith('dispatch.syncAction')?.disabled).toBe(true);
    expect(mocks.syncResult).not.toHaveBeenCalled();
  });

  it('shows an actionable sync failure without exposing backend diagnostics', async () => {
    mocks.syncResult.mockRejectedValue(new Error('target worktree is locked'));
    await render();

    await act(async () => {
      buttonWith('dispatch.syncAction')?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('dispatch.syncFailed');
    expect(container.textContent).not.toContain('target worktree is locked');
  });
});
