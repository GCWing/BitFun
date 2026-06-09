import { useSyncExternalStore } from 'react';
import { flowChatStore } from '../store/FlowChatStore';

export function useSessionGoalModeActive(sessionId: string | undefined): boolean {
  return useSyncExternalStore(
    (callback) => flowChatStore.subscribe(() => callback()),
    () => (
      sessionId
        ? Boolean(flowChatStore.getState().sessions.get(sessionId)?.goalModeActive)
        : false
    ),
    () => false,
  );
}
