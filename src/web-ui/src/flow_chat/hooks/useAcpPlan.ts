import { useEffect, useRef, useState } from 'react';

import type { AcpPlanEntry } from '@/infrastructure/api/service-api/ACPClientAPI';
import { ACPClientAPI } from '@/infrastructure/api/service-api/ACPClientAPI';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';

export function useAcpPlan(sessionId: string | null): { entries: AcpPlanEntry[] } {
  const [entries, setEntries] = useState<AcpPlanEntry[]>([]);
  const latestTurnIdRef = useRef<string | null>(null);

  useEffect(() => {
    latestTurnIdRef.current = null;
    setEntries([]);
  }, [sessionId]);

  useEffect(() => {
    if (!sessionId) return;

    return ACPClientAPI.onPlanUpdated((event) => {
      if (event.sessionId !== sessionId) return;
      latestTurnIdRef.current = event.turnId;
      setEntries(event.entries);
    });
  }, [sessionId]);

  useEffect(() => {
    if (!sessionId) return;

    const maybeClear = (event: { sessionId?: string; turnId?: string }) => {
      if (event.sessionId !== sessionId) return;
      const latestTurnId = latestTurnIdRef.current;
      if (!latestTurnId || event.turnId === latestTurnId) {
        latestTurnIdRef.current = null;
        setEntries([]);
      }
    };

    const unlistenCompleted = agentAPI.onDialogTurnCompleted(maybeClear);
    const unlistenCancelled = agentAPI.onDialogTurnCancelled(maybeClear);
    const unlistenFailed = agentAPI.onDialogTurnFailed(maybeClear);

    return () => {
      unlistenCompleted();
      unlistenCancelled();
      unlistenFailed();
    };
  }, [sessionId]);

  return { entries };
}
