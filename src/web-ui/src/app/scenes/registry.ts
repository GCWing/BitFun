/**
 * Scene registry — re-exports from the overlay system.
 *
 * The canonical scene definitions now live in app/overlay/overlayRegistry.ts.
 * This file exists for backwards-compat imports that reference app/scenes/registry.
 */

export { OVERLAY_SCENE_REGISTRY as SCENE_TAB_REGISTRY, getOverlayDef as getSceneDef } from '../overlay/overlayRegistry';
export type { OverlaySceneDef as SceneTabDef } from '../overlay/types';
