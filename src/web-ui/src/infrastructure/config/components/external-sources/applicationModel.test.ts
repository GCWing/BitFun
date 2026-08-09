import { describe, expect, it } from 'vitest';
import type {
  ExternalSourceCatalogSnapshot,
  ExternalSourceRecord,
} from '@/infrastructure/api/service-api/ExternalSourcesAPI';
import { buildExternalApplicationsView } from './applicationModel';

const CAPABILITIES = [
  { capabilityId: 'command', recommendedAccess: 'auto' as const, safetyCeiling: 'auto' as const },
  { capabilityId: 'tool', recommendedAccess: 'ask_before_use' as const, safetyCeiling: 'auto' as const },
];

function policy(
  overrides: Partial<ExternalSourceCatalogSnapshot['integrationPolicy']> = {},
): ExternalSourceCatalogSnapshot['integrationPolicy'] {
  return {
    schemaMajor: 1,
    status: 'compatible',
    userDefaults: { enabled: true, ecosystems: {} },
    globalEffective: { enabled: true, ecosystems: {} },
    effective: { enabled: true, ecosystems: {} },
    registeredEcosystems: [
      { ecosystemId: 'opencode', displayName: 'OpenCode', adapterRevision: 'r1', capabilities: CAPABILITIES },
      { ecosystemId: 'claude-code', displayName: 'Claude Code', adapterRevision: 'r1', capabilities: CAPABILITIES },
    ],
    ...overrides,
  };
}

function source(
  ecosystemId: string,
  overrides: Partial<ExternalSourceRecord> = {},
): ExternalSourceCatalogSnapshot['sources'][number] {
  return {
    stableKey: `${ecosystemId}-user`,
    lifecycle: 'available',
    record: {
      key: { providerId: `${ecosystemId}.commands`, sourceId: 'user' },
      ecosystemId,
      displayName: ecosystemId,
      sourceKind: 'configuration',
      scope: 'user_global',
      location: `~/.config/${ecosystemId}`,
      executionDomainId: 'local',
      health: 'available',
      contentVersion: 'v1',
      ...overrides,
    },
  };
}

function snapshot(
  overrides: Partial<ExternalSourceCatalogSnapshot> = {},
): ExternalSourceCatalogSnapshot {
  return {
    hostCapabilities: {
      canRefresh: true,
      canMutatePolicy: true,
      canManageSources: true,
      canApproveRuntime: true,
      canExecuteExternalAssets: true,
      canSetSafeMode: true,
      canRevealSourceLocation: true,
    },
    generation: 1,
    discoveryPending: false,
    sources: [],
    commands: [],
    tools: [],
    mcpServers: [],
    subagents: [],
    integrationPolicy: policy(),
    ...overrides,
  };
}

describe('external application model', () => {
  it('hides registered ecosystems that were neither discovered nor explicitly configured', () => {
    expect(buildExternalApplicationsView(snapshot(), 'workspace')).toEqual([]);
  });

  it('shows a discovered application with the effective policy state', () => {
    const opencodePolicy = {
      ecosystemId: 'opencode',
      mode: 'recommended' as const,
      capabilities: {},
    };
    const result = buildExternalApplicationsView(snapshot({
      sources: [source('opencode')],
      integrationPolicy: policy({
        globalEffective: { enabled: true, ecosystems: { opencode: opencodePolicy } },
        effective: { enabled: true, ecosystems: { opencode: opencodePolicy } },
      }),
    }), 'workspace');

    expect(result).toEqual([{
      ecosystemId: 'opencode',
      displayName: 'OpenCode',
      enabled: true,
      attentionCount: 0,
    }]);
  });

  it('keeps an explicitly disabled application visible after its source disappears', () => {
    const disabled = { ecosystemId: 'opencode', mode: 'disabled' as const, capabilities: {} };
    const result = buildExternalApplicationsView(snapshot({
      integrationPolicy: policy({
        userDefaults: {
          enabled: true,
          ecosystems: { opencode: { mode: 'disabled' } },
        },
        globalEffective: { enabled: true, ecosystems: { opencode: disabled } },
        effective: { enabled: true, ecosystems: { opencode: disabled } },
      }),
    }), 'workspace');

    expect(result).toMatchObject([{
      ecosystemId: 'opencode',
      enabled: false,
    }]);
  });

  it('signals an owner permission without exposing a review workflow', () => {
    const result = buildExternalApplicationsView(snapshot({
      sources: [source('opencode')],
      toolApprovalRequests: [{
        approvalKey: 'approval-1',
        decisionKey: 'decision-1',
        targetId: {
          source: { providerId: 'opencode.commands', sourceId: 'user' },
          localId: 'tool-a',
        },
        sourceDisplayName: 'OpenCode',
        sourceLocation: '~/.config/opencode',
        sourceScope: 'user_global',
        toolNames: ['tool-a'],
        runtimeKind: 'node',
        workingDirectory: '~/.config/opencode',
        capabilities: ['file_system'],
        contentVersion: 'v1',
      }],
    }), 'workspace');

    expect(result[0]).toMatchObject({
      ecosystemId: 'opencode',
      attentionCount: 1,
    });
  });

  it('keeps a cross-application conflict out of either application row', () => {
    const result = buildExternalApplicationsView(snapshot({
      sources: [source('opencode'), source('claude-code')],
      commandConflicts: [{
        conflictKey: 'conflict-1',
        commandName: 'test',
        candidates: [
          {
            candidateId: 'opencode-test',
            source: { providerId: 'opencode.commands', sourceId: 'user' },
            sourceDisplayName: 'OpenCode',
            ecosystemId: 'opencode',
            contentVersion: 'v1',
            commandDescription: 'Test',
            sourceScope: 'user_global',
            sourceLocation: '~/.config/opencode',
            availability: { state: 'available' },
          },
          {
            candidateId: 'claude-test',
            source: { providerId: 'claude-code.commands', sourceId: 'user' },
            sourceDisplayName: 'Claude Code',
            ecosystemId: 'claude-code',
            contentVersion: 'v1',
            commandDescription: 'Test',
            sourceScope: 'user_global',
            sourceLocation: '~/.claude',
            availability: { state: 'available' },
          },
        ],
      }],
    }), 'workspace');

    expect(result.every((application) => application.attentionCount === 0))
      .toBe(true);
  });
});
