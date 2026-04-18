/**
 * overlayStore — two-layer scene navigation state.
 *
 * Model:
 *   - activeOverlay === null  → Agentic OS base session is the visible layer
 *   - activeOverlay !== null  → an overlay scene covers the base session
 *
 * The base session (SceneId = 'session') is always mounted and never "closed".
 * Navigating between overlay scenes is instant (the previous overlay unmounts).
 *
 * Left-side nav sync:
 *   When an overlay with a registered scene-nav opens, navSceneStore is updated
 *   so the sidebar shows the appropriate context nav (e.g. Settings sidebar).
 *   Closing an overlay clears scene-specific sidebar nav state.
 */

import { create } from 'zustand';
import { getSceneNav } from '../scenes/nav-registry';
import { useNavSceneStore } from './navSceneStore';
import type { OverlaySceneId } from '../overlay/types';

/** Only overlays with a registered scene-nav sync the legacy NavPanel layer. */
function resolveNavSceneId(id: OverlaySceneId): OverlaySceneId | null {
  if (typeof id === 'string' && id.startsWith('live-app:')) return null;
  return getSceneNav(id) ? id : null;
}

interface OverlayState {
  /** Currently active overlay scene; null means the base session is fully visible. */
  activeOverlay: OverlaySceneId | null;

  /** Open (or switch to) an overlay scene. */
  openOverlay: (id: OverlaySceneId) => void;

  /** Close the active overlay and return to the base session. */
  closeOverlay: () => void;

  /**
   * goBack — semantic alias for closeOverlay.
   * Kept for backward compatibility; the NavBar back button has been removed.
   */
  goBack: () => void;
}

export const useOverlayStore = create<OverlayState>((set, get) => ({
  activeOverlay: null,

  openOverlay: (id) => {
    if (get().activeOverlay === id) {
      // Already active: re-sync left nav in case it drifted
      const navId = resolveNavSceneId(id);
      const navStore = useNavSceneStore.getState();
      if (navId && (!navStore.showSceneNav || navStore.navSceneId !== navId)) {
        navStore.openNavScene(navId);
      }
      return;
    }

    set({ activeOverlay: id });

    const navId = resolveNavSceneId(id);
    const navStore = useNavSceneStore.getState();
    if (navId) {
      navStore.openNavScene(navId);
    } else {
      navStore.closeNavScene();
    }
  },

  closeOverlay: () => {
    set({ activeOverlay: null });
    useNavSceneStore.getState().closeNavScene();
  },

  goBack: () => {
    get().closeOverlay();
  },
}));

/** Whether an overlay is currently open (i.e. a non-session scene is active). */
export function selectHasActiveOverlay(state: OverlayState): boolean {
  return state.activeOverlay !== null;
}
