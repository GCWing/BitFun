// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import AcpAgentsConfig from './AcpAgentsConfig';

const loadJsonConfigMock = vi.hoisted(() => vi.fn());
const getClientsMock = vi.hoisted(() => vi.fn());
const probeClientRequirementsMock = vi.hoisted(() => vi.fn());
const saveJsonConfigMock = vi.hoisted(() => vi.fn());
const installClientCliMock = vi.hoisted(() => vi.fn());
const predownloadClientAdapterMock = vi.hoisted(() => vi.fn());
const listSavedConnectionsMock = vi.hoisted(() => vi.fn());
const notifyErrorMock = vi.hoisted(() => vi.fn());
const notifyInfoMock = vi.hoisted(() => vi.fn());
const notifySuccessMock = vi.hoisted(() => vi.fn());
const translate = (_key: string, options?: Record<string, unknown> & { defaultValue?: string }) => (
  options?.defaultValue ?? _key
);

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: translate,
  }),
}));

vi.mock('@bitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@bitfun/ui')>(),
  Tooltip: ({ children }: React.PropsWithChildren) => <>{children}</>,
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
  IconButton: ({
    children,
    disabled,
    isLoading,
    onClick,
    tooltip: _tooltip,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement> & {
    children: React.ReactNode;
    isLoading?: boolean;
    tooltip?: React.ReactNode;
  }) => (
    <button type="button" disabled={disabled || isLoading} onClick={onClick} {...props}>
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
  ConfigPageHeader: ({ title, subtitle, extra }: {
    title: string;
    subtitle: string;
    extra?: React.ReactNode;
  }) => (
    <header>
      <h1>{title}</h1>
      <p>{subtitle}</p>
      {extra}
    </header>
  ),
  ConfigPageLayout: ({ children }: { children: React.ReactNode }) => <main>{children}</main>,
  ConfigPageSectionStack: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  ConfigPageSection: ({
    children,
    title,
    description,
    extra,
  }: {
    children: React.ReactNode;
    title: string;
    description?: string;
    extra?: React.ReactNode;
  }) => (
    <section>
      <div>
        <h2>{title}</h2>
        {extra}
      </div>
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
    saveJsonConfig: saveJsonConfigMock,
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
    info: notifyInfoMock,
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
    localStorage.clear();
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
    saveJsonConfigMock.mockImplementation(async () => {
      window.dispatchEvent(new Event('bitfun:acp-clients-changed'));
    });
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

  it('keeps page help and agent detection with their owning surfaces', async () => {
    await act(async () => {
      root.render(<AcpAgentsConfig />);
    });

    await act(async () => {
      await Promise.resolve();
    });

    expect(container.querySelector('header')?.textContent).toContain('actions.learnMore');
    const registryHeading = Array.from(container.querySelectorAll('h2'))
      .find(heading => heading.textContent === 'registry.title');
    const registrySection = registryHeading?.closest('section');
    expect(registrySection?.textContent).toContain('actions.refresh');
    expect(registrySection?.textContent).toContain('presets.opencode.description');
    expect(registrySection?.textContent).not.toContain('registry.description');
    expect(registrySection?.textContent).not.toContain('Native ACP coding agent');
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

  it('hides a saved remote server without deleting its SSH connection', async () => {
    listSavedConnectionsMock.mockResolvedValue([{
      id: 'huawei-server',
      name: 'Huawei Server',
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

    const hideButton = container.querySelector<HTMLButtonElement>(
      'button[aria-label="remote.hideConnection"]'
    );
    expect(hideButton).not.toBeNull();

    await act(async () => {
      hideButton?.click();
      await Promise.resolve();
    });

    expect(listSavedConnectionsMock).toHaveBeenCalledTimes(1);
    expect(container.textContent).not.toContain('Huawei Server');
    expect(JSON.parse(localStorage.getItem('bitfun:settings:acp-agents:hidden-remote-connections:v1') || '[]'))
      .toEqual(['huawei-server']);
    expect(container.textContent).toContain('remote.showHiddenConnections');
  });

  it('restores a hidden remote server from the hidden list', async () => {
    localStorage.setItem(
      'bitfun:settings:acp-agents:hidden-remote-connections:v1',
      JSON.stringify(['huawei-server'])
    );
    listSavedConnectionsMock.mockResolvedValue([{
      id: 'huawei-server',
      name: 'Huawei Server',
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

    const showHiddenButton = Array.from(container.querySelectorAll('button'))
      .find(button => button.textContent?.includes('remote.showHiddenConnections'));
    expect(showHiddenButton).not.toBeUndefined();

    await act(async () => {
      showHiddenButton?.click();
      await Promise.resolve();
    });

    const restoreButton = container.querySelector<HTMLButtonElement>(
      'button[aria-label="remote.restoreConnection"]'
    );
    expect(restoreButton).not.toBeNull();

    await act(async () => {
      restoreButton?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(localStorage.getItem('bitfun:settings:acp-agents:hidden-remote-connections:v1'))
      .toBe('[]');
    expect(container.textContent).toContain('Huawei Server');
  });

  it('does not probe hidden remote servers until they are restored', async () => {
    localStorage.setItem(
      'bitfun:settings:acp-agents:hidden-remote-connections:v1',
      JSON.stringify(['huawei-server'])
    );
    listSavedConnectionsMock.mockResolvedValue([{
      id: 'huawei-server',
      name: 'Huawei Server',
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

    expect(probeClientRequirementsMock).not.toHaveBeenCalledWith({
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
        adapter: { name: '@agentclientprotocol/claude-agent-acp', installed: false },
        runnable: false,
        notes: [],
      },
      {
        id: 'codex',
        tool: { name: 'codex', installed: true },
        adapter: { name: '@agentclientprotocol/codex-acp', installed: false },
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

  it('keeps enabled agents stable when adding another preset', async () => {
    const healthyProbes = [
      {
        id: 'opencode',
        tool: { name: 'opencode', installed: true },
        runnable: true,
        notes: [],
      },
      {
        id: 'claude-code',
        tool: { name: 'claude', installed: true },
        adapter: { name: '@agentclientprotocol/claude-agent-acp', installed: true },
        runnable: true,
        notes: [],
      },
      {
        id: 'codex',
        tool: { name: 'codex', installed: true },
        runnable: true,
        notes: [],
      },
    ];
    probeClientRequirementsMock.mockResolvedValue(healthyProbes);
    saveJsonConfigMock.mockImplementation(async () => {
      window.dispatchEvent(new Event('bitfun:acp-clients-changed'));
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
          'claude-code': {
            name: 'Claude Code',
            command: 'npx',
            args: ['--yes', '@agentclientprotocol/claude-agent-acp@latest'],
            env: {},
            enabled: true,
            readonly: false,
            permissionMode: 'ask',
          },
          codex: {
            name: 'Codex',
            command: 'npx',
            args: ['--yes', '@agentclientprotocol/codex-acp@latest'],
            env: {},
            enabled: true,
            readonly: false,
            permissionMode: 'ask',
          },
        },
      }));
    });

    await act(async () => {
      root.render(<AcpAgentsConfig />);
    });

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('registry.enabled');

    const addButtons = Array.from(container.querySelectorAll('button'))
      .filter(button => button.textContent?.includes('actions.add'));
    expect(addButtons.length).toBeGreaterThan(0);

    await act(async () => {
      addButtons[addButtons.length - 1].click();
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(saveJsonConfigMock).toHaveBeenCalled();
    expect(container.textContent).toContain('registry.enabled');
    expect(container.textContent).not.toContain('registry.cliMissing');
    expect(container.textContent).not.toContain('registry.configInvalid');
  });

  it('labels self-managed missing CLIs as config-only before adding', async () => {
    probeClientRequirementsMock.mockResolvedValue([
      {
        id: 'opencode',
        tool: { name: 'opencode', installed: true },
        runnable: true,
        notes: [],
      },
      {
        id: 'omp',
        tool: { name: 'omp', installed: false },
        runnable: false,
        notes: ['omp is not available on PATH'],
      },
    ]);

    await act(async () => {
      root.render(<AcpAgentsConfig />);
    });

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const addConfigButtons = Array.from(container.querySelectorAll('button'))
      .filter(button => button.textContent?.includes('actions.addConfig'));
    expect(addConfigButtons.length).toBeGreaterThan(0);

    await act(async () => {
      addConfigButtons[0].click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(installClientCliMock).not.toHaveBeenCalled();
    expect(saveJsonConfigMock).toHaveBeenCalledWith(expect.stringContaining('"omp"'));
    expect(notifySuccessMock).toHaveBeenCalledWith('notifications.configAddedManualCliRequired');
  });

  it('offers the one-click installer for DeepSeek Harness and launches the bundled profile', async () => {
    probeClientRequirementsMock.mockResolvedValue([
      {
        id: 'dsh',
        tool: { name: 'dsh', installed: false },
        runnable: false,
        notes: ['dsh is not available on PATH'],
      },
    ]);

    await act(async () => {
      root.render(<AcpAgentsConfig />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('DeepSeek Harness');
    // Unlike omp, the harness is a plain npm global, so BitFun installs it.
    // The bridge is not a separate adapter — it ships inside BitFun.
    expect(container.textContent).not.toContain('registry.adapterMissing');

    const installButtons = Array.from(container.querySelectorAll('button'))
      .filter(button => button.textContent?.includes('actions.installCli'));
    expect(installButtons.length).toBeGreaterThan(0);

    await act(async () => {
      installButtons[0].click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(installClientCliMock).toHaveBeenCalledWith(
      expect.objectContaining({ clientId: 'dsh' }),
    );
  });

  it('adds DeepSeek Harness as a launch of the profile BitFun materializes', async () => {
    probeClientRequirementsMock.mockResolvedValue([
      {
        id: 'dsh',
        tool: { name: 'dsh', installed: true, version: '0.1.0-rc.6' },
        runnable: true,
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

    // Address the harness row by name: row order is whatever the saved config
    // and the preset list happen to produce, so "the last add button" belongs
    // to some other agent as often as not.
    const harnessRow = Array.from(
      container.querySelectorAll('.bitfun-acp-agents__registry-row'),
    ).find(row => row.querySelector('.bitfun-acp-agents__registry-name')
      ?.textContent === 'DeepSeek Harness');
    expect(harnessRow).toBeTruthy();

    const addButton = Array.from(harnessRow!.querySelectorAll('button'))
      .find(button => button.textContent?.includes('actions.add'));
    expect(addButton).toBeTruthy();

    await act(async () => {
      addButton!.click();
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    // The command has to name the profile BitFun materializes; a bare `dsh`
    // would drop the user into the harness's own default composition, which
    // does not speak ACP at all.
    expect(saveJsonConfigMock).toHaveBeenCalledWith(expect.stringContaining('bitfun-acp'));
  });

  it('does not downgrade enabled agents on transient probe timeouts during refresh', async () => {
    probeClientRequirementsMock
      .mockResolvedValueOnce([
        {
          id: 'opencode',
          tool: { name: 'opencode', installed: true },
          runnable: true,
          notes: [],
        },
        {
          id: 'claude-code',
          tool: { name: 'claude', installed: true },
          adapter: { name: '@agentclientprotocol/claude-agent-acp', installed: true },
          runnable: true,
          notes: [],
        },
        {
          id: 'codex',
          tool: { name: 'codex', installed: true },
          runnable: true,
          notes: [],
        },
      ])
      .mockResolvedValueOnce([
        {
          id: 'opencode',
          tool: {
            name: 'opencode',
            installed: false,
            error: 'Timed out while checking command',
          },
          runnable: false,
          notes: [],
        },
        {
          id: 'claude-code',
          tool: { name: 'claude', installed: true },
          adapter: { name: '@agentclientprotocol/claude-agent-acp', installed: true },
          runnable: true,
          notes: [],
        },
        {
          id: 'codex',
          tool: { name: 'codex', installed: true },
          runnable: true,
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

    expect(container.textContent).toContain('registry.enabled');
    expect(container.textContent).not.toContain('registry.cliMissing');
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
