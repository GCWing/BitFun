import { useLayoutEffect } from 'react';
import { flowChatStore } from '../store/FlowChatStore';
import {
  reconcileSubagentIdentitiesFromFlowState,
  useSubagentIdentityStore,
} from './store';

export function useSubagentIdentity(sessionId?: string | null) {
  const identity = useSubagentIdentityStore(state =>
    sessionId ? state.assignments[sessionId] : undefined
  );

  useLayoutEffect(() => {
    if (!sessionId) return;
    reconcileSubagentIdentitiesFromFlowState(flowChatStore.getState(), sessionId);
  }, [sessionId]);

  return identity;
}
