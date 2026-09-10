// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from 'vitest';
import { useSceneStore } from '../../stores/sceneStore';
import { useNavSceneStore } from '../../stores/navSceneStore';
import { normalizeResourceLayout, useWorkspaceResourceState } from './workspaceResourceState';
import { getSessionSceneTabId } from '../../components/SceneBar/types';

const sessionTarget = { surfaceId: 'local', workspaceKey: 'project', sessionId: 'task' };

describe('workspace resource navigation', () => {
  beforeEach(() => {
    useSceneStore.getState().resetForPeerSwitch();
    useNavSceneStore.getState().closeNavScene();
  });
  it('opens resources without replacing the active conversation', () => {
    useSceneStore.getState().openSessionScene(sessionTarget);
    useNavSceneStore.getState().openNavScene('file-viewer');
    expect(useSceneStore.getState().activeTabId).toBe(getSessionSceneTabId(sessionTarget));
  });
  it('keeps resources during file, terminal and conversation navigation', () => {
    useNavSceneStore.getState().openNavScene('file-viewer');
    for (const scene of ['file-viewer', 'terminal', 'session'] as const) {
      if (scene === 'session') useSceneStore.getState().openSessionScene(sessionTarget);
      else useSceneStore.getState().openScene(scene);
      expect(useNavSceneStore.getState().showSceneNav).toBe(true);
      expect(useNavSceneStore.getState().navSceneId).toBe('file-viewer');
    }
  });
  it('respects going back to the main navigation', () => {
    useNavSceneStore.getState().openNavScene('file-viewer');
    useNavSceneStore.getState().goBack();
    useSceneStore.getState().openScene('terminal');
    expect(useNavSceneStore.getState().showSceneNav).toBe(false);
    useNavSceneStore.getState().goForward();
    expect(useNavSceneStore.getState().navSceneId).toBe('file-viewer');
  });
  it('lets settings own their navigation and clears resources on a device switch', () => {
    useNavSceneStore.getState().openNavScene('file-viewer');
    useSceneStore.getState().openScene('settings');
    expect(useNavSceneStore.getState().navSceneId).toBe('settings');
    useSceneStore.getState().resetForPeerSwitch();
    expect(useNavSceneStore.getState().showSceneNav).toBe(false);
  });
});

describe('workspace resource preferences', () => {
  it('isolates layouts by complete resource key', () => {
    const state = useWorkspaceResourceState.getState();
    state.updateLayout('local/project', { terminalFraction: 0.5, filesCollapsed: true });
    state.updateLayout('peer/project', { terminalsCollapsed: true });
    const layouts = useWorkspaceResourceState.getState().layouts;
    expect(layouts['local/project'].filesCollapsed).toBe(true);
    expect(layouts['peer/project'].filesCollapsed).toBe(false);
    expect(layouts['peer/project'].terminalFraction).toBe(0.3);
  });
  it('tolerates missing and invalid persisted preferences', () => {
    expect(normalizeResourceLayout({}).terminalFraction).toBe(0.3);
    expect(normalizeResourceLayout({ terminalFraction: Number.NaN }).terminalFraction).toBe(0.3);
    expect(normalizeResourceLayout({ terminalFraction: 10 }).terminalFraction).toBe(0.75);
    expect(normalizeResourceLayout({ terminalFraction: -1 }).terminalFraction).toBe(0.15);
  });
});
