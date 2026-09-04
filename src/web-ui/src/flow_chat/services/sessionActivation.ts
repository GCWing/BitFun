import { appManager } from '@/app/services/AppManager';
import { useSceneStore } from '@/app/stores/sceneStore';
import { flowChatStore } from '../store/FlowChatStore';
import { flowChatManager } from './FlowChatManager';
import { syncSessionToModernStore } from './storeSync';

/**
 * Leave the Session scene when archiving removed the session that was active
 * before the operation. FlowChat intentionally clears activeSessionId for this
 * transition; keeping the scene open would leave its composer without a
 * session state machine and render a disabled send button.
 */
export function closeSessionSceneAfterActiveSessionArchive(
  activeSessionIdBeforeArchive: string | null,
): boolean {
  if (!activeSessionIdBeforeArchive) {
    return false;
  }

  const flowChatState = flowChatStore.getState();
  if (
    flowChatState.activeSessionId !== null
    || flowChatState.sessions.has(activeSessionIdBeforeArchive)
  ) {
    return false;
  }

  const sceneState = useSceneStore.getState();
  if (!sceneState.openTabs.some(tab => tab.id === 'session')) {
    return false;
  }

  sceneState.closeScene('session');
  return true;
}

export async function openMainSession(
  sessionId: string,
  options?: {
    workspaceId?: string;
    activateWorkspace?: (workspaceId: string) => void | Promise<unknown>;
  }
): Promise<void> {
  if (options?.workspaceId && options.activateWorkspace) {
    await options.activateWorkspace(options.workspaceId);
  }

  appManager.updateLayout({
    leftPanelActiveTab: 'sessions',
    leftPanelCollapsed: false,
  });

  const activated = await activateMainSession(sessionId);
  if (!activated) {
    return;
  }

  useSceneStore.getState().openScene('session');
}

export async function activateMainSession(sessionId: string): Promise<boolean> {
  const isTargetActive = () => flowChatStore.getState().activeSessionId === sessionId;
  const targetSession = flowChatStore.getState().sessions.get(sessionId) ?? null;
  if (!targetSession) {
    return false;
  }

  if (isTargetActive()) {
    const activeSession = flowChatStore.getState().sessions.get(sessionId) ?? null;
    if (
      activeSession?.isHistorical &&
      (activeSession.historyState === 'metadata-only' || activeSession.historyState === 'failed')
    ) {
      await flowChatManager.switchChatSession(sessionId);
      if (!isTargetActive()) {
        return false;
      }
    }
    syncSessionToModernStore(sessionId);
  } else {
    await flowChatManager.switchChatSession(sessionId);
    if (!isTargetActive()) {
      return false;
    }
    syncSessionToModernStore(sessionId);
  }

  return true;
}
