// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useSettingsStore } from '@/app/scenes/settings/settingsStore';
import ExternalMcpOverview from './ExternalMcpOverview';

const getSnapshotMock = vi.hoisted(() => vi.fn());
const workspaceState = vi.hoisted(() => ({ path: 'D:/workspace/project' }));
const peerState = vi.hoisted(() => ({ deviceId: '' }));
const warnMock = vi.hoisted(() => vi.fn());
const apiErrorState = vi.hoisted(() => ({
  ExternalSourceApiError: class ExternalSourceApiError extends Error {
    constructor(
      public readonly code: string,
      public readonly detail: string,
      public readonly retryable: boolean,
      public readonly correlationId?: string,
    ) {
      super(detail);
    }
  },
}));

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useCurrentWorkspace: () => ({
    workspace: { id: workspaceState.path, workspaceKind: 'local' },
    workspacePath: workspaceState.path,
  }),
}));
vi.mock('@/infrastructure/peer-device/PeerDeviceContext', () => ({
  usePeerDeviceModeOptional: () => ({
    peerMode: peerState.deviceId
      ? { active: true, deviceId: peerState.deviceId, deviceName: peerState.deviceId }
      : { active: false },
  }),
}));

vi.mock('@/infrastructure/api/service-api/ExternalSourcesAPI', () => ({
  ExternalSourceApiError: apiErrorState.ExternalSourceApiError,
  externalSourcesAPI: { getSnapshot: getSnapshotMock },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    trace: vi.fn(),
    debug: vi.fn(),
    info: vi.fn(),
    warn: warnMock,
    error: vi.fn(),
  }),
}));

const snapshot = {
  hostCapabilities: {
    canRefresh: true,
    canMutatePolicy: true,
    canManageSources: true,
    canApproveRuntime: true,
    canExecuteExternalAssets: true,
  },
  generation: 1,
  discoveryPending: false,
  sources: [{
    stableKey: 'opencode-mcp-project',
    lifecycle: 'available',
    record: {
      key: { providerId: 'opencode.mcp', sourceId: 'project' },
      ecosystemId: 'opencode',
      displayName: 'OpenCode project MCP',
      sourceKind: 'mcp',
      scope: 'project',
      location: '<workspace>/.opencode/opencode.json',
      executionDomainId: 'local:test',
      health: 'available',
      contentVersion: '1',
    },
  }],
  commands: [],
  mcpServers: [{
    candidateId: 'opencode-project-docs',
    approvalKey: 'approval-1',
    decisionKey: 'decision-1',
    activationState: { state: 'approval_required' },
    definition: {
      id: {
        source: { providerId: 'opencode.mcp', sourceId: 'project' },
        localId: 'docs',
      },
      provenance: [{ providerId: 'opencode.mcp', sourceId: 'project' }],
      name: 'docs',
      transport: 'local_stdio',
      argumentCount: 1,
      environmentKeys: [],
      headerNames: [],
      sourceEnabled: true,
      behaviorVersion: '1',
      staticStatus: { state: 'ready' },
    },
  }],
  integrationPolicy: {
    schemaMajor: 1,
    status: 'compatible',
    userDefaults: { enabled: true, ecosystems: {} },
    globalEffective: { enabled: true, ecosystems: {} },
    effective: { enabled: true, ecosystems: {} },
    registeredEcosystems: [{
      ecosystemId: 'opencode',
      displayName: 'OpenCode',
      adapterRevision: '1',
      capabilities: [],
    }],
  },
};

describe('ExternalMcpOverview', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    getSnapshotMock.mockReset().mockResolvedValue(snapshot);
    warnMock.mockReset();
    workspaceState.path = 'D:/workspace/project';
    peerState.deviceId = '';
    useSettingsStore.setState({ activeTab: 'mcp-tools', searchQuery: '' });
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
  });

  it('shows external source, scope, status, and configuration location', async () => {
    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(getSnapshotMock).toHaveBeenCalledWith('D:/workspace/project');
    expect(container.textContent).toContain('OpenCode');
    expect(container.textContent).toContain('external.scope.project');
    expect(container.textContent).toContain('external.status.approvalRequired');

    await act(async () => {
      (container.querySelector('[data-testid="external-mcp-item"] .bitfun-collection-item__details-toggle') as HTMLButtonElement).click();
    });
    expect(container.textContent).toContain('<workspace>/.opencode/opencode.json');
  });

  it('links the native MCP page to the external integration owner', async () => {
    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
      await Promise.resolve();
    });

    await act(async () => {
      (container.querySelector('[aria-label="external.manage"]') as HTMLButtonElement).click();
    });
    expect(useSettingsStore.getState().activeTab).toBe('external-sources');
  });

  it('clears the previous workspace snapshot while the next host loads or fails', async () => {
    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(container.textContent).toContain('OpenCode');

    let rejectNext: ((reason?: unknown) => void) | undefined;
    getSnapshotMock.mockImplementationOnce(() => new Promise((_, reject) => {
      rejectNext = reject;
    }));
    workspaceState.path = 'E:/workspace/other';

    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
    });

    expect(getSnapshotMock).toHaveBeenLastCalledWith('E:/workspace/other');
    expect(container.textContent).not.toContain('OpenCode');
    expect(container.textContent).not.toContain('<workspace>/.opencode/opencode.json');
    expect(container.textContent).toContain('external.loading');

    await act(async () => {
      rejectNext?.(new Error('host unavailable'));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('external.unavailable');
    expect(container.textContent).not.toContain('OpenCode');
  });

  it('does not project a snapshot returned by the previous Peer Host', async () => {
    let resolvePeerA: ((value: typeof snapshot) => void) | undefined;
    getSnapshotMock
      .mockImplementationOnce(() => new Promise<typeof snapshot>((resolve) => {
        resolvePeerA = resolve;
      }))
      .mockResolvedValueOnce({
        ...snapshot,
        mcpServers: [{
          ...snapshot.mcpServers[0],
          candidateId: 'peer-b-docs',
          definition: { ...snapshot.mcpServers[0].definition, name: 'peer-b-docs' },
        }],
      });

    peerState.deviceId = 'peer-a';
    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
    });
    peerState.deviceId = 'peer-b';
    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
      await Promise.resolve();
    });
    await act(async () => {
      resolvePeerA?.(snapshot);
      await Promise.resolve();
    });

    expect(Array.from(
      container.querySelectorAll('[data-testid="external-mcp-item"] .bitfun-collection-item__name'),
    ).map((node) => node.textContent)).toEqual(['peer-b-docs']);
  });

  it('logs typed failure facts without logging raw error details', async () => {
    getSnapshotMock.mockRejectedValue(new apiErrorState.ExternalSourceApiError(
      'unavailable',
      'failed at C:/private/workspace',
      true,
      'corr_123',
    ));

    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(warnMock).toHaveBeenCalledWith('Failed to load external MCP summary', {
      error_type: 'external_source_api',
      code: 'unavailable',
      correlation_id: 'corr_123',
      retryable: true,
    });
    expect(JSON.stringify(warnMock.mock.calls)).not.toContain('private/workspace');
  });

  it('projects discovery and read-only host state without reporting a false empty result', async () => {
    getSnapshotMock.mockResolvedValue({
      ...snapshot,
      hostCapabilities: {
        canRefresh: false,
        canMutatePolicy: false,
        canManageSources: false,
        canApproveRuntime: false,
        canExecuteExternalAssets: false,
      },
      discoveryPending: true,
      sources: [],
      mcpServers: [],
    });

    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('external.status.checking');
    expect(container.textContent).toContain('external.status.readOnly');
    expect(container.textContent).toContain('external.loading');
    expect(container.textContent).not.toContain('external.empty');
  });

  it('polls pending discovery until the external MCP snapshot settles', async () => {
    vi.useFakeTimers();
    try {
      getSnapshotMock
        .mockResolvedValueOnce({
          ...snapshot,
          discoveryPending: true,
          sources: [],
          mcpServers: [],
        })
        .mockResolvedValueOnce(snapshot);

      await act(async () => {
        root.render(<ExternalMcpOverview />);
        await Promise.resolve();
        await Promise.resolve();
      });
      expect(container.textContent).toContain('external.status.checking');

      await act(async () => {
        await vi.advanceTimersByTimeAsync(750);
        await Promise.resolve();
      });

      expect(getSnapshotMock).toHaveBeenCalledTimes(2);
      expect(container.textContent).toContain('docs');
      expect(container.textContent).not.toContain('external.status.checking');
    } finally {
      vi.useRealTimers();
    }
  });

  it('keeps the last external MCP snapshot when a refresh fails', async () => {
    vi.useFakeTimers();
    try {
      getSnapshotMock
        .mockResolvedValueOnce({ ...snapshot, discoveryPending: true })
        .mockRejectedValueOnce(new Error('refresh failed'));

      await act(async () => {
        root.render(<ExternalMcpOverview />);
        await Promise.resolve();
        await Promise.resolve();
      });
      await act(async () => {
        await vi.advanceTimersByTimeAsync(750);
        await Promise.resolve();
      });

      expect(container.textContent).toContain('docs');
      expect(container.textContent).toContain('external.status.stale');
      expect(container.querySelector('[aria-label="external.retry"]')).not.toBeNull();
      expect(container.textContent).not.toContain('external.unavailable');
    } finally {
      vi.useRealTimers();
    }
  });

  it('cancels pending discovery polling when the workspace changes', async () => {
    vi.useFakeTimers();
    try {
      getSnapshotMock.mockResolvedValueOnce({
        ...snapshot,
        discoveryPending: true,
        sources: [],
        mcpServers: [],
      });

      await act(async () => {
        root.render(<ExternalMcpOverview />);
        await Promise.resolve();
        await Promise.resolve();
      });

      getSnapshotMock.mockResolvedValueOnce(snapshot);
      workspaceState.path = 'E:/workspace/other';
      await act(async () => {
        root.render(<ExternalMcpOverview />);
        await Promise.resolve();
        await Promise.resolve();
      });

      await act(async () => {
        await vi.advanceTimersByTimeAsync(1500);
        await Promise.resolve();
      });

      expect(getSnapshotMock.mock.calls).toEqual([
        ['D:/workspace/project'],
        ['E:/workspace/other'],
      ]);
      expect(container.textContent).toContain('docs');
      expect(container.textContent).not.toContain('external.status.checking');
    } finally {
      vi.useRealTimers();
    }
  });

  it('backs off repeated pending discovery polling', async () => {
    vi.useFakeTimers();
    try {
      getSnapshotMock.mockResolvedValue({
        ...snapshot,
        discoveryPending: true,
        sources: [],
        mcpServers: [],
      });

      await act(async () => {
        root.render(<ExternalMcpOverview />);
        await Promise.resolve();
        await Promise.resolve();
      });

      await act(async () => vi.advanceTimersByTimeAsync(750));
      expect(getSnapshotMock).toHaveBeenCalledTimes(2);

      await act(async () => vi.advanceTimersByTimeAsync(1_499));
      expect(getSnapshotMock).toHaveBeenCalledTimes(2);
      await act(async () => vi.advanceTimersByTimeAsync(1));
      expect(getSnapshotMock).toHaveBeenCalledTimes(3);

      await act(async () => vi.advanceTimersByTimeAsync(2_999));
      expect(getSnapshotMock).toHaveBeenCalledTimes(3);
      await act(async () => vi.advanceTimersByTimeAsync(1));
      expect(getSnapshotMock).toHaveBeenCalledTimes(4);
    } finally {
      vi.useRealTimers();
    }
  });

  it('surfaces stale source health ahead of a misleading active status', async () => {
    getSnapshotMock.mockResolvedValue({
      ...snapshot,
      sources: [{
        ...snapshot.sources[0],
        lifecycle: 'using_last_valid_version',
      }],
      mcpServers: [{
        ...snapshot.mcpServers[0],
        activationState: { state: 'active' },
      }],
    });

    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('external.status.stale');
    expect(container.textContent).toContain('external.status.active');
  });

  it('surfaces degraded source diagnostics ahead of a misleading active status', async () => {
    getSnapshotMock.mockResolvedValue({
      ...snapshot,
      sources: [{
        ...snapshot.sources[0],
        record: {
          ...snapshot.sources[0].record,
          health: 'degraded',
          diagnostics: [{ severity: 'warning', code: 'source_stale', message: 'details' }],
        },
      }],
      mcpServers: snapshot.mcpServers,
    });

    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('external.status.degraded');
    expect(container.textContent).toContain('external.status.approvalRequired');
    expect(container.textContent).not.toContain('details');
  });

  it('summarizes top-level MCP diagnostics without exposing their raw message', async () => {
    getSnapshotMock.mockResolvedValue({
      ...snapshot,
      diagnostics: [{
        severity: 'warning',
        assetKind: 'mcp',
        code: 'opencode.mcp.server_invalid',
        message: 'failed at C:/private/workspace',
      }],
    });

    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('external.status.degraded');
    expect(container.textContent).toContain('external.status.approvalRequired');
    expect(container.textContent).not.toContain('private/workspace');
  });
});
