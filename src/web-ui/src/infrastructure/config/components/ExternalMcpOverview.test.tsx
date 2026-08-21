// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import ExternalMcpOverview from './ExternalMcpOverview';

const getSnapshotMock = vi.hoisted(() => vi.fn());
const planMcpImportMock = vi.hoisted(() => vi.fn());
const applyMcpImportMock = vi.hoisted(() => vi.fn());
const workspaceState = vi.hoisted(() => ({ path: 'D:/workspace/project', kind: 'normal' }));
const peerState = vi.hoisted(() => ({ deviceId: '' }));
const warnMock = vi.hoisted(() => vi.fn());
const openEcosystemCompatibilityMock = vi.hoisted(() => vi.fn());
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
    workspace: { id: workspaceState.path, workspaceKind: workspaceState.kind },
    workspacePath: workspaceState.path,
  }),
}));
vi.mock('@/infrastructure/peer-device/peerDeviceContextState', () => ({
  usePeerDeviceModeOptional: () => ({
    peerMode: peerState.deviceId
      ? { active: true, deviceId: peerState.deviceId, deviceName: peerState.deviceId }
      : { active: false },
  }),
}));

vi.mock('@/infrastructure/api/service-api/ExternalSourcesAPI', () => ({
  ExternalSourceApiError: apiErrorState.ExternalSourceApiError,
  externalSourcesAPI: {
    getSnapshot: getSnapshotMock,
    planMcpImport: planMcpImportMock,
    applyMcpImport: applyMcpImportMock,
  },
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
vi.mock('@/app/scenes/ecosystem-compatibility/ecosystemCompatibilityStore', () => ({
  openEcosystemCompatibility: openEcosystemCompatibilityMock,
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
    planMcpImportMock.mockReset().mockResolvedValue({
      schemaVersion: 1,
      planFingerprint: 'sha256:plan-v1',
      items: [{
        candidateId: 'opencode-project-docs',
        displayName: 'docs',
        transport: 'local_stdio',
        proposedNativeId: 'docs',
        disposition: 'eligible',
      }],
    });
    applyMcpImportMock.mockReset().mockResolvedValue({
      schemaVersion: 1,
      outcome: {
        status: 'applied',
        imported: [{ candidateId: 'opencode-project-docs', nativeId: 'docs' }],
      },
    });
    warnMock.mockReset();
    workspaceState.path = 'D:/workspace/project';
    workspaceState.kind = 'normal';
    peerState.deviceId = '';
    openEcosystemCompatibilityMock.mockReset();
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

  it('links the native MCP page to ecosystem source governance', async () => {
    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
      await Promise.resolve();
    });

    await act(async () => {
      (container.querySelector('[aria-label="external.manage"]') as HTMLButtonElement).click();
    });
    expect(openEcosystemCompatibilityMock).toHaveBeenCalledWith({
      ownerSurface: 'external-sources',
    });
  });

  it('previews before applying and delegates later enablement to MCP settings', async () => {
    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
      await Promise.resolve();
    });
    const importArea = container.querySelector('[data-testid="external-mcp-import"]')!;
    await act(async () => {
      (importArea.querySelector('button') as HTMLButtonElement).click();
      await Promise.resolve();
    });
    expect(planMcpImportMock).toHaveBeenCalledWith('D:/workspace/project');
    expect(importArea.textContent).toContain('external.import.confirm');
    await act(async () => {
      (importArea.querySelector('button') as HTMLButtonElement).click();
      await Promise.resolve();
    });
    expect(applyMcpImportMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      expect.objectContaining({ planFingerprint: 'sha256:plan-v1' }),
      [{ candidateId: 'opencode-project-docs' }],
    );
    expect(importArea.textContent).toContain('external.import.applied');
  });

  it('locks import choices while an apply request is in flight', async () => {
    let resolveApply!: (value: unknown) => void;
    applyMcpImportMock.mockImplementationOnce(() => new Promise((resolve) => {
      resolveApply = resolve;
    }));
    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
      await Promise.resolve();
    });
    const importArea = container.querySelector('[data-testid="external-mcp-import"]')!;
    await act(async () => {
      (importArea.querySelector('button') as HTMLButtonElement).click();
      await Promise.resolve();
    });
    const checkbox = importArea.querySelector('input[type="checkbox"]') as HTMLInputElement;

    await act(async () => {
      (importArea.querySelector('.bitfun-mcp-tools__import-actions button') as HTMLButtonElement).click();
      await Promise.resolve();
    });
    expect(checkbox.disabled).toBe(true);

    await act(async () => {
      resolveApply({
        schemaVersion: 1,
        outcome: {
          status: 'applied',
          imported: [{ candidateId: 'opencode-project-docs', nativeId: 'docs' }],
        },
      });
      await Promise.resolve();
    });
  });

  it('defaults eligible imports to selected and allows a compact per-item choice with source scope', async () => {
    const codexSource = {
      stableKey: 'codex-mcp-user',
      lifecycle: 'available',
      record: {
        key: { providerId: 'codex.mcp', sourceId: 'user' },
        ecosystemId: 'codex',
        displayName: 'Codex user MCP',
        sourceKind: 'mcp',
        scope: 'user_global',
        location: '~/.codex/config.toml',
        executionDomainId: 'local:test',
        health: 'available',
        contentVersion: '1',
      },
    };
    getSnapshotMock.mockResolvedValue({
      ...snapshot,
      sources: [...snapshot.sources, codexSource],
      mcpServers: [
        snapshot.mcpServers[0],
        {
          ...snapshot.mcpServers[0],
          candidateId: 'codex-user-search',
          definition: {
            ...snapshot.mcpServers[0].definition,
            id: {
              source: { providerId: 'codex.mcp', sourceId: 'user' },
              localId: 'search',
            },
            provenance: [{ providerId: 'codex.mcp', sourceId: 'user' }],
            name: 'search',
          },
        },
      ],
      integrationPolicy: {
        ...snapshot.integrationPolicy,
        registeredEcosystems: [
          ...snapshot.integrationPolicy.registeredEcosystems,
          {
            ecosystemId: 'codex',
            displayName: 'Codex',
            adapterRevision: '1',
            capabilities: [],
          },
        ],
      },
    });
    planMcpImportMock.mockResolvedValue({
      schemaVersion: 1,
      planFingerprint: 'sha256:multi',
      items: [
        {
          candidateId: 'opencode-project-docs',
          displayName: 'docs',
          transport: 'local_stdio',
          proposedNativeId: 'docs',
          disposition: 'eligible',
        },
        {
          candidateId: 'codex-user-search',
          displayName: 'search',
          transport: 'local_stdio',
          proposedNativeId: 'search',
          disposition: 'eligible',
        },
      ],
    });

    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
      await Promise.resolve();
    });
    const importArea = container.querySelector('[data-testid="external-mcp-import"]')!;
    await act(async () => {
      (importArea.querySelector('button') as HTMLButtonElement).click();
      await Promise.resolve();
    });

    const checkboxes = Array.from(importArea.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'));
    expect(checkboxes).toHaveLength(2);
    expect(checkboxes.every((checkbox) => checkbox.checked)).toBe(true);
    expect(importArea.textContent).toContain('OpenCode');
    expect(importArea.textContent).toContain('external.scope.project');
    expect(importArea.textContent).toContain('Codex');
    expect(importArea.textContent).toContain('external.scope.userGlobal');

    await act(async () => {
      checkboxes[1].click();
    });
    await act(async () => {
      (importArea.querySelector('.bitfun-mcp-tools__import-actions button') as HTMLButtonElement).click();
      await Promise.resolve();
    });
    expect(applyMcpImportMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      expect.objectContaining({ planFingerprint: 'sha256:multi' }),
      [{ candidateId: 'opencode-project-docs' }],
    );
  });

  it('keeps only still-eligible previous selections when a stale plan is refreshed', async () => {
    planMcpImportMock.mockResolvedValue({
      schemaVersion: 1,
      planFingerprint: 'sha256:old',
      items: [
        {
          candidateId: 'opencode-project-docs',
          displayName: 'docs',
          transport: 'local_stdio',
          proposedNativeId: 'docs',
          disposition: 'eligible',
        },
        {
          candidateId: 'old-candidate',
          displayName: 'old',
          transport: 'local_stdio',
          proposedNativeId: 'old',
          disposition: 'eligible',
        },
      ],
    });
    applyMcpImportMock.mockResolvedValue({
      schemaVersion: 1,
      outcome: {
        status: 'stale',
        refreshedPlan: {
          schemaVersion: 1,
          planFingerprint: 'sha256:new',
          items: [
            {
              candidateId: 'opencode-project-docs',
              displayName: 'docs',
              transport: 'local_stdio',
              proposedNativeId: 'docs',
              disposition: 'eligible',
            },
            {
              candidateId: 'new-candidate',
              displayName: 'new',
              transport: 'local_stdio',
              proposedNativeId: 'new',
              disposition: 'eligible',
            },
          ],
        },
      },
    });

    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
      await Promise.resolve();
    });
    const importArea = container.querySelector('[data-testid="external-mcp-import"]')!;
    await act(async () => {
      (importArea.querySelector('button') as HTMLButtonElement).click();
      await Promise.resolve();
    });
    let checkboxes = Array.from(importArea.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'));
    await act(async () => {
      checkboxes[1].click();
    });
    await act(async () => {
      (importArea.querySelector('.bitfun-mcp-tools__import-actions button') as HTMLButtonElement).click();
      await Promise.resolve();
    });

    checkboxes = Array.from(importArea.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'));
    expect(checkboxes).toHaveLength(2);
    expect(checkboxes[0].checked).toBe(true);
    expect(checkboxes[1].checked).toBe(false);
    expect(importArea.textContent).toContain('external.import.stale');
  });

  it('does not expose import mutation in Peer or remote workspace mode', async () => {
    peerState.deviceId = 'peer-a';
    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(container.querySelector('[data-testid="external-mcp-import"]')).toBeNull();
    expect(planMcpImportMock).not.toHaveBeenCalled();

    peerState.deviceId = '';
    workspaceState.kind = 'remote';
    await act(async () => {
      root.render(<ExternalMcpOverview />);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(container.querySelector('[data-testid="external-mcp-import"]')).toBeNull();
    expect(applyMcpImportMock).not.toHaveBeenCalled();
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

  it('shows a neutral stale badge when an active server uses the last working content', async () => {
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

    const stale = container.querySelector('[data-bf-state="stale"]');
    expect(stale?.textContent).toContain('external.status.stale');
    expect(stale?.classList.contains('is-error')).toBe(false);
    expect(stale?.classList.contains('is-pending')).toBe(false);
  });

  it('surfaces degraded source diagnostics ahead of a misleading active status', async () => {
    getSnapshotMock.mockResolvedValue({
      ...snapshot,
      sources: [{
        ...snapshot.sources[0],
        record: {
          ...snapshot.sources[0].record,
          health: 'degraded',
          diagnostics: [{
            severity: 'warning',
            code: 'source_stale',
            message: 'sensitive source diagnostic',
          }],
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
    expect(container.textContent).not.toContain('sensitive source diagnostic');
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
