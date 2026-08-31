/** @vitest-environment jsdom */

import React, { forwardRef, useRef, useState } from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { BranchQuickSwitch } from './BranchQuickSwitch';

const mocks = vi.hoisted(() => ({
  addFiles: vi.fn(),
  checkoutBranch: vi.fn(),
  commit: vi.fn(),
  emit: vi.fn(),
  getBranches: vi.fn(),
  getDiff: vi.fn(),
  getState: vi.fn(),
  notificationError: vi.fn(),
  notificationSuccess: vi.fn(),
  refresh: vi.fn(async () => undefined),
}));

vi.mock('@bitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@bitfun/ui')>(),
  Button: forwardRef<HTMLButtonElement, React.ButtonHTMLAttributes<HTMLButtonElement> & {
    loading?: boolean;
    variant?: string;
  }>(({
    children,
    loading: _loading,
    variant: _variant,
    ...props
  }, ref) => <button ref={ref} type="button" {...props}>{children}</button>),
  Icon: ({ name }: { name: string }) => <span data-icon={name} />,
  Input: forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>(
    (props, ref) => <input ref={ref} {...props} />,
  ),
  Dialog: ({
    children,
    open,
    'data-testid': testId,
  }: React.PropsWithChildren<{ open: boolean; 'data-testid'?: string }>) => open ? (
    <div role="dialog" data-testid={testId}>{children}</div>
  ) : null,
  DialogBody: ({ children }: React.PropsWithChildren) => <div>{children}</div>,
  DialogClose: (props: React.ButtonHTMLAttributes<HTMLButtonElement>) => <button type="button" {...props} />,
  DialogFooter: ({ children }: React.PropsWithChildren) => <footer>{children}</footer>,
  DialogHeader: ({ children }: React.PropsWithChildren) => <header>{children}</header>,
  DialogHeading: ({ children }: React.PropsWithChildren) => <div>{children}</div>,
  DialogTitle: ({ children }: React.PropsWithChildren) => <h2>{children}</h2>,
  ScrollArea: forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
    ({ children, ...props }, ref) => <div ref={ref} {...props}>{children}</div>,
  ),
}));

vi.mock('@/infrastructure/appearance/runtime/AppearanceOverlayHost', () => ({
  getAppearanceOverlayHost: () => document.body,
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string, options?: Record<string, string>) => {
      const labels: Record<string, string> = {
        'quickSwitch.conflict.cancel': 'Cancel',
        'quickSwitch.conflict.commitAction': 'Commit and switch',
        'quickSwitch.conflict.commitAndSwitch': 'Commit and switch branch…',
        'quickSwitch.conflict.commitDescription': `Commit all changes before ${options?.branch ?? ''}`,
        'quickSwitch.conflict.commitMessageLabel': 'Commit message',
        'quickSwitch.conflict.commitMessagePlaceholder': 'Enter commit message...',
        'quickSwitch.conflict.commitTitle': `Commit and switch to ${options?.branch ?? ''}`,
        'quickSwitch.conflict.description': 'These files would be overwritten:',
        'quickSwitch.conflict.instruction': 'Please commit to continue.',
        'quickSwitch.conflict.retryFailed': `Retry failed: ${options?.error ?? ''}`,
        'quickSwitch.conflict.retrySwitchAction': 'Retry switch',
        'quickSwitch.conflict.title': 'Commit changes to switch branch',
        'quickSwitch.menuLabel': 'Switch branch',
        'quickSwitch.searchLabel': 'Search branches',
      };
      return labels[key] ?? key;
    },
  }),
}));

vi.mock('@/shared/notification-system/services/NotificationService', () => ({
  notificationService: {
    error: mocks.notificationError,
    success: mocks.notificationSuccess,
  },
}));

vi.mock('@/shared/utils/useAnchoredPopoverPosition', () => ({
  useAnchoredPopoverPosition: () => ({ top: 10, left: 12, placement: 'top' }),
}));

vi.mock('@/tools/git/services', () => ({
  gitEventService: { emit: mocks.emit },
  gitService: {
    addFiles: mocks.addFiles,
    checkoutBranch: mocks.checkoutBranch,
    commit: mocks.commit,
    getBranches: mocks.getBranches,
    getDiff: mocks.getDiff,
  },
}));

vi.mock('@/tools/git/state/GitStateManager', () => ({
  gitStateManager: {
    getState: mocks.getState,
    refresh: mocks.refresh,
  },
}));

const branches = [
  { name: 'main', current: true, remote: false, ahead: 0, behind: 0 },
  { name: 'feature', current: false, remote: false, ahead: 0, behind: 0 },
];

function Harness({ onSwitchSuccess }: { onSwitchSuccess: (branch: string) => void }) {
  const [open, setOpen] = useState(true);
  const anchorRef = useRef<HTMLButtonElement>(null);
  return (
    <>
      <button ref={anchorRef} type="button">branch</button>
      <BranchQuickSwitch
        isOpen={open}
        onClose={() => setOpen(false)}
        repositoryPath="/repo"
        currentBranch="main"
        anchorRef={anchorRef}
        onSwitchSuccess={onSwitchSuccess}
      />
    </>
  );
}

describe('BranchQuickSwitch', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean })
      .IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    vi.clearAllMocks();
    mocks.getState.mockReturnValue({ branches, staged: [], unstaged: [], untracked: [], conflicts: [] });
    mocks.getBranches.mockResolvedValue(branches);
    mocks.getDiff.mockResolvedValue('');
    mocks.addFiles.mockResolvedValue({ success: true });
    mocks.commit.mockResolvedValue({ success: true });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.querySelectorAll('[data-testid="branch-quick-switch"], [data-testid="branch-switch-conflict-dialog"], [data-testid="branch-switch-commit-dialog"]')
      .forEach(node => node.remove());
  });

  it('keeps the design-system search field as one labeled visual surface', async () => {
    await act(async () => {
      root.render(<Harness onSwitchSuccess={vi.fn()} />);
    });

    const searchField = document.querySelector('[data-bf-component="search-field"]');
    const fieldSurface = searchField?.querySelector('[data-bf-component="input"]');
    const input = searchField?.querySelector<HTMLInputElement>('input[type="search"]');

    expect(searchField).not.toBeNull();
    expect(fieldSurface).not.toBeNull();
    expect(input?.getAttribute('aria-label')).toBe('Search branches');
    expect(input?.classList.contains('branch-quick-switch__input')).toBe(false);
  });

  it('checks out a selected branch and publishes the shared branch-change event', async () => {
    const onSwitchSuccess = vi.fn();
    mocks.checkoutBranch.mockResolvedValue({ success: true });

    await act(async () => {
      root.render(<Harness onSwitchSuccess={onSwitchSuccess} />);
    });
    await vi.waitFor(() => expect(
      document.querySelector('[data-testid="branch-quick-switch-option-feature"]'),
    ).not.toBeNull());

    await act(async () => {
      document.querySelector<HTMLButtonElement>(
        '[data-testid="branch-quick-switch-option-feature"]',
      )?.click();
    });

    expect(mocks.checkoutBranch).toHaveBeenCalledWith('/repo', 'feature');
    expect(mocks.emit).toHaveBeenCalledWith('branch:changed', expect.objectContaining({
      repositoryPath: '/repo',
      branch: expect.objectContaining({ name: 'feature', current: true }),
    }));
    expect(onSwitchSuccess).toHaveBeenCalledWith('feature');
    expect(document.querySelector('[data-testid="branch-quick-switch"]')).toBeNull();
  });

  it('shows the blocking files, then commits and retries the intended switch', async () => {
    const onSwitchSuccess = vi.fn();
    mocks.checkoutBranch
      .mockResolvedValueOnce({
        success: false,
        error: [
          'error: Your local changes to the following files would be overwritten by checkout:',
          '\tSECURITY.md',
          'Please commit your changes or stash them before you switch branches.',
          'Aborting',
        ].join('\n'),
      })
      .mockResolvedValueOnce({ success: true });
    mocks.getDiff
      .mockResolvedValueOnce([
        'diff --git a/SECURITY.md b/SECURITY.md',
        '--- a/SECURITY.md',
        '+++ b/SECURITY.md',
        '@@ -1 +1 @@',
        '-old',
        '+new',
      ].join('\n'))
      .mockResolvedValueOnce('');

    await act(async () => {
      root.render(<Harness onSwitchSuccess={onSwitchSuccess} />);
    });
    await vi.waitFor(() => expect(
      document.querySelector('[data-testid="branch-quick-switch-option-feature"]'),
    ).not.toBeNull());

    await act(async () => {
      document.querySelector<HTMLButtonElement>(
        '[data-testid="branch-quick-switch-option-feature"]',
      )?.click();
    });

    await vi.waitFor(() => {
      const dialog = document.querySelector('[data-testid="branch-switch-conflict-dialog"]');
      expect(dialog?.textContent).toContain('SECURITY.md');
      expect(dialog?.textContent).toContain('+1');
      expect(dialog?.textContent).toContain('−1');
    });

    await act(async () => {
      document.querySelector<HTMLButtonElement>('[data-testid="branch-switch-conflict-confirm"]')?.click();
    });
    expect(document.querySelector('[data-testid="branch-switch-commit-dialog"]')).not.toBeNull();

    const messageInput = document.querySelector<HTMLInputElement>('input[aria-label="Commit message"]');
    await act(async () => {
      if (messageInput) {
        const valueSetter = Object.getOwnPropertyDescriptor(
          HTMLInputElement.prototype,
          'value',
        )?.set;
        valueSetter?.call(messageInput, 'Save work before branch switch');
        messageInput.dispatchEvent(new Event('input', { bubbles: true }));
      }
    });

    await act(async () => {
      document.querySelector<HTMLButtonElement>(
        '[data-testid="branch-switch-commit-confirm"]',
      )?.click();
    });

    expect(mocks.addFiles).toHaveBeenCalledWith('/repo', { files: [], all: true });
    expect(mocks.commit).toHaveBeenCalledWith('/repo', {
      message: 'Save work before branch switch',
    });
    expect(mocks.checkoutBranch).toHaveBeenCalledTimes(2);
    expect(mocks.checkoutBranch).toHaveBeenLastCalledWith('/repo', 'feature');
    expect(onSwitchSuccess).toHaveBeenCalledWith('feature');
    expect(document.querySelector('[data-testid="branch-switch-commit-dialog"]')).toBeNull();
  });

  it('retries checkout without creating a duplicate commit', async () => {
    const onSwitchSuccess = vi.fn();
    mocks.checkoutBranch
      .mockResolvedValueOnce({
        success: false,
        error: [
          'error: Your local changes to the following files would be overwritten by checkout:',
          '\tSECURITY.md',
          'Please commit your changes or stash them before you switch branches.',
          'Aborting',
        ].join('\n'),
      })
      .mockResolvedValueOnce({ success: false, error: 'remote temporarily unavailable' })
      .mockResolvedValueOnce({ success: true });

    await act(async () => {
      root.render(<Harness onSwitchSuccess={onSwitchSuccess} />);
    });
    await vi.waitFor(() => expect(
      document.querySelector('[data-testid="branch-quick-switch-option-feature"]'),
    ).not.toBeNull());

    await act(async () => {
      document.querySelector<HTMLButtonElement>(
        '[data-testid="branch-quick-switch-option-feature"]',
      )?.click();
    });
    await vi.waitFor(() => expect(
      document.querySelector('[data-testid="branch-switch-conflict-dialog"]'),
    ).not.toBeNull());
    await act(async () => {
      document.querySelector<HTMLButtonElement>(
        '[data-testid="branch-switch-conflict-confirm"]',
      )?.click();
    });

    const messageInput = document.querySelector<HTMLInputElement>('input[aria-label="Commit message"]');
    await act(async () => {
      if (messageInput) {
        const valueSetter = Object.getOwnPropertyDescriptor(
          HTMLInputElement.prototype,
          'value',
        )?.set;
        valueSetter?.call(messageInput, 'Checkpoint before retry');
        messageInput.dispatchEvent(new Event('input', { bubbles: true }));
      }
    });
    await act(async () => {
      document.querySelector<HTMLButtonElement>(
        '[data-testid="branch-switch-commit-confirm"]',
      )?.click();
    });

    await vi.waitFor(() => expect(mocks.checkoutBranch).toHaveBeenCalledTimes(2));
    expect(mocks.commit).toHaveBeenCalledTimes(1);
    expect(document.querySelector('[data-testid="branch-switch-commit-dialog"]')).not.toBeNull();
    expect(document.querySelector('[role="alert"]')?.textContent)
      .toContain('remote temporarily unavailable');

    await act(async () => {
      document.querySelector<HTMLButtonElement>(
        '[data-testid="branch-switch-commit-confirm"]',
      )?.click();
    });

    await vi.waitFor(() => expect(mocks.checkoutBranch).toHaveBeenCalledTimes(3));
    expect(mocks.commit).toHaveBeenCalledTimes(1);
    expect(onSwitchSuccess).toHaveBeenCalledWith('feature');
  });
});
