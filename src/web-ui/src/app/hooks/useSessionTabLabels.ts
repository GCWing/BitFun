import { useMemo, useSyncExternalStore } from 'react';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import type { SceneTab } from '../components/SceneBar/types';
import { getActiveSurfaceId } from '@/infrastructure/peer-device/deviceSurface';

function readSessionTabLabels(tabs: readonly SceneTab[]): Record<string, string> {
  const source = flowChatStore.getState();
  const labels: Record<string, string> = {};
  for (const tab of tabs) {
    if (!tab.session || tab.session.surfaceId !== getActiveSurfaceId()) continue;
    const session = source.sessions.get(tab.session.sessionId);
    if (!session) continue;
    labels[tab.id] = session.title ?? '';
  }
  return labels;
}

function subscribeToLabels(notify: () => void): () => void {
  return flowChatStore.subscribe(notify);
}

/** A synchronous resource projection with stable snapshots during streaming. */
export function useSessionTabLabels(tabs: readonly SceneTab[]): Record<string, string> {
  const getSnapshot = useMemo(() => {
    let snapshot: Record<string, string> = {};
    return () => {
      const next = readSessionTabLabels(tabs);
      if (Object.keys(snapshot).length !== Object.keys(next).length
        || Object.entries(next).some(([id, label]) => snapshot[id] !== label)) {
        snapshot = next;
      }
      return snapshot;
    };
  }, [tabs]);
  return useSyncExternalStore(subscribeToLabels, getSnapshot, getSnapshot);
}
