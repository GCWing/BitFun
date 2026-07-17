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
const workspaceState = vi.hoisted(() => ({ path: 'D:/workspace/project' }));

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
    workspace: { rootPath: workspaceState.path },
    workspacePath: workspaceState.path,
  }),
}));

vi.mock('@/infrastructure/runtime', () => ({ isTauriRuntime: () => true }));
vi.mock('@/shared/types', () => ({ isRemoteWorkspace: () => false }));
vi.mock('@/infrastructure/api/service-api/ExternalSourcesAPI', () => ({
  externalSourcesAPI: {
    getSnapshot: getSnapshotMock,
    setSourceEnabled: setSourceEnabledMock,
    setConflictChoice: setConflictChoiceMock,
    setToolTargetDecision: setToolTargetDecisionMock,
    setToolConflictChoice: setToolConflictChoiceMock,
    setSubagentActivation: setSubagentActivationMock,
    chooseSubagentConflict: chooseSubagentConflictMock,
  },
}));

const snapshot = {
  generation: 1,
  discoveryPending: false,
  sources: [{
    stableKey: 'source-key',
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
};

describe('ExternalSourcesConfig', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    workspaceState.path = 'D:/workspace/project';
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
    );
    expect(container.textContent).not.toContain('conflicts.commandName');

    const sourceToggle = container.querySelector('input[type="checkbox"]') as HTMLInputElement;
    expect(sourceToggle.checked).toBe(true);
    await act(async () => sourceToggle.click());
    expect(setSourceEnabledMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      'source-key',
      false,
    );
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

  it('distinguishes initial load failures from uncertain mutation results', async () => {
    getSnapshotMock.mockRejectedValueOnce(new Error('initial load failed'));
    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    expect(container.textContent).toContain('errors.loadFailed');
    expect(container.textContent).toContain('initial load failed');

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
    const sourceToggle = container.querySelector('input[type="checkbox"]') as HTMLInputElement;
    await act(async () => sourceToggle.click());
    expect(container.textContent).toContain('errors.mutationUnknown');
    expect(container.textContent).toContain('save result unknown');
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
      button.textContent?.includes('actions.refresh'));
    await act(async () => refresh?.click());
    expect(container.textContent).toContain('errors.refreshFailed');
    expect(container.textContent).toContain('OpenCode project commands');
    expect(container.textContent).toContain('refresh failed');
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
    expect(container.textContent).toContain('sources.agentCount:{"count":1}');
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
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(container.textContent).toContain('agentChanges.unavailable');
    expect(container.textContent).toContain('External Review');
    expect(container.querySelectorAll('[role="status"]')).toHaveLength(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(container.querySelectorAll('[role="status"]')).toHaveLength(1);

    getSnapshotMock.mockResolvedValue({
      ...activeSnapshot,
      generation: 4,
      subagentGeneration: 6,
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
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
    await act(async () => enable?.click());

    expect(setToolTargetDecisionMock).toHaveBeenCalledWith(
      'D:/workspace/project',
      'approval-1',
      'decision-1',
      true,
    );
    const operationStatus = container.querySelector('[role="status"][tabindex="-1"]');
    expect(operationStatus?.textContent).toContain('actions.updated');
    expect(document.activeElement).toBe(operationStatus);
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
    const sourceToggle = container.querySelector('input[type="checkbox"]') as HTMLInputElement;
    expect(sourceToggle.disabled).toBe(true);
    expect(sourceToggle.checked).toBe(false);
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
      diagnostics: [],
      commandConflicts: [],
    };

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    const sourceToggle = container.querySelector('input[type="checkbox"]') as HTMLInputElement;
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

  it('keeps the latest mutation authoritative over an intervening poll', async () => {
    let resolveMutation: ((value: typeof snapshot) => void) | undefined;
    setSourceEnabledMock.mockReturnValue(new Promise<typeof snapshot>((resolve) => {
      resolveMutation = resolve;
    }));

    await act(async () => {
      root.render(<ExternalSourcesConfig />);
      await Promise.resolve();
    });
    const sourceToggle = container.querySelector('input[type="checkbox"]') as HTMLInputElement;
    await act(async () => sourceToggle.click());

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    await act(async () => {
      resolveMutation?.({
        ...snapshot,
        generation: 2,
        sources: [{ ...snapshot.sources[0], lifecycle: 'suppressed' }],
      });
      await Promise.resolve();
    });

    const updatedToggle = container.querySelector('input[type="checkbox"]') as HTMLInputElement;
    expect(updatedToggle.checked).toBe(false);
    expect(container.textContent).toContain('lifecycle.suppressed');
  });
});
