/**
 * @vitest-environment jsdom
 */

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { WorktreeLauncherModal } from './WorktreeLauncherModal';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  getRepositoryBasic: vi.fn(),
  getStatus: vi.fn(),
  getConfig: vi.fn(),
  resolveRevision: vi.fn(),
  t: (key: string, values?: Record<string, unknown>) =>
    values ? `${key}:${JSON.stringify(values)}` : key,
}));

vi.mock('@/infrastructure/api', () => ({
  configAPI: { getConfig: mocks.getConfig },
  gitAPI: {
    getRepositoryBasic: mocks.getRepositoryBasic,
    getStatus: mocks.getStatus,
    resolveRevision: mocks.resolveRevision,
  },
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: mocks.t,
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
  Checkbox: ({
    checked,
    disabled,
    label,
    description,
    onChange,
  }: {
    checked: boolean;
    disabled?: boolean;
    label: React.ReactNode;
    description?: React.ReactNode;
    onChange: React.ChangeEventHandler<HTMLInputElement>;
  }) => (
    <label>
      <input
        data-testid="copy-local-changes"
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={onChange}
      />
      {label}
      <span>{description}</span>
    </label>
  ),
  Input: (props: React.InputHTMLAttributes<HTMLInputElement>) => <input {...props} />,
  Modal: ({
    children,
    isOpen,
    title,
  }: {
    children: React.ReactNode;
    isOpen: boolean;
    title: React.ReactNode;
  }) => isOpen ? <section><h1>{title}</h1>{children}</section> : null,
  Select: ({
    id,
    value,
    disabled,
    options,
    onChange,
  }: {
    id?: string;
    value: string;
    disabled?: boolean;
    options: Array<{ value: string; label: string }>;
    onChange: (value: string) => void;
  }) => (
    <select
      id={id}
      value={value}
      disabled={disabled}
      onChange={event => onChange(event.target.value)}
    >
      {options.map(option => (
        <option key={option.value} value={option.value}>{option.label}</option>
      ))}
    </select>
  ),
}));

async function flushLauncherProbe(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    vi.advanceTimersByTime(200);
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('WorktreeLauncherModal', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mocks.getRepositoryBasic.mockResolvedValue({ current_branch: 'main' });
    mocks.getStatus.mockResolvedValue({
      staged: ['src/staged.ts'],
      unstaged: ['src/unstaged.ts'],
      untracked: ['notes.txt'],
      conflicts: [],
    });
    mocks.getConfig.mockResolvedValue({
      rootPath: '/managed',
      branchPrefix: 'bitfun/',
      defaultTarget: 'local',
      copyLocalChanges: true,
    });
    mocks.resolveRevision.mockResolvedValue('0123456789abcdef');
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('resolves the base and preserves the opt-in copy default only at source HEAD', async () => {
    const onSubmit = vi.fn(async () => undefined);
    await act(async () => {
      root.render(
        <WorktreeLauncherModal
          isOpen
          projectWorkspacePath="/repo"
          projectName="Repo"
          onClose={vi.fn()}
          onSubmit={onSubmit}
        />
      );
    });
    await flushLauncherProbe();

    const copy = container.querySelector<HTMLInputElement>('[data-testid="copy-local-changes"]');
    expect(copy?.disabled).toBe(false);
    expect(copy?.checked).toBe(true);
    expect(container.textContent).toContain('resolvedCommit');
    expect(container.textContent).toContain('/managed/Repo/…');

    const createButton = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('launcher.create'));
    await act(async () => {
      createButton?.click();
      await Promise.resolve();
    });
    expect(onSubmit).toHaveBeenCalledWith({
      mode: 'agentic',
      baseRef: 'main',
      copyLocalChanges: true,
    });
  });

  it('shows a clear unsupported state without probing a remote repository', async () => {
    await act(async () => {
      root.render(
        <WorktreeLauncherModal
          isOpen
          remote
          projectWorkspacePath="/remote/repo"
          projectName="Remote"
          onClose={vi.fn()}
          onSubmit={vi.fn(async () => undefined)}
        />
      );
    });

    expect(container.textContent).toContain('launcher.remoteUnsupported');
    expect(mocks.getRepositoryBasic).not.toHaveBeenCalled();
    const createButton = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('launcher.create'));
    expect(createButton?.disabled).toBe(true);
  });
});
