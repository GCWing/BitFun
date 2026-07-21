// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import ExternalSourcesConfig from './ExternalSourcesConfig';

const getSnapshotMock = vi.hoisted(() => vi.fn());
const setSourceEnabledMock = vi.hoisted(() => vi.fn());
const setConflictChoiceMock = vi.hoisted(() => vi.fn());
const setToolTargetDecisionMock = vi.hoisted(() => vi.fn());
const setToolConflictChoiceMock = vi.hoisted(() => vi.fn());
const setSubagentActivationMock = vi.hoisted(() => vi.fn());
const chooseSubagentConflictMock = vi.hoisted(() => vi.fn());
const setMcpServerDecisionMock = vi.hoisted(() => vi.fn());
const chooseMcpConflictMock = vi.hoisted(() => vi.fn());
const updateIntegrationPolicyMock = vi.hoisted(() => vi.fn());
const revealSourceLocationMock = vi.hoisted(() => vi.fn());
const workspaceState = vi.hoisted(() => ({ path: 'D:/workspace/project', kind: 'normal' }));
const peerState = vi.hoisted(() => ({ deviceId: '' }));

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: (key: string, params?: Record<string, unknown>) =>
      params ? `${key}:${JSON.stringify(params)}` : key,
  }),
}));

vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useCurrentWorkspace: () => ({
    workspace: { id: workspaceState.path, workspaceKind: workspaceState.kind, rootPath: workspaceState.path },
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

vi.mock('@/infrastructure/runtime', () => ({ isTauriRuntime: () => true }));
vi.mock('@/shared/utils/logger', () => {
  const fn = () => vi.fn();
  const contextLogger = { trace: fn(), debug: fn(), info: fn(), warn: fn(), error: fn() };
  return {
    createLogger: () => contextLogger,
    logger: contextLogger,
    log: contextLogger,
  };
});
vi.mock('@/infrastructure/api/service-api/WorkspaceAPI', () => ({
  workspaceAPI: {
    revealInExplorer: vi.fn().mockResolvedValue(undefined),
  },
}));
vi.mock('@/shared/types', () => ({
  isRemoteWorkspace: () => false,
  WorkspaceKind: { Normal: 'normal', Assistant: 'assistant', Remote: 'remote' },
}));
vi.mock('@/infrastructure/api/service-api/ExternalSourcesAPI', () => ({
  externalSourcesAPI: {
    getSnapshot: getSnapshotMock,
    setSourceEnabled: setSourceEnabledMock,
    setConflictChoice: setConflictChoiceMock,
    setToolTargetDecision: setToolTargetDecisionMock,
    setToolConflictChoice: setToolConflictChoiceMock,
    setSubagentActivation: setSubagentActivationMock,
    chooseSubagentConflict: chooseSubagentConflictMock,
    setMcpServerDecision: setMcpServerDecisionMock,
    chooseMcpConflict: chooseMcpConflictMock,
    updateIntegrationPolicy: updateIntegrationPolicyMock,
    revealSourceLocation: revealSourceLocationMock,
  },
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
    stableKey: 'source-key',
    presentationGroupId: 'source-key',
    record: {
      key: { providerId: 'opencode.commands', sourceId: 'project' },
      ecosystemId: 'other',
      displayName: 'OpenCode project commands',
      sourceKind: 'prompt_commands',
      scope: 'project',
      location: '<workspace>/.opencode/commands',
      health: 'available',
      contentVersion: 'v1',
    },
    lifecycle: 'available',
  }],
  commands: [],
  diagnostics: [{
    severity: 'warning',
    code: 'opencode.command.parse_failed',
    message: 'One command file could not be parsed.',
  }],
  commandConflicts: [{
    conflictKey: 'conflict-v1',
    commandName: 'review',
    candidates: [{
      candidateId: 'candidate-opencode',
      source: { providerId: 'opencode.commands', sourceId: 'project' },
      sourceDisplayName: 'OpenCode project commands',
      ecosystemId: 'opencode',
      contentVersion: 'v1',
      commandDescription: 'Review with OpenCode',
      sourceScope: 'project',
      sourceLocation: '<workspace>/.opencode/commands',
      availability: { state: 'available' },
    }, {
      candidateId: 'candidate-other',
      source: { providerId: 'other.commands', sourceId: 'project' },
      sourceDisplayName: 'Other project commands',
      ecosystemId: 'other',
      contentVersion: 'v1',
      commandDescription: 'Review with another source',
      sourceScope: 'project',
      sourceLocation: '<workspace>/.other/commands',
      availability: { state: 'available' },
    }],
  }],
  tools: [],
  toolApprovalRequests: [],
  toolConflicts: [],
  integrationPolicy: {
    schemaMajor: 1,
    status: 'compatible',
    userDefaults: { enabled: true, ecosystems: {} },
    globalEffective: { enabled: true, ecosystems: {} },
    effective: { enabled: true, ecosystems: {} },
    registeredEcosystems: [],
  },
};

const discoveredCommand = {
  definition: {
    id: {
      source: { providerId: 'opencode.commands', sourceId: 'project' },
      localId: 'review',
    },
    name: 'review',
    description: 'Review with OpenCode',
    availability: { state: 'available' },
    contentVersion: 'v1',
  },
};

const integrationPolicy = {
  schemaMajor: 1,
  status: 'compatible',
  userDefaults: {
    enabled: true,
    ecosystems: {
      opencode: { mode: 'recommended', capabilityOverrides: {} },
    },
  },
  globalEffective: {
    enabled: true,
    ecosystems: {
      opencode: {
        ecosystemId: 'opencode',
        mode: 'recommended',
        capabilities: {
          command: 'auto',
          tool: 'ask_before_use',
          subagent: 'ask_before_use',
          mcp: 'ask_before_use',
        },
        policyLimitedCapabilities: [],
      },
    },
  },
  effective: {
    enabled: true,
    ecosystems: {
      opencode: {
        ecosystemId: 'opencode',
        mode: 'recommended',
        capabilities: {
          command: 'auto',
          tool: 'ask_before_use',
          subagent: 'ask_before_use',
          mcp: 'ask_before_use',
        },
        policyLimitedCapabilities: [],
      },
    },
  },
  registeredEcosystems: [{
    ecosystemId: 'opencode',
    displayName: 'OpenCode',
    adapterRevision: '1',
    capabilities: [
      { capabilityId: 'command', recommendedAccess: 'auto', safetyCeiling: 'auto' },
      {
        capabilityId: 'tool',
        recommendedAccess: 'ask_before_use',
        safetyCeiling: 'ask_before_use',
      },
      {
        capabilityId: 'subagent',
        recommendedAccess: 'ask_before_use',
        safetyCeiling: 'ask_before_use',
      },
      {
        capabilityId: 'mcp',
        recommendedAccess: 'ask_before_use',
        safetyCeiling: 'ask_before_use',
      },
    ],
  }],
};

describe('ExternalSourcesConfig', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    workspaceState.path = 'D:/workspace/project';
    workspaceState.kind = 'normal';
    peerState.deviceId = '';
    getSnapshotMock.mockResolvedValue(snapshot);
    setSourceEnabledMock.mockResolvedValue(snapshot);
    setConflictChoiceMock.mockResolvedValue({
      ...snapshot,
      commandConflicts: [{
        ...snapshot.commandConflicts[0],
        selectedCandidateId: 'candidate-opencode',
      }],
    });
    setToolTargetDecisionMock.mockResolvedValue(snapshot);
    setToolConflictChoiceMock.mockResolvedValue(snapshot);
    setSubagentActivationMock.mockResolvedValue(snapshot);
    chooseSubagentConflictMock.mockResolvedValue(snapshot);
    setMcpServerDecisionMock.mockResolvedValue(snapshot);
    chooseMcpConflictMock.mockResolvedValue(snapshot);
    updateIntegrationPolicyMock.mockResolvedValue(snapshot);
    revealSourceLocationMock.mockResolvedValue(undefined);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('keeps compatibility controls compact and applies the safe OpenCode defaults', async () => {
    const policySnapshot = {
      ...snapshot,
      preferenceRevision: 4,
      integrationPolicy,
    };
    getSnapshotMock.mockResolvedValue(policySnapshot);
    updateIntegrationPolicyMock.mockResolvedValue({
      ...policySnapshot,
      preferenceRevision: 5,
    });

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('policy.title');
    expect(container.textContent).not.toContain('policy.externalBadge');
    expect(container.textContent).toContain('OpenCode');
    expect(container.textContent).toContain('policy.mode.recommended');
    expect(container.textContent).toContain('policy.inherited');
    expect(container.querySelectorAll(
      '.bitfun-external-sources-config__capability-row',
    )).toHaveLength(0);

    const capabilityButton = Array.from(container.querySelectorAll('button')).find((button) =>
      button.getAttribute('aria-label') === 'policy.capabilitiesFor:{"ecosystem":"OpenCode"}');
    await act(async () => capabilityButton?.click());
    expect(container.textContent).toContain('policy.capability.command');
    expect(container.textContent).toContain('policy.access.auto');
    expect(container.textContent).toContain('policy.capability.tool');
    expect(container.textContent).toContain('policy.access.askBeforeUse');

    const policyToggle = container.querySelector(
      '.bitfun-external-sources-config__policy-card input[type="checkbox"]',
    ) as HTMLInputElement;
    expect(policyToggle.checked).toBe(true);
    await act(async () => policyToggle.click());
    expect(updateIntegrationPolicyMock).toHaveBeenCalledWith('D:/workspace/project', {
      expectedPreferenceRevision: 4,
      scope: 'workspace',
      change: { operation: 'set_enabled', enabled: false },
    });
  });

  it('shows the effective safety ceiling when stored policy requests broader access', async () => {
    getSnapshotMock.mockResolvedValue({
      ...snapshot,
      integrationPolicy: {
        ...integrationPolicy,
        userDefaults: {
          enabled: true,
          ecosystems: {
            opencode: {
              mode: 'custom',
              capabilityOverrides: { tool: 'auto' },
            },
          },
        },
        effective: {
          enabled: true,
          ecosystems: {
            opencode: {
              ...integrationPolicy.effective.ecosystems.opencode,
              mode: 'custom',
              capabilities: {
                ...integrationPolicy.effective.ecosystems.opencode.capabilities,
                tool: 'ask_before_use',
              },
              policyLimitedCapabilities: ['tool'],
            },
          },
        },
      },
    });

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    const capabilityButton = Array.from(container.querySelectorAll('button')).find((button) =>
      button.getAttribute('aria-label') === 'policy.capabilitiesFor:{"ecosystem":"OpenCode"}');
    await act(async () => capabilityButton?.click());
    const toolRow = Array.from(container.querySelectorAll(
      '.bitfun-external-sources-config__capability-row',
    )).find((row) => row.textContent?.includes('policy.capability.tool'));

    expect(toolRow?.textContent).toContain('policy.access.askBeforeUse');
    expect(toolRow?.textContent).toContain('policy.safetyLimited');
    expect(toolRow?.textContent).not.toContain('policy.access.auto');
  });

  it('requires one explicit conflict choice and persists source toggles', async () => {
    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    expect(getSnapshotMock).toHaveBeenCalledWith('D:/workspace/project', false);

    const candidateButton = Array.from(container.querySelectorAll('button')).find((button) =>
      button.textContent?.includes('OpenCode project commands'));
    expect(container.textContent).toContain('diagnostics.summary');
    expect(container.textContent).toContain('diagnostics.category.invalidSettings');
    expect(container.textContent).toContain('One command file could not be parsed.');
    expect(candidateButton).toBeDefined();
    await act(async () => candidateButton?.click());
    expect(setConflictChoiceMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      'conflict-v1',
      'candidate-opencode',
      0,
    );
    expect(container.textContent).toContain('conflicts.commandName');
    expect(container.textContent).toContain('conflicts.currentSelection');
    expect(container.textContent).toContain('common.selected');
    expect(container.textContent).toContain('common.notSelected');

    const sourceToggle = container.querySelector(
      'input[aria-label^="sources.toggleLabel"]',
    ) as HTMLInputElement;
    expect(sourceToggle.checked).toBe(true);
    await act(async () => sourceToggle.click());
    expect(setSourceEnabledMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      'source-key',
      false,
      0,
    );
  });

  it('combines duplicate physical sources and exposes honest capability-level controls', async () => {
    const sharedRecord = {
      ...snapshot.sources[0].record,
      ecosystemId: 'other',
      sourceKind: 'other_user_configuration',
      scope: 'user_global',
      location: '~\\.config\\other\\other.json',
      executionDomainId: 'local',
      displayName: 'Other user configuration',
    };
    const groupedSnapshot = {
      ...snapshot,
      preferenceRevision: 7,
      diagnostics: [],
      commandConflicts: [],
      sources: [{
        stableKey: 'other-command-source',
        presentationGroupId: 'other-user-config',
        record: {
          ...sharedRecord,
          key: { providerId: 'other.commands', sourceId: 'user-configuration' },
        },
        lifecycle: 'available',
      }, {
        stableKey: 'other-agent-source',
        presentationGroupId: 'other-user-config',
        record: {
          ...sharedRecord,
          key: { providerId: 'other.subagents', sourceId: 'user-configuration' },
        },
        lifecycle: 'suppressed',
      }],
      commands: [{
        definition: {
          id: {
            source: { providerId: 'other.commands', sourceId: 'user-configuration' },
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
        providerLabel: 'Other',
        scope: 'user_global',
        sourceKeys: [{ providerId: 'other.subagents', sourceId: 'user-configuration' }],
        sourceLocationLabels: ['~/.config/other/other.json'],
        sourceCount: 1,
        effectiveToolLabels: [],
        supportsFollowUp: false,
        compatibilityState: 'ready',
        diagnostics: [],
        activationState: { state: 'active' },
        decisionKey: 'smoke-agent',
      }],
    };
    getSnapshotMock.mockResolvedValue(groupedSnapshot);
    setSourceEnabledMock.mockImplementation(async () => ({
      ...groupedSnapshot,
      preferenceRevision: 8,
    }));

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    expect(container.querySelectorAll(
      '.bitfun-external-sources-config__source-group',
    )).toHaveLength(1);
    expect(container.textContent?.match(/Other user configuration/g)).toHaveLength(1);
    expect(container.textContent).toContain('sources.commandCount:{"count":1}');
    expect(container.textContent).toContain('sources.agentCount:{"count":1}');
    expect(container.textContent).not.toContain('sources.toolCount:{"count":0}');
    expect(container.textContent).not.toContain('sources.mcpCount:{"count":0}');

    const sourceToggles = Array.from(container.querySelectorAll(
      'input[aria-label^="sources.toggleLabel"]',
    )) as HTMLInputElement[];
    expect(sourceToggles).toHaveLength(2);
    expect(sourceToggles.map((toggle) => toggle.checked).sort()).toEqual([false, true]);
    expect(sourceToggles.every((toggle) => (
      toggle.getAttribute('aria-label')?.includes('scope.user_global')
    ))).toBe(true);
    expect(sourceToggles.some((toggle) => (
      toggle.getAttribute('aria-label')?.includes('lifecycle.suppressed')
    ))).toBe(true);
    const enabledToggle = sourceToggles.find((toggle) => toggle.checked);
    await act(async () => enabledToggle?.click());

    expect(setSourceEnabledMock).toHaveBeenCalledOnce();
    expect(setSourceEnabledMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      'other-command-source',
      false,
      7,
    );
  });

  it('does not show source configuration UI when discovery found no usable content', async () => {
    getSnapshotMock.mockResolvedValue({
      ...snapshot,
      sources: [{
        ...snapshot.sources[0],
        record: {
          ...snapshot.sources[0].record,
          diagnostics: [{ severity: 'info', code: 'discovered', message: 'Discovered.' }],
        },
      }],
      commands: [],
      commandConflicts: [],
      diagnostics: [],
    });

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    expect(container.textContent).not.toContain('sources.title');
    expect(container.textContent).not.toContain('sources.empty');
    expect(container.textContent).not.toContain('OpenCode project commands');
  });

  it('reviews MCP risk and binds approval and conflict choices to the visible versions', async () => {
    const mcpSnapshot = {
      ...snapshot,
      commandConflicts: [],
      preferenceRevision: 9,
      mcpGeneration: 5,
      sources: [{
        ...snapshot.sources[0],
        stableKey: 'opencode-mcp-project',
        record: {
          ...snapshot.sources[0].record,
          key: { providerId: 'opencode.mcp', sourceId: 'project' },
          displayName: 'OpenCode project MCP',
          sourceKind: 'opencode_mcp_config',
          location: '<workspace>/opencode.json',
        },
      }],
      mcpServers: [{
        candidateId: 'external-mcp-github',
        decisionKey: 'mcp-decision-v1',
        definition: {
          id: {
            source: { providerId: 'opencode.mcp', sourceId: 'project' },
            localId: 'github',
          },
          provenance: [{ providerId: 'opencode.mcp', sourceId: 'project' }],
          name: 'github',
          transport: 'local_stdio',
          commandPreview: 'npx',
          argumentCount: 2,
          workingDirectory: '<workspace>',
          environmentKeys: ['GITHUB_TOKEN'],
          environmentReferenceNames: ['OPENCODE_TOKEN'],
          headerNames: [],
          sourceEnabled: true,
          behaviorVersion: 'behavior-v1',
          staticStatus: { state: 'ready' },
        },
        activationState: { state: 'approval_required' },
      }],
      mcpApprovalRequests: [{
        candidateId: 'external-mcp-github',
        approvalKey: 'mcp-approval-v1',
        decisionKey: 'mcp-decision-v1',
        definition: {
          id: {
            source: { providerId: 'opencode.mcp', sourceId: 'project' },
            localId: 'github',
          },
          provenance: [{ providerId: 'opencode.mcp', sourceId: 'project' }],
          name: 'github',
          transport: 'local_stdio',
          commandPreview: 'npx',
          argumentCount: 2,
          workingDirectory: '<workspace>',
          environmentKeys: ['GITHUB_TOKEN'],
          environmentReferenceNames: ['OPENCODE_TOKEN'],
          headerNames: [],
          sourceEnabled: true,
          behaviorVersion: 'behavior-v1',
          staticStatus: { state: 'ready' },
        },
      }],
      mcpConflicts: [{
        conflictKey: 'mcp-conflict-v1',
        serverName: 'github',
        candidates: [{
          candidateId: 'native-mcp-github',
          displayName: 'BitFun: github',
          external: false,
          behaviorVersion: 'native-v1',
          available: true,
        }, {
          candidateId: 'external-mcp-github',
          displayName: 'OpenCode: github',
          external: true,
          behaviorVersion: 'behavior-v1',
          available: true,
        }],
      }],
    };
    getSnapshotMock.mockResolvedValue(mcpSnapshot);
    setMcpServerDecisionMock.mockResolvedValue({
      ...mcpSnapshot,
      mcpApprovalRequests: [],
    });
    chooseMcpConflictMock.mockResolvedValue({
      ...mcpSnapshot,
      mcpConflicts: [{
        ...mcpSnapshot.mcpConflicts[0],
        selectedCandidateId: 'native-mcp-github',
      }],
    });

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('mcpApprovals.warning');
    expect(container.textContent).toContain('OpenCode project MCP');
    expect(container.textContent).not.toContain('D:/workspace/project/opencode.json');
    expect(container.textContent).toContain('mcp.command:{"command":"npx"}');
    expect(container.textContent).toContain('mcp.workingDirectory:{"location":"<workspace>"}');
    expect(container.textContent).toContain('GITHUB_TOKEN');
    expect(container.textContent).toContain('OPENCODE_TOKEN');

    const externalConflictCandidate = Array.from(
      container.querySelectorAll('.bitfun-external-sources-config__candidate'),
    ).find((candidate) => candidate.textContent?.includes('OpenCode: github'));
    expect(externalConflictCandidate?.textContent).toContain('mcpConflicts.review');
    expect(externalConflictCandidate?.textContent).not.toContain('mcp.argumentCount');

    const reviewExternalCandidate = Array.from(container.querySelectorAll('button')).find((button) =>
      button.textContent?.includes('mcpConflicts.review'));
    await act(async () => reviewExternalCandidate?.click());
    expect(chooseMcpConflictMock).not.toHaveBeenCalled();
    expect(externalConflictCandidate?.textContent).toContain('mcp.argumentCount:{"count":2}');
    expect(externalConflictCandidate?.textContent).toContain('mcp.scope');
    expect(externalConflictCandidate?.textContent).toContain('mcpApprovals.warning');

    const approveExternalCandidate = Array.from(container.querySelectorAll('button')).find((button) =>
      button.textContent?.includes('mcpConflicts.approveAndUse'));
    await act(async () => approveExternalCandidate?.click());
    expect(chooseMcpConflictMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      'mcp-conflict-v1',
      'external-mcp-github',
      true,
      5,
      9,
    );

    const enable = Array.from(container.querySelectorAll('button')).find((button) =>
      button.textContent?.includes('mcpApprovals.enable'));
    await act(async () => enable?.click());
    expect(setMcpServerDecisionMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      'external-mcp-github',
      'mcp-decision-v1',
      true,
      5,
      9,
    );

    const nativeCandidate = Array.from(container.querySelectorAll('button')).find((button) =>
      button.textContent?.includes('BitFun: github'));
    await act(async () => nativeCandidate?.click());
    expect(chooseMcpConflictMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      'mcp-conflict-v1',
      'native-mcp-github',
      false,
      5,
      9,
    );
  });

  it('keeps remembered command, tool, and agent choices visible and changeable', async () => {
    const resolvedSnapshot = {
      ...snapshot,
      commandConflicts: [{
        ...snapshot.commandConflicts[0],
        selectedCandidateId: 'candidate-opencode',
      }],
      toolConflicts: [{
        conflictKey: 'tool-conflict-v1',
        toolName: 'review',
        selectedCandidateId: 'builtin-review',
        candidates: [{
          candidateId: 'builtin-review',
          displayName: 'BitFun Review',
          kind: 'built_in',
          providerId: 'bitfun.builtin',
          contentVersion: 'builtin-v1',
        }, {
          candidateId: 'external-review',
          displayName: 'OpenCode Review Tool',
          kind: 'external',
          providerId: 'opencode.tools',
          contentVersion: 'external-v1',
          sourceLocation: '<workspace>/.opencode/tools/review.js',
        }],
      }],
      subagentGeneration: 4,
      preferenceRevision: 7,
      subagents: [],
      subagentConflicts: [{
        conflictKey: 'agent-conflict-v1',
        logicalId: 'review',
        selectedCandidateId: '__bitfun_disabled__',
        candidates: [{
          candidateId: 'builtin-agent-review',
          displayName: 'BitFun Review Agent',
          sourceLabel: 'BitFun',
          external: false,
        }, {
          candidateId: 'external-agent-review',
          displayName: 'OpenCode Review Agent',
          sourceLabel: 'OpenCode',
          external: true,
        }],
      }],
      pendingSubagentApprovals: [],
    };
    getSnapshotMock.mockResolvedValue(resolvedSnapshot);
    setToolConflictChoiceMock.mockResolvedValue(resolvedSnapshot);
    chooseSubagentConflictMock.mockResolvedValue(resolvedSnapshot);

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('conflicts.currentSelection');
    expect(container.textContent).toContain('toolConflicts.currentSelection');
    expect(container.textContent).toContain('agentConflicts.keptUnavailable');
    expect(container.textContent).toContain('BitFun Review');
    expect(container.textContent).toContain('OpenCode Review Tool');
    expect(container.textContent).toContain('BitFun Review Agent');
    expect(container.textContent).toContain('OpenCode Review Agent');

    const externalTool = Array.from(container.querySelectorAll('button')).find((button) =>
      button.textContent?.includes('OpenCode Review Tool'));
    await act(async () => externalTool?.click());
    expect(setToolConflictChoiceMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      'tool-conflict-v1',
      'external-review',
      7,
    );

    const bitfunAgent = Array.from(container.querySelectorAll('button')).find((button) =>
      button.textContent?.includes('BitFun Review Agent'));
    await act(async () => bitfunAgent?.click());
    expect(chooseSubagentConflictMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      'agent-conflict-v1',
      'builtin-agent-review',
      false,
      4,
      7,
    );
  });

  it('does not present a selected but disabled external agent as currently used', async () => {
    const selectedDisabledSnapshot = {
      ...snapshot,
      commandConflicts: [{
        ...snapshot.commandConflicts[0],
        selectedCandidateId: 'candidate-opencode',
        candidates: [{
          ...snapshot.commandConflicts[0].candidates[0],
          availability: {
            state: 'restricted',
            reason: 'Unsupported command capability',
            required_capabilities: ['shell'],
          },
        }, snapshot.commandConflicts[0].candidates[1]],
      }],
      tools: [{
        definition: {
          id: {
            target: {
              source: { providerId: 'opencode.tools', sourceId: 'project' },
              localId: 'review',
            },
            exportId: 'other',
          },
          name: 'other',
          descriptionPreview: 'Another export in the same module',
          modulePath: '<workspace>/.opencode/tools/review.js',
          workingDirectory: '<workspace>',
          runtimeKind: 'java_script',
          capabilities: [],
          contentVersion: 'tool-v1',
          staticStatus: { state: 'ready' },
        },
        approvalKey: 'other-tool-approval-v1',
        decisionKey: 'other-tool-decision-v1',
        activation: { state: 'active' },
      }, {
        definition: {
          id: {
            target: {
              source: { providerId: 'opencode.tools', sourceId: 'project' },
              localId: 'review',
            },
            exportId: 'review',
          },
          name: 'review',
          descriptionPreview: 'Review a change',
          modulePath: '<workspace>/.opencode/tools/review.js',
          workingDirectory: '<workspace>',
          runtimeKind: 'java_script',
          capabilities: [],
          contentVersion: 'tool-v1',
          staticStatus: { state: 'ready' },
        },
        approvalKey: 'tool-approval-v1',
        decisionKey: 'tool-decision-v1',
        activation: { state: 'disabled' },
      }],
      toolConflicts: [{
        conflictKey: 'tool-conflict-v1',
        toolName: 'review',
        selectedCandidateId: 'external-review',
        candidates: [{
          candidateId: 'builtin-review',
          displayName: 'BitFun Review',
          kind: 'built_in',
          providerId: 'bitfun.builtin',
          contentVersion: 'builtin-v1',
        }, {
          candidateId: 'external-review',
          displayName: 'OpenCode Review',
          kind: 'external',
          providerId: 'opencode.tools',
          contentVersion: 'tool-v1',
          source: { providerId: 'opencode.tools', sourceId: 'project' },
          sourceLocation: '<workspace>/.opencode/tools/review.js',
        }],
      }],
      subagentGeneration: 4,
      preferenceRevision: 7,
      subagents: [{
        candidateId: 'external-agent-review',
        logicalId: 'review',
        displayName: 'OpenCode Review Agent',
        description: 'Review a change',
        providerLabel: 'OpenCode',
        scope: 'project',
        sourceKeys: [{ providerId: 'opencode.agents', sourceId: 'review' }],
        sourceLocationLabels: ['<workspace>/.opencode/agents/review.md'],
        sourceCount: 1,
        effectiveModelLabel: 'fast',
        effectiveToolLabels: ['Read'],
        supportsFollowUp: false,
        compatibilityState: 'ready',
        diagnostics: [],
        activationState: { state: 'disabled' },
        decisionKey: 'agent-decision-v1',
      }],
      subagentConflicts: [{
        conflictKey: 'agent-conflict-v1',
        logicalId: 'review',
        selectedCandidateId: 'external-agent-review',
        candidates: [{
          candidateId: 'builtin-agent-review',
          displayName: 'BitFun Review Agent',
          sourceLabel: 'BitFun',
          external: false,
        }, {
          candidateId: 'external-agent-review',
          displayName: 'OpenCode Review Agent',
          sourceLabel: 'OpenCode',
          external: true,
        }],
      }],
      pendingSubagentApprovals: [],
    };
    getSnapshotMock.mockResolvedValue(selectedDisabledSnapshot);

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('common.selectedUnavailable');
    expect(container.textContent).toContain('conflicts.currentSelectionUnavailable');
    expect(container.textContent).toContain('toolConflicts.currentSelectionUnavailable');
    expect(container.textContent).toContain('agentConflicts.currentSelectionUnavailable');
  });

  it('keeps discovery non-blocking while an initial refresh completes', async () => {
    getSnapshotMock
      .mockResolvedValueOnce({
        ...snapshot,
        discoveryPending: true,
        sources: [],
        diagnostics: [],
        commandConflicts: [],
      })
      .mockResolvedValue(snapshot);

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    expect(container.textContent).toContain('checkingNonBlocking');
    expect(container.textContent).not.toContain('sources.empty');

    await act(async () => {
      await vi.advanceTimersByTimeAsync(750);
    });
    expect(container.textContent).toContain('OpenCode project commands');
    expect(container.textContent).not.toContain('checkingNonBlocking');
  });

  it('waits for each discovery poll before scheduling the next backoff step', async () => {
    let resolvePoll: ((value: typeof snapshot) => void) | undefined;
    getSnapshotMock
      .mockResolvedValueOnce({
        ...snapshot,
        discoveryPending: true,
        sources: [],
        diagnostics: [],
        commandConflicts: [],
      })
      .mockImplementationOnce(() => new Promise<typeof snapshot>((resolve) => {
        resolvePoll = resolve;
      }))
      .mockResolvedValue(snapshot);

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    await act(async () => vi.advanceTimersByTimeAsync(750));
    expect(getSnapshotMock).toHaveBeenCalledTimes(2);

    await act(async () => vi.advanceTimersByTimeAsync(5_000));
    expect(getSnapshotMock).toHaveBeenCalledTimes(2);

    await act(async () => {
      resolvePoll?.({
        ...snapshot,
        discoveryPending: true,
        sources: [],
        diagnostics: [],
        commandConflicts: [],
      });
      await Promise.resolve();
    });
    await act(async () => vi.advanceTimersByTimeAsync(1_499));
    expect(getSnapshotMock).toHaveBeenCalledTimes(2);
    await act(async () => vi.advanceTimersByTimeAsync(1));
    expect(getSnapshotMock).toHaveBeenCalledTimes(3);
  });

  it('keeps polling when a read overlaps a source change', async () => {
    let resolveMutation: ((value: typeof snapshot) => void) | undefined;
    const pendingSnapshot = { ...snapshot, discoveryPending: true };
    getSnapshotMock
      .mockResolvedValueOnce(pendingSnapshot)
      .mockResolvedValueOnce(pendingSnapshot)
      .mockResolvedValue(snapshot);
    setSourceEnabledMock.mockImplementationOnce(() => new Promise<typeof snapshot>((resolve) => {
      resolveMutation = resolve;
    }));

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    const sourceToggle = container.querySelector(
      'input[aria-label^="sources.toggleLabel"]',
    ) as HTMLInputElement;
    await act(async () => sourceToggle.click());
    await act(async () => vi.advanceTimersByTimeAsync(750));
    expect(getSnapshotMock).toHaveBeenCalledTimes(2);

    await act(async () => {
      resolveMutation?.(pendingSnapshot);
      await Promise.resolve();
    });
    await act(async () => vi.advanceTimersByTimeAsync(1_500));
    expect(getSnapshotMock).toHaveBeenCalledTimes(3);
  });

  it('recovers discovery polling after a transient read failure', async () => {
    getSnapshotMock
      .mockResolvedValueOnce({ ...snapshot, discoveryPending: true })
      .mockRejectedValueOnce(new Error('temporary discovery failure'))
      .mockResolvedValue(snapshot);

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    await act(async () => vi.advanceTimersByTimeAsync(750));
    expect(getSnapshotMock).toHaveBeenCalledTimes(2);
    await act(async () => vi.advanceTimersByTimeAsync(1_500));
    expect(getSnapshotMock).toHaveBeenCalledTimes(3);
    expect(container.textContent).toContain('OpenCode project commands');
  });

  it('distinguishes initial load failures from uncertain mutation results', async () => {
    getSnapshotMock.mockRejectedValueOnce(new Error('initial load failed'));
    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    expect(container.textContent).toContain('errors.loadFailed');
    expect(container.textContent).not.toContain('initial load failed');
    expect(container.textContent).not.toContain('sources.empty');

    await act(async () => root.unmount());
    container.remove();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    getSnapshotMock.mockResolvedValue(snapshot);
    setSourceEnabledMock.mockRejectedValueOnce(new Error('save result unknown'));
    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    const sourceToggle = container.querySelector(
      'input[aria-label^="sources.toggleLabel"]',
    ) as HTMLInputElement;
    await act(async () => sourceToggle.click());
    expect(container.textContent).toContain('errors.mutationUnknown');
    expect(container.textContent).not.toContain('save result unknown');
  });

  it('keeps the visible snapshot but warns when checking for changes fails', async () => {
    getSnapshotMock
      .mockResolvedValueOnce(snapshot)
      .mockRejectedValueOnce(new Error('refresh failed'));
    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    const refresh = Array.from(container.querySelectorAll('button')).find((button) =>
      button.getAttribute('aria-label') === 'actions.refresh');
    await act(async () => {
      refresh?.click();
      await Promise.resolve();
    });
    expect(container.textContent).toContain('errors.refreshFailed');
    expect(container.textContent).toContain('OpenCode project commands');
    expect(container.textContent).not.toContain('refresh failed');
  });

  it('describes BitFun preference-storage diagnostics without blaming source files', async () => {
    getSnapshotMock.mockResolvedValue({
      ...snapshot,
      diagnostics: [{
        severity: 'error',
        code: 'external_tool.preference_read_failed',
        message: 'internal preference read failed',
      }, {
        severity: 'error',
        code: 'external_subagent.conflict_history_write_failed',
        message: 'internal conflict history write failed',
      }],
    });
    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    expect(container.textContent).toContain('diagnostics.category.confirmationStateUnavailable');
    expect(container.textContent).toContain('diagnostics.category.conflictHistoryUnavailable');
    expect(container.textContent).not.toContain('diagnostics.category.unreadableSource');
  });

  it('binds external agent approval and conflicts to the visible generation', async () => {
    const agentSnapshot = {
      ...snapshot,
      sources: [{
        ...snapshot.sources[0],
        record: {
          ...snapshot.sources[0].record,
          location: '<workspace>/agents',
        },
      }, {
        ...snapshot.sources[0],
        stableKey: 'explicit-agent-source',
        record: {
          ...snapshot.sources[0].record,
          key: { providerId: 'opencode.agents', sourceId: 'explore' },
          displayName: 'OpenCode explicit agent config',
          location: '<workspace>/shared/opencode.json',
        },
      }],
      generation: 2,
      commandConflicts: [],
      subagentGeneration: 4,
      preferenceRevision: 7,
      subagents: [{
        candidateId: 'external-agent-review-v1',
        logicalId: 'review',
        displayName: 'OpenCode Review',
        description: 'Review a change',
        providerLabel: 'OpenCode',
        scope: 'project',
        sourceKeys: [{ providerId: 'opencode.commands', sourceId: 'project' }],
        sourceLocationLabels: ['<workspace>/.opencode/agents/review.md'],
        sourceCount: 1,
        effectiveModelLabel: 'fast',
        effectiveToolLabels: ['Read', 'Grep'],
        supportsFollowUp: false,
        compatibilityState: 'ready',
        diagnostics: [{
          code: 'opencode_agent_prompt_not_imported',
          blocksActivation: true,
        }, {
          code: 'opencode_default_permission_semantics_not_imported',
          blocksActivation: false,
        }, {
          code: 'opencode_agent_definition_type_invalid',
          blocksActivation: true,
        }],
        activationState: { state: 'approval_required' },
        decisionKey: 'agent-decision-v1',
        prompt: 'SECRET AGENT PROMPT',
      }, {
        candidateId: 'external-explore-v1',
        logicalId: 'explore',
        displayName: 'OpenCode Explore',
        description: 'Explore a codebase',
        providerLabel: 'OpenCode',
        scope: 'project',
        sourceKeys: [{ providerId: 'opencode.agents', sourceId: 'explore' }],
        sourceLocationLabels: ['<workspace>/.opencode/agents/explore.md'],
        sourceCount: 1,
        effectiveModelLabel: 'fast',
        effectiveToolLabels: ['Read', 'Grep'],
        supportsFollowUp: false,
        compatibilityState: 'ready',
        diagnostics: [],
        activationState: { state: 'conflict' },
        decisionKey: 'agent-decision-explore-v1',
      }],
      subagentConflicts: [{
        conflictKey: 'agent-conflict-v1',
        logicalId: 'explore',
        candidates: [{
          candidateId: 'builtin-explore',
          displayName: 'BitFun Explore',
          sourceLabel: 'BitFun',
          external: false,
        }, {
          candidateId: 'external-explore-v1',
          displayName: 'OpenCode Explore',
          sourceLabel: 'OpenCode',
          external: true,
        }],
      }],
      pendingSubagentApprovals: ['external-agent-review-v1'],
    };
    const activatedAgentSnapshot = {
      ...agentSnapshot,
      // An unrelated catalog refresh may already have advanced the visible
      // generation while this agent-scoped mutation was in flight.
      generation: 1,
      subagents: agentSnapshot.subagents.map((agent, index) => (
        index === 0 ? { ...agent, activationState: { state: 'active' } } : agent
      )),
      pendingSubagentApprovals: [],
    };
    getSnapshotMock
      .mockResolvedValueOnce(agentSnapshot)
      .mockResolvedValue(activatedAgentSnapshot);
    setSubagentActivationMock.mockResolvedValue(activatedAgentSnapshot);
    chooseSubagentConflictMock.mockResolvedValue({
      ...agentSnapshot,
      subagentConflicts: [{
        ...agentSnapshot.subagentConflicts[0],
        selectedCandidateId: 'external-explore-v1',
      }],
    });

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    const details = Array.from(container.querySelectorAll('button')).find((button) =>
      button.textContent?.includes('common.details'));
    await act(async () => details?.click());
    expect(container.textContent).toContain('agents.singleRun');
    expect(container.textContent).toContain('fast');
    expect(container.textContent).toContain('Read, Grep');
    expect(container.textContent).toContain('agents.executionDomain');
    expect(container.textContent).toContain('agentDiagnostics.unsupportedBehavior.reason');
    expect(container.textContent).toContain('agentDiagnostics.ignoredOption.reason');
    expect(container.textContent).toContain('agentDiagnostics.invalidDefinition.reason');
    expect(container.textContent).toContain('agentConflicts.selectionApproves');
    expect(container.textContent).toContain('.opencode/agents/explore.md');
    expect(container.textContent).not.toContain('D:/workspace/project/.opencode/agents');
    expect(container.innerHTML).not.toContain('D:');
    expect(container.innerHTML).not.toContain('D:/shared');
    expect(container.textContent).not.toContain('SECRET AGENT PROMPT');

    const enable = Array.from(container.querySelectorAll('button')).find((button) =>
      button.textContent?.includes('agents.enable'));
    await act(async () => enable?.click());
    expect(setSubagentActivationMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      'external-agent-review-v1',
      true,
      4,
      7,
      'agent-decision-v1',
    );
    expect(container.textContent).toContain('agentState.active');

    const externalCandidate = Array.from(container.querySelectorAll('button')).find((button) =>
      button.textContent?.includes('OpenCode Explore'));
    await act(async () => externalCandidate?.click());
    expect(chooseSubagentConflictMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      'agent-conflict-v1',
      'external-explore-v1',
      true,
      4,
      7,
    );
  });

  it('shows model-setting read failures as temporarily unavailable', async () => {
    getSnapshotMock.mockResolvedValue({
      ...snapshot,
      commandConflicts: [],
      subagentGeneration: 4,
      preferenceRevision: 7,
      subagents: [{
        candidateId: 'external-agent-review-v1',
        logicalId: 'review',
        displayName: 'External Review',
        description: 'Review a change',
        providerLabel: 'Future AI',
        scope: 'project',
        sourceKeys: [{ providerId: 'future.agents', sourceId: 'review' }],
        sourceLocationLabels: ['<workspace>/.future/agents/review.md'],
        sourceCount: 1,
        effectiveToolLabels: ['Read'],
        supportsFollowUp: false,
        compatibilityState: 'ready',
        diagnostics: [{
          code: 'external_subagent.configuration_unavailable',
          blocksActivation: true,
        }],
        activationState: { state: 'unavailable' },
        decisionKey: 'agent-decision-unavailable',
      }],
      subagentConflicts: [],
      pendingSubagentApprovals: [],
    });

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    const details = Array.from(container.querySelectorAll('button')).find((button) =>
      button.textContent?.includes('common.details'));
    await act(async () => details?.click());
    expect(container.textContent).toContain('agentState.unavailable');
    expect(container.textContent).not.toContain('agentState.blocked');
    expect(container.textContent).toContain('agentDiagnostics.configurationUnavailable.reason');
    expect(container.textContent).not.toContain('agents.enable');
  });

  it('non-blockingly reports when an enabled external agent becomes unavailable', async () => {
    const activeAgent = {
      candidateId: 'external-agent-review-v1',
      logicalId: 'review',
      displayName: 'External Review',
      description: 'Review a change',
      providerLabel: 'Future AI',
      scope: 'project',
      sourceKeys: [{ providerId: 'future.agents', sourceId: 'review' }],
      sourceLocationLabels: ['<workspace>/.future/agents/review.md'],
      sourceCount: 1,
      effectiveModelLabel: 'review-model',
      effectiveToolLabels: ['Read'],
      supportsFollowUp: false,
      compatibilityState: 'ready',
      diagnostics: [],
      activationState: { state: 'active' },
      decisionKey: 'agent-decision-v1',
    };
    const activeSnapshot = {
      ...snapshot,
      generation: 2,
      commandConflicts: [],
      subagentGeneration: 4,
      preferenceRevision: 7,
      subagents: [activeAgent],
      subagentConflicts: [],
      pendingSubagentApprovals: [],
    };
    const blockedSnapshot = {
      ...activeSnapshot,
      generation: 3,
      subagentGeneration: 5,
      subagents: [{
        ...activeAgent,
        activationState: { state: 'blocked' },
        decisionKey: 'agent-decision-v2',
      }],
    };
    getSnapshotMock
      .mockResolvedValueOnce(activeSnapshot)
      .mockResolvedValue(blockedSnapshot);

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    expect(container.textContent).not.toContain('agentChanges.unavailable');

    await act(async () => {
      window.dispatchEvent(new Event('focus'));
      await Promise.resolve();
    });
    expect(container.textContent).toContain('agentChanges.unavailable');
    expect(container.textContent).toContain('External Review');
    expect(container.querySelectorAll('[role="status"]')).toHaveLength(1);

    await act(async () => {
      window.dispatchEvent(new Event('focus'));
      await Promise.resolve();
    });
    expect(container.querySelectorAll('[role="status"]')).toHaveLength(1);

    getSnapshotMock.mockResolvedValue({
      ...activeSnapshot,
      generation: 4,
      subagentGeneration: 6,
    });
    await act(async () => {
      window.dispatchEvent(new Event('focus'));
      await Promise.resolve();
    });
    expect(container.textContent).not.toContain('agentChanges.unavailable');
  });

  it('shows source, working directory, and capabilities before enabling tool code', async () => {
    const approvalSnapshot = {
      ...snapshot,
      sources: [{
        stableKey: 'tool-source',
        record: {
          ...snapshot.sources[0].record,
          key: { providerId: 'opencode.tools', sourceId: 'project' },
          displayName: 'OpenCode project tools',
          sourceKind: 'tools',
          location: '<workspace>/.opencode/tools',
          executionDomainId: 'local:D:/workspace/project',
        },
        lifecycle: 'available',
      }],
      commandConflicts: [],
      tools: [{
        definition: {
          id: {
            target: {
              source: { providerId: 'opencode.tools', sourceId: 'project' },
              localId: 'weather.js',
            },
            exportId: 'default',
          },
          name: 'weather',
          descriptionPreview: 'Read the weather',
          modulePath: '<workspace>/.opencode/tools/weather.js',
          workingDirectory: '<workspace>/',
          runtimeKind: 'java_script',
          capabilities: ['file_system', 'network', 'environment', 'process'],
          contentVersion: 'v1',
          staticStatus: { state: 'ready' },
        },
        approvalKey: 'approval-1',
        decisionKey: 'decision-1',
        activation: { state: 'approval_required' },
      }],
      toolApprovalRequests: [{
        approvalKey: 'approval-1',
        decisionKey: 'decision-1',
        targetId: {
          source: { providerId: 'opencode.tools', sourceId: 'project' },
          localId: 'weather.js',
        },
        sourceDisplayName: 'OpenCode project tools',
        sourceScope: 'project',
        sourceLocation: '<workspace>/.opencode/tools/weather.js',
        workingDirectory: '<workspace>/',
        runtimeKind: 'java_script',
        capabilities: ['file_system', 'network', 'environment', 'process'],
        contentVersion: 'v1',
        toolNames: ['weather'],
      }],
    };
    getSnapshotMock.mockResolvedValue(approvalSnapshot);
    setToolTargetDecisionMock.mockResolvedValue({
      ...approvalSnapshot,
      toolApprovalRequests: [],
    });

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('toolApprovals.sourceRoot');
    expect(container.textContent).toContain('toolApprovals.modulePath');
    expect(container.textContent).toContain('<workspace>/.opencode/tools/weather.js');
    expect(container.textContent).toContain('executionLocation.local');
    expect(container.textContent).not.toContain('local:D:/workspace/project');
    expect(container.textContent).toContain('toolApprovals.workingDirectory');
    expect(container.textContent).toContain('capability.file_system');
    expect(container.textContent).toContain('capability.environment');
    const enable = Array.from(container.querySelectorAll('button')).find((button) =>
      button.textContent?.includes('toolApprovals.enable'));
    enable?.focus();
    await act(async () => enable?.click());

    expect(setToolTargetDecisionMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      'approval-1',
      'decision-1',
      true,
      0,
    );
    const operationStatus = Array.from(container.querySelectorAll('[role="status"]')).find(
      (candidate) => candidate.textContent?.includes('actions.updated'),
    );
    expect(operationStatus?.textContent).toContain('actions.updated');
    expect(document.activeElement).not.toBe(operationStatus);
  });

  it('lets a previously declined tool be reviewed and enabled without another automatic prompt', async () => {
    const disabledSnapshot = {
      ...snapshot,
      commandConflicts: [],
      tools: [{
        definition: {
          id: {
            target: {
              source: { providerId: 'opencode.tools', sourceId: 'project' },
              localId: 'weather.js',
            },
            exportId: 'default',
          },
          name: 'weather',
          descriptionPreview: 'Read the weather',
          modulePath: '<workspace>/.opencode/tools/weather.js',
          workingDirectory: '<workspace>/',
          runtimeKind: 'java_script',
          capabilities: ['file_system', 'network', 'environment', 'process'],
          contentVersion: 'v1',
          staticStatus: { state: 'ready' },
        },
        approvalKey: 'approval-1',
        decisionKey: 'decision-1',
        activation: { state: 'disabled' },
      }],
      toolApprovalRequests: [],
    };
    getSnapshotMock.mockResolvedValue(disabledSnapshot);
    setToolTargetDecisionMock.mockResolvedValue({
      ...disabledSnapshot,
      tools: [{ ...disabledSnapshot.tools[0], activation: { state: 'active' } }],
    });

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    expect(container.textContent).not.toContain('toolApprovals.warning');
    const review = Array.from(container.querySelectorAll('button')).find((button) =>
      button.textContent?.includes('common.details'));
    await act(async () => review?.click());
    expect(container.textContent).toContain('toolApprovals.warning');
    expect(container.textContent).toContain('capability.network');

    const enable = Array.from(container.querySelectorAll('button')).find((button) =>
      button.textContent?.includes('toolApprovals.enable'));
    await act(async () => enable?.click());
    expect(setToolTargetDecisionMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      'approval-1',
      'decision-1',
      true,
      0,
    );
  });

  it('shows source, execution scope, failure reason, and next step for every tool state', async () => {
    const toolSource = {
      stableKey: 'tool-source',
      record: {
        ...snapshot.sources[0].record,
        key: { providerId: 'opencode.tools', sourceId: 'project' },
        displayName: 'OpenCode project tools',
        sourceKind: 'tools',
        location: '<workspace>/.opencode/tools',
        executionDomainId: 'custom:D:/workspace/project',
      },
      lifecycle: 'available',
    };
    const toolDefinition = {
      id: {
        target: {
          source: { providerId: 'opencode.tools', sourceId: 'project' },
          localId: 'weather.ts',
        },
        exportId: 'default',
      },
      name: 'weather',
      descriptionPreview: 'Read the weather',
      modulePath: '<workspace>/.opencode/tools/weather.ts',
      workingDirectory: '<workspace>/',
      runtimeKind: 'type_script',
      capabilities: ['file_system', 'network'],
      contentVersion: 'v1',
      staticStatus: { state: 'ready' },
    };
    const stateSnapshot = {
      ...snapshot,
      sources: [toolSource],
      commandConflicts: [],
      tools: [
        {
          definition: toolDefinition,
          approvalKey: 'approval-disabled',
          decisionKey: 'decision-disabled',
          activation: { state: 'disabled' },
        },
        {
          definition: {
            ...toolDefinition,
            id: {
              ...toolDefinition.id,
              target: { ...toolDefinition.id.target, localId: 'broken.ts' },
            },
            name: 'broken',
            modulePath: '<workspace>/.opencode/tools/broken.ts',
          },
          approvalKey: 'approval-broken',
          decisionKey: 'decision-broken',
          activation: { state: 'load_failed', reason: 'Worker could not import the module.' },
        },
      ],
      toolApprovalRequests: [],
    };
    getSnapshotMock.mockResolvedValue(stateSnapshot);

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    const detailButtons = Array.from(container.querySelectorAll('button')).filter((button) =>
      button.textContent?.includes('common.details'));
    expect(detailButtons).toHaveLength(2);
    await act(async () => detailButtons[1]?.click());

    expect(container.textContent).toContain('<workspace>/.opencode/tools/broken.ts');
    expect(container.textContent).toContain('<workspace>/.opencode/tools');
    expect(container.textContent).toContain('executionLocation.unknown');
    expect(container.textContent).not.toContain('custom:D:/workspace/project');
    expect(container.textContent).not.toContain('Worker could not import the module.');
    expect(container.textContent).toContain('toolReason.load_failed');
    expect(container.textContent).toContain('toolNextStep.load_failed');
    expect(container.textContent).toContain('tools.targetScope');
  });

  it('renders a removed source as disabled and off', async () => {
    getSnapshotMock.mockResolvedValue({
      ...snapshot,
      sources: [{ ...snapshot.sources[0], lifecycle: 'removed' }],
      commandConflicts: [],
    });

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    const sourceToggle = container.querySelector(
      'input[aria-label^="sources.toggleLabel"]',
    ) as HTMLInputElement;
    expect(sourceToggle).not.toBeNull();
    expect(sourceToggle.checked).toBe(false);
    expect(sourceToggle.disabled).toBe(true);
    expect(container.textContent).toContain('sources.title');
  });

  it('counts a repeated grouped source diagnostic only once in the attention summary', async () => {
    const diagnostic = {
      severity: 'warning',
      code: 'opencode.configuration.invalid',
      message: 'The configuration could not be parsed.',
    };
    getSnapshotMock.mockResolvedValue({
      ...snapshot,
      diagnostics: [diagnostic],
      commandConflicts: [],
      sources: [{
        ...snapshot.sources[0],
        stableKey: 'command-source',
        presentationGroupId: 'project-config',
        record: { ...snapshot.sources[0].record, diagnostics: [diagnostic] },
      }, {
        ...snapshot.sources[0],
        stableKey: 'agent-source',
        presentationGroupId: 'project-config',
        record: {
          ...snapshot.sources[0].record,
          key: { providerId: 'opencode.subagents', sourceId: 'project' },
          diagnostics: [diagnostic],
        },
      }],
    });

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('policy.attentionSummary:{"count":1}');
    expect(container.textContent?.match(/opencode\.configuration\.invalid/g)).toHaveLength(1);
  });

  it('keeps grouped OpenCode diagnostics and lifecycle states visible', async () => {
    const diagnostic = {
      severity: 'warning',
      code: 'opencode.configuration.invalid',
      message: 'The configuration could not be parsed.',
    };
    getSnapshotMock.mockResolvedValue({
      ...snapshot,
      integrationPolicy,
      diagnostics: [diagnostic],
      commandConflicts: [],
      sources: [{
        ...snapshot.sources[0],
        stableKey: 'opencode-project',
        presentationGroupId: 'opencode-project',
        lifecycle: 'degraded',
        record: {
          ...snapshot.sources[0].record,
          ecosystemId: 'opencode',
          diagnostics: [diagnostic],
        },
      }],
    });

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('lifecycle.degraded');
    expect(container.textContent).toContain('opencode.configuration.invalid');
    expect(container.querySelector(
      '.bitfun-external-sources-config__opencode-card [data-external-attention="true"]',
    )).not.toBeNull();
    expect(container.textContent?.match(/opencode\.configuration\.invalid/g)).toHaveLength(1);
    const sourceToggle = container.querySelector(
      '.bitfun-external-sources-config__opencode-card input[aria-label^="sources.toggleLabel"]',
    ) as HTMLInputElement;
    expect(sourceToggle).not.toBeNull();
    await act(async () => sourceToggle.click());
    expect(setSourceEnabledMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      'opencode-project',
      false,
      0,
    );
  });

  it('ignores an older workspace response after switching workspaces', async () => {
    let resolveProject: ((value: typeof snapshot) => void) | undefined;
    const projectRequest = new Promise<typeof snapshot>((resolve) => {
      resolveProject = resolve;
    });
    const otherSnapshot = {
      ...snapshot,
      generation: 2,
      sources: [{
        ...snapshot.sources[0],
        stableKey: 'other-source',
        record: {
          ...snapshot.sources[0].record,
          displayName: 'Other workspace commands',
          location: '<workspace>/.opencode/commands',
        },
      }],
      commands: [discoveredCommand],
      diagnostics: [],
      commandConflicts: [],
    };
    getSnapshotMock.mockImplementation((workspacePath: string) => (
      workspacePath === 'D:/workspace/project'
        ? projectRequest
        : Promise.resolve(otherSnapshot)
    ));

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    workspaceState.path = 'D:/workspace/other';
    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    await act(async () => {
      resolveProject?.(snapshot);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('Other workspace commands');
    expect(container.textContent).not.toContain('OpenCode project commands');
  });

  it('isolates an in-flight response when the controlling Peer Host changes', async () => {
    let resolveFirstHost: ((value: typeof snapshot) => void) | undefined;
    getSnapshotMock
      .mockImplementationOnce(() => new Promise<typeof snapshot>((resolve) => {
        resolveFirstHost = resolve;
      }))
      .mockResolvedValueOnce({
        ...snapshot,
        generation: 2,
        sources: [{
          ...snapshot.sources[0],
          stableKey: 'peer-b-source',
          record: {
            ...snapshot.sources[0].record,
            displayName: 'Peer B commands',
          },
        }],
        commands: [discoveredCommand],
        diagnostics: [],
        commandConflicts: [],
      });

    peerState.deviceId = 'peer-a';
    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    peerState.deviceId = 'peer-b';
    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
      await Promise.resolve();
    });
    await act(async () => {
      resolveFirstHost?.(snapshot);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('Peer B commands');
    expect(container.textContent).not.toContain('OpenCode project commands');
  });

  it('ignores a source mutation response from the previous workspace', async () => {
    let resolveMutation: ((value: typeof snapshot) => void) | undefined;
    const pendingMutation = new Promise<typeof snapshot>((resolve) => {
      resolveMutation = resolve;
    });
    setSourceEnabledMock.mockReturnValue(pendingMutation);
    const otherSnapshot = {
      ...snapshot,
      generation: 2,
      sources: [{
        ...snapshot.sources[0],
        stableKey: 'other-source',
        record: {
          ...snapshot.sources[0].record,
          displayName: 'Other workspace commands',
        },
      }],
      commands: [discoveredCommand],
      diagnostics: [],
      commandConflicts: [],
    };

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    const sourceToggle = container.querySelector(
      'input[aria-label^="sources.toggleLabel"]',
    ) as HTMLInputElement;
    await act(async () => sourceToggle.click());

    workspaceState.path = 'D:/workspace/other';
    getSnapshotMock.mockResolvedValue(otherSnapshot);
    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    await act(async () => {
      resolveMutation?.(snapshot);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('Other workspace commands');
    expect(container.textContent).not.toContain('OpenCode project commands');
  });

  it('keeps the latest mutation authoritative over a focus refresh', async () => {
    let resolveMutation: ((value: typeof snapshot) => void) | undefined;
    setSourceEnabledMock.mockReturnValue(new Promise<typeof snapshot>((resolve) => {
      resolveMutation = resolve;
    }));

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    const sourceToggle = container.querySelector(
      'input[aria-label^="sources.toggleLabel"]',
    ) as HTMLInputElement;
    await act(async () => sourceToggle.click());

    await act(async () => {
      window.dispatchEvent(new Event('focus'));
      await Promise.resolve();
    });
    await act(async () => {
      resolveMutation?.({
        ...snapshot,
        generation: 2,
        sources: [{ ...snapshot.sources[0], lifecycle: 'suppressed' }],
      });
      await Promise.resolve();
    });

    const updatedToggle = container.querySelector(
      'input[aria-label^="sources.toggleLabel"]',
    ) as HTMLInputElement;
    expect(updatedToggle.checked).toBe(false);
    expect(container.textContent).toContain('lifecycle.suppressed');
  });

  it('ignores a failed refresh while a source mutation is pending', async () => {
    let resolveMutation: ((value: typeof snapshot) => void) | undefined;
    getSnapshotMock
      .mockResolvedValueOnce(snapshot)
      .mockRejectedValueOnce(new Error('stale refresh failure'));
    setSourceEnabledMock.mockReturnValue(new Promise<typeof snapshot>((resolve) => {
      resolveMutation = resolve;
    }));

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    const sourceToggle = container.querySelector(
      'input[aria-label^="sources.toggleLabel"]',
    ) as HTMLInputElement;
    await act(async () => sourceToggle.click());
    await act(async () => {
      window.dispatchEvent(new Event('focus'));
      await Promise.resolve();
    });

    expect(container.textContent).not.toContain('errors.refreshFailed');

    await act(async () => {
      resolveMutation?.({
        ...snapshot,
        generation: 2,
        sources: [{ ...snapshot.sources[0], lifecycle: 'suppressed' }],
      });
      await Promise.resolve();
    });

    expect(container.textContent).toContain('lifecycle.suppressed');
    expect(container.textContent).not.toContain('errors.refreshFailed');
  });

  it('gives remote projects a useful unavailable next step', async () => {
    workspaceState.kind = 'remote';
    getSnapshotMock.mockRejectedValueOnce({
      code: 'host_unavailable',
      message: 'unavailable',
    });

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('unavailable.remoteDescription');
    expect(container.textContent).not.toContain('unavailable.hostDescription');
  });

  it('gives a failed remote connection a useful next step', async () => {
    peerState.deviceId = 'remote-device';
    getSnapshotMock.mockRejectedValueOnce({
      code: 'host_unavailable',
      message: 'unavailable',
    });

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('unavailable.remoteConnectionDescription');
    expect(container.textContent).not.toContain('unavailable.hostDescription');
    expect(container.textContent).not.toContain('unavailable.remoteDescription');
  });

  it('fails closed for a legacy read-only host and never sends a mutation', async () => {
    const { hostCapabilities: _omitted, ...legacySnapshot } = snapshot;
    const withoutGroupId = snapshot.sources.map((source) => {
      const { presentationGroupId: _groupId, ...legacySource } = source;
      return legacySource;
    });
    getSnapshotMock.mockResolvedValue({
      ...legacySnapshot,
      sources: [
        ...withoutGroupId,
        {
          ...withoutGroupId[0],
          stableKey: 'legacy-suppressed-agent',
          lifecycle: 'suppressed',
          record: {
            ...withoutGroupId[0].record,
            key: { providerId: 'opencode.agents', sourceId: 'project' },
          },
        },
      ],
    });

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    const sourceToggle = container.querySelector(
      'input[aria-label^="sources.toggleLabel"]',
    ) as HTMLInputElement;
    const policyToggle = container.querySelector(
      '.bitfun-external-sources-config__policy-card input[type="checkbox"]',
    ) as HTMLInputElement;
    expect(container.querySelectorAll(
      '.bitfun-external-sources-config__source-group',
    )).toHaveLength(2);
    expect(sourceToggle.disabled).toBe(true);
    expect(policyToggle.disabled).toBe(true);
    await act(async () => {
      sourceToggle.click();
      policyToggle.click();
    });
    expect(setSourceEnabledMock).not.toHaveBeenCalled();
    expect(updateIntegrationPolicyMock).not.toHaveBeenCalled();
  });

  it('requires explicit confirmation before resetting an incompatible policy', async () => {
    const incompatibleSnapshot = {
      ...snapshot,
      preferenceRevision: 9,
      integrationPolicy: {
        ...integrationPolicy,
        schemaMajor: 13,
        status: 'incompatible_schema',
      },
    };
    getSnapshotMock.mockResolvedValue(incompatibleSnapshot);
    updateIntegrationPolicyMock.mockResolvedValue({
      ...snapshot,
      preferenceRevision: 10,
      integrationPolicy,
    });

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    const reset = Array.from(container.querySelectorAll('button')).find((button) =>
      button.textContent === 'policy.backupAndReset');
    await act(async () => reset?.click());
    expect(updateIntegrationPolicyMock).not.toHaveBeenCalled();
    expect(document.body.textContent).toContain('policy.resetConfirmTitle');

    const confirm = Array.from(document.body.querySelectorAll('button')).filter((button) =>
      button.textContent === 'policy.backupAndReset').at(-1);
    await act(async () => confirm?.click());
    expect(updateIntegrationPolicyMock).toHaveBeenCalledWith('D:/workspace/project', {
      expectedPreferenceRevision: 9,
      scope: 'user',
      change: { operation: 'reset_incompatible_policy' },
    });
  });

  it('opens and focuses the first diagnostic when the attention summary is activated', async () => {
    const scrollIntoView = vi.fn();
    Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', {
      configurable: true,
      value: scrollIntoView,
    });
    getSnapshotMock.mockResolvedValue({
      ...snapshot,
      commandConflicts: [],
      sources: [{
        ...snapshot.sources[0],
        record: {
          ...snapshot.sources[0].record,
          diagnostics: [{
            severity: 'warning',
            code: 'opencode.command.source_warning',
            message: 'A source needs attention.',
          }],
        },
      }],
    });

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    const attention = container.querySelector(
      'button[aria-controls="external-integration-attention-region"]',
    ) as HTMLButtonElement;
    await act(async () => attention.click());
    const firstDiagnostic = container.querySelector(
      'details[data-external-attention="true"]',
    ) as HTMLDetailsElement;
    expect(firstDiagnostic.open).toBe(true);
    expect(document.activeElement).toBe(firstDiagnostic.querySelector('summary'));
    expect(scrollIntoView).toHaveBeenCalled();
    expect(container.textContent).toContain('opencode.command.source_warning');
  });

  it('shows a stable reference without exposing an internal mutation message', async () => {
    getSnapshotMock.mockResolvedValue(snapshot);
    setSourceEnabledMock.mockRejectedValueOnce(Object.assign(
      new Error('database connection string should stay private'),
      { code: 'internal', retryable: true, correlationId: 'external-source-ref-7' },
    ));

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    const sourceToggle = container.querySelector(
      'input[aria-label^="sources.toggleLabel"]',
    ) as HTMLInputElement;
    await act(async () => sourceToggle.click());

    expect(container.textContent).toContain('operationErrors.internal');
    expect(container.textContent).toContain('external-source-ref-7');
    expect(container.textContent).not.toContain('database connection string');
  });

  it('shows MCP empty state when no MCP servers found', async () => {
    const emptyMcpSnapshot = {
      ...snapshot,
      mcpServers: [],
      mcpApprovalRequests: [],
      mcpConflicts: [],
    };
    getSnapshotMock.mockResolvedValue(emptyMcpSnapshot);

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('mcp.empty');
    expect(container.textContent).toContain('mcp.emptyGuidance');
  });

  it('does not show the MCP empty state when the initial snapshot failed', async () => {
    getSnapshotMock.mockRejectedValueOnce(new Error('initial load failed'));

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('errors.loadFailed');
    expect(container.textContent).not.toContain('mcp.empty');
  });

  it('renders subagent source locations as path links', async () => {
    const agentSnapshot = {
      ...snapshot,
      commandConflicts: [],
      sources: [{
        ...snapshot.sources[0],
        stableKey: 'opencode-agent-user',
        record: {
          ...snapshot.sources[0].record,
          key: { providerId: 'opencode.agents', sourceId: 'user' },
          location: '<workspace>/.opencode/agents/review.md',
        },
      }, {
        ...snapshot.sources[0],
        stableKey: 'opencode-agent-admin',
        record: {
          ...snapshot.sources[0].record,
          key: { providerId: 'opencode.agents', sourceId: 'admin' },
          location: '<workspace>/.opencode/agents/review.md',
        },
      }],
      subagents: [{
        candidateId: 'agent-path-test',
        logicalId: 'test',
        displayName: 'Test Agent',
        description: 'A test agent',
        providerLabel: 'OpenCode',
        scope: 'project',
        sourceKeys: [
          { providerId: 'opencode.agents', sourceId: 'user' },
          { providerId: 'opencode.agents', sourceId: 'admin' },
        ],
        sourceLocationLabels: ['<workspace>/.opencode/agents/review.md'],
        sourceCount: 2,
        effectiveModelLabel: 'gpt-4',
        effectiveToolLabels: ['read'],
        supportsFollowUp: false,
        compatibilityState: 'ready',
        diagnostics: [],
        activationState: { state: 'approval_required' },
        decisionKey: 'agent-path-key',
      }],
    };
    getSnapshotMock.mockResolvedValue(agentSnapshot);

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    const detailsButton = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent === 'common.details',
    );
    await act(async () => detailsButton?.click());

    const pathLinks = container.querySelectorAll(
      '.bitfun-external-sources-config__source-detail-toggle .bitfun-external-sources-config__path-link',
    );
    expect(pathLinks).toHaveLength(2);
    await act(async () => {
      (pathLinks[0] as HTMLAnchorElement).click();
      (pathLinks[1] as HTMLAnchorElement).click();
      await Promise.resolve();
    });
    expect(revealSourceLocationMock).toHaveBeenNthCalledWith(
      1,
      'D:/workspace/project',
      'opencode-agent-user',
    );
    expect(revealSourceLocationMock).toHaveBeenNthCalledWith(
      2,
      'D:/workspace/project',
      'opencode-agent-admin',
    );
  });

  it('renders the OpenCode aggregation card with summary counts and config locations', async () => {
    const opencodeSnapshot = {
      ...snapshot,
      integrationPolicy,
      commandConflicts: [],
      sources: [{
        stableKey: 'opencode-source',
        presentationGroupId: 'opencode-source',
        record: {
          key: { providerId: 'opencode.commands', sourceId: 'project' },
          ecosystemId: 'opencode',
          displayName: 'OpenCode project commands',
          sourceKind: 'prompt_commands',
          scope: 'project',
          location: '<workspace>/.opencode/commands',
          health: 'available',
          contentVersion: 'v1',
        },
        lifecycle: 'suppressed',
      }, {
        stableKey: 'opencode-source-secondary',
        presentationGroupId: 'opencode-source-secondary',
        record: {
          key: { providerId: 'opencode.commands', sourceId: 'user' },
          ecosystemId: 'opencode',
          displayName: 'OpenCode user commands',
          sourceKind: 'prompt_commands',
          scope: 'user_global',
          location: '<workspace>/.opencode/commands',
          health: 'available',
          contentVersion: 'v2',
        },
        lifecycle: 'suppressed',
      }],
    };
    getSnapshotMock.mockResolvedValue(opencodeSnapshot);

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('opencode.title');
    expect(container.querySelector(
      '.bitfun-external-sources-config__opencode-card',
    )).not.toBeNull();
    expect(container.textContent).not.toContain('sources.title');

    const expandButton = container.querySelector(
      'button[aria-controls="external-capabilities-opencode"]',
    );
    expect(expandButton).not.toBeNull();
    await act(async () => {
      (expandButton as HTMLButtonElement).click();
      await Promise.resolve();
    });
    expect(expandButton?.getAttribute('aria-expanded')).toBe('true');
    const locationBlocks = container.querySelectorAll(
      '.bitfun-external-sources-config__opencode-locations',
    );
    expect(locationBlocks).toHaveLength(2);
    const expandedLinks = locationBlocks[1].querySelectorAll(
      '.bitfun-external-sources-config__path-link',
    );
    expect(expandedLinks).toHaveLength(2);
    await act(async () => {
      (expandedLinks[1] as HTMLAnchorElement).click();
      await Promise.resolve();
    });
    expect(revealSourceLocationMock).toHaveBeenLastCalledWith(
      'D:/workspace/project',
      'opencode-source-secondary',
    );
  });

  it('labels all OpenCode scopes with supported translations and includes tools in the summary', async () => {
    const opencodeSnapshot = {
      ...snapshot,
      integrationPolicy,
      commandConflicts: [],
      commands: [{
        ...discoveredCommand,
        definition: {
          ...discoveredCommand.definition,
          id: {
            ...discoveredCommand.definition.id,
            source: { providerId: 'opencode.commands', sourceId: 'user' },
          },
        },
      }],
      tools: [{
        definition: {
          id: {
            target: {
              source: { providerId: 'opencode.tools', sourceId: 'config-dir' },
              localId: 'weather',
            },
            exportId: 'weather',
          },
          displayName: 'Weather',
          description: 'Weather tool',
          modulePath: '<workspace>/.opencode/tools/weather.js',
          workingDirectory: '<workspace>',
          runtimeKind: 'javascript',
          capabilities: [],
          contentVersion: 'v1',
        },
        activation: { state: 'disabled' },
      }],
      sources: [{
        ...snapshot.sources[0],
        stableKey: 'opencode-workspace',
        presentationGroupId: 'opencode-workspace',
        record: {
          ...snapshot.sources[0].record,
          key: { providerId: 'opencode.tools', sourceId: 'config-dir' },
          ecosystemId: 'opencode',
          scope: 'workspace_local',
        },
      }, {
        ...snapshot.sources[0],
        stableKey: 'opencode-user',
        presentationGroupId: 'opencode-user',
        record: {
          ...snapshot.sources[0].record,
          key: { providerId: 'opencode.commands', sourceId: 'user' },
          ecosystemId: 'opencode',
          scope: 'user_global',
          location: '~/.config/opencode/opencode.json',
        },
      }],
    };
    getSnapshotMock.mockResolvedValue(opencodeSnapshot);

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('shared:features.workspace');
    expect(container.textContent).toContain('scope.user_global');
    expect(container.textContent).not.toContain('scope.workspace_local');
    expect(container.textContent).toContain('"tools":1');
  });
});
