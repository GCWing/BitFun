import type { Session } from '../types/flow-chat';

export const HISTORY_SESSION_OPEN_INTENT_EVENT = 'flowchat:history-session-open-intent';

export interface HistorySessionOpenIntentDetail {
  sessionId: string;
  sessionTitle?: string;
}

export function shouldShowHistorySessionOpenIntent(session: Session | null | undefined): boolean {
  if (!session) {
    return false;
  }

  if (
    session.isHistorical ||
    session.historyState === 'metadata-only' ||
    session.historyState === 'hydrating' ||
    session.historyState === 'failed'
  ) {
    return true;
  }

  return session.historyState === 'ready' && session.contextRestoreState === 'pending';
}

export function dispatchHistorySessionOpenIntent(sessionId: string, sessionTitle?: string): void {
  if (typeof window === 'undefined') {
    return;
  }

  window.dispatchEvent(new CustomEvent<HistorySessionOpenIntentDetail>(
    HISTORY_SESSION_OPEN_INTENT_EVENT,
    { detail: { sessionId, sessionTitle } },
  ));
}
