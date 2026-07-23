/**
 * Store sync service
 * Syncs data from old FlowChatStore to new ModernFlowChatStore
 * Maintains original concept: Session → DialogTurn → ModelRound → FlowItem
 */

import { flowChatStore } from '../store/FlowChatStore';
import { useModernFlowChatStore } from '../store/modernFlowChatStore';
import type { Session } from '../types/flow-chat';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('StoreSync');

function isSessionAlreadySynced(
  sessionId: string,
  session: Session,
  modernStore: ReturnType<typeof useModernFlowChatStore.getState>
): boolean {
  if (
    modernStore.activeSession?.sessionId !== sessionId ||
    modernStore.activeSession !== session
  ) {
    return false;
  }

  if (session.historyState === 'ready' && hasRenderableContent(session) && modernStore.virtualItems.length === 0) {
    return false;
  }

  return true;
}

function hasRenderableContent(session: Session): boolean {
  return session.dialogTurns.some(turn =>
    Boolean(turn.userMessage) ||
    (turn.status === 'image_analyzing' && turn.modelRounds.length === 0) ||
    turn.modelRounds.some(round => round.items.length > 0)
  );
}

/**
 * Sync session data to new Store
 */
export function syncSessionToModernStore(sessionId: string): void {
  const oldState = flowChatStore.getState();
  const session = oldState.sessions.get(sessionId);

  if (!session) {
    log.warn('Session not found', { sessionId });
    return;
  }

  const modernStore = useModernFlowChatStore.getState();
  if (isSessionAlreadySynced(sessionId, session, modernStore)) {
    return;
  }
  modernStore.setActiveSession(session);
}

/**
 * Start auto sync
 * Listens to old Store changes and automatically syncs to new Store
 *
 * Performance optimization: relies on FlowChatStore's immutable updates, each update creates a new session reference.
 * Uses reference comparison to skip redundant syncs — if the active session object hasn't changed, no work is done.
 */
export function startAutoSync(): () => void {
  let lastSyncedSessionId: string | null = null;
  let lastSyncedSession: object | null = null;

  const unsubscribe = flowChatStore.subscribe((state) => {
    const modernStore = useModernFlowChatStore.getState();

    if (state.activeSessionId) {
      const session = state.sessions.get(state.activeSessionId);
      if (
        session &&
        (
          session !== lastSyncedSession ||
          state.activeSessionId !== lastSyncedSessionId ||
          !isSessionAlreadySynced(state.activeSessionId, session, modernStore)
        )
      ) {
        lastSyncedSessionId = state.activeSessionId;
        lastSyncedSession = session;
        modernStore.setActiveSession(session);
      }
    } else if (lastSyncedSessionId !== null) {
      lastSyncedSessionId = null;
      lastSyncedSession = null;
      modernStore.clear();
    }
  });

  const currentState = flowChatStore.getState();
  if (currentState.activeSessionId) {
    const session = currentState.sessions.get(currentState.activeSessionId);
    if (session) {
      lastSyncedSessionId = currentState.activeSessionId;
      lastSyncedSession = session;
      const modernStore = useModernFlowChatStore.getState();
      if (!isSessionAlreadySynced(currentState.activeSessionId, session, modernStore)) {
        modernStore.setActiveSession(session);
      }
    }
  }

  return unsubscribe;
}
