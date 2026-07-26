import { useCallback, useSyncExternalStore } from 'react';
import type { RemoteSessionManager } from '../services/RemoteSessionManager';

/**
 * React subscription for the transport-owned control-target generation.
 * useSyncExternalStore also closes the render-to-subscribe missed-event window
 * and gives StrictMode setup/cleanup deterministic semantics.
 */
export function useControlTargetEpoch(sessionMgr: RemoteSessionManager): number {
  const subscribe = useCallback(
    (onStoreChange: () => void) => sessionMgr.onControlTargetChange(onStoreChange),
    [sessionMgr],
  );
  const getSnapshot = useCallback(
    () => sessionMgr.controlTargetEpoch,
    [sessionMgr],
  );

  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}
