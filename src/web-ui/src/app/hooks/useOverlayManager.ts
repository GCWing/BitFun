/**
 * useOverlayManager — primary hook for overlay scene navigation.
 *
 * Wraps overlayStore to expose a stable API for components that
 * open, close, or query the active overlay scene.
 */

import { useOverlayStore } from '../stores/overlayStore';
import { OVERLAY_SCENE_REGISTRY, getOverlayDef } from '../overlay/overlayRegistry';
import type { OverlaySceneId, OverlaySceneDef } from '../overlay/types';

export interface UseOverlayManagerReturn {
  activeOverlay: OverlaySceneId | null;
  overlayDefs: OverlaySceneDef[];
  openOverlay: (id: OverlaySceneId) => void;
  closeOverlay: () => void;
  /** Whether a given overlay scene is the currently active one. */
  isOverlayActive: (id: OverlaySceneId) => boolean;
  /** Convenience: open or close based on current state (toggle). */
  toggleOverlay: (id: OverlaySceneId) => void;
}

export function useOverlayManager(): UseOverlayManagerReturn {
  const { activeOverlay, openOverlay, closeOverlay } = useOverlayStore();

  return {
    activeOverlay,
    overlayDefs: OVERLAY_SCENE_REGISTRY,
    openOverlay,
    closeOverlay,
    isOverlayActive: (id) => activeOverlay === id,
    toggleOverlay: (id) => {
      if (activeOverlay === id) {
        closeOverlay();
      } else {
        openOverlay(id);
      }
    },
  };
}

export { getOverlayDef };
