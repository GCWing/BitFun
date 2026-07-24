// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { SSHConnectionDialog } from './SSHConnectionDialog';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const sshApiMock = vi.hoisted(() => ({
  listSavedConnections: vi.fn(),
  listSSHConfigHosts: vi.fn(),
  getSSHConfig: vi.fn(),
}));

const remoteContextMock = vi.hoisted(() => ({
  connect: vi.fn(),
  clearError: vi.fn(),
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('./SSHRemoteContext', () => ({
  useSSHRemoteContext: () => ({
    connect: remoteContextMock.connect,
    status: 'disconnected',
    connectionError: null,
    clearError: remoteContextMock.clearError,
  }),
}));

vi.mock('./sshApi', () => ({
  sshApi: sshApiMock,
}));

vi.mock('./pickSshPrivateKeyPath', () => ({
  pickSshPrivateKeyPath: vi.fn(),
}));

vi.mock('./SSHAuthPromptDialog', () => ({
  SSHAuthPromptDialog: () => null,
}));

vi.mock('@/component-library', () => ({
  Modal: ({
    isOpen,
    children,
  }: React.PropsWithChildren<{ isOpen: boolean }>) => isOpen ? <div>{children}</div> : null,
  Button: ({
    children,
    onClick,
    disabled,
    title,
    className,
  }: React.PropsWithChildren<{
    onClick?: React.MouseEventHandler<HTMLButtonElement>;
    disabled?: boolean;
    title?: string;
    className?: string;
  }>) => (
    <button type="button" onClick={onClick} disabled={disabled} title={title} className={className}>
      {children}
    </button>
  ),
  IconButton: ({
    children,
    onClick,
    disabled,
  }: React.PropsWithChildren<{
    onClick?: React.MouseEventHandler<HTMLButtonElement>;
    disabled?: boolean;
  }>) => (
    <button type="button" onClick={onClick} disabled={disabled}>
      {children}
    </button>
  ),
  Input: ({
    label,
    value,
    onChange,
    className,
  }: {
    label?: string;
    value?: string;
    onChange?: React.ChangeEventHandler<HTMLInputElement>;
    className?: string;
  }) => (
    <label className={className}>
      {label}
      <input aria-label={label} value={value} onChange={onChange} />
    </label>
  ),
  Select: ({
    options,
    value,
    onChange,
  }: {
    options: Array<{ label: string; value: string }>;
    value: string;
    onChange: (value: string) => void;
  }) => (
    <select value={value} onChange={(event) => onChange(event.target.value)}>
      {options.map((option) => (
        <option key={option.value} value={option.value}>{option.label}</option>
      ))}
    </select>
  ),
  Alert: () => null,
}));

describe('SSHConnectionDialog advanced settings', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    sshApiMock.listSavedConnections.mockResolvedValue([]);
    sshApiMock.listSSHConfigHosts.mockResolvedValue([]);
    sshApiMock.getSSHConfig.mockResolvedValue({ found: false });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  async function renderDialog(): Promise<void> {
    await act(async () => {
      root.render(<SSHConnectionDialog open onClose={vi.fn()} />);
    });
    await act(async () => {
      await Promise.resolve();
    });
  }

  it('keeps optional connection fields collapsed for a new connection', async () => {
    await renderDialog();

    const toggle = container.querySelector<HTMLButtonElement>('button[aria-expanded]');
    expect(toggle?.getAttribute('aria-expanded')).toBe('false');
    expect(container.querySelector('input[aria-label="ssh.remote.connectionName"]')).toBeNull();
    expect(container.querySelector('input[aria-label="ssh.remote.proxyJump"]')).toBeNull();

    act(() => {
      toggle?.click();
    });

    expect(toggle?.getAttribute('aria-expanded')).toBe('true');
    expect(container.querySelector('input[aria-label="ssh.remote.connectionName"]')).not.toBeNull();
    expect(container.querySelector('input[aria-label="ssh.remote.proxyJump"]')).not.toBeNull();
    expect(container.querySelector('input[aria-label="ssh.remote.connectTimeout"]')).not.toBeNull();
  });

  it('reveals non-default settings when editing an existing connection', async () => {
    sshApiMock.listSavedConnections.mockResolvedValue([
      {
        id: 'ssh-dev@example.test',
        name: 'Development',
        host: 'example.test',
        port: 22,
        username: 'dev',
        authType: { type: 'PrivateKey', keyPath: '/keys/dev' },
        proxyJump: 'jump.example.test',
        options: {
          connectTimeoutSecs: 45,
          authTimeoutSecs: 60,
          authAttempts: 3,
          connectAttempts: 2,
        },
      },
    ]);

    await renderDialog();
    const editButton = container.querySelector<HTMLButtonElement>('button[title="actions.edit"]');

    act(() => {
      editButton?.click();
    });

    const toggle = container.querySelector<HTMLButtonElement>('button[aria-expanded]');
    expect(toggle?.getAttribute('aria-expanded')).toBe('true');
    expect(
      container.querySelector<HTMLInputElement>('input[aria-label="ssh.remote.proxyJump"]')?.value
    ).toBe('jump.example.test');
    expect(
      container.querySelector<HTMLInputElement>('input[aria-label="ssh.remote.connectAttempts"]')?.value
    ).toBe('2');
  });
});
