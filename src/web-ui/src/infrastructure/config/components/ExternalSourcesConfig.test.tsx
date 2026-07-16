// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import ExternalSourcesConfig from './ExternalSourcesConfig';

const getSnapshotMock = vi.hoisted(() => vi.fn());
const setSourceEnabledMock = vi.hoisted(() => vi.fn());
const setConflictChoiceMock = vi.hoisted(() => vi.fn());
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
      location: 'D:/workspace/project/.opencode/commands',
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
      sourceLocation: 'D:/workspace/project/.opencode/commands',
      availability: { state: 'available' },
    }, {
      candidateId: 'candidate-other',
      source: { providerId: 'other.commands', sourceId: 'project' },
      sourceDisplayName: 'Other project commands',
      ecosystemId: 'other',
      contentVersion: 'v1',
      commandDescription: 'Review with another source',
      sourceScope: 'project',
      sourceLocation: 'D:/workspace/project/.other/commands',
      availability: { state: 'available' },
    }],
  }],
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

  it('keeps a neutral checking state until initial discovery completes', async () => {
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
    expect(container.textContent).toContain('loading');
    expect(container.textContent).not.toContain('sources.empty');

    await act(async () => {
      await vi.advanceTimersByTimeAsync(750);
    });
    expect(container.textContent).toContain('OpenCode project commands');
    expect(container.textContent).not.toContain('loading');
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
          location: 'D:/workspace/other/.opencode/commands',
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
