import { useEffect, useSyncExternalStore } from 'react';
import {
  DEFAULT_MOUSE_GLOW_ENABLED,
  mouseGlowService,
} from '../core/MouseGlowService';

export function useMouseGlowPreference() {
  useEffect(() => {
    mouseGlowService.initialize();
  }, []);

  const enabled = useSyncExternalStore(
    mouseGlowService.subscribe,
    mouseGlowService.getEnabled,
    () => DEFAULT_MOUSE_GLOW_ENABLED,
  );

  return {
    enabled,
    setEnabled: mouseGlowService.setEnabled,
  };
}
