import { flowChatStore } from '../store/FlowChatStore';
import { flowChatManager } from './FlowChatManager';
import { openMainSession } from './openBtwSession';

/**
 * Focuses the latest Agentic OS (Dispatcher) session, or creates one if missing.
 * Mirrors the nav "Agentic OS" entry behavior.
 */
export async function openDispatcherSession(): Promise<void> {
  const storeState = flowChatStore.getState();
  const existing =
    Array.from(storeState.sessions.values())
      .filter((s) => s.mode === 'Dispatcher')
      .sort(
        (a, b) =>
          (b.lastActiveAt ?? b.createdAt ?? 0) - (a.lastActiveAt ?? a.createdAt ?? 0)
      )[0] ?? null;

  if (existing) {
    await openMainSession(existing.sessionId);
    return;
  }

  await flowChatManager.createChatSession({ storageScope: 'agentic_os' }, 'Dispatcher');
}
