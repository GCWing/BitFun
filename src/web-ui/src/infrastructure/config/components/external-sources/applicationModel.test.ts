import { describe, expect, it } from 'vitest';
import type {
  ExternalSourceCatalogSnapshot,
  ExternalSourceRecord,
} from '@/infrastructure/api/service-api/ExternalSourcesAPI';
import { buildExternalSourcePresentationGroups } from '../../externalSourcePresentation';
import { buildExternalApplicationsView } from './applicationModel';

const OPENCODE_CAPABILITIES = [
  { capabilityId: 'command', recommendedAccess: 'auto' as const, safetyCeiling: 'auto' as const },
  { capabilityId: 'tool', recommendedAccess: 'ask_before_use' as const, safetyCeiling: 'auto' as const },
  { capabilityId: 'subagent', recommendedAccess: 'ask_before_use' as const, safetyCeiling: 'auto' as const },
  { capabilityId: 'mcp', recommendedAccess: 'ask_before_use' as const, safetyCeiling: 'auto' as const },
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
      { ecosystemId: 'opencode', displayName: 'OpenCode', adapterRevision: 'r1', capabilities: OPENCODE_CAPABILITIES },
      { ecosystemId: 'claude-code', displayName: 'Claude Code', adapterRevision: 'r1', capabilities: OPENCODE_CAPABILITIES },
    ],
    ...overrides,
  };
}

function withMode(
  ecosystemId: string,
  mode: 'recommended' | 'discover_only' | 'disabled' | 'custom',
): ExternalSourceCatalogSnapshot['integrationPolicy'] {
  const ecosystems = {
    [ecosystemId]: { ecosystemId, mode, capabilities: {} },
  };
  return policy({
    effective: { enabled: true, ecosystems },
    globalEffective: { enabled: true, ecosystems },
  });
}

function source(
  stableKey: string,
  ecosystemId: string,
  overrides: Partial<ExternalSourceRecord> = {},
): ExternalSourceCatalogSnapshot['sources'][number] {
  return {
    stableKey,
    presentationGroupId: `${ecosystemId}-config`,
    lifecycle: 'available',
    record: {
      key: { providerId: `${ecosystemId}.commands`, sourceId: 'user-configuration' },
      ecosystemId,
      displayName: `${ecosystemId} configuration`,
      sourceKind: 'configuration',
      scope: 'user_global',
      location: `~/.config/${ecosystemId}/config.json`,
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

function view(input: ExternalSourceCatalogSnapshot) {
  return buildExternalApplicationsView(
    input,
    buildExternalSourcePresentationGroups(input),
    'workspace',
  );
}

describe('external application model', () => {
  it('lists every registered ecosystem even when nothing was discovered', () => {
    const result = view(snapshot());

    expect(result.applications.map((application) => application.ecosystemId))
      .toEqual(['opencode', 'claude-code']);
    expect(result.applications[0].status).toBe('no_configuration');
    expect(result.applications[0].primaryAction).toBe('none');
  });

  it('reports checking while discovery is still running', () => {
    const result = view(snapshot({ discoveryPending: true }));

    expect(result.applications[0].status).toBe('checking');
  });

  it('treats a recommended ecosystem with sources as connected', () => {
    const result = view(snapshot({
      sources: [source('opencode-user', 'opencode')],
      integrationPolicy: withMode('opencode', 'recommended'),
    }));

    const opencode = result.applications[0];
    expect(opencode.status).toBe('connected');
    expect(opencode.primaryAction).toBe('manage');
    expect(opencode.enabled).toBe(true);
  });

  it('reports only capability types that are active under the effective policy', () => {
    const result = view(snapshot({
      sources: [source('opencode-user', 'opencode')],
      integrationPolicy: withMode('opencode', 'recommended'),
      commands: [{
        candidateId: 'command-1',
        definition: {
          id: {
            source: { providerId: 'opencode.commands', sourceId: 'user-configuration' },
            localId: 'review',
          },
          name: 'review',
          description: 'Review',
          availability: { state: 'available' },
          contentVersion: 'v1',
        },
      }],
    }));

    expect(result.applications[0].activeCapabilities).toEqual([
      { capabilityId: 'command', count: 1 },
    ]);
  });

  it('keeps recommended access separate from the authoritative effective access', () => {
    const disabledCommandPolicy = policy({
      effective: {
        enabled: true,
        ecosystems: {
          opencode: {
            ecosystemId: 'opencode',
            mode: 'custom',
            capabilities: { command: 'disabled' },
          },
        },
      },
      globalEffective: {
        enabled: true,
        ecosystems: {
          opencode: {
            ecosystemId: 'opencode',
            mode: 'custom',
            capabilities: { command: 'disabled' },
          },
        },
      },
    });
    const result = view(snapshot({
      sources: [source('opencode-user', 'opencode')],
      integrationPolicy: disabledCommandPolicy,
      commands: [{
        candidateId: 'command-1',
        definition: {
          id: {
            source: { providerId: 'opencode.commands', sourceId: 'user-configuration' },
            localId: 'review',
          },
          name: 'review',
          description: 'Review',
          availability: { state: 'available' },
          contentVersion: 'v1',
        },
      }],
    }));

    expect(result.applications[0].connectPlan.find(
      (entry) => entry.capabilityId === 'command',
    )).toMatchObject({
      recommendedAccess: 'auto',
      effectiveAccess: 'disabled',
      count: 1,
    });
  });

  it('keeps custom ecosystems on manage so a two-state toggle cannot flatten them', () => {
    const result = view(snapshot({
      sources: [source('opencode-user', 'opencode')],
      integrationPolicy: withMode('opencode', 'custom'),
    }));

    expect(result.applications[0].status).toBe('connected_custom');
    expect(result.applications[0].primaryAction).toBe('manage');
  });

  it('offers connect for a discovered but discover-only ecosystem', () => {
    const result = view(snapshot({
      sources: [source('opencode-user', 'opencode')],
      integrationPolicy: withMode('opencode', 'discover_only'),
    }));

    expect(result.applications[0].status).toBe('discovered');
    expect(result.applications[0].primaryAction).toBe('connect');
    expect(result.applications[0].enabled).toBe(false);
  });

  it('attributes tool approvals to the owning ecosystem', () => {
    const result = view(snapshot({
      sources: [source('opencode-user', 'opencode')],
      integrationPolicy: withMode('opencode', 'recommended'),
      toolApprovalRequests: [{
        approvalKey: 'approval-1',
        decisionKey: 'decision-1',
        targetId: {
          source: { providerId: 'opencode.commands', sourceId: 'user-configuration' },
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
    }));

    const opencode = result.applications[0];
    expect(opencode.attentionCount).toBe(1);
    expect(opencode.status).toBe('needs_attention');
    expect(opencode.primaryAction).toBe('review');
    expect(result.unattributedAttentionCount).toBe(0);
  });

  it('keeps catalog diagnostics and policy incompatibility out of per-application counts', () => {
    const result = view(snapshot({
      sources: [source('opencode-user', 'opencode')],
      integrationPolicy: policy({ status: 'incompatible_schema' }),
    }));

    expect(result.applications.every((application) => application.attentionCount === 0))
      .toBe(true);
    expect(result.unattributedAttentionCount).toBe(0);
    expect(result.totalAttentionCount).toBe(0);
  });

  it('does not attribute a conflict that spans two ecosystems', () => {
    const result = view(snapshot({
      sources: [source('opencode-user', 'opencode'), source('claude-user', 'claude-code')],
      commandConflicts: [{
        conflictKey: 'conflict-1',
        commandName: 'review',
        candidates: [
          {
            candidateId: 'candidate-opencode',
            source: { providerId: 'opencode.commands', sourceId: 'user-configuration' },
            sourceDisplayName: 'OpenCode',
            ecosystemId: 'opencode',
            contentVersion: 'v1',
            commandDescription: 'Review',
            sourceScope: 'user_global',
            sourceLocation: '~/.config/opencode',
            availability: { state: 'available' },
          },
          {
            candidateId: 'candidate-claude',
            source: { providerId: 'claude-code.commands', sourceId: 'user-configuration' },
            sourceDisplayName: 'Claude Code',
            ecosystemId: 'claude-code',
            contentVersion: 'v1',
            commandDescription: 'Review',
            sourceScope: 'user_global',
            sourceLocation: '~/.claude',
            availability: { state: 'available' },
          },
        ],
      }],
    }));

    expect(result.applications.every((application) => application.attentionCount === 0))
      .toBe(true);
    expect(result.unattributedAttentionCount).toBe(1);
  });

  it('ignores conflicts the user already resolved', () => {
    const result = view(snapshot({
      sources: [source('opencode-user', 'opencode')],
      commandConflicts: [{
        conflictKey: 'conflict-1',
        commandName: 'review',
        selectedCandidateId: 'candidate-opencode',
        candidates: [{
          candidateId: 'candidate-opencode',
          source: { providerId: 'opencode.commands', sourceId: 'user-configuration' },
          sourceDisplayName: 'OpenCode',
          ecosystemId: 'opencode',
          contentVersion: 'v1',
          commandDescription: 'Review',
          sourceScope: 'user_global',
          sourceLocation: '~/.config/opencode',
          availability: { state: 'available' },
        }],
      }],
    }));

    expect(result.totalAttentionCount).toBe(0);
  });

  it('exposes what connecting would enable so the dialog never hard-codes access levels', () => {
    const result = view(snapshot({
      sources: [source('opencode-user', 'opencode')],
      integrationPolicy: withMode('opencode', 'discover_only'),
      commands: [{
        candidateId: 'command-1',
        definition: {
          id: {
            source: { providerId: 'opencode.commands', sourceId: 'user-configuration' },
            localId: 'review',
          },
          name: 'review',
          description: 'Review',
          availability: { state: 'available' },
          contentVersion: 'v1',
        },
      }],
    }));

    const plan = result.applications[0].connectPlan;
    expect(plan.find((entry) => entry.capabilityId === 'command'))
      .toMatchObject({ recommendedAccess: 'auto', count: 1 });
    expect(plan.find((entry) => entry.capabilityId === 'tool'))
      .toMatchObject({ recommendedAccess: 'ask_before_use', count: 0 });
  });
});
