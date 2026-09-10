import { useMemo, useSyncExternalStore } from 'react';
import { sessionNavStatusService } from '../services/sessionNavStatusService';
import type { SessionNavStatus } from '../utils/sessionNavStatus';

/** Rows subscribe only to their cached status, never directly to runtime sources. */
export function useSessionNavStatus(sessionId: string): SessionNavStatus {
  const source = useMemo(() => ({
    subscribe: (notify: () => void) => sessionNavStatusService.subscribe(sessionId, notify),
    getSnapshot: () => sessionNavStatusService.getSnapshot(sessionId),
  }), [sessionId]);
  return useSyncExternalStore(source.subscribe, source.getSnapshot, source.getSnapshot);
}
