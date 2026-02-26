/**
 * useSceneManager — thin wrapper around the shared sceneStore.
 *
 * All consumers (SceneBar, SceneViewport, NavPanel, …) now read from and
 * write to the same Zustand store, so state is always in sync.
 */

import { SCENE_TAB_REGISTRY } from '../scenes/registry';
import type { SceneTabDef } from '../components/SceneBar/types';
import { useSceneStore } from '../stores/sceneStore';

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

  return {
    openTabs,
    activeTabId,
    tabDefs: SCENE_TAB_REGISTRY,
    activateScene,
    openScene,
    closeScene,
  };
}
