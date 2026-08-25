// @vitest-environment jsdom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
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

const authFilePickerMock = vi.hoisted(() => ({
  pickSshPrivateKeyPath: vi.fn(),
  pickSshCertificatePath: vi.fn(),
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
  ...authFilePickerMock,
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
    className,
    title,
    'aria-label': ariaLabel,
  }: React.PropsWithChildren<{
    onClick?: React.MouseEventHandler<HTMLButtonElement>;
    disabled?: boolean;
    className?: string;
    title?: string;
    'aria-label'?: string;
  }>) => (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={className}
      title={title}
      aria-label={ariaLabel}
    >
      {children}
    </button>
  ),
  Input: ({
    label,
    value,
    onChange,
    className,
    placeholder,
    suffix,
  }: {
    label?: string;
    value?: string;
    onChange?: React.ChangeEventHandler<HTMLInputElement>;
    className?: string;
    placeholder?: string;
    suffix?: React.ReactNode;
  }) => (
    <label className={className}>
      {label}
      <input aria-label={label} value={value} onChange={onChange} placeholder={placeholder} />
      {suffix}
    </label>
  ),
  Select: ({
    options,
    value,
    onChange,
    dropdownClassName,
  }: {
    options: Array<{ label: string; value: string }>;
    value: string;
    onChange: (value: string) => void;
    dropdownClassName?: string;
  }) => (
    <select
      value={value}
      onChange={(event) => onChange(event.target.value)}
      data-dropdown-class-name={dropdownClassName}
    >
      {options.map((option) => (
        <option key={option.value} value={option.value}>{option.label}</option>
      ))}
    </select>
  ),
  Alert: () => null,
}));

describe('SSHConnectionDialog', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', {
      configurable: true,
      value: vi.fn(),
    });
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    sshApiMock.listSavedConnections.mockResolvedValue([]);
    sshApiMock.listSSHConfigHosts.mockResolvedValue([]);
    sshApiMock.getSSHConfig.mockResolvedValue({ found: false });
    authFilePickerMock.pickSshPrivateKeyPath.mockResolvedValue(null);
    authFilePickerMock.pickSshCertificatePath.mockResolvedValue(null);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  async function renderDialog(onClose = vi.fn()): Promise<void> {
    await act(async () => {
      root.render(<SSHConnectionDialog open onClose={onClose} />);
    });
    await act(async () => {
      await Promise.resolve();
    });
  }

  function setInputValue(label: string, value: string): void {
    const input = container.querySelector<HTMLInputElement>(`input[aria-label="${label}"]`);
    expect(input).not.toBeNull();
    act(() => {
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
      setter?.call(input, value);
      input?.dispatchEvent(new Event('input', { bubbles: true }));
    });
  }

  function setSelectValue(select: HTMLSelectElement | null, value: string): void {
    expect(select).not.toBeNull();
    act(() => {
      const setter = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value')?.set;
      setter?.call(select, value);
      select?.dispatchEvent(new Event('change', { bubbles: true }));
    });
  }

  function findTargetSelect(): HTMLSelectElement | null {
    return Array.from(container.querySelectorAll<HTMLSelectElement>('select')).find((select) => (
      select.querySelector('option[value="localDocker"]') !== null
    )) ?? null;
  }

  function findConnectButton(): HTMLButtonElement | undefined {
    return Array.from(container.querySelectorAll<HTMLButtonElement>('button'))
      .find((button) => button.textContent?.includes('ssh.remote.connect'));
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

  it('keeps portalled select menus above the raised dialog overlay', async () => {
    await renderDialog();

    const selects = Array.from(container.querySelectorAll<HTMLSelectElement>('select'));
    expect(selects.length).toBeGreaterThan(0);
    expect(selects.every((select) => (
      select.dataset.dropdownClassName === 'ssh-connection-dialog__select-dropdown'
    ))).toBe(true);

    const stylesheet = readFileSync(
      resolve(process.cwd(), 'src/features/ssh-remote/SSHConnectionDialog.scss'),
      'utf8',
    );
    const overlayZIndex = Number(stylesheet.match(
      /\.ssh-connection-dialog__modal-overlay\s*\{[^}]*z-index:\s*(\d+)/,
    )?.[1]);
    const selectZIndex = Number(stylesheet.match(
      /\.select__dropdown\.ssh-connection-dialog__select-dropdown\s*\{[^}]*z-index:\s*(\d+)/,
    )?.[1]);

    expect(overlayZIndex).toBeGreaterThan(0);
    expect(selectZIndex).toBeGreaterThan(overlayZIndex);
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

  it('selects an OpenSSH certificate with the native file picker', async () => {
    authFilePickerMock.pickSshCertificatePath.mockResolvedValue('/keys/dev-cert.pub');
    sshApiMock.listSavedConnections.mockResolvedValue([
      {
        id: 'ssh-dev@example.test',
        name: 'dev@example.test',
        host: 'example.test',
        port: 22,
        username: 'dev',
        authType: { type: 'PrivateKey', keyPath: '/keys/dev' },
      },
    ]);
    await renderDialog();

    const editButton = container.querySelector<HTMLButtonElement>('button[title="actions.edit"]');
    act(() => {
      editButton?.click();
    });

    const browseCertificate = container.querySelector<HTMLButtonElement>(
      'button[aria-label="ssh.remote.browseCertificate"]'
    );
    expect(browseCertificate).not.toBeNull();
    await act(async () => {
      browseCertificate?.click();
      await Promise.resolve();
    });

    expect(authFilePickerMock.pickSshCertificatePath).toHaveBeenCalledWith({
      title: 'ssh.remote.pickCertificateDialogTitle',
    });
    expect(
      container.querySelector<HTMLInputElement>('input[aria-label="ssh.remote.certificatePath"]')?.value
    ).toBe('/keys/dev-cert.pub');
  });

  it('notifies the controlling surface after a successful connection', async () => {
    const onClose = vi.fn();
    remoteContextMock.connect.mockResolvedValue(undefined);
    await renderDialog(onClose);

    setInputValue('ssh.remote.host', 'example.test');
    setInputValue('ssh.remote.username', 'dev');
    setInputValue('ssh.remote.password', 'secret');

    const connectButton = findConnectButton();
    expect(connectButton).not.toBeUndefined();
    await act(async () => {
      connectButton?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(remoteContextMock.connect).toHaveBeenCalledWith(
      expect.any(String),
      expect.objectContaining({
        host: 'example.test',
        username: 'dev',
      }),
      { browseAfterConnect: true },
    );
    expect(remoteContextMock.connect.mock.calls[0]?.[1].container).toBeUndefined();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it.each([
    ['remote Docker', 'remoteDocker', false, 'auto'],
    ['local Docker', 'localDocker', true, 'auto'],
    ['container sshd', 'containerSshd', false, 'sshd'],
  ] as const)(
    'builds the expected connection config for %s',
    async (_label, targetType, local, access) => {
      remoteContextMock.connect.mockResolvedValue(undefined);
      await renderDialog();

      setSelectValue(findTargetSelect(), targetType);
      const containerNameInput = container.querySelector<HTMLInputElement>(
        'input[placeholder="ssh.remote.containerNamePlaceholder"]',
      );
      expect(containerNameInput).not.toBeNull();
      act(() => {
        const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
        setter?.call(containerNameInput, 'devbox');
        containerNameInput?.dispatchEvent(new Event('input', { bubbles: true }));
      });

      if (!local) {
        setInputValue('ssh.remote.host', 'example.test');
        setInputValue('ssh.remote.username', 'dev');
        setInputValue('ssh.remote.password', 'secret');
      }

      const connectButton = findConnectButton();
      expect(connectButton).not.toBeUndefined();
      await act(async () => {
        connectButton?.click();
        await Promise.resolve();
        await Promise.resolve();
      });

      const expectedId = local
        ? 'docker-local-devbox'
        : 'ssh-dev@example.test-container-devbox';
      expect(remoteContextMock.connect).toHaveBeenCalledWith(
        expectedId,
        expect.objectContaining({
          id: expectedId,
          host: local ? 'local-docker' : 'example.test',
          username: local ? 'docker' : 'dev',
          auth: local
            ? { type: 'PrivateKey', keyPath: '' }
            : { type: 'Password', password: 'secret' },
          container: {
            name: 'devbox',
            access,
            local,
            dockerPath: 'docker',
            shell: '/bin/sh',
            user: undefined,
            interactive: true,
          },
        }),
        { browseAfterConnect: true },
      );
    },
  );
});
