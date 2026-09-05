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

vi.mock('@openbitfun/ui', () => ({
  Alert: () => null,
  Icon: ({ name, ...props }: { name: string } & React.HTMLAttributes<HTMLSpanElement>) => <span data-icon={name} {...props} />,
  Dialog: ({
    open,
    children,
  }: React.PropsWithChildren<{ open: boolean }>) => open ? <div role="dialog">{children}</div> : null,
  DialogBody: ({ children }: React.PropsWithChildren) => <div>{children}</div>,
  DialogClose: () => <button type="button" aria-label="Close" />,
  DialogHeader: ({ children }: React.PropsWithChildren) => <header>{children}</header>,
  DialogHeading: ({ children }: React.PropsWithChildren) => <div>{children}</div>,
  DialogTitle: ({ children }: React.PropsWithChildren) => <h2>{children}</h2>,
  Button: ({
    children,
    leadingIcon: _leadingIcon,
    loading: _loading,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement> & {
    leadingIcon?: React.ReactNode;
    loading?: boolean;
  }) => <button type="button" {...props}>{children}</button>,
  IconButton: ({ children, ...props }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button type="button" {...props}>{children}</button>
  ),
  Field: ({
    label,
    children,
  }: React.PropsWithChildren<{ label?: string }>) => (
    <label>
      {label}
      {React.isValidElement(children)
        ? React.cloneElement(
            children as React.ReactElement<{ 'aria-label'?: string }>,
            { 'aria-label': label },
          )
        : children}
    </label>
  ),
  Input: ({
    leading,
    trailing,
    ...props
  }: React.InputHTMLAttributes<HTMLInputElement> & {
    leading?: React.ReactNode;
    trailing?: React.ReactNode;
  }) => <label>{leading}<input {...props} />{trailing}</label>,
  Select: ({
    options,
    value,
    onValueChange,
  }: {
    options: Array<{ label: string; value: string }>;
    value: string;
    onValueChange: (value: string) => void;
  }) => (
    <select value={value} onChange={(event) => onValueChange(event.target.value)}>
      {options.map((option) => (
        <option key={option.value} value={option.value}>{option.label}</option>
      ))}
    </select>
  ),
  Tooltip: ({ children }: React.PropsWithChildren) => <>{children}</>,
  ScrollArea: ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>,
  FormSection: ({
    children,
    title,
    actions,
    ...props
  }: React.HTMLAttributes<HTMLElement> & { title?: React.ReactNode; actions?: React.ReactNode; headingAs?: string }) => (
    <section {...props}>
      {title}
      {actions}
      {children}
    </section>
  ),
  FieldGroup: React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
    function FieldGroup({ children, ...props }, ref) {
      return <div ref={ref} {...props}>{children}</div>;
    },
  ),
  FieldRow: ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>,
  // The real Field associates its label with the child control; the mock
  // mirrors that accessible name via aria-label so queries stay realistic.
  Field: ({
    label,
    error,
    children,
  }: { label?: React.ReactNode; error?: React.ReactNode; children: React.ReactElement }) => (
    <div>
      {React.cloneElement(children as React.ReactElement<Record<string, unknown>>, {
        'aria-label': typeof label === 'string' ? label : undefined,
      })}
      {error}
    </div>
  ),
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

  it('uses native select controls inside the dialog', async () => {
    await renderDialog();

    const selects = Array.from(container.querySelectorAll<HTMLSelectElement>('select'));
    expect(selects.length).toBeGreaterThan(0);
    expect(selects.every((select) => container.contains(select))).toBe(true);
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
