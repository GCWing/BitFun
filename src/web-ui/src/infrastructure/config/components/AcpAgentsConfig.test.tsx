// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import AcpAgentsConfig from './AcpAgentsConfig';

const loadJsonConfigMock = vi.hoisted(() => vi.fn());
const getClientsMock = vi.hoisted(() => vi.fn());
const probeClientRequirementsMock = vi.hoisted(() => vi.fn());
const installClientCliMock = vi.hoisted(() => vi.fn());
const predownloadClientAdapterMock = vi.hoisted(() => vi.fn());
const listSavedConnectionsMock = vi.hoisted(() => vi.fn());
const notifyErrorMock = vi.hoisted(() => vi.fn());
const notifySuccessMock = vi.hoisted(() => vi.fn());
const translate = (_key: string, options?: Record<string, unknown> & { defaultValue?: string }) => (
  options?.defaultValue ?? _key
);

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: translate,
  }),
}));

vi.mock('@/component-library', () => ({
  Button: ({
    children,
    disabled,
    isLoading,
    onClick,
  }: {
    children: React.ReactNode;
    disabled?: boolean;
    isLoading?: boolean;
    onClick?: () => void;
  }) => (
    <button type="button" disabled={disabled || isLoading} onClick={onClick}>
      {children}
    </button>
  ),
  Input: ({
    value,
    onChange,
    placeholder,
  }: {
    value?: string;
    onChange?: React.ChangeEventHandler<HTMLInputElement>;
    placeholder?: string;
  }) => <input value={value} onChange={onChange} placeholder={placeholder} />,
  Select: ({
    value,
    onChange,
    options,
  }: {
    value?: string;
    onChange?: (value: string) => void;
    options?: Array<{ value: string; label: string }>;
  }) => (
    <select value={value} onChange={(event) => onChange?.(event.target.value)}>
      {(options ?? []).map((option) => (
        <option key={option.value} value={option.value}>{option.label}</option>
      ))}
    </select>
  ),
  Textarea: React.forwardRef<HTMLTextAreaElement, React.TextareaHTMLAttributes<HTMLTextAreaElement>>(
    (props, ref) => <textarea ref={ref} {...props} />,
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
  ConfigPageSection: ({
    children,
    title,
    description,
  }: {
    children: React.ReactNode;
    title: string;
    description?: string;
  }) => (
    <section>
      <h2>{title}</h2>
      {description ? <p>{description}</p> : null}
      {children}
    </section>
  ),
}));

vi.mock('../../api/service-api/ACPClientAPI', () => ({
  ACPClientAPI: {
    loadJsonConfig: loadJsonConfigMock,
    getClients: getClientsMock,
    probeClientRequirements: probeClientRequirementsMock,
    installClientCli: installClientCliMock,
    predownloadClientAdapter: predownloadClientAdapterMock,
    saveJsonConfig: vi.fn(),
  },
}));

vi.mock('../../api/service-api/SystemAPI', () => ({
  systemAPI: {
    openExternal: vi.fn(),
  },
}));

vi.mock('@/features/ssh-remote/sshApi', () => ({
  sshApi: {
    listSavedConnections: listSavedConnectionsMock,
  },
}));

vi.mock('@/shared/notification-system', () => ({
  useNotification: () => ({
    error: notifyErrorMock,
    success: notifySuccessMock,
  }),
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    error: vi.fn(),
    warn: vi.fn(),
  }),
}));

describe('AcpAgentsConfig', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    loadJsonConfigMock.mockResolvedValue(JSON.stringify({
      acpClients: {
        opencode: {
          name: 'opencode',
          command: 'opencode',
          args: ['acp'],
          env: {},
          enabled: true,
          readonly: false,
          permissionMode: 'ask',
        },
      },
    }));
    getClientsMock.mockResolvedValue([{
      id: 'opencode',
      name: 'opencode',
      command: 'opencode',
      args: ['acp'],
      enabled: true,
      readonly: false,
      permissionMode: 'ask',
      status: 'configured',
      sessionCount: 0,
      toolName: 'acp__opencode__prompt',
    }]);
    listSavedConnectionsMock.mockResolvedValue([]);
    probeClientRequirementsMock.mockResolvedValue([]);
    installClientCliMock.mockResolvedValue(undefined);
    predownloadClientAdapterMock.mockResolvedValue(undefined);

    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    if (root) {
      act(() => {
        root.unmount();
      });
    }
    container?.remove();
    vi.clearAllMocks();
  });

  it('probes requirements when opened and does not treat missing probe data as invalid config', async () => {
    await act(async () => {
      root.render(<AcpAgentsConfig />);
    });

    await act(async () => {
      await Promise.resolve();
    });

    expect(loadJsonConfigMock).toHaveBeenCalledTimes(1);
    expect(getClientsMock).toHaveBeenCalledTimes(1);
    expect(probeClientRequirementsMock).toHaveBeenCalledTimes(1);
    expect(container.textContent).not.toContain('registry.configInvalid');
  });

  it('renders saved remote servers as global agent rows without override controls', async () => {
    listSavedConnectionsMock.mockResolvedValue([{
      id: 'huawei-server',
      name: 'huawei-server',
      host: '119.8.182.138',
      port: 22,
      username: 'ssh-root',
      authType: { type: 'Password' },
    }]);

    await act(async () => {
      root.render(<AcpAgentsConfig />);
    });

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('huawei-server');
    expect(container.textContent).toContain('ssh-root@119.8.182.138');
    expect(container.textContent).toContain('remote.refreshDetection');
    expect(container.textContent).not.toContain('remote.env');
    expect(probeClientRequirementsMock).toHaveBeenCalledWith({
      remoteConnectionId: 'huawei-server',
      force: undefined,
    });
  });

  it('configures a preset adapter when the CLI is ready but the ACP layer is missing', async () => {
    probeClientRequirementsMock.mockResolvedValue([
      {
        id: 'opencode',
        tool: { name: 'opencode', installed: true },
        runnable: true,
        notes: [],
      },
      {
        id: 'claude-code',
        tool: { name: 'claude', installed: true },
        adapter: { name: '@zed-industries/claude-code-acp', installed: false },
        runnable: false,
        notes: [],
      },
      {
        id: 'codex',
        tool: { name: 'codex', installed: true },
        adapter: { name: '@zed-industries/codex-acp', installed: false },
        runnable: false,
        notes: [],
      },
    ]);

    await act(async () => {
      root.render(<AcpAgentsConfig />);
    });

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const refreshButtons = Array.from(container.querySelectorAll('button'))
      .filter(button => button.textContent?.includes('actions.refresh'));
    expect(refreshButtons.length).toBeGreaterThan(0);

    await act(async () => {
      refreshButtons[0].click();
      await Promise.resolve();
      await Promise.resolve();
    });

    const configureButtons = Array.from(container.querySelectorAll('button'))
      .filter(button => button.textContent?.includes('actions.configureAcp'));
    expect(configureButtons.length).toBeGreaterThan(0);

    await act(async () => {
      configureButtons[configureButtons.length - 1].click();
      await Promise.resolve();
    });

    expect(predownloadClientAdapterMock).toHaveBeenCalledWith({
      clientId: 'codex',
    });
  });

  it('installs a missing remote preset CLI on that remote server', async () => {
    listSavedConnectionsMock.mockResolvedValue([{
      id: 'huawei-server',
      name: 'huawei-server',
      host: '119.8.182.138',
      port: 22,
      username: 'ssh-root',
      authType: { type: 'Password' },
    }]);
    probeClientRequirementsMock.mockImplementation((options?: { remoteConnectionId?: string }) => {
      if (options?.remoteConnectionId === 'huawei-server') {
        return Promise.resolve([
          {
            id: 'opencode',
            tool: { name: 'opencode', installed: true },
            runnable: true,
            notes: [],
          },
          {
            id: 'claude-code',
            tool: { name: 'claude', installed: true },
            runnable: true,
            notes: [],
          },
          {
            id: 'codex',
            tool: { name: 'codex', installed: false },
            runnable: false,
            notes: [],
          },
        ]);
      }
      return Promise.resolve([]);
    });

    await act(async () => {
      root.render(<AcpAgentsConfig />);
    });

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const installButtons = Array.from(container.querySelectorAll('button'))
      .filter(button => button.textContent?.includes('actions.installCli'));
    expect(installButtons.length).toBeGreaterThan(0);

    await act(async () => {
      installButtons[installButtons.length - 1].click();
      await Promise.resolve();
    });

    expect(installClientCliMock).toHaveBeenCalledWith({
      clientId: 'codex',
      remoteConnectionId: 'huawei-server',
    });
  });
});
