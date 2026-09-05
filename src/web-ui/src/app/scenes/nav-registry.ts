/**
 * nav-registry — maps SceneTabId → lazy-loaded scene-specific NavPanel component.
 *
 * Extension pattern:
 *   1. Create `src/app/scenes/<scene>/XxxNav.tsx`
 *   2. Register its component, bootstrap title key, and preloader below
 *
 * Scenes without a registered nav component fall back to MainNav (the default sidebar).
 */

import { lazy } from 'react';
import type { ComponentType } from 'react';
import type { SceneTabId } from '../components/SceneBar/types';

type LazyNavComponent = ReturnType<typeof lazy<ComponentType>>;

interface SceneNavRegistration {
  component: LazyNavComponent;
  /** Resolved by NavBar through the bootstrap common/shared namespaces. */
  titleKey: string;
  preload: () => Promise<unknown>;
}

/**
 * The settings nav renders every label from the lazy `settings` namespace, so the
 * chunk and that namespace are loaded together — mounting before it resolves
 * paints raw i18n keys for a frame.
 */
const loadSettingsNav = async () => {
  const [navModule] = await Promise.all([
    import('./settings/SettingsNav'),
    import('./settings/settingsRegistry').then((module) => module.preloadSettingsShell()),
  ]);
  return navModule;
};
const loadFileViewerNav = () => import('./file-viewer/FileViewerNav');

const SCENE_NAV_REGISTRY: Partial<Record<SceneTabId, SceneNavRegistration>> = {
  settings: {
    component: lazy(loadSettingsNav),
    titleKey: 'shared:features.settings',
    preload: loadSettingsNav,
  },
  'file-viewer': {
    component: lazy(loadFileViewerNav),
    titleKey: 'nav.items.project',
    preload: loadFileViewerNav,
  },
};

/**
 * Returns the lazy nav component registered for the given scene,
 * or `null` if the scene uses the default MainNav.
 */
export function getSceneNav(sceneId: SceneTabId): LazyNavComponent | null {
  return SCENE_NAV_REGISTRY[sceneId]?.component ?? null;
}

export function getSceneNavTitleKey(sceneId: SceneTabId): string | null {
  return SCENE_NAV_REGISTRY[sceneId]?.titleKey ?? null;
}

export async function preloadSceneNav(sceneId: SceneTabId): Promise<void> {
  await SCENE_NAV_REGISTRY[sceneId]?.preload();
}
