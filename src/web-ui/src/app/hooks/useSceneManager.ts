/**
 * useSceneManager — thin wrapper around the shared sceneStore.
 *
 * All consumers (SceneBar, SceneViewport, NavPanel, …) now read from and
 * write to the same Zustand store, so state is always in sync.
 */

import { SCENE_TAB_REGISTRY, getMiniAppSceneDef } from '../scenes/registry';
import type { SceneTabDef } from '../components/SceneBar/types';
import { useSceneStore } from '../stores/sceneStore';
import { useToolboxStore } from '../scenes/toolbox/toolboxStore';

export interface UseSceneManagerReturn {
  openTabs: ReturnType<typeof useSceneStore.getState>['openTabs'];
  activeTabId: ReturnType<typeof useSceneStore.getState>['activeTabId'];
  tabDefs: SceneTabDef[];
  activateScene: (id: string) => void;
  openScene: (id: string) => void;
  closeScene: (id: string) => void;
}

export function useSceneManager(): UseSceneManagerReturn {
  const { openTabs, activeTabId, activateScene, openScene, closeScene } = useSceneStore();
  const apps = useToolboxStore((s) => s.apps);

  const miniAppDefs: SceneTabDef[] = openTabs
    .filter((t) => typeof t.id === 'string' && t.id.startsWith('miniapp:'))
    .map((t) => {
      const appId = (t.id as string).slice('miniapp:'.length);
      const app = apps.find((a) => a.id === appId);
      return getMiniAppSceneDef(appId, app?.name);
    });

  return {
    openTabs,
    activeTabId,
    tabDefs: [...SCENE_TAB_REGISTRY, ...miniAppDefs],
    activateScene,
    openScene,
    closeScene,
  };
}
