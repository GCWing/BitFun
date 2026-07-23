import { describe, expect, it } from 'vitest';
import type {
  ExternalSourceCatalogSnapshot,
  ExternalSourceRecord,
} from '@/infrastructure/api/service-api/ExternalSourcesAPI';
import { buildExternalSourcePresentationGroups } from './externalSourcePresentation';

const integrationPolicy: ExternalSourceCatalogSnapshot['integrationPolicy'] = {
  schemaMajor: 1,
  status: 'compatible',
  userDefaults: { enabled: true, ecosystems: {} },
  globalEffective: { enabled: true, ecosystems: {} },
  effective: { enabled: true, ecosystems: {} },
  registeredEcosystems: [],
};

function source(
  stableKey: string,
  providerId: string,
  overrides: Partial<ExternalSourceRecord> = {},
  presentationGroupId = 'opencode-user-config',
): ExternalSourceCatalogSnapshot['sources'][number] {
  return {
    stableKey,
    presentationGroupId,
    lifecycle: 'available',
    record: {
      key: { providerId, sourceId: 'user-configuration' },
      ecosystemId: 'opencode',
      displayName: 'OpenCode user configuration',
      sourceKind: 'configuration',
      scope: 'user_global',
      location: '~\\.config\\opencode\\opencode.json',
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
    },
    generation: 1,
    discoveryPending: false,
    sources: [],
    commands: [],
    tools: [],
    mcpServers: [],
    subagents: [],
    integrationPolicy,
    ...overrides,
  };
}

describe('external source presentation', () => {
  it('combines provider records that describe the same physical configuration', () => {
    const groups = buildExternalSourcePresentationGroups(snapshot({
      sources: [
        source('command-source', 'opencode.commands'),
        source('agent-source', 'opencode.subagents'),
        source('mcp-source', 'opencode.mcp'),
      ],
      commands: [{
        definition: {
          id: {
            source: { providerId: 'opencode.commands', sourceId: 'user-configuration' },
            localId: 'smoke-command',
          },
          name: 'smoke-command',
          description: 'Smoke command',
          availability: { state: 'available' },
          contentVersion: 'v1',
        },
      }],
      subagents: [{
        candidateId: 'smoke-agent',
        logicalId: 'smoke-agent',
        displayName: 'Smoke agent',
        description: 'Smoke agent',
        providerLabel: 'OpenCode',
        scope: 'user_global',
        sourceKeys: [{ providerId: 'opencode.subagents', sourceId: 'user-configuration' }],
        sourceLocationLabels: ['~/.config/opencode/opencode.json'],
        sourceCount: 1,
        effectiveToolLabels: [],
        supportsFollowUp: false,
        compatibilityState: 'ready',
        diagnostics: [],
        activationState: { state: 'active' },
        decisionKey: 'smoke-agent',
      }],
      mcpServers: [{
        candidateId: 'smoke-mcp',
        approvalKey: 'smoke-mcp',
        decisionKey: 'smoke-mcp',
        activationState: { state: 'active' },
        definition: {
          id: {
            source: { providerId: 'opencode.mcp', sourceId: 'user-configuration' },
            localId: 'smoke-mcp',
          },
          provenance: [],
          name: 'smoke-mcp',
          transport: 'local_stdio',
          argumentCount: 0,
          environmentKeys: [],
          headerNames: [],
          sourceEnabled: true,
          behaviorVersion: 'v1',
          staticStatus: { state: 'ready' },
        },
      }],
    }));

    expect(groups).toHaveLength(1);
    expect(groups[0]).toMatchObject({
      ecosystemId: 'opencode',
      displayName: 'OpenCode user configuration',
      location: '~/.config/opencode/opencode.json',
      counts: { commands: 1, tools: 0, agents: 1, mcps: 1 },
    });
    expect(groups[0].members.map((member) => member.stableKey)).toEqual([
      'agent-source',
      'command-source',
      'mcp-source',
    ]);
  });

  it('omits sources that have no supported assets or actionable diagnostics', () => {
    expect(buildExternalSourcePresentationGroups(snapshot({
      sources: [source('empty-source', 'opencode.commands')],
    }))).toEqual([]);

    expect(buildExternalSourcePresentationGroups(snapshot({
      sources: [source('info-only-source', 'opencode.commands', {
        diagnostics: [{ severity: 'info', code: 'discovered', message: 'Discovered.' }],
      })],
    }))).toEqual([]);
  });

  it('does not combine matching paths from different execution domains', () => {
    const diagnostic = {
      severity: 'warning',
      code: 'opencode.configuration.invalid',
      message: 'The configuration could not be parsed.',
    };
    const groups = buildExternalSourcePresentationGroups(snapshot({
      sources: [
        source('local-source', 'opencode.commands', { diagnostics: [diagnostic] }, 'local-source'),
        source('peer-source', 'opencode.subagents', {
          executionDomainId: 'peer:device-b',
          diagnostics: [diagnostic],
        }, 'peer-source'),
      ],
    }));

    expect(groups).toHaveLength(2);
  });

  it('combines the same physical source across provider-specific scopes', () => {
    const diagnostic = {
      severity: 'warning',
      code: 'opencode.configuration.invalid',
      message: 'The configuration could not be parsed.',
    };
    const groups = buildExternalSourcePresentationGroups(snapshot({
      sources: [
        source('command-source', 'opencode.commands', {
          scope: 'workspace_local',
          diagnostics: [diagnostic],
        }),
        source('mcp-source', 'opencode.mcp', {
          scope: 'user_global',
          diagnostics: [diagnostic],
        }),
      ],
    }));

    expect(groups).toHaveLength(1);
    expect(groups[0].scopes).toEqual(['user_global', 'workspace_local']);
  });

  it('keeps broad scope disclosure even when that member has no visible assets', () => {
    const groups = buildExternalSourcePresentationGroups(snapshot({
      sources: [
        source('command-source', 'opencode.commands', { scope: 'workspace_local' }),
        source('empty-global-source', 'opencode.mcp', { scope: 'user_global' }),
      ],
      commands: [{
        definition: {
          id: {
            source: { providerId: 'opencode.commands', sourceId: 'user-configuration' },
            localId: 'smoke-command',
          },
          name: 'smoke-command',
          description: 'Smoke command',
          availability: { state: 'available' },
          contentVersion: 'v1',
        },
      }],
    }));

    expect(groups).toHaveLength(1);
    expect(groups[0].members).toHaveLength(1);
    expect(groups[0].scopes).toEqual(['user_global', 'workspace_local']);
  });

  it('does not merge distinct raw sources that share the same redacted location', () => {
    const diagnostic = {
      severity: 'warning',
      code: 'opencode.configuration.invalid',
      message: 'The configuration could not be parsed.',
    };
    const groups = buildExternalSourcePresentationGroups(snapshot({
      sources: [
        source('source-a', 'opencode.commands', { diagnostics: [diagnostic] }, 'raw-source-a'),
        source('source-b', 'opencode.subagents', { diagnostics: [diagnostic] }, 'raw-source-b'),
      ],
    }));

    expect(groups).toHaveLength(2);
  });

  it('keeps a source with only actionable diagnostics and deduplicates repeated diagnostics', () => {
    const diagnostic = {
      severity: 'warning',
      code: 'opencode.configuration.invalid',
      message: 'The configuration could not be parsed.',
    };
    const groups = buildExternalSourcePresentationGroups(snapshot({
      sources: [
        source('command-source', 'opencode.commands', { diagnostics: [diagnostic] }),
        source('agent-source', 'opencode.subagents', { diagnostics: [diagnostic] }),
      ],
    }));

    expect(groups).toHaveLength(1);
    expect(groups[0].diagnostics).toEqual([diagnostic]);
  });

  it('keeps a user-suppressed source visible so it can be enabled again', () => {
    const suppressedSource = source('suppressed-source', 'opencode.mcp');
    suppressedSource.lifecycle = 'suppressed';

    const groups = buildExternalSourcePresentationGroups(snapshot({
      sources: [suppressedSource],
    }));

    expect(groups).toHaveLength(1);
    expect(groups[0]).toMatchObject({
      lifecycle: 'suppressed',
      counts: { commands: 0, tools: 0, agents: 0, mcps: 0 },
    });
    expect(groups[0].members[0]).toMatchObject({
      capability: 'mcp',
      enabled: false,
      scope: 'user_global',
    });
  });

  it('preserves mixed member states instead of presenting a misleading group switch', () => {
    const available = source('command-source', 'opencode.commands');
    const suppressed = source('agent-source', 'opencode.subagents');
    suppressed.lifecycle = 'suppressed';

    const groups = buildExternalSourcePresentationGroups(snapshot({
      sources: [available, suppressed],
      commands: [{
        definition: {
          id: {
            source: available.record.key,
            localId: 'smoke-command',
          },
          name: 'smoke-command',
          description: 'Smoke command',
          availability: { state: 'available' },
          contentVersion: 'v1',
        },
      }],
    }));

    expect(groups).toHaveLength(1);
    expect(groups[0].members).toEqual([
      expect.objectContaining({ stableKey: 'agent-source', capability: 'subagent', enabled: false }),
      expect.objectContaining({ stableKey: 'command-source', capability: 'command', enabled: true }),
    ]);
  });

  it.each([
    'removed',
    'unavailable',
    'degraded',
    'restricted',
    'using_last_valid_version',
  ] as const)('keeps an empty %s source visible because its lifecycle is actionable', (lifecycle) => {
    const abnormalSource = source(`${lifecycle}-source`, 'opencode.commands');
    abnormalSource.lifecycle = lifecycle;

    const groups = buildExternalSourcePresentationGroups(snapshot({
      sources: [abnormalSource],
    }));

    expect(groups).toHaveLength(1);
    expect(groups[0].lifecycle).toBe(lifecycle);
  });
});
