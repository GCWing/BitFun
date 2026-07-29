// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import WorktreesConfig from './WorktreesConfig';

const getConfigMock = vi.hoisted(() => vi.fn());
const setConfigMock = vi.hoisted(() => vi.fn());
const listProjectsMock = vi.hoisted(() => vi.fn());
const removeMock = vi.hoisted(() => vi.fn());
const onChangedMock = vi.hoisted(() => vi.fn(() => vi.fn()));
const translateMock = vi.hoisted(() => vi.fn(
  (key: string, params?: Record<string, unknown>) => {
    if (key === 'labels.detached') return `detached ${params?.commit}`;
    if (key === 'management.deleted') return `deleted ${params?.path}`;
    return key;
  },
));

vi.mock('@/infrastructure/api', () => ({
  configAPI: {
    getConfig: getConfigMock,
    setConfig: setConfigMock,
  },
  worktreeAPI: {
    listProjects: listProjectsMock,
    remove: removeMock,
    onChanged: onChangedMock,
  },
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: translateMock,
  }),
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
  ConfigPageLoading: ({ text }: { text: string }) => <div>{text}</div>,
  ConfigPageMessage: ({
    message,
  }: {
    message: { text: string } | null;
  }) => message ? <div>{message.text}</div> : null,
  ConfigPageRefreshButton: ({
    onClick,
  }: {
    onClick: () => void;
  }) => <button type="button" onClick={onClick}>refresh</button>,
  ConfirmDialog: ({
    confirmText,
    isOpen,
    message,
    onConfirm,
    title,
  }: {
    confirmText: string;
    isOpen: boolean;
    message: React.ReactNode;
    onConfirm: () => void;
    title: string;
  }) => isOpen ? (
    <div role="dialog">
      <h2>{title}</h2>
      <div>{message}</div>
      <button type="button" data-testid="confirm-delete" onClick={onConfirm}>
        {confirmText}
      </button>
    </div>
  ) : null,
  Input: (props: React.InputHTMLAttributes<HTMLInputElement>) => <input {...props} />,
  NumberInput: ({
    disabled,
    label,
    onChange,
    value,
  }: {
    disabled?: boolean;
    label?: string;
    onChange: (value: number) => void;
    value: number;
  }) => (
    <input
      aria-label={label}
      disabled={disabled}
      type="number"
      value={value}
      onChange={event => onChange(Number(event.currentTarget.value))}
    />
  ),
  Switch: ({
    checked,
    disabled,
    onChange,
  }: {
    checked: boolean;
    disabled?: boolean;
    onChange: React.ChangeEventHandler<HTMLInputElement>;
  }) => (
    <input
      checked={checked}
      disabled={disabled}
      type="checkbox"
      onChange={onChange}
    />
  ),
}));

vi.mock('./common', () => ({
  ConfigPageContent: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  ConfigPageHeader: ({ title, subtitle }: { title: string; subtitle: string }) => (
    <header>
      <h1>{title}</h1>
      <p>{subtitle}</p>
    </header>
  ),
  ConfigPageLayout: ({ children }: { children: React.ReactNode }) => <main>{children}</main>,
  ConfigPageRow: ({
    children,
    description,
    label,
  }: {
    children: React.ReactNode;
    description?: React.ReactNode;
    label: React.ReactNode;
  }) => (
    <label>
      <span>{label}</span>
      <span>{description}</span>
      {children}
    </label>
  ),
  ConfigPageSection: ({
    children,
    description,
    extra,
    title,
  }: {
    children: React.ReactNode;
    description?: React.ReactNode;
    extra?: React.ReactNode;
    title: string;
  }) => (
    <section>
      <h2>{title}</h2>
      <p>{description}</p>
      {extra}
      {children}
    </section>
  ),
}));

function worktree(overrides: Record<string, unknown> = {}) {
  return {
    worktreeId: 'wt-1',
    projectWorkspacePath: '/repo',
    path: '/managed/BitFun-wt-1',
    head: '0123456789abcdef',
    lifecycle: 'managed',
    isMain: false,
    dirty: false,
    locked: false,
    missing: false,
    hasUnpublishedCommits: false,
    associatedSessionCount: 1,
    runningSessionCount: 0,
    sessions: [{
      sessionId: 'session-1',
      sessionName: 'Ship worktree management',
      status: 'archived',
      archived: true,
    }],
    ...overrides,
  };
}

async function flushPromises() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('WorktreesConfig', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
      .IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    vi.clearAllMocks();

    getConfigMock.mockResolvedValue({
      rootPath: '/custom/worktrees',
      branchPrefix: 'custom/',
      copyLocalChanges: false,
    });
    setConfigMock.mockResolvedValue(undefined);
    listProjectsMock.mockResolvedValue([{
      projectWorkspacePath: '/repo',
      worktrees: [worktree()],
    }]);
    removeMock.mockResolvedValue({ worktreeId: 'wt-1', removed: true });
    onChangedMock.mockReturnValue(vi.fn());
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('adds safe auto-delete defaults and renders workspace worktrees with sessions', async () => {
    await act(async () => {
      root.render(<WorktreesConfig />);
    });
    await flushPromises();

    const checkboxes = container.querySelectorAll<HTMLInputElement>('input[type="checkbox"]');
    const limit = container.querySelector<HTMLInputElement>('input[type="number"]');

    expect(checkboxes).toHaveLength(2);
    expect(checkboxes[1].checked).toBe(true);
    expect(limit?.value).toBe('15');
    expect(container.textContent).toContain('/repo');
    expect(container.textContent).toContain('/managed/BitFun-wt-1');
    expect(container.textContent).toContain('Ship worktree management');
  });

  it('saves the auto-delete policy with the existing worktree settings', async () => {
    await act(async () => {
      root.render(<WorktreesConfig />);
    });
    await flushPromises();

    const limit = container.querySelector<HTMLInputElement>('input[type="number"]');
    expect(limit).not.toBeNull();
    await act(async () => {
      if (limit) {
        const setValue = Object.getOwnPropertyDescriptor(
          HTMLInputElement.prototype,
          'value',
        )?.set;
        setValue?.call(limit, '24');
        limit.dispatchEvent(new Event('input', { bubbles: true }));
      }
    });

    const saveButton = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('settings.save'));
    await act(async () => {
      saveButton?.click();
      await Promise.resolve();
    });

    expect(setConfigMock).toHaveBeenCalledWith('app.worktrees', {
      rootPath: '/custom/worktrees',
      branchPrefix: 'custom/',
      copyLocalChanges: false,
      autoDeleteEnabled: true,
      autoDeleteLimit: 24,
    });
  });

  it('requires confirmation and uses force only when local work would be discarded', async () => {
    listProjectsMock.mockResolvedValueOnce([{
      projectWorkspacePath: '/repo',
      worktrees: [worktree({
        associatedSessionCount: 0,
        dirty: true,
        sessions: [],
      })],
    }]);

    await act(async () => {
      root.render(<WorktreesConfig />);
    });
    await flushPromises();

    const deleteButton = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('management.delete.action'));
    act(() => deleteButton?.click());

    expect(container.querySelector('[role="dialog"]')?.textContent)
      .toContain('management.delete.forceTitle');

    const confirmButton = container.querySelector<HTMLButtonElement>('[data-testid="confirm-delete"]');
    await act(async () => {
      confirmButton?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(removeMock).toHaveBeenCalledWith(
      '/repo',
      'wt-1',
      expect.any(String),
      true,
    );
  });

  it('disables deletion while a worktree still has associated archived sessions', async () => {
    listProjectsMock.mockResolvedValueOnce([{
      projectWorkspacePath: '/repo',
      worktrees: [worktree()],
    }]);

    await act(async () => {
      root.render(<WorktreesConfig />);
    });
    await flushPromises();

    const deleteButton = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('management.delete.action'));

    expect(deleteButton?.disabled).toBe(true);
    expect(container.textContent).toContain('management.protection.associatedSessions');
  });
});
