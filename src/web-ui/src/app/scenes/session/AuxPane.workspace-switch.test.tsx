/**
 * @vitest-environment jsdom
 */

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  workspaceId: 'workspace-a' as string | undefined,
  workspaceEventListener: null as ((event: any) => void) | null,
  removeWorkspaceEventListener: vi.fn(),
  switchAgentCanvasWorkspace: vi.fn(),
  removeAgentCanvasSnapshot: vi.fn(),
  canvasStore: {
    addTab: vi.fn(),
    switchToTab: vi.fn(),
    findTabByMetadata: vi.fn(() => null),
    updateTabContent: vi.fn(),
    closeAllTabs: vi.fn(),
    primaryGroup: { tabs: [] },
    secondaryGroup: { tabs: [] },
    tertiaryGroup: { tabs: [] },
  },
}));

vi.mock('../../components/panels/content-canvas', () => ({
  ContentCanvas: () => <div data-testid="content-canvas" />,
  useCanvasStore: (selector: (state: typeof mocks.canvasStore) => unknown) => (
    selector(mocks.canvasStore)
  ),
}));

vi.mock('../../components/panels/content-canvas/stores', () => ({
  switchAgentCanvasWorkspace: mocks.switchAgentCanvasWorkspace,
  removeAgentCanvasSnapshot: mocks.removeAgentCanvasSnapshot,
}));

vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useCurrentWorkspace: () => ({
    workspace: mocks.workspaceId ? { id: mocks.workspaceId } : null,
  }),
}));

vi.mock('@/infrastructure/services/business/workspaceManager', () => ({
  workspaceManager: {
    addEventListener: (listener: (event: any) => void) => {
      mocks.workspaceEventListener = listener;
      return mocks.removeWorkspaceEventListener;
    },
  },
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    debug: vi.fn(),
  }),
}));

import AuxPane from './AuxPane';

vi.mock('./sessionPanelLayout', () => ({
  expandSessionAuxPane: vi.fn(),
  collapseSessionAuxPane: vi.fn(),
}));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('AuxPane workspace canvas switching', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    mocks.workspaceId = 'workspace-a';
    mocks.workspaceEventListener = null;
    mocks.removeWorkspaceEventListener.mockReset();
    mocks.switchAgentCanvasWorkspace.mockReset();
    mocks.removeAgentCanvasSnapshot.mockReset();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('swaps canvas snapshots synchronously before the target review tab can open', () => {
    act(() => {
      root.render(<AuxPane />);
    });

    expect(mocks.switchAgentCanvasWorkspace).toHaveBeenCalledWith(null, 'workspace-a');
    mocks.switchAgentCanvasWorkspace.mockClear();

    const order: string[] = [];
    mocks.switchAgentCanvasWorkspace.mockImplementation(() => {
      order.push('canvas-swapped');
    });

    act(() => {
      mocks.workspaceEventListener?.({
        type: 'workspace:switched',
        workspace: { id: 'workspace-b' },
      });
      order.push('open-target-review');
    });

    expect(mocks.switchAgentCanvasWorkspace).toHaveBeenCalledWith(
      'workspace-a',
      'workspace-b',
    );
    expect(order).toEqual(['canvas-swapped', 'open-target-review']);

    act(() => {
      mocks.workspaceEventListener?.({
        type: 'workspace:active-changed',
        workspace: { id: 'workspace-b' },
      });
    });

    expect(mocks.switchAgentCanvasWorkspace).toHaveBeenCalledTimes(1);
  });
});
