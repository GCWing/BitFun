/**
 * nav-registry — maps SceneTabId → lazy-loaded scene-specific NavPanel component.
 *
 * Extension pattern:
 *   1. Create `src/app/scenes/<scene>/XxxNav.tsx`
 *   2. Add one entry to SCENE_NAV_REGISTRY below
 *
 * Scenes without a registered nav component fall back to MainNav (the default sidebar).
 */

import { lazy } from 'react';
import type { ComponentType } from 'react';
import type { SceneTabId } from '../components/SceneBar/types';

type LazyNavComponent = ReturnType<typeof lazy<ComponentType>>;

const SCENE_NAV_REGISTRY: Partial<Record<SceneTabId, LazyNavComponent>> = {
  settings: lazy(() => import('./settings/SettingsNav')),
  'file-viewer': lazy(() => import('./file-viewer/FileViewerNav')),
  // terminal: lazy(() => import('./terminal/TerminalNav')),
};

/**
 * Returns the lazy nav component registered for the given scene,
 * or `null` if the scene uses the default MainNav.
 */
export function getSceneNav(sceneId: SceneTabId): LazyNavComponent | null {
  return SCENE_NAV_REGISTRY[sceneId] ?? null;
}
