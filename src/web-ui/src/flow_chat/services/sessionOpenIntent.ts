import type { Session } from '../types/flow-chat';

export const HISTORY_SESSION_OPEN_INTENT_EVENT = 'flowchat:history-session-open-intent';
const RECENT_HISTORY_OPEN_INTENT_MS = 750;
const HISTORY_SESSION_OPEN_TRANSITION_MAX_MS = 4_000;

let recentHistoryOpenIntent: { sessionId: string; atMs: number } | null = null;
let activeHistorySessionOpenTransition: { sessionId: string; atMs: number } | null = null;
let activeHistorySessionOpenTransitionTimer: ReturnType<typeof setTimeout> | null = null;
const transitionListeners = new Set<() => void>();

export interface HistorySessionOpenIntentDetail {
  sessionId: string;
  sessionTitle?: string;
}

export interface HistorySessionOpenTransitionSnapshot {
  sessionId: string;
  atMs: number;
}

const nowMs = (): number => (
  typeof performance !== 'undefined' ? performance.now() : Date.now()
);

function notifyHistorySessionOpenTransitionListeners(): void {
  for (const listener of transitionListeners) {
    listener();
  }
}

function clearHistorySessionOpenTransitionTimer(): void {
  if (activeHistorySessionOpenTransitionTimer !== null) {
    clearTimeout(activeHistorySessionOpenTransitionTimer);
    activeHistorySessionOpenTransitionTimer = null;
  }
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
  const atMs = nowMs();
  recentHistoryOpenIntent = {
    sessionId,
    atMs,
  };
  activeHistorySessionOpenTransition = { sessionId, atMs };
  clearHistorySessionOpenTransitionTimer();
  activeHistorySessionOpenTransitionTimer = setTimeout(() => {
    if (
      activeHistorySessionOpenTransition?.sessionId === sessionId &&
      activeHistorySessionOpenTransition.atMs === atMs
    ) {
      activeHistorySessionOpenTransition = null;
      activeHistorySessionOpenTransitionTimer = null;
      notifyHistorySessionOpenTransitionListeners();
    }
  }, HISTORY_SESSION_OPEN_TRANSITION_MAX_MS);
  notifyHistorySessionOpenTransitionListeners();

  if (typeof window === 'undefined') {
    return;
  }

  window.dispatchEvent(new CustomEvent<HistorySessionOpenIntentDetail>(
    HISTORY_SESSION_OPEN_INTENT_EVENT,
    { detail: { sessionId, sessionTitle } },
  ));
}

export function consumeRecentHistorySessionOpenIntent(sessionId: string): boolean {
  const recent = recentHistoryOpenIntent;
  if (!recent || recent.sessionId !== sessionId) {
    return false;
  }

  const now = nowMs();
  recentHistoryOpenIntent = null;
  return now - recent.atMs <= RECENT_HISTORY_OPEN_INTENT_MS;
}

export function clearRecentHistorySessionOpenIntent(sessionId?: string): void {
  if (!sessionId || recentHistoryOpenIntent?.sessionId === sessionId) {
    recentHistoryOpenIntent = null;
  }
}

export function getHistorySessionOpenTransitionSnapshot(): HistorySessionOpenTransitionSnapshot | null {
  const transition = activeHistorySessionOpenTransition;
  if (!transition) {
    return null;
  }

  if (nowMs() - transition.atMs > HISTORY_SESSION_OPEN_TRANSITION_MAX_MS) {
    return null;
  }

  return transition;
}

export function clearHistorySessionOpenTransition(sessionId?: string): void {
  if (!sessionId || activeHistorySessionOpenTransition?.sessionId === sessionId) {
    activeHistorySessionOpenTransition = null;
    clearHistorySessionOpenTransitionTimer();
    notifyHistorySessionOpenTransitionListeners();
  }
}

export function subscribeHistorySessionOpenTransition(listener: () => void): () => void {
  transitionListeners.add(listener);
  return () => {
    transitionListeners.delete(listener);
  };
}
