/**
 * useSceneManager — compatibility wrapper around useOverlayManager.
 *
 * @deprecated Use useOverlayManager directly.
 * This shim translates the old tab-based API surface to the new overlay model:
 *   - openScene('session') → closeOverlay()
 *   - openScene(overlayId) → openOverlay(overlayId)
 *   - activeTabId          → activeOverlay ?? 'session'
 */

import { useOverlayStore } from '../stores/overlayStore';
import type { OverlaySceneId } from '../overlay/types';

export interface UseSceneManagerReturn {
  activeTabId: string;
  openScene: (id: string) => void;
  closeScene: (id: string) => void;
  activateScene: (id: string) => void;
}

export function useSceneManager(): UseSceneManagerReturn {
  const { activeOverlay, openOverlay, closeOverlay } = useOverlayStore();

  const openScene = (id: string) => {
    if (id === 'session' || id === 'welcome') {
      closeOverlay();
    } else {
      openOverlay(id as OverlaySceneId);
    }
  };

  return {
    activeTabId: activeOverlay ?? 'session',
    openScene,
    closeScene: (id) => {
      if (activeOverlay === id) closeOverlay();
    },
    activateScene: openScene,
  };
}
