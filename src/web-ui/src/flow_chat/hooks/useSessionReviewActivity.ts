import { useEffect, useState } from 'react';
import { stateMachineManager } from '../state-machine';
import { flowChatStore } from '../store/FlowChatStore';
import {
  deriveSessionReviewActivity,
  type SessionReviewActivity,
} from '../utils/sessionReviewActivity';

function resolveCurrentActivity(parentSessionId?: string | null): SessionReviewActivity | null {
  return deriveSessionReviewActivity(
    flowChatStore.getState(),
    parentSessionId,
    sessionId => stateMachineManager.getCurrentState(sessionId),
  );
}

export function useSessionReviewActivity(
  parentSessionId?: string | null,
): SessionReviewActivity | null {
  const [activity, setActivity] = useState<SessionReviewActivity | null>(() =>
    resolveCurrentActivity(parentSessionId),
  );

  useEffect(() => {
    const sync = () => {
      setActivity(resolveCurrentActivity(parentSessionId));
    };

    sync();
    const unsubscribeStore = flowChatStore.subscribe(sync);
    const unsubscribeMachine = stateMachineManager.subscribeGlobal(sync);
    return () => {
      unsubscribeStore();
      unsubscribeMachine();
    };
  }, [parentSessionId]);

  return activity;
}
