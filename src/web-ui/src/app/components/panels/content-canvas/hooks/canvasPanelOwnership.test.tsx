// @vitest-environment jsdom
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  CanvasStoreModeContext,
  clearAgentCanvasForPeerSwitch,
  switchAgentCanvasWorkspace,
  useAgentCanvasStore,
  useBottomTerminalCanvasStore,
  useCanvasStore,
  useGitCanvasStore,
  usePanelViewCanvasStore,
  useProjectCanvasStore,
} from '../stores';
import type { CanvasStoreMode } from '../stores/canvasStore';
import { TAB_EVENTS, type PanelContent } from '../types';
import { appManager } from '@/app/services/AppManager';
import {
  collapseSessionAuxPane,
  collapseSessionBottomTerminalPane,
  expandSessionAuxPane,
  expandSessionBottomTerminalPane,
} from '@/app/scenes/session/sessionPanelLayout';
import { fileTabManager } from '@/shared/services/FileTabManager';
import { drainPendingTabs } from '@/shared/services/pendingTabQueue';
import { usePanelTabCoordinator } from './usePanelTabCoordinator';
import { useTabLifecycle } from './useTabLifecycle';

// Real stores, layout owner, file producer and lifecycle hooks. These probes
// exercise state transitions without replacing adapters or rendering editors.
function CanvasProbe({ mode, onReveal }: { mode: CanvasStoreMode; onReveal?: () => void }) {
  useTabLifecycle({ mode, onReveal });
  return null;
}

const expandBottom = () => expandSessionBottomTerminalPane(240);

function PanelProbe({ bottom = false }: { bottom?: boolean }) {
  const scopeKey = useCanvasStore(state => state.workspaceKey);
  const visibleTabCount = useCanvasStore(state => (
    [state.primaryGroup, state.secondaryGroup, state.tertiaryGroup]
      .reduce((count, group) => count + group.tabs.filter(tab => !tab.isHidden).length, 0)
  ));
  const { expandPanel } = usePanelTabCoordinator({
    visibleTabCount,
    scopeKey,
    expandEventName: bottom ? TAB_EVENTS.EXPAND_BOTTOM_TERMINAL_PANEL : TAB_EVENTS.EXPAND_RIGHT_PANEL,
    onExpand: bottom ? expandBottom : expandSessionAuxPane,
    onCollapse: bottom ? collapseSessionBottomTerminalPane : collapseSessionAuxPane,
  });
  return <CanvasProbe mode={bottom ? 'bottom-terminal' : 'agent'} onReveal={expandPanel} />;
}

function Hosts({ project = true }: { project?: boolean }) {
  return <>
    <CanvasStoreModeContext.Provider value="agent">
      <PanelProbe />
    </CanvasStoreModeContext.Provider>
    <CanvasStoreModeContext.Provider value="bottom-terminal">
      <PanelProbe bottom />
    </CanvasStoreModeContext.Provider>
    {project && <CanvasStoreModeContext.Provider value="project">
      <CanvasProbe mode="project" />
    </CanvasStoreModeContext.Provider>}
    <CanvasStoreModeContext.Provider value="git">
      <CanvasProbe mode="git" />
    </CanvasStoreModeContext.Provider>
    <CanvasStoreModeContext.Provider value="panel-view">
      <CanvasProbe mode="panel-view" />
    </CanvasStoreModeContext.Provider>
  </>;
}

const content = (title: string): PanelContent => ({ type: 'text-viewer', title, data: { content: title } });

describe('canvas host panel ownership', () => {
  let root: Root;
  let container: HTMLDivElement;
  const stores = [useAgentCanvasStore, useProjectCanvasStore, useGitCanvasStore,
    usePanelViewCanvasStore, useBottomTerminalCanvasStore];

  beforeEach(() => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    clearAgentCanvasForPeerSwitch();
    stores.forEach(store => store.getState().reset());
    ['agent', 'project', 'git'].forEach(mode => drainPendingTabs(mode as 'agent' | 'project' | 'git'));
    appManager.updateLayout({
      chatCollapsed: false,
      rightPanelCollapsed: true,
      rightPanelWidth: 520,
      bottomTerminalPanelCollapsed: true,
    });
    container = document.createElement('div');
    root = createRoot(container);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    clearAgentCanvasForPeerSwitch();
    stores.forEach(store => store.getState().reset());
    container.remove();
  });

  it.each([true, false])('preserves a session panel with collapsed=%s through file-view lifetime', async collapsed => {
    appManager.updateLayout({ rightPanelCollapsed: collapsed });
    useAgentCanvasStore.getState().addTab(content('session'), 'active');
    useProjectCanvasStore.getState().addTab(content('restored-file'), 'active');
    const before = appManager.getState().layout;
    await act(async () => root.render(<Hosts />));
    expect(appManager.getState().layout).toEqual(before);

    let rightPanelRequests = 0;
    const onRightPanelRequest = () => { rightPanelRequests++; };
    window.addEventListener(TAB_EVENTS.EXPAND_RIGHT_PANEL, onRightPanelRequest);
    try {
      await act(async () => {
        const options = { filePath: '/workspace/example.ts', workspacePath: '/workspace',
          remoteConnectionId: 'ssh-workspace', mode: 'project' as const };
        fileTabManager.openFile(options);
        fileTabManager.openFile({ ...options, jumpToLine: 12 });
      });
      const tabs = useProjectCanvasStore.getState().primaryGroup.tabs;
      expect(tabs).toHaveLength(2);
      expect(tabs.find(tab => tab.content.data?.filePath === '/workspace/example.ts')?.content.data).toMatchObject({
        filePath: '/workspace/example.ts', remoteConnectionId: 'ssh-workspace', jumpToRange: { start: 12 },
      });
      expect(rightPanelRequests).toBe(0);
      await act(async () => useProjectCanvasStore.getState().closeAllTabs());
      await act(async () => root.render(<Hosts project={false} />));
      await act(async () => { await new Promise<void>(resolve => requestAnimationFrame(() => resolve())); });
      expect(appManager.getState().layout).toEqual(before);
      expect(useAgentCanvasStore.getState().primaryGroup.tabs).toHaveLength(1);
    } finally {
      window.removeEventListener(TAB_EVENTS.EXPAND_RIGHT_PANEL, onRightPanelRequest);
    }
  });

  it('drains the first file open into its target without expanding the session panel', async () => {
    await act(async () => root.render(<Hosts project={false} />));
    fileTabManager.openFile({ filePath: '/workspace/queued.ts', mode: 'project', sceneJustOpened: true });
    expect(useProjectCanvasStore.getState().primaryGroup.tabs).toHaveLength(0);
    await act(async () => root.render(<Hosts />));
    expect(useProjectCanvasStore.getState().primaryGroup.tabs).toHaveLength(1);
    expect(appManager.getState().layout.rightPanelCollapsed).toBe(true);
  });

  it('reveals a session file for new and duplicate requests without broadcasting to standalone canvases', async () => {
    await act(async () => root.render(<Hosts />));
    const options = { filePath: '/workspace/session.ts', mode: 'agent' as const };
    await act(async () => {
      fileTabManager.openFile(options);
      fileTabManager.openFile(options);
    });
    expect(appManager.getState().layout.rightPanelCollapsed).toBe(false);
    expect(useAgentCanvasStore.getState().primaryGroup.tabs).toHaveLength(1);
    expect(usePanelViewCanvasStore.getState().primaryGroup.tabs).toHaveLength(0);
    expect(useProjectCanvasStore.getState().primaryGroup.tabs).toHaveLength(0);

    collapseSessionAuxPane();
    await act(async () => fileTabManager.openFile(options));
    expect(appManager.getState().layout.rightPanelCollapsed).toBe(false);
    expect(useAgentCanvasStore.getState().primaryGroup.tabs).toHaveLength(1);
  });

  it('preserves manual collapse across content changes, workspace restore and host remount', async () => {
    switchAgentCanvasWorkspace(null, 'workspace-a');
    useAgentCanvasStore.getState().addTab(content('session'), 'active');
    await act(async () => root.render(<Hosts />));
    const tab = useAgentCanvasStore.getState().primaryGroup.tabs[0];
    await act(async () => useAgentCanvasStore.getState().updateTabContent(tab.id, 'primary', content('updated')));
    expect(appManager.getState().layout.rightPanelCollapsed).toBe(true);
    await act(async () => {
      switchAgentCanvasWorkspace('workspace-a', 'workspace-b');
      root.render(<Hosts />);
    });
    await act(async () => {
      switchAgentCanvasWorkspace('workspace-b', 'workspace-a');
      root.render(<Hosts />);
    });
    expect(useAgentCanvasStore.getState().primaryGroup.tabs).toHaveLength(1);
    expect(appManager.getState().layout.rightPanelCollapsed).toBe(true);
    await act(async () => root.render(null));
    await act(async () => root.render(<Hosts />));
    expect(appManager.getState().layout.rightPanelCollapsed).toBe(true);
  });

  it('keeps an explicitly opened empty panel and counts all editor groups when closing', async () => {
    appManager.updateLayout({ rightPanelCollapsed: false });
    await act(async () => root.render(<Hosts />));
    expect(appManager.getState().layout.rightPanelCollapsed).toBe(false);
    await act(async () => {
      const canvas = useAgentCanvasStore.getState();
      canvas.setSplitMode('grid');
      canvas.addTab(content('primary'), 'active', 'primary');
      canvas.addTab(content('tertiary'), 'active', 'tertiary');
    });
    await act(async () => useAgentCanvasStore.getState().closeAllTabs('primary'));
    expect(appManager.getState().layout.rightPanelCollapsed).toBe(false);
    // Closing a group can compact the remaining tabs into the primary group.
    await act(async () => useAgentCanvasStore.getState().closeAllTabs());
    expect(appManager.getState().layout.rightPanelCollapsed).toBe(true);
  });

  it('preserves an open panel when workspace snapshots change before the shell rerenders', async () => {
    switchAgentCanvasWorkspace(null, 'workspace-a');
    useAgentCanvasStore.getState().addTab(content('workspace-a'), 'active');
    appManager.updateLayout({ rightPanelCollapsed: false });
    await act(async () => root.render(<Hosts />));
    await act(async () => switchAgentCanvasWorkspace('workspace-a', 'workspace-b'));
    expect(useAgentCanvasStore.getState().workspaceKey).toBe('workspace-b');
    expect(useAgentCanvasStore.getState().primaryGroup.tabs).toHaveLength(0);
    expect(appManager.getState().layout.rightPanelCollapsed).toBe(false);
    await act(async () => switchAgentCanvasWorkspace('workspace-b', 'workspace-a'));
    expect(useAgentCanvasStore.getState().primaryGroup.tabs).toHaveLength(1);
    expect(appManager.getState().layout.rightPanelCollapsed).toBe(false);
  });

  it('keeps Git and bottom terminal operations scoped to their hosts', async () => {
    await act(async () => root.render(<Hosts />));
    await act(async () => {
      window.dispatchEvent(new CustomEvent(TAB_EVENTS.GIT_CREATE_TAB, { detail: content('git-diff') }));
      window.dispatchEvent(new CustomEvent(TAB_EVENTS.BOTTOM_TERMINAL_CREATE_TAB, {
        detail: { type: 'terminal', title: 'terminal', data: { sessionId: 'terminal-1' },
          metadata: { terminalCloseBehavior: 'detach' } },
      }));
    });
    expect(useGitCanvasStore.getState().primaryGroup.tabs).toHaveLength(1);
    expect(appManager.getState().layout).toMatchObject({
      rightPanelCollapsed: true, bottomTerminalPanelCollapsed: false,
    });
    await act(async () => useBottomTerminalCanvasStore.getState().closeAllTabs());
    expect(appManager.getState().layout).toMatchObject({
      rightPanelCollapsed: true, bottomTerminalPanelCollapsed: true,
    });
  });

  it('applies repeated reveal requests idempotently and leaves no deferred layout mutation after unmount', async () => {
    await act(async () => root.render(<Hosts />));
    await act(async () => {
      window.dispatchEvent(new CustomEvent(TAB_EVENTS.EXPAND_RIGHT_PANEL));
      window.dispatchEvent(new CustomEvent(TAB_EVENTS.EXPAND_RIGHT_PANEL));
      window.dispatchEvent(new CustomEvent(TAB_EVENTS.EXPAND_BOTTOM_TERMINAL_PANEL));
      window.dispatchEvent(new CustomEvent(TAB_EVENTS.EXPAND_BOTTOM_TERMINAL_PANEL));
    });
    expect(appManager.getState().layout).toMatchObject({
      rightPanelCollapsed: false, bottomTerminalPanelCollapsed: false,
    });
    await act(async () => root.render(null));
    collapseSessionAuxPane();
    collapseSessionBottomTerminalPane();
    window.dispatchEvent(new CustomEvent(TAB_EVENTS.EXPAND_RIGHT_PANEL));
    await act(async () => { await new Promise<void>(resolve => requestAnimationFrame(() => resolve())); });
    expect(appManager.getState().layout).toMatchObject({
      rightPanelCollapsed: true, bottomTerminalPanelCollapsed: true,
    });
  });
});
