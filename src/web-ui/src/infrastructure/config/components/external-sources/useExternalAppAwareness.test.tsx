// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useExternalAppAwareness } from './useExternalAppAwareness';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const getAwarenessMock = vi.hoisted(() => vi.fn());
const acknowledgeMock = vi.hoisted(() => vi.fn());
const workspaceState = vi.hoisted(() => ({ path: 'D:/workspace/project', kind: 'normal' }));

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
vi.mock('@/shared/types', () => ({
  isRemoteWorkspace: (workspace: { workspaceKind?: string } | null) => workspace?.workspaceKind === 'remote',
}));
vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({ debug: vi.fn() }),
}));

function Harness({ active = false }: { active?: boolean }) {
  const hasUnseen = useExternalAppAwareness(active);
  return <div data-testid="awareness" data-unseen={hasUnseen ? 'true' : 'false'} />;
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
    getAwarenessMock.mockReset().mockResolvedValue(['opencode']);
    acknowledgeMock.mockReset().mockResolvedValue(undefined);
  });

  afterEach(() => {
    act(() => root.unmount());
  });

  it('does not call local-only awareness commands for remote workspaces', async () => {
    workspaceState.kind = 'remote';
    await act(async () => { root.render(<Harness />); });
    await flush();

    expect(getAwarenessMock).not.toHaveBeenCalled();
    expect(container.querySelector('[data-testid="awareness"]')?.getAttribute('data-unseen'))
      .toBe('false');
  });

  it('reports discoveries while the ecosystem scene has not been opened', async () => {
    await act(async () => { root.render(<Harness />); });
    await flush();

    expect(container.querySelector('[data-testid="awareness"]')?.getAttribute('data-unseen'))
      .toBe('true');
  });

  it('does not restore a stale dot after acknowledgement wins the initial-read race', async () => {
    let resolveInitial: ((ids: string[]) => void) | undefined;
    getAwarenessMock
      .mockImplementationOnce(() => new Promise<string[]>((resolve) => { resolveInitial = resolve; }))
      .mockResolvedValueOnce(['opencode']);

    await act(async () => { root.render(<Harness active />); });
    await flush();
    await flush();
    await act(async () => { resolveInitial?.(['opencode']); });

    expect(container.querySelector('[data-testid="awareness"]')?.getAttribute('data-unseen'))
      .toBe('false');
  });

  it('allows acknowledgement to retry after a failed persistence attempt', async () => {
    acknowledgeMock.mockRejectedValueOnce(new Error('write failed')).mockResolvedValueOnce(undefined);
    await act(async () => { root.render(<Harness active />); });
    await flush();
    await flush();

    expect(container.querySelector('[data-testid="awareness"]')?.getAttribute('data-unseen'))
      .toBe('true');

    await act(async () => { root.render(<Harness active={false} />); });
    await act(async () => { root.render(<Harness active />); });
    await flush();
    await flush();

    expect(acknowledgeMock).toHaveBeenCalledTimes(2);
  });
});
