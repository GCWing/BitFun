import { appManager } from '@/app/services/AppManager';
import { useSceneStore } from '@/app/stores/sceneStore';
import { flowChatStore } from '../store/FlowChatStore';
import { flowChatManager } from './FlowChatManager';
import { syncSessionToModernStore } from './storeSync';

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
