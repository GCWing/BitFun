import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ExternalSourceApiError, externalSourcesAPI } from './ExternalSourcesAPI';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({
  api: {
    invoke: invokeMock,
  },
}));

describe('ExternalSourcesAPI', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue({});
  });

  it('keeps workspace ownership and refresh intent in the public snapshot request', async () => {
    await externalSourcesAPI.getSnapshot('D:/workspace/project', true);

    expect(invokeMock).toHaveBeenCalledWith('get_external_source_snapshot', {
      request: {
        workspacePath: 'D:/workspace/project',
        forceRefresh: true,
      },
    });
  });

  it('treats an empty workspace path as the global scope', async () => {
    await externalSourcesAPI.getSnapshot('', false);

    expect(invokeMock).toHaveBeenCalledWith('get_external_source_snapshot', {
      request: {
        workspacePath: undefined,
        forceRefresh: false,
      },
    });
  });

  it('reveals a source by stable identity without sending its display location', async () => {
    await externalSourcesAPI.revealSourceLocation(
      'D:/workspace/project',
      'opencode.commands:project',
    );

    expect(invokeMock).toHaveBeenCalledWith('reveal_external_source_location', {
      request: {
        workspacePath: 'D:/workspace/project',
        sourceKey: 'opencode.commands:project',
      },
    });
  });

  it('sends policy scope and optimistic revision as one atomic mutation', async () => {
    await externalSourcesAPI.updateIntegrationPolicy('D:/workspace/project', {
      expectedPreferenceRevision: 8,
      scope: 'workspace',
      change: {
        operation: 'set_capability_access',
        ecosystemId: 'opencode',
        capabilityId: 'mcp',
        access: 'ask_before_use',
      },
    });

    expect(invokeMock).toHaveBeenCalledWith('update_external_integration_policy_command', {
      request: {
        workspacePath: 'D:/workspace/project',
        mutation: {
          expectedPreferenceRevision: 8,
          scope: 'workspace',
          change: {
            operation: 'set_capability_access',
            ecosystemId: 'opencode',
            capabilityId: 'mcp',
            access: 'ask_before_use',
          },
        },
      },
    });
  });

  it('normalizes typed host errors without matching user-visible strings', async () => {
    invokeMock.mockRejectedValue({
      details: {
        originalError: JSON.stringify({
          code: 'host_capability_unavailable',
          detail: 'This host is read-only',
          retryable: false,
        }),
      },
    });

    const request = externalSourcesAPI.getSnapshot();
    await expect(request).rejects.toBeInstanceOf(ExternalSourceApiError);
    await expect(request).rejects.toMatchObject({
      code: 'host_capability_unavailable',
      detail: 'This host is read-only',
      retryable: false,
    });
  });

  it('fails closed when a legacy host omits capabilities and policy', async () => {
    invokeMock.mockResolvedValue({
      generation: 1,
      discoveryPending: false,
      sources: [],
      commands: [],
    });

    const result = await externalSourcesAPI.getSnapshot();

    expect(result.hostCapabilities).toEqual({
      canRefresh: false,
      canMutatePolicy: false,
      canManageSources: false,
      canApproveRuntime: false,
      canExecuteExternalAssets: false,
    });
    expect(result.integrationPolicy).toMatchObject({
      status: 'unknown',
      schemaMajor: 0,
      userDefaults: { enabled: false },
      globalEffective: { enabled: false, ecosystems: {} },
      effective: { enabled: false, ecosystems: {} },
    });
  });

  it('keeps an incompatible schema identifiable while projecting it safely off', async () => {
    invokeMock.mockResolvedValue({
      generation: 1,
      discoveryPending: false,
      sources: [],
      commands: [],
      integrationPolicy: {
        schemaMajor: 13,
        status: 'incompatible_schema',
        futurePolicyField: { doNotExpose: true },
      },
    });

    const result = await externalSourcesAPI.getSnapshot();

    expect(result.integrationPolicy).toEqual(expect.objectContaining({
      status: 'incompatible_schema',
      schemaMajor: 13,
      effective: { enabled: false, ecosystems: {} },
    }));
    expect(result.integrationPolicy).not.toHaveProperty('futurePolicyField');
  });

  it('normalizes partial snapshots returned from mutations too', async () => {
    invokeMock.mockResolvedValue({ generation: 2, discoveryPending: false, sources: [], commands: [] });

    const result = await externalSourcesAPI.setSourceEnabled(
      'D:/workspace/project',
      'opencode:project',
      false,
      4,
    );

    expect(result.hostCapabilities.canManageSources).toBe(false);
    expect(result.integrationPolicy.status).toBe('unknown');
    expect(result.integrationPolicy.effective.enabled).toBe(false);
  });

  it('restores omitted empty MCP collections at the API boundary', async () => {
    invokeMock.mockResolvedValue({
      generation: 3,
      discoveryPending: false,
      sources: [{
        stableKey: 'opencode-user',
        lifecycle: 'available',
        record: {
          key: { providerId: 'opencode.mcp', sourceId: 'user' },
          ecosystemId: 'opencode',
          displayName: 'OpenCode user configuration',
          sourceKind: 'opencode_mcp_config',
          scope: 'user_global',
          location: '~/.config/opencode/opencode.json',
          executionDomainId: 'local-user',
          health: 'available',
          contentVersion: '1',
        },
      }],
      commands: [],
      mcpServers: [{
        candidateId: 'opencode-user-docs',
        approvalKey: 'approval',
        decisionKey: 'decision',
        activationState: { state: 'approval_required' },
        definition: {
          id: {
            source: { providerId: 'opencode.mcp', sourceId: 'user' },
            localId: 'docs',
          },
          name: 'docs',
          transport: 'streamable_http',
          argumentCount: 0,
          sourceEnabled: true,
          behaviorVersion: '1',
          staticStatus: { state: 'ready' },
        },
      }],
    });

    const result = await externalSourcesAPI.getSnapshot();

    expect(result.sources[0].record.diagnostics).toEqual([]);
    expect(result.mcpServers?.[0].definition).toMatchObject({
      provenance: [],
      environmentKeys: [],
      environmentReferenceNames: [],
      headerNames: [],
    });
    expect(result.mcpApprovalRequests).toEqual([]);
    expect(result.toolConflicts).toEqual([]);
    expect(result.pendingSubagentApprovals).toEqual([]);
    expect(result.diagnostics).toEqual([]);
  });

  it('rejects non-array collection fields instead of presenting them as empty', async () => {
    invokeMock.mockResolvedValue({
      generation: 4,
      discoveryPending: false,
      sources: [],
      commands: [],
      mcpServers: 'not-an-array',
    });

    await expect(externalSourcesAPI.getSnapshot()).rejects.toMatchObject({
      code: 'internal',
      retryable: true,
    });

    invokeMock.mockResolvedValue({
      generation: 5,
      discoveryPending: false,
      sources: [],
      commands: [],
      mcpServers: [{
        candidateId: 'invalid-collections',
        approvalKey: 'approval',
        decisionKey: 'decision',
        activationState: { state: 'approval_required' },
        definition: {
          id: {
            source: { providerId: 'opencode.mcp', sourceId: 'user' },
            localId: 'docs',
          },
          name: 'docs',
          transport: 'streamable_http',
          argumentCount: 0,
          environmentKeys: { unexpected: true },
          sourceEnabled: true,
          behaviorVersion: '1',
          staticStatus: { state: 'ready' },
        },
      }],
    });

    await expect(externalSourcesAPI.getSnapshot()).rejects.toMatchObject({
      code: 'internal',
      retryable: true,
    });
  });
});
