import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ExternalSourceApiError, externalSourcesAPI } from './ExternalSourcesAPI';
import { webSocketResponseError } from '../adapters/websocket-adapter';
import { PeerProductCommandError } from '../adapters/peer-device-adapter';
import { ApiClient } from './ApiClient';

const invokeMock = vi.hoisted(() => vi.fn());
const adapterMocks = vi.hoisted(() => ({
  request: vi.fn(),
  listen: vi.fn(),
  connect: vi.fn(),
  disconnect: vi.fn(),
  isConnected: vi.fn(() => true),
}));

function surface(catalog: Record<string, unknown>) {
  const generation = typeof catalog.generation === 'number' ? catalog.generation : 0;
  return {
    control: {
      schemaVersion: 1,
      executionDomainId: 'local-user',
      refreshGeneration: generation,
      preferenceRevision: typeof catalog.preferenceRevision === 'number'
        ? catalog.preferenceRevision
        : 0,
      safeMode: false,
      hostCapabilities: {
        canRefresh: true,
        canMutatePolicy: true,
        canManageSources: true,
        canApproveRuntime: true,
        canExecuteExternalAssets: true,
        canSetSafeMode: true,
        canRevealSourceLocation: true,
      },
      sources: [],
      capabilities: [],
      diagnostics: [],
      recoveryActions: [],
    },
    catalog,
  };
}

vi.mock('../adapters', async importOriginal => ({
  ...await importOriginal<typeof import('../adapters')>(),
  getTransportAdapter: () => adapterMocks,
}));

vi.mock('./ApiClient', async importOriginal => {
  const actual = await importOriginal<typeof import('./ApiClient')>();
  return {
    ...actual,
    api: {
      ...actual.api,
      invoke: invokeMock,
    },
  };
});

describe('ExternalSourcesAPI', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    invokeMock.mockResolvedValue(surface({}));
    adapterMocks.isConnected.mockReturnValue(true);
  });

  it('keeps workspace ownership and refresh intent in the public snapshot request', async () => {
    await externalSourcesAPI.getSnapshot('D:/workspace/project', true);

    expect(invokeMock).toHaveBeenCalledWith('get_external_source_control_snapshot', {
      request: {
        workspacePath: 'D:/workspace/project',
        forceRefresh: true,
      },
    });
  });

  it('treats an empty workspace path as the global scope', async () => {
    await externalSourcesAPI.getSnapshot('', false);

    expect(invokeMock).toHaveBeenCalledWith('get_external_source_control_snapshot', {
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

  it('expands a prompt command only with the selected candidate and behavior version', async () => {
    invokeMock.mockResolvedValueOnce({ content: 'expanded prompt' });

    await externalSourcesAPI.expandPromptCommand(
      'D:/workspace/project',
      'review',
      'focus on auth',
      'claude-code.commands:project:review',
      'behavior-v1',
      [{
        commandName: 'review',
        candidateId: 'bitfun.desktop:action:review',
        behaviorVersion: 'action:review:v1',
      }],
      {
        conflictKey: 'native:prompt_command:local-user:review:v1',
        expectedPreferenceRevision: 7,
      },
    );

    expect(invokeMock).toHaveBeenCalledWith('expand_external_prompt_command_command', {
      request: {
        workspacePath: 'D:/workspace/project',
        name: 'review',
        arguments: 'focus on auth',
        nativeCommands: [{
          commandName: 'review',
          candidateId: 'bitfun.desktop:action:review',
          behaviorVersion: 'action:review:v1',
        }],
        candidateId: 'claude-code.commands:project:review',
        expectedContentVersion: 'behavior-v1',
        expectedNativeConflictKey: 'native:prompt_command:local-user:review:v1',
        expectedPreferenceRevision: 7,
      },
    });
  });

  it('projects native prompt command conflicts through the shared control plane', async () => {
    invokeMock.mockResolvedValueOnce({ preferenceRevision: 3, conflicts: [] });
    const nativeCommands = [{
      commandName: 'review',
      candidateId: 'bitfun.desktop:action:review',
      behaviorVersion: 'action:review:v1',
    }];

    await externalSourcesAPI.getNativePromptCommandConflicts(
      'D:/workspace/project',
      nativeCommands,
    );

    expect(invokeMock).toHaveBeenCalledWith('get_native_prompt_command_conflicts_command', {
      request: {
        workspacePath: 'D:/workspace/project',
        nativeCommands,
      },
    });
  });

  it('persists a native prompt command choice with optimistic concurrency', async () => {
    invokeMock.mockResolvedValueOnce({ preferenceRevision: 4, conflicts: [] });
    const nativeCommands = [{
      commandName: 'review',
      candidateId: 'bitfun.desktop:action:review',
      behaviorVersion: 'action:review:v1',
    }];

    await externalSourcesAPI.setNativePromptCommandConflictChoice(
      'D:/workspace/project',
      nativeCommands,
      'bitfun.desktop:action:review',
      3,
    );

    expect(invokeMock).toHaveBeenCalledWith(
      'set_native_prompt_command_conflict_choice_command',
      {
        request: {
          workspacePath: 'D:/workspace/project',
          nativeCommands,
          selectedCandidateId: 'bitfun.desktop:action:review',
          expectedPreferenceRevision: 3,
        },
      },
    );
  });

  it('falls back to the legacy read path for an older Peer Host', async () => {
    invokeMock
      .mockRejectedValueOnce(
        "command 'get_external_source_control_snapshot' is not supported on CLI peer host",
      )
      .mockResolvedValueOnce({
        generation: 3,
        discoveryPending: false,
        preferenceRevision: 2,
        hostCapabilities: {
          canRefresh: true,
          canMutatePolicy: true,
          canManageSources: true,
          canApproveRuntime: true,
          canExecuteExternalAssets: true,
          canSetSafeMode: false,
        },
        sources: [],
        commands: [],
      });

    const result = await externalSourcesAPI.getSnapshot('D:/workspace/project');

    expect(invokeMock).toHaveBeenNthCalledWith(2, 'get_external_source_snapshot', {
      request: {
        workspacePath: 'D:/workspace/project',
        forceRefresh: false,
      },
    });
    expect(result.control).toMatchObject({
      executionDomainId: 'legacy-host',
      safeMode: false,
      hostCapabilities: {
        canManageSources: true,
        canSetSafeMode: false,
        canRevealSourceLocation: false,
      },
      recoveryActions: [{ type: 'reconnect_host' }],
    });
  });

  it('accepts a complete older Host surface when both capability copies omit additive fields', async () => {
    const legacyCapabilities = {
      canRefresh: true,
      canMutatePolicy: true,
      canManageSources: true,
      canApproveRuntime: true,
      canExecuteExternalAssets: true,
      canSetSafeMode: true,
    };
    invokeMock.mockResolvedValue({
      control: {
        ...surface({}).control,
        hostCapabilities: legacyCapabilities,
      },
      catalog: {
        generation: 0,
        discoveryPending: false,
        preferenceRevision: 0,
        hostCapabilities: legacyCapabilities,
        sources: [],
        commands: [],
      },
    });

    const result = await externalSourcesAPI.getSnapshot();

    expect(result.hostCapabilities.canRevealSourceLocation).toBe(false);
    expect(result.control?.hostCapabilities.canRevealSourceLocation).toBe(false);
  });

  it('preserves a legacy Server Host read-only boundary through ApiClient wrapping', async () => {
    const client = new ApiClient({ enableLogging: false, retries: 0 });
    adapterMocks.request.mockRejectedValueOnce(webSocketResponseError({
      code: -32004,
      message: 'Unknown Server Host operation',
      data: {
        code: 'host_capability_unavailable',
        detail: 'Unknown Server Host operation',
        retryable: false,
        stage: 'execute_remote',
        correlationId: 'server-legacy-1',
        recoveryActions: [{ type: 'reconnect_host' }],
      },
    }));
    invokeMock
      .mockImplementationOnce((command, args) => client.invoke(command, args))
      .mockResolvedValueOnce({
        generation: 3,
        discoveryPending: false,
        preferenceRevision: 2,
        hostCapabilities: {
          canRefresh: false,
          canMutatePolicy: false,
          canManageSources: false,
          canApproveRuntime: false,
          canExecuteExternalAssets: false,
          canSetSafeMode: false,
        },
        sources: [],
        commands: [],
      });

    const result = await externalSourcesAPI.getSnapshot('D:/workspace/project');

    expect(result.control.hostCapabilities).toEqual({
      canRefresh: false,
      canMutatePolicy: false,
      canManageSources: false,
      canApproveRuntime: false,
      canExecuteExternalAssets: false,
      canSetSafeMode: false,
      canRevealSourceLocation: false,
    });
    expect(result.hostCapabilities).toEqual(result.control.hostCapabilities);
  });

  it('uses the legacy source mutation when an older Peer Host lacks control actions', async () => {
    invokeMock
      .mockRejectedValueOnce(
        "command 'apply_external_source_control_action_command' is not supported on CLI peer host",
      )
      .mockResolvedValueOnce({})
      .mockRejectedValueOnce(
        "command 'get_external_source_control_snapshot' is not supported on CLI peer host",
      )
      .mockResolvedValueOnce({
        generation: 4,
        discoveryPending: false,
        preferenceRevision: 3,
        sources: [],
        commands: [],
      });

    await externalSourcesAPI.setSourceEnabled(
      'D:/workspace/project',
      'opencode.commands:project',
      false,
      2,
    );

    expect(invokeMock).toHaveBeenNthCalledWith(2, 'set_external_source_enabled_command', {
      request: {
        workspacePath: 'D:/workspace/project',
        sourceKey: 'opencode.commands:project',
        enabled: false,
        expectedPreferenceRevision: 2,
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
    invokeMock.mockResolvedValue(surface({
      generation: 1,
      discoveryPending: false,
      sources: [],
      commands: [],
    }));

    const result = await externalSourcesAPI.getSnapshot();

    expect(result.hostCapabilities).toEqual({
      canRefresh: false,
      canMutatePolicy: false,
      canManageSources: false,
      canApproveRuntime: false,
      canExecuteExternalAssets: false,
      canSetSafeMode: false,
      canRevealSourceLocation: false,
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
    invokeMock.mockResolvedValue(surface({
      generation: 1,
      discoveryPending: false,
      sources: [],
      commands: [],
      integrationPolicy: {
        schemaMajor: 13,
        status: 'incompatible_schema',
        futurePolicyField: { doNotExpose: true },
      },
    }));

    const result = await externalSourcesAPI.getSnapshot();

    expect(result.integrationPolicy).toEqual(expect.objectContaining({
      status: 'incompatible_schema',
      schemaMajor: 13,
      effective: { enabled: false, ecosystems: {} },
    }));
    expect(result.integrationPolicy).not.toHaveProperty('futurePolicyField');
  });

  it('normalizes partial snapshots returned from mutations too', async () => {
    invokeMock.mockResolvedValue(surface({
      generation: 2,
      discoveryPending: false,
      sources: [],
      commands: [],
    }));

    const result = await externalSourcesAPI.setSourceEnabled(
      'D:/workspace/project',
      'opencode:project',
      false,
      4,
    );

    expect(result.hostCapabilities.canManageSources).toBe(false);
    expect(result.integrationPolicy.status).toBe('unknown');
    expect(result.integrationPolicy.effective.enabled).toBe(false);
    expect(invokeMock).toHaveBeenCalledWith(
      'apply_external_source_control_action_command',
      {
        request: {
          workspacePath: 'D:/workspace/project',
          control: {
            schemaVersion: 1,
            operationId: expect.any(String),
            expectedPreferenceRevision: 4,
            action: {
              type: 'set_source_enabled',
              sourceKey: 'opencode:project',
              enabled: false,
            },
          },
        },
      },
    );
  });

  it('restores omitted empty MCP collections at the API boundary', async () => {
    invokeMock.mockResolvedValue(surface({
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
    }));

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
    invokeMock.mockResolvedValue(surface({
      generation: 4,
      discoveryPending: false,
      sources: [],
      commands: [],
      mcpServers: 'not-an-array',
    }));

    await expect(externalSourcesAPI.getSnapshot()).rejects.toMatchObject({
      code: 'internal',
      retryable: true,
    });

    invokeMock.mockResolvedValue(surface({
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
    }));

    await expect(externalSourcesAPI.getSnapshot()).rejects.toMatchObject({
      code: 'internal',
      retryable: true,
    });
  });

  it('preserves structured error facts for consistent recovery UX', async () => {
    invokeMock.mockRejectedValue(JSON.stringify({
      code: 'runtime_unavailable',
      detail: 'Node.js is not available',
      retryable: false,
      correlationId: 'correlation-1',
      causationId: 'refresh-8',
      stage: 'activate_runtime',
      recoveryActions: [{ type: 'install_runtime' }, { type: 'refresh' }],
    }));

    await expect(externalSourcesAPI.getSnapshot()).rejects.toMatchObject({
      code: 'runtime_unavailable',
      correlationId: 'correlation-1',
      causationId: 'refresh-8',
      stage: 'activate_runtime',
      recoveryActions: [{ type: 'install_runtime' }, { type: 'refresh' }],
    });
  });

  it('fails closed when the shared control projection contains malformed recovery actions', async () => {
    const malformed = surface({
      generation: 8,
      discoveryPending: false,
      sources: [],
      commands: [],
    });
    malformed.control.recoveryActions = [{ type: 'run_arbitrary_command' }];
    invokeMock.mockResolvedValue(malformed);

    await expect(externalSourcesAPI.getSnapshot()).rejects.toMatchObject({
      code: 'invalid_response',
      retryable: false,
    });
  });

  it('fails closed when v1 host capability facts are malformed or inconsistent', async () => {
    const malformed = surface({
      generation: 9,
      discoveryPending: false,
      sources: [],
      commands: [],
    });
    malformed.control.hostCapabilities.canSetSafeMode = 'yes' as unknown as boolean;
    invokeMock.mockResolvedValue(malformed);

    await expect(externalSourcesAPI.getSnapshot()).rejects.toMatchObject({
      code: 'invalid_response',
      retryable: false,
    });

    const inconsistent = surface({
      generation: 10,
      discoveryPending: false,
      sources: [],
      commands: [],
      hostCapabilities: {
        canRefresh: true,
        canMutatePolicy: true,
        canManageSources: true,
        canApproveRuntime: true,
        canExecuteExternalAssets: true,
        canSetSafeMode: false,
      },
    });
    invokeMock.mockResolvedValue(inconsistent);

    await expect(externalSourcesAPI.getSnapshot()).rejects.toMatchObject({
      code: 'invalid_response',
      retryable: false,
    });

    const stalePreferences = surface({
      generation: 11,
      preferenceRevision: 3,
      discoveryPending: false,
      sources: [],
      commands: [],
    });
    stalePreferences.control.preferenceRevision = 2;
    invokeMock.mockResolvedValue(stalePreferences);

    await expect(externalSourcesAPI.getSnapshot()).rejects.toMatchObject({
      code: 'invalid_response',
      retryable: true,
    });
  });

  it('preserves Peer Host typed recovery through ApiClient wrapping', async () => {
    const client = new ApiClient({ enableLogging: false, retries: 0 });
    adapterMocks.request.mockRejectedValueOnce(new PeerProductCommandError(JSON.stringify({
      code: 'stale_revision',
      detail: 'Refresh and try again',
      retryable: true,
      stage: 'apply_preference',
      causationId: 'peer-preference-8',
      recoveryActions: [{ type: 'refresh' }],
    })));
    invokeMock.mockImplementationOnce((command, args) => client.invoke(command, args));

    await expect(externalSourcesAPI.setSafeMode(
      'D:/workspace/project',
      true,
      8,
    )).rejects.toMatchObject({
      code: 'stale_revision',
      stage: 'apply_preference',
      causationId: 'peer-preference-8',
      recoveryActions: [{ type: 'refresh' }],
    });
  });

  it('bounds untrusted legacy error references before they reach a product surface', async () => {
    invokeMock.mockRejectedValue(JSON.stringify({
      code: 'stale_revision',
      detail: `retry\n${'x'.repeat(5000)}`,
      retryable: true,
      correlationId: 'forged\nreference',
      recoveryActions: [{ type: 'refresh' }],
    }));

    await expect(externalSourcesAPI.getSnapshot()).rejects.toMatchObject({
      code: 'stale_revision',
      correlationId: undefined,
      recoveryActions: [{ type: 'refresh' }],
    });
    await externalSourcesAPI.getSnapshot().catch((error: ExternalSourceApiError) => {
      expect(Array.from(error.message)).toHaveLength(4096);
      expect(error.message).not.toContain('\n');
    });
  });
});
