/**
 * Active-session presentation projection.
 * FlowChatStore owns selection; ModernFlowChatStore only projects that selection.
 * Maintains original concept: Session → DialogTurn → ModelRound → FlowItem
 */

import { flowChatStore } from '../store/FlowChatStore';
import { useModernFlowChatStore } from '../store/modernFlowChatStore';
import type { Session } from '../types/flow-chat';

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
  // An async opener may finish after selection changed or the session was
  // removed. Presentation sync must never become another selection writer.
  if (flowChatStore.getState().activeSessionId !== sessionId) return;
  syncActiveSessionToModernStore();
}

function syncActiveSessionToModernStore(): void {
  const state = flowChatStore.getState();
  const session = state.activeSessionId ? state.sessions.get(state.activeSessionId) : undefined;
  const modernStore = useModernFlowChatStore.getState();
  if (!session) {
    // Empty is an authoritative projection too, including the first sync after
    // remount and a dangling selection. Do not depend on subscriber-local history.
    if (modernStore.activeSession || modernStore.virtualItems.length || modernStore.visibleTurnInfo) {
      modernStore.clear();
    }
    return;
  }

  if (!isSessionAlreadySynced(session.sessionId, session, modernStore)) {
    modernStore.setActiveSession(session);
  }
}

const syncConsumers = new Set<symbol>();
let unsubscribeSource: (() => void) | undefined;

/** Share one source subscription across the shell and standalone chat hosts. */
export function startAutoSync(): () => void {
  const consumer = Symbol();
  syncConsumers.add(consumer);
  unsubscribeSource ??= flowChatStore.subscribe(syncActiveSessionToModernStore);
  syncActiveSessionToModernStore();

  return () => {
    syncConsumers.delete(consumer);
    if (syncConsumers.size === 0) {
      unsubscribeSource?.();
      unsubscribeSource = undefined;
    }
  };
}
