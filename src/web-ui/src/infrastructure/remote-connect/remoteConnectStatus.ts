import { useSyncExternalStore } from 'react';
import { remoteConnectAPI, type RemoteConnectStatus } from '../api/service-api/RemoteConnectAPI';

export interface RemoteConnectStatusSnapshot {
  status: RemoteConnectStatus | null;
  state: 'loading' | 'ready' | 'unavailable';
}

/** One confirmed snapshot for the dialog and chrome, regardless of who refreshed it. */
export function createRemoteConnectStatusSource(readStatus: () => Promise<RemoteConnectStatus>) {
  let snapshot: RemoteConnectStatusSnapshot = { status: null, state: 'loading' };
  let readGeneration = 0;
  let inFlight: Promise<RemoteConnectStatus | null> | null = null;
  const listeners = new Set<() => void>();
  const publish = (next: RemoteConnectStatusSnapshot) => {
    snapshot = next;
    for (const listener of listeners) listener();
  };
  return {
    getSnapshot: () => snapshot,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
    invalidate() {
      readGeneration += 1;
      inFlight = null;
      publish({ status: null, state: 'loading' });
    },
    invalidateReads() {
      readGeneration += 1;
      inFlight = null;
    },
    refresh(): Promise<RemoteConnectStatus | null> {
      if (inFlight) return inFlight;
      const generation = readGeneration;
      const request = (async () => {
        try {
          const status = await readStatus();
          if (generation !== readGeneration) return null;
          publish({ status, state: 'ready' });
          return status;
        } catch (error) {
          if (generation === readGeneration) publish({ status: null, state: 'unavailable' });
          throw error;
        }
      })();
      inFlight = request;
      void request.finally(() => {
        if (inFlight === request) inFlight = null;
      }).catch(() => undefined);
      return request;
    },
  };
}

export const remoteConnectStatusSource = createRemoteConnectStatusSource(() => remoteConnectAPI.getStatus());

export function useRemoteConnectStatus() {
  return useSyncExternalStore(
    remoteConnectStatusSource.subscribe,
    remoteConnectStatusSource.getSnapshot,
    remoteConnectStatusSource.getSnapshot,
  );
}
