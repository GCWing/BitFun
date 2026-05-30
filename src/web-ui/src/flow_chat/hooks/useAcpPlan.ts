import { useEffect, useState } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import type {
  AcpPlanEntry,
  AcpPlanUpdatedEvent,
} from '@/infrastructure/api/service-api/ACPClientAPI';

const PLAN_UPDATED_EVENT = 'agentic://acp-plan-updated';

/**
 * Track the execution plan an ACP agent publishes for the given session.
 *
 * The agent sends the full plan on each update (it replaces the prior one), so
 * the latest event wins. Returns an empty list for non-ACP sessions or before
 * the agent publishes a plan. Resets when the session changes.
 */
export function useAcpPlan(sessionId: string | null): { entries: AcpPlanEntry[] } {
  const [entries, setEntries] = useState<AcpPlanEntry[]>([]);

  useEffect(() => {
    setEntries([]);
  }, [sessionId]);

  useEffect(() => {
    if (!sessionId) return;
    let active = true;
    let unlisten: UnlistenFn | undefined;
    listen<AcpPlanUpdatedEvent>(PLAN_UPDATED_EVENT, (event) => {
      if (event.payload.sessionId === sessionId) {
        setEntries(event.payload.entries);
      }
    }).then((fn) => {
      if (active) unlisten = fn;
      else fn();
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [sessionId]);

  return { entries };
}
