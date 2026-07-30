// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DispatchResultDialog } from './DispatchResultDialog';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  pullResult: vi.fn(),
  applyResult: vi.fn(),
  confirmWarning: vi.fn(),
}));

vi.mock('./dispatchApi', () => ({
  dispatchApi: {
    pullResult: mocks.pullResult,
    applyResult: mocks.applyResult,
  },
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock('@/component-library', () => ({
  Alert: ({ message }: { message: string }) => <div role="alert">{message}</div>,
  Button: ({
    children,
    disabled,
    onClick,
  }: React.PropsWithChildren<{ disabled?: boolean; onClick?: () => void }>) => (
    <button type="button" disabled={disabled} onClick={onClick}>
      {children}
    </button>
  ),
  Modal: ({ children, isOpen }: React.PropsWithChildren<{ isOpen: boolean }>) =>
    isOpen ? <div>{children}</div> : null,
  confirmWarning: mocks.confirmWarning,
}));

const BUNDLE = {
  bundlePath: '/root/.bitfun/dispatch/workspaces/job-1/result.tar.gz',
  localBundlePath: '/home/me/.bitfun/dispatch/outbound/.results/job-1.tar.gz',
  workspacePath: '/root/.bitfun/dispatch/workspaces/job-1/current',
  summary: {
    added: ['new.txt'],
    modified: ['edit.txt'],
    deleted: ['gone.txt'],
    baselineSha256: { 'edit.txt': 'a'.repeat(64), 'gone.txt': 'b'.repeat(64) },
    archiveSize: 1024,
    archiveSha256: 'c'.repeat(64),
  },
};

describe('DispatchResultDialog', () => {
  let container: HTMLDivElement;
  let root: Root;

  const buttons = () => Array.from(container.querySelectorAll('button'));
  const buttonWith = (text: string) =>
    buttons().find(button => button.textContent?.includes(text));

  const render = async (props?: Partial<React.ComponentProps<typeof DispatchResultDialog>>) => {
    await act(async () => {
      root.render(
        <DispatchResultDialog
          open
          jobId="job-1"
          workspacePath="/home/me/project"
          onClose={vi.fn()}
          {...props}
        />,
      );
      await Promise.resolve();
      await Promise.resolve();
    });
  };

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.pullResult.mockResolvedValue(BUNDLE);
    mocks.confirmWarning.mockResolvedValue(true);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('lists every change before anything can be applied', async () => {
    mocks.applyResult.mockResolvedValue({
      written: [],
      removed: [],
      conflicts: [],
      aborted: false,
    });
    await render();

    expect(mocks.pullResult).toHaveBeenCalledWith('job-1');
    expect(container.textContent).toContain('new.txt');
    expect(container.textContent).toContain('edit.txt');
    expect(container.textContent).toContain('gone.txt');
    // Pulling alone must never write.
    expect(mocks.applyResult).not.toHaveBeenCalled();

    await act(async () => {
      buttonWith('dispatch.resultApply')?.click();
      await Promise.resolve();
    });
    expect(mocks.applyResult).toHaveBeenCalledWith('job-1', '/home/me/project', false);
  });

  it('surfaces conflicts and requires an explicit confirmation to overwrite', async () => {
    mocks.applyResult.mockResolvedValueOnce({
      written: [],
      removed: [],
      conflicts: [{ path: 'edit.txt', reason: 'locallyModified' }],
      aborted: true,
    });
    await render();

    await act(async () => {
      buttonWith('dispatch.resultApply')?.click();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('dispatch.resultConflictWarning');
    expect(container.textContent).toContain('dispatch.resultConflictModified');
    // The plain apply is replaced by an explicit overwrite action.
    expect(buttonWith('dispatch.resultApply')).toBeUndefined();

    mocks.applyResult.mockResolvedValueOnce({
      written: ['edit.txt'],
      removed: [],
      conflicts: [],
      aborted: false,
    });
    await act(async () => {
      buttonWith('dispatch.resultOverwriteConfirm')?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.confirmWarning).toHaveBeenCalled();
    expect(mocks.applyResult).toHaveBeenLastCalledWith('job-1', '/home/me/project', true);
  });

  it('does not overwrite when the confirmation is declined', async () => {
    mocks.applyResult.mockResolvedValueOnce({
      written: [],
      removed: [],
      conflicts: [{ path: 'edit.txt', reason: 'locallyModified' }],
      aborted: true,
    });
    await render();
    await act(async () => {
      buttonWith('dispatch.resultApply')?.click();
      await Promise.resolve();
    });

    mocks.confirmWarning.mockResolvedValue(false);
    await act(async () => {
      buttonWith('dispatch.resultOverwriteConfirm')?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.applyResult).toHaveBeenCalledTimes(1);
    expect(mocks.applyResult).not.toHaveBeenCalledWith('job-1', '/home/me/project', true);
  });

  it('reports a job that changed nothing instead of offering an empty apply', async () => {
    mocks.pullResult.mockResolvedValue({
      ...BUNDLE,
      summary: {
        added: [],
        modified: [],
        deleted: [],
        baselineSha256: {},
        archiveSize: 64,
        archiveSha256: 'd'.repeat(64),
      },
    });
    await render();

    expect(container.textContent).toContain('dispatch.resultNoChanges');
    expect(buttonWith('dispatch.resultApply')?.disabled).toBe(true);
  });

  it('shows the pull failure rather than a blank dialog', async () => {
    mocks.pullResult.mockRejectedValue(new Error('target refused the result request'));
    await render();

    expect(container.textContent).toContain('target refused the result request');
    expect(buttonWith('dispatch.resultApply')?.disabled).toBe(true);
  });
});
