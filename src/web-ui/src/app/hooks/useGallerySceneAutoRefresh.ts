import { useEffect, useRef } from 'react';
import type { OverlaySceneId } from '@/app/overlay/types';
import { useOverlayStore } from '@/app/stores/overlayStore';

export interface UseGallerySceneAutoRefreshOptions {
  /** Overlay scene id (e.g. 'skills', 'agents', 'miniapps'). */
  sceneId: OverlaySceneId;
  /** Reload lists; may be async. */
  refetch: () => void | Promise<void>;
  enabled?: boolean;
}

/**
 * Gallery overlay scenes are unmounted when not active (single overlay slot).
 * This hook refreshes data when:
 * 1. The overlay becomes active (user navigates to it).
 * 2. The window regains visibility while this overlay is active.
 *
 * Initial load remains the responsibility of each feature hook.
 */
export function useGallerySceneAutoRefresh({
  sceneId,
  refetch,
  enabled = true,
}: UseGallerySceneAutoRefreshOptions): void {
  const activeOverlay = useOverlayStore(s => s.activeOverlay);
  const isActive = activeOverlay === sceneId;
  const refetchRef = useRef(refetch);
  refetchRef.current = refetch;

  const wasActiveRef = useRef<boolean | null>(null);

  useEffect(() => {
    if (!enabled) return;
    if (wasActiveRef.current === null) {
      wasActiveRef.current = isActive;
      return;
    }
    if (isActive && !wasActiveRef.current) {
      void Promise.resolve(refetchRef.current());
    }
    wasActiveRef.current = isActive;
  }, [enabled, isActive]);

  useEffect(() => {
    if (!enabled) return;
    const onVisibility = () => {
      if (document.visibilityState !== 'visible') return;
      if (activeOverlay !== sceneId) return;
      void Promise.resolve(refetchRef.current());
    };

    document.addEventListener('visibilitychange', onVisibility);
    return () => document.removeEventListener('visibilitychange', onVisibility);
  }, [enabled, activeOverlay, sceneId]);
}
