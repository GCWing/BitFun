/**
 * Shared FlowChat session summary for the floating chat surfaces.
 *
 * Both the floating window mode header and the floating chat bubble header need
 * the same three things — the active session's title, the recent session list,
 * and the active session itself to derive stream state from. Keeping that in one
 * hook means one store subscription per surface and one definition of "recent
 * sessions" instead of two copies drifting apart.
 */

import { useEffect, useMemo, useState } from 'react';
import { flowChatStore } from '../../store/FlowChatStore';
import type { FlowChatState, Session } from '../../types/flow-chat';
import { compareSessionsForDisplay } from '../../utils/sessionOrdering';
import { resolveSessionTitle } from '../../utils/sessionTitle';
import { i18nService } from '@/infrastructure/i18n';

/** How many recent sessions the session menu offers. */
export const RECENT_SESSION_LIMIT = 10;

export interface FlowChatSessionsSnapshot {
  activeSessionId: string | null;
  activeSession: Session | undefined;
  sessionTitle: string;
  sessions: Session[];
}

export function resolveDisplayTitle(session: Session | undefined): string {
  return resolveSessionTitle(session, (key, options) => i18nService.t(key, options));
}

export function useFlowChatSessions(): FlowChatSessionsSnapshot {
  const [state, setState] = useState<FlowChatState>(() => flowChatStore.getState());

  useEffect(() => {
    const unsubscribe = flowChatStore.subscribe(setState);
    return () => unsubscribe();
  }, []);

  const activeSessionId = state.activeSessionId ?? null;
  const activeSession = useMemo(
    () => (activeSessionId ? state.sessions.get(activeSessionId) : undefined),
    [state, activeSessionId]
  );

  const sessionTitle = useMemo(() => resolveDisplayTitle(activeSession), [activeSession]);

  const sessions = useMemo(
    () =>
      Array.from(state.sessions.values())
        .sort(compareSessionsForDisplay)
        .slice(0, RECENT_SESSION_LIMIT),
    [state]
  );

  return { activeSessionId, activeSession, sessionTitle, sessions };
}
