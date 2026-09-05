// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { PortForwardDialog } from './PortForwardDialog';
import type { PortForward } from './types';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const sshApiMock = vi.hoisted(() => ({
  listPortForwards: vi.fn(),
  startPortForward: vi.fn(),
  stopPortForward: vi.fn(),
  listRemoteListeningPorts: vi.fn(),
}));

const systemApiMock = vi.hoisted(() => ({
  openExternal: vi.fn(),
  setClipboard: vi.fn(),
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    // Interpolations are echoed so assertions can prove which values reached
    // the copy without depending on any locale's wording.
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key,
  }),
}));

vi.mock('./sshApi', () => ({ sshApi: sshApiMock }));

vi.mock('@/infrastructure/api/service-api/SystemAPI', () => ({ systemAPI: systemApiMock }));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({ error: vi.fn(), warn: vi.fn(), info: vi.fn(), debug: vi.fn() }),
}));

vi.mock('@openbitfun/ui', () => ({
  Icon: ({ name, ...props }: { name: string } & React.HTMLAttributes<HTMLSpanElement>) => <span data-icon={name} {...props} />,
  Dialog: ({ open, children }: React.PropsWithChildren<{ open: boolean }>) => (
    open ? <div role="dialog">{children}</div> : null
  ),
  DialogBody: ({ children }: React.PropsWithChildren) => <div>{children}</div>,
  DialogClose: (props: React.ButtonHTMLAttributes<HTMLButtonElement>) => <button type="button" {...props} />,
  DialogHeader: ({ children }: React.PropsWithChildren) => <header>{children}</header>,
  DialogHeading: ({ children }: React.PropsWithChildren) => <div>{children}</div>,
  DialogTitle: ({ children }: React.PropsWithChildren) => <h2>{children}</h2>,
  Button: ({
    children,
    onClick,
    disabled,
    'data-testid': testId,
  }: React.PropsWithChildren<{
    onClick?: React.MouseEventHandler<HTMLButtonElement>;
    disabled?: boolean;
    'data-testid'?: string;
  }>) => (
    <button type="button" onClick={onClick} disabled={disabled} data-testid={testId}>
      {children}
    </button>
  ),
  Checkbox: ({ checked, onChange, label }: {
    checked?: boolean;
    onChange?: React.ChangeEventHandler<HTMLInputElement>;
    label?: React.ReactNode;
  }) => (
    <label>
      <input type="checkbox" checked={checked} onChange={onChange} />
      {label}
    </label>
  ),
  IconButton: ({
    children,
    onClick,
    disabled,
    'aria-label': ariaLabel,
    'data-testid': testId,
  }: React.PropsWithChildren<{
    onClick?: React.MouseEventHandler<HTMLButtonElement>;
    disabled?: boolean;
    'aria-label'?: string;
    'data-testid'?: string;
  }>) => (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={ariaLabel}
      data-testid={testId}
    >
      {children}
    </button>
  ),
  Input: React.forwardRef<
    HTMLInputElement,
    {
      value?: string;
      onChange?: React.ChangeEventHandler<HTMLInputElement>;
      placeholder?: string;
      'data-testid'?: string;
    }
  >(({ value, onChange, placeholder, 'data-testid': testId }, ref) => (
    <input
      ref={ref}
      value={value}
      onChange={onChange}
      placeholder={placeholder}
      data-testid={testId}
    />
  )),
  Tooltip: ({ children }: React.PropsWithChildren) => <>{children}</>,
  FormSection: ({
    children,
    title,
    actions,
    ...props
  }: React.HTMLAttributes<HTMLElement> & { title?: React.ReactNode; actions?: React.ReactNode }) => (
    <section {...props}>{title}{actions}{children}</section>
  ),
  FieldGroup: ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>,
}));

function makeForward(overrides: Partial<PortForward> = {}): PortForward {
  return {
    id: 'forward-1',
    connectionId: 'conn-1',
    direction: 'local',
    localHost: '127.0.0.1',
    localPort: 3000,
    remoteHost: '127.0.0.1',
    remotePort: 3000,
    activeConnections: 0,
    totalConnections: 0,
    ...overrides,
  };
}

describe('PortForwardDialog', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    sshApiMock.listPortForwards.mockResolvedValue([]);
    sshApiMock.startPortForward.mockResolvedValue(makeForward());
    sshApiMock.stopPortForward.mockResolvedValue(undefined);
    sshApiMock.listRemoteListeningPorts.mockResolvedValue([]);
    systemApiMock.openExternal.mockResolvedValue(undefined);
    systemApiMock.setClipboard.mockResolvedValue(undefined);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  async function renderDialog(): Promise<void> {
    await act(async () => {
      root.render(
        <PortForwardDialog open connectionId="conn-1" onClose={vi.fn()} />
      );
    });
    await act(async () => {
      await Promise.resolve();
    });
  }

  function setInput(testId: string, value: string): void {
    const input = container.querySelector<HTMLInputElement>(`[data-testid="${testId}"]`);
    if (!input) throw new Error(`missing input ${testId}`);
    const setter = Object.getOwnPropertyDescriptor(
      window.HTMLInputElement.prototype,
      'value'
    )?.set;
    act(() => {
      setter?.call(input, value);
      input.dispatchEvent(new Event('input', { bubbles: true }));
    });
  }

  function click(testId: string): void {
    const button = container.querySelector<HTMLElement>(`[data-testid="${testId}"]`);
    if (!button) throw new Error(`missing button ${testId}`);
    act(() => {
      button.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
  }

  it('detects listening ports on open without forwarding any of them', async () => {
    sshApiMock.listRemoteListeningPorts.mockResolvedValue([
      { port: 8899, bindAddress: '0.0.0.0', process: 'python3' },
    ]);
    await renderDialog();

    expect(sshApiMock.listRemoteListeningPorts).toHaveBeenCalledWith('conn-1');
    expect(container.textContent).toContain('8899');
    expect(
      sshApiMock.startPortForward,
      'detection must never forward on its own'
    ).not.toHaveBeenCalled();
  });

  it('forwards a detected port in one click, letting the backend pick the local port', async () => {
    sshApiMock.listRemoteListeningPorts.mockResolvedValue([
      { port: 8899, bindAddress: '0.0.0.0', process: 'python3' },
    ]);
    await renderDialog();

    await act(async () => {
      container
        .querySelector<HTMLElement>('[data-testid="ssh-port-forward-chip"][data-port="8899"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(sshApiMock.startPortForward).toHaveBeenCalledWith({
      connectionId: 'conn-1',
      remotePort: 8899,
      localPort: undefined,
      exposeOnLan: false,
      label: 'python3',
    });
  });

  it('does not offer to forward a port that is already forwarded', async () => {
    sshApiMock.listRemoteListeningPorts.mockResolvedValue([
      { port: 3000, bindAddress: '127.0.0.1', process: 'node' },
    ]);
    sshApiMock.listPortForwards.mockResolvedValue([makeForward({ remotePort: 3000 })]);
    await renderDialog();

    const chip = container.querySelector<HTMLButtonElement>(
      '[data-testid="ssh-port-forward-chip"][data-port="3000"]'
    );
    expect(chip?.disabled).toBe(true);
  });

  it('sends only the remote port when the local port is left blank', async () => {
    await renderDialog();
    setInput('ssh-port-forward-remote-port', '3000');

    await act(async () => {
      click('ssh-port-forward-add');
      await Promise.resolve();
    });

    expect(sshApiMock.startPortForward).toHaveBeenCalledWith({
      connectionId: 'conn-1',
      remotePort: 3000,
      localPort: undefined,
      exposeOnLan: false,
      label: undefined,
    });
  });

  it('refuses to submit a port outside the valid range', async () => {
    await renderDialog();
    setInput('ssh-port-forward-remote-port', '70000');

    const add = container.querySelector<HTMLButtonElement>(
      '[data-testid="ssh-port-forward-add"]'
    );
    expect(add?.disabled).toBe(true);

    await act(async () => {
      click('ssh-port-forward-add');
      await Promise.resolve();
    });
    expect(sshApiMock.startPortForward).not.toHaveBeenCalled();
  });

  it('surfaces the backend error instead of leaving the row silently missing', async () => {
    sshApiMock.startPortForward.mockRejectedValue(
      new Error('127.0.0.1:3000 is already forwarded to 127.0.0.1:3001')
    );
    await renderDialog();
    setInput('ssh-port-forward-remote-port', '3000');

    await act(async () => {
      click('ssh-port-forward-add');
      await Promise.resolve();
    });

    expect(container.textContent).toContain('already forwarded');
  });

  it('keeps the typed values when the mapping was rejected', async () => {
    sshApiMock.startPortForward.mockRejectedValue(new Error('nope'));
    await renderDialog();
    setInput('ssh-port-forward-remote-port', '3000');

    await act(async () => {
      click('ssh-port-forward-add');
      await Promise.resolve();
    });

    const input = container.querySelector<HTMLInputElement>(
      '[data-testid="ssh-port-forward-remote-port"]'
    );
    expect(input?.value, 'a rejected mapping must not clear what was typed').toBe('3000');
  });

  it('tells the user when the mapping moved to a different local port', async () => {
    sshApiMock.listPortForwards.mockResolvedValue([
      makeForward({ localPort: 3001, requestedLocalPort: 3000 }),
    ]);
    await renderDialog();

    expect(container.textContent).toContain(
      'ssh.portForward.portMoved:{"requested":3000,"bound":3001}'
    );
  });

  it('hands the browser the bound local port, not the remote one', async () => {
    sshApiMock.listPortForwards.mockResolvedValue([
      makeForward({ localPort: 3001, requestedLocalPort: 3000 }),
    ]);
    await renderDialog();

    const open = container.querySelector<HTMLElement>(
      '[aria-label="ssh.portForward.openInBrowser"]'
    );
    act(() => {
      open?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(systemApiMock.openExternal).toHaveBeenCalledWith('http://127.0.0.1:3001');
  });

  it('reports a wildcard bind as a loopback address the user can actually open', async () => {
    sshApiMock.listPortForwards.mockResolvedValue([
      makeForward({ localHost: '0.0.0.0', localPort: 8080 }),
    ]);
    await renderDialog();

    const copy = container.querySelector<HTMLElement>(
      '[aria-label="ssh.portForward.copyAddress"]'
    );
    act(() => {
      copy?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(systemApiMock.setClipboard).toHaveBeenCalledWith('127.0.0.1:8080');
  });

  it('stops a forward by id', async () => {
    sshApiMock.listPortForwards.mockResolvedValue([makeForward({ id: 'forward-9' })]);
    await renderDialog();

    await act(async () => {
      click('ssh-port-forward-stop');
      await Promise.resolve();
    });

    expect(sshApiMock.stopPortForward).toHaveBeenCalledWith('forward-9');
  });
});
