// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useExternalAppAwareness } from './useExternalAppAwareness';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const getAwarenessMock = vi.hoisted(() => vi.fn());
const acknowledgeMock = vi.hoisted(() => vi.fn());
const workspaceState = vi.hoisted(() => ({ path: 'D:/workspace/project', kind: 'normal' }));
const settingsState = vi.hoisted(() => ({ activeTab: 'general', markTabUnseen: vi.fn() }));

vi.mock('@/infrastructure/api/service-api/ExternalSourcesAPI', () => ({
  externalSourcesAPI: {
    getEcosystemAwareness: getAwarenessMock,
    acknowledgeEcosystems: acknowledgeMock,
  },
}));
vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useOptionalCurrentWorkspace: () => ({
    workspace: { workspaceKind: workspaceState.kind },
    workspacePath: workspaceState.path,
  }),
}));
vi.mock('@/app/scenes/settings/settingsStore', () => ({
  useSettingsStore: (selector: (state: typeof settingsState) => unknown) => selector(settingsState),
}));
vi.mock('@/shared/types', () => ({
  isRemoteWorkspace: (workspace: { workspaceKind?: string } | null) => workspace?.workspaceKind === 'remote',
}));
vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({ debug: vi.fn() }),
}));

function Harness() {
  useExternalAppAwareness();
  return null;
}

async function flush() {
  await act(async () => { await Promise.resolve(); });
}

describe('useExternalAppAwareness', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    root = createRoot(container);
    workspaceState.path = 'D:/workspace/project';
    workspaceState.kind = 'normal';
    settingsState.activeTab = 'general';
    settingsState.markTabUnseen.mockReset();
    getAwarenessMock.mockReset().mockResolvedValue(['opencode']);
    acknowledgeMock.mockReset().mockResolvedValue(undefined);
  });

  it('does not call local-only awareness commands for remote workspaces', async () => {
    workspaceState.kind = 'remote';
    await act(async () => { root.render(<Harness />); });
    await flush();

    expect(getAwarenessMock).not.toHaveBeenCalled();
  });

  it('does not restore a stale dot after acknowledgement wins the initial-read race', async () => {
    let resolveInitial: ((ids: string[]) => void) | undefined;
    getAwarenessMock
      .mockImplementationOnce(() => new Promise<string[]>((resolve) => { resolveInitial = resolve; }))
      .mockResolvedValueOnce(['opencode']);
    settingsState.activeTab = 'external-sources';

    await act(async () => { root.render(<Harness />); });
    await flush();
    await flush();
    await act(async () => { resolveInitial?.(['opencode']); });

    expect(settingsState.markTabUnseen).not.toHaveBeenLastCalledWith('external-sources', true);
  });

  it('allows acknowledgement to retry after a failed persistence attempt', async () => {
    settingsState.activeTab = 'external-sources';
    acknowledgeMock.mockRejectedValueOnce(new Error('write failed')).mockResolvedValueOnce(undefined);
    await act(async () => { root.render(<Harness />); });
    await flush();
    await flush();

    settingsState.activeTab = 'general';
    await act(async () => { root.render(<Harness />); });
    settingsState.activeTab = 'external-sources';
    await act(async () => { root.render(<Harness />); });
    await flush();
    await flush();

    expect(acknowledgeMock).toHaveBeenCalledTimes(2);
  });
});
