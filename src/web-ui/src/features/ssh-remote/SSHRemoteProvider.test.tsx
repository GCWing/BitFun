// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

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
    sshApiMock.getWorkspaceInfo.mockResolvedValue(null);
    sshApiMock.listSavedConnections.mockResolvedValue([]);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
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
});
