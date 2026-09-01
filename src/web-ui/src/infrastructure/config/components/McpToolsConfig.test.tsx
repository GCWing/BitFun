// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import McpToolsConfig from './McpToolsConfig';

const peerState = vi.hoisted(() => ({ active: true }));
const runtimeState = vi.hoisted(() => ({ desktop: true }));
const getServersMock = vi.hoisted(() => vi.fn());
const loadJsonConfigMock = vi.hoisted(() => vi.fn());
const saveJsonConfigMock = vi.hoisted(() => vi.fn());
const initializeServersMock = vi.hoisted(() => vi.fn());
const startServerMock = vi.hoisted(() => vi.fn());
const startRemoteOAuthMock = vi.hoisted(() => vi.fn());
const getRemoteOAuthSessionMock = vi.hoisted(() => vi.fn());
const cancelRemoteOAuthMock = vi.hoisted(() => vi.fn());
const deleteServerMock = vi.hoisted(() => vi.fn());
const confirmDangerMock = vi.hoisted(() => vi.fn());
const openExternalMock = vi.hoisted(() => vi.fn());
const notificationMocks = vi.hoisted(() => ({
  success: vi.fn(),
  warning: vi.fn(),
  error: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@/infrastructure/peer-device/peerDeviceContextState', () => ({
  usePeerDeviceModeOptional: () => ({
    peerMode: peerState.active
      ? { active: true, deviceId: 'remote-device', deviceName: 'Remote device' }
      : { active: false },
  }),
}));
vi.mock('@/infrastructure/runtime', () => ({
  isTauriRuntime: () => runtimeState.desktop,
}));
vi.mock('@/shared/notification-system', () => ({
  useNotification: () => notificationMocks,
}));
vi.mock('@/infrastructure/confirm-dialog', async () => {
  const actual = await vi.importActual<typeof import('@/infrastructure/confirm-dialog')>(
    '@/infrastructure/confirm-dialog',
  );
  return { ...actual, confirmDanger: confirmDangerMock };
});
vi.mock('../../api/service-api/MCPAPI', () => ({
  MCPAPI: {
    getServers: getServersMock,
    loadMCPJsonConfig: loadJsonConfigMock,
    saveMCPJsonConfig: saveJsonConfigMock,
    initializeServers: initializeServersMock,
    startServer: startServerMock,
    startRemoteOAuth: startRemoteOAuthMock,
    getRemoteOAuthSession: getRemoteOAuthSessionMock,
    cancelRemoteOAuth: cancelRemoteOAuthMock,
    deleteServer: deleteServerMock,
  },
}));
vi.mock('../../api/service-api/SystemAPI', () => ({
  systemAPI: { openExternal: openExternalMock },
}));
vi.mock('./ExternalMcpOverview', () => ({
  default: () => <div data-testid="external-mcp-overview" />,
}));

describe('McpToolsConfig remote behavior', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    peerState.active = true;
    runtimeState.desktop = true;
    getServersMock.mockReset().mockResolvedValue([]);
    loadJsonConfigMock.mockReset().mockResolvedValue({
      jsonConfig: '{"mcpServers":{}}',
      fingerprint: 'sha256:test',
    });
    saveJsonConfigMock.mockReset().mockResolvedValue(undefined);
    initializeServersMock.mockReset().mockResolvedValue(undefined);
    startServerMock.mockReset().mockResolvedValue(undefined);
    startRemoteOAuthMock.mockReset().mockResolvedValue({
      serverId: 'notion',
      status: 'awaitingBrowser',
      authorizationUrl: 'https://mcp.notion.test/authorize',
      redirectUri: 'http://127.0.0.1:31337/callback',
      message: 'Authorization started',
    });
    getRemoteOAuthSessionMock.mockReset().mockResolvedValue(null);
    cancelRemoteOAuthMock.mockReset().mockResolvedValue(undefined);
    openExternalMock.mockReset().mockResolvedValue(undefined);
    deleteServerMock.mockReset().mockResolvedValue(undefined);
    confirmDangerMock.mockReset().mockResolvedValue(true);
    notificationMocks.success.mockReset();
    notificationMocks.warning.mockReset();
    notificationMocks.error.mockReset();
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
  });

  it('does not call desktop MCP management APIs during a remote connection', async () => {
    await act(async () => {
      root.render(<McpToolsConfig />);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(getServersMock).not.toHaveBeenCalled();
    expect(loadJsonConfigMock).not.toHaveBeenCalled();
    expect(container.textContent).toContain('section.serverList.remoteUnavailable');

    peerState.active = false;
    runtimeState.desktop = false;
    await act(async () => {
      root.render(<McpToolsConfig />);
      await Promise.resolve();
    });

    expect(getServersMock).not.toHaveBeenCalled();
    expect(loadJsonConfigMock).not.toHaveBeenCalled();
    expect(container.textContent).toContain('section.serverList.desktopUnavailable');

    runtimeState.desktop = true;
    await act(async () => {
      root.render(<McpToolsConfig />);
      await Promise.resolve();
    });

    expect(getServersMock).toHaveBeenCalledTimes(1);
    expect(loadJsonConfigMock).toHaveBeenCalledTimes(1);
  });

  it('ignores a desktop MCP load that finishes after switching to a remote connection', async () => {
    let resolveServers: ((servers: Array<Record<string, unknown>>) => void) | undefined;
    getServersMock.mockReturnValueOnce(new Promise((resolve) => {
      resolveServers = resolve;
    }));
    peerState.active = false;

    await act(async () => {
      root.render(<McpToolsConfig />);
      await Promise.resolve();
    });
    peerState.active = true;
    await act(async () => {
      root.render(<McpToolsConfig />);
      await Promise.resolve();
    });
    await act(async () => {
      resolveServers?.([{
        id: 'local-test',
        name: 'Local test server',
        status: 'Stopped',
        serverType: 'local',
        transport: 'stdio',
        enabled: true,
        autoStart: false,
        startSupported: true,
      }]);
      await Promise.resolve();
    });

    expect(container.textContent).not.toContain('Local test server');
    expect(container.textContent).toContain('section.serverList.remoteUnavailable');
  });

  it('shows a retryable failure instead of an empty native MCP list', async () => {
    getServersMock.mockRejectedValueOnce(new Error('load failed'));
    peerState.active = false;

    await act(async () => {
      root.render(<McpToolsConfig />);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('section.serverList.loadFailed');
    const retry = container.querySelector('[aria-label="actions.refresh"]') as HTMLButtonElement;
    expect(retry).not.toBeNull();
    await act(async () => {
      retry.click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(getServersMock).toHaveBeenCalledTimes(2);
    expect(container.textContent).not.toContain('section.serverList.loadFailed');
  });

  it('does not replace an unreadable MCP config with example JSON', async () => {
    loadJsonConfigMock.mockRejectedValueOnce(new Error('config unavailable'));
    peerState.active = false;

    await act(async () => {
      root.render(<McpToolsConfig />);
      await Promise.resolve();
      await Promise.resolve();
    });
    const openEditor = container.querySelector(
      '[aria-label="actions.jsonConfig"]',
    ) as HTMLButtonElement;
    await act(async () => openEditor.click());

    expect(container.textContent).toContain('jsonEditor.loadFailed');
    expect(container.querySelector('.bitfun-mcp-tools__json-textarea')).toBeNull();
    expect(container.textContent).not.toContain('example-server');

    const retry = container.querySelector('[aria-label="actions.refresh"]') as HTMLButtonElement;
    await act(async () => {
      retry.click();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(loadJsonConfigMock).toHaveBeenCalledTimes(2);
    expect(container.textContent).not.toContain('jsonEditor.loadFailed');
    expect(container.querySelector('.bitfun-mcp-tools__json-textarea')).not.toBeNull();
  });

  it('saves the JSON editor against the fingerprint that was loaded with it', async () => {
    peerState.active = false;
    await act(async () => {
      root.render(<McpToolsConfig />);
      await Promise.resolve();
      await Promise.resolve();
    });
    await act(async () => {
      (container.querySelector('[aria-label="actions.jsonConfig"]') as HTMLButtonElement).click();
    });
    const saveButton = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent === 'actions.saveConfig',
    );
    await act(async () => {
      saveButton?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(saveJsonConfigMock).toHaveBeenCalledWith('{"mcpServers":{}}', 'sha256:test');
    expect(initializeServersMock).not.toHaveBeenCalled();
  });

  it('offers start rather than stop for an uninitialized server', async () => {
    getServersMock.mockResolvedValueOnce([{
      id: 'local-test',
      name: 'Local test server',
      status: 'Uninitialized',
      serverType: 'local',
      transport: 'stdio',
      enabled: true,
      autoStart: false,
      commandAvailable: true,
      startSupported: true,
    }]);
    peerState.active = false;

    await act(async () => {
      root.render(<McpToolsConfig />);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.querySelector('[aria-label="actions.start"]')).not.toBeNull();
    expect(container.querySelector('[aria-label="actions.stop"]')).toBeNull();
  });

  it('coalesces repeated clicks while a start is pending and ignores completion after capability loss', async () => {
    let resolveStart: (() => void) | undefined;
    startServerMock.mockReturnValueOnce(new Promise<void>((resolve) => {
      resolveStart = resolve;
    }));
    getServersMock.mockResolvedValueOnce([{
      id: 'local-test',
      name: 'Local test server',
      status: 'Stopped',
      serverType: 'local',
      transport: 'stdio',
      enabled: true,
      autoStart: false,
      commandAvailable: true,
      startSupported: true,
    }]);
    peerState.active = false;

    await act(async () => {
      root.render(<McpToolsConfig />);
      await Promise.resolve();
      await Promise.resolve();
    });
    const startButton = Array.from(container.querySelectorAll('button')).find((button) => (
      button.getAttribute('aria-label') === 'actions.start'
      || button.getAttribute('title') === 'actions.start'
    ));
    expect(startButton).toBeDefined();
    await act(async () => {
      startButton?.click();
      startButton?.click();
      await Promise.resolve();
    });
    expect(startServerMock).toHaveBeenCalledTimes(1);
    expect((startButton as HTMLButtonElement).disabled).toBe(true);
    peerState.active = true;
    await act(async () => {
      root.render(<McpToolsConfig />);
      await Promise.resolve();
    });
    await act(async () => {
      resolveStart?.();
      await Promise.resolve();
    });

    expect(notificationMocks.success).not.toHaveBeenCalled();
    expect(notificationMocks.error).not.toHaveBeenCalled();
    expect(getServersMock).toHaveBeenCalledTimes(1);
  });

  it('starts OAuth directly for an unauthorized remote server without reporting a start failure', async () => {
    getServersMock.mockResolvedValueOnce([{
      id: 'notion',
      name: 'Notion',
      status: 'Uninitialized',
      serverType: 'Remote',
      transport: 'streamable-http',
      enabled: true,
      autoStart: false,
      authConfigured: false,
      oauthEnabled: true,
      startSupported: true,
    }]);
    peerState.active = false;

    await act(async () => {
      root.render(<McpToolsConfig />);
      await Promise.resolve();
      await Promise.resolve();
    });
    const startButton = container.querySelector<HTMLButtonElement>('[data-testid="mcp-server-start"]');
    await act(async () => {
      startButton?.click();
      startButton?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(startServerMock).not.toHaveBeenCalled();
    expect(startRemoteOAuthMock).toHaveBeenCalledTimes(1);
    expect(startRemoteOAuthMock).toHaveBeenCalledWith({ serverId: 'notion' });
    expect(openExternalMock).toHaveBeenCalledWith('https://mcp.notion.test/authorize');
    expect(getRemoteOAuthSessionMock).not.toHaveBeenCalled();
    expect(notificationMocks.error).not.toHaveBeenCalled();
    expect(document.querySelector('[data-bf-part="authEditor"]')).not.toBeNull();
  });

  it('reauthorizes after an auth handshake challenge without reporting a start failure', async () => {
    startServerMock.mockRejectedValueOnce(new Error(
      'Handshake failed: Auth required, when send initialize request',
    ));
    getServersMock.mockResolvedValueOnce([{
      id: 'notion',
      name: 'Notion',
      status: 'NeedsAuth',
      serverType: 'Remote',
      transport: 'streamable-http',
      enabled: true,
      autoStart: false,
      authConfigured: true,
      oauthEnabled: true,
      startSupported: true,
    }]);
    peerState.active = false;

    await act(async () => {
      root.render(<McpToolsConfig />);
      await Promise.resolve();
      await Promise.resolve();
    });
    const startButton = container.querySelector<HTMLButtonElement>('[data-testid="mcp-server-start"]');
    await act(async () => {
      startButton?.click();
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(startServerMock).toHaveBeenCalledWith('notion');
    expect(startRemoteOAuthMock).toHaveBeenCalledWith({ serverId: 'notion' });
    expect(notificationMocks.error).not.toHaveBeenCalled();
    expect(document.querySelector('[data-bf-part="authEditor"]')).not.toBeNull();
  });

  it('deletes a server after confirmation and reloads the list', async () => {
    peerState.active = false;
    const server = {
      id: 'local-test',
      name: 'Local test server',
      status: 'Stopped',
      serverType: 'local',
      transport: 'stdio',
      enabled: true,
      autoStart: false,
      commandAvailable: true,
      startSupported: true,
    };
    getServersMock
      .mockResolvedValueOnce([server])
      .mockResolvedValueOnce([]);

    await act(async () => {
      root.render(<McpToolsConfig />);
      await Promise.resolve();
      await Promise.resolve();
    });

    const deleteButton = container.querySelector<HTMLButtonElement>('[aria-label="actions.delete"]');
    expect(deleteButton).not.toBeNull();
    await act(async () => {
      deleteButton?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(confirmDangerMock).toHaveBeenCalledWith(
      'actions.delete',
      'messages.deleteConfirm',
      { confirmText: 'actions.delete', cancelText: 'actions.cancel' },
    );
    expect(deleteServerMock).toHaveBeenCalledWith({ serverId: 'local-test' });
    expect(getServersMock).toHaveBeenCalledTimes(2);
    expect(container.textContent).not.toContain('Local test server');
  });
});
