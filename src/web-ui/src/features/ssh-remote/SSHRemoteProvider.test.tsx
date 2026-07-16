// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { WorkspaceKind, WorkspaceType } from '@/shared/types/global-state';
import { notificationService } from '@/shared/notification-system';

import { SSHRemoteProvider } from './SSHRemoteProvider';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const workspaceManagerMock = vi.hoisted(() => ({
  getState: vi.fn(),
  addEventListener: vi.fn(),
  consumeStartupLegacyRemoteWorkspaceSnapshot: vi.fn(),
  openRemoteWorkspace: vi.fn(),
  removeRemoteWorkspace: vi.fn(),
}));

const sshApiMock = vi.hoisted(() => ({
  getWorkspaceInfo: vi.fn(),
  listSavedConnections: vi.fn(),
  hasStoredPassword: vi.fn(),
  isConnected: vi.fn(),
  connect: vi.fn(),
  openWorkspace: vi.fn(),
  disconnect: vi.fn(),
  closeWorkspace: vi.fn(),
  removeWorkspace: vi.fn(),
}));

vi.mock('@/infrastructure/services/business/workspaceManager', () => ({
  workspaceManager: workspaceManagerMock,
}));

vi.mock('./sshApi', () => ({
  sshApi: sshApiMock,
}));

vi.mock('@/flow_chat/store/FlowChatStore', () => ({
  flowChatStore: {
    initializeFromDisk: vi.fn(() => Promise.resolve()),
  },
}));

vi.mock('@/infrastructure/api/service-api/ACPClientAPI', () => ({
  ACPClientAPI: {
    probeClientRequirements: vi.fn(() => Promise.resolve()),
  },
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    warning: vi.fn(),
    error: vi.fn(),
    success: vi.fn(),
  },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

describe('SSHRemoteProvider startup restore', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    workspaceManagerMock.getState.mockReturnValue({
      loading: false,
      openedWorkspaces: new Map(),
      activeWorkspaceId: null,
    });
    workspaceManagerMock.addEventListener.mockReturnValue(() => undefined);
    workspaceManagerMock.consumeStartupLegacyRemoteWorkspaceSnapshot.mockReturnValue({
      available: true,
      workspace: null,
    });
    sshApiMock.getWorkspaceInfo.mockResolvedValue(null);
    sshApiMock.listSavedConnections.mockResolvedValue([]);
    sshApiMock.isConnected.mockResolvedValue(false);
    sshApiMock.connect.mockResolvedValue({ success: false, error: 'connection refused' });
    sshApiMock.openWorkspace.mockResolvedValue(undefined);
    sshApiMock.removeWorkspace.mockResolvedValue(undefined);
    workspaceManagerMock.removeRemoteWorkspace.mockResolvedValue(undefined);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.useRealTimers();
  });

  async function renderProvider(): Promise<void> {
    await act(async () => {
      root.render(
        <SSHRemoteProvider>
          <div />
        </SSHRemoteProvider>
      );
    });
    await act(async () => {
      await Promise.resolve();
    });
  }

  it('skips the legacy remote IPC when the startup snapshot is available', async () => {
    workspaceManagerMock.consumeStartupLegacyRemoteWorkspaceSnapshot.mockReturnValue({
      available: true,
      workspace: null,
    });

    await renderProvider();

    expect(workspaceManagerMock.consumeStartupLegacyRemoteWorkspaceSnapshot).toHaveBeenCalledTimes(1);
    expect(sshApiMock.getWorkspaceInfo).not.toHaveBeenCalled();
  });

  it('falls back to the legacy remote IPC when no startup snapshot is available', async () => {
    workspaceManagerMock.consumeStartupLegacyRemoteWorkspaceSnapshot.mockReturnValue({
      available: false,
      workspace: null,
    });

    await renderProvider();

    expect(sshApiMock.getWorkspaceInfo).toHaveBeenCalledTimes(1);
  });

  it('does not remove a disconnected remote workspace until the 180s reconnect budget elapses', async () => {
    vi.useFakeTimers();

    const remoteWorkspace = {
      id: 'ws-remote-1',
      name: 'repos',
      rootPath: '/root/repos',
      workspaceType: WorkspaceType.SingleProject,
      workspaceKind: WorkspaceKind.Remote,
      languages: [] as string[],
      openedAt: new Date().toISOString(),
      lastAccessed: new Date().toISOString(),
      tags: [] as string[],
      connectionId: 'conn-1',
      connectionName: 'dev-box',
      sshHost: 'example.com',
    };

    workspaceManagerMock.getState.mockReturnValue({
      loading: false,
      openedWorkspaces: new Map([[remoteWorkspace.id, remoteWorkspace]]),
      activeWorkspaceId: remoteWorkspace.id,
    });
    sshApiMock.listSavedConnections.mockResolvedValue([
      {
        id: 'conn-1',
        name: 'dev-box',
        host: 'example.com',
        port: 22,
        username: 'root',
        authType: { type: 'PrivateKey', keyPath: '/tmp/id_rsa' },
      },
    ]);

    await renderProvider();

    // Fast connect failures must keep retrying; workspace must stay until budget ends.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000);
    });
    expect(workspaceManagerMock.removeRemoteWorkspace).not.toHaveBeenCalled();
    expect(sshApiMock.removeWorkspace).not.toHaveBeenCalled();
    expect(sshApiMock.connect.mock.calls.length).toBeGreaterThan(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(120_000);
    });

    expect(workspaceManagerMock.removeRemoteWorkspace).toHaveBeenCalledWith('conn-1', '/root/repos');
    expect(sshApiMock.removeWorkspace).toHaveBeenCalledWith('conn-1', '/root/repos');
    expect(notificationService.error).toHaveBeenCalledWith(
      'Remote workspace could not reconnect within 180 seconds and was removed: /root/repos',
      { duration: 8000 }
    );
  });
});
