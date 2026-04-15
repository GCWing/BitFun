import { i18nService } from '@/infrastructure/i18n';
import { appManager } from '@/app/services/AppManager';
import { useOverlayStore } from '@/app/stores/overlayStore';
import { createTab } from '@/shared/utils/tabUtils';
import type { PanelContent } from '@/app/components/panels/base/types';
import { useAgentCanvasStore } from '@/app/components/panels/content-canvas/stores';
import type { CanvasTab } from '@/app/components/panels/content-canvas/types';
import { flowChatStore } from '../store/FlowChatStore';
import { flowChatManager } from './FlowChatManager';
import { syncSessionToModernStore } from './storeSync';

export const BTW_SESSION_PANEL_TYPE = 'btw-session' as const;

export interface BtwSessionPanelData {
  childSessionId: string;
  parentSessionId: string;
  workspacePath?: string;
}

export interface BtwSessionPanelMetadata {
  duplicateCheckKey: string;
  childSessionId: string;
  parentSessionId: string;
  contentRole: 'btw-session';
}

type AgentCanvasState = ReturnType<typeof useAgentCanvasStore.getState>;

const getBtwSessionDuplicateKey = (childSessionId: string) => `btw-session-${childSessionId}`;

const resolveBtwSessionTitle = (childSessionId: string): string => {
  const session = flowChatStore.getState().sessions.get(childSessionId);
  const title = session?.title?.trim();
  if (title) return title;
  return i18nService.t('flow-chat:btw.threadLabel', { defaultValue: 'Side thread' });
};

export const isBtwSessionPanelContent = (content: PanelContent | null | undefined): boolean =>
  content?.type === BTW_SESSION_PANEL_TYPE;

export const buildBtwSessionPanelContent = (
  childSessionId: string,
  parentSessionId: string,
  workspacePath?: string
): PanelContent => ({
  type: BTW_SESSION_PANEL_TYPE,
  title: resolveBtwSessionTitle(childSessionId),
  data: {
    childSessionId,
    parentSessionId,
    workspacePath,
  } satisfies BtwSessionPanelData,
  metadata: {
    duplicateCheckKey: getBtwSessionDuplicateKey(childSessionId),
    childSessionId,
    parentSessionId,
    contentRole: 'btw-session',
  } satisfies BtwSessionPanelMetadata,
});

export const selectActiveAgentTab = (state: AgentCanvasState) => {
  const activeGroup = state.activeGroupId === 'primary'
    ? state.primaryGroup
    : state.activeGroupId === 'secondary'
      ? state.secondaryGroup
      : state.tertiaryGroup;
  const activeTabId = activeGroup.activeTabId;
  if (!activeTabId) return null;
  return activeGroup.tabs.find(tab => tab.id === activeTabId && !tab.isHidden) ?? null;
};

export const selectActiveBtwSessionTab = (state: AgentCanvasState): CanvasTab | null => {
  const activeTab = selectActiveAgentTab(state);
  if (!activeTab || !isBtwSessionPanelContent(activeTab.content)) {
    return null;
  }

  const data = activeTab.content.data as BtwSessionPanelData | undefined;
  if (!data?.childSessionId || !data.parentSessionId) {
    return null;
  }

  return activeTab;
};

export async function openMainSession(
  sessionId: string,
  options?: {
    workspaceId?: string;
    activateWorkspace?: (workspaceId: string) => void | Promise<unknown>;
  }
): Promise<void> {
  appManager.updateLayout({
    leftPanelActiveTab: 'sessions',
  });

  if (options?.workspaceId && options.activateWorkspace) {
    await options.activateWorkspace(options.workspaceId);
  }

  if (flowChatStore.getState().activeSessionId === sessionId) {
    syncSessionToModernStore(sessionId);
  } else {
    await flowChatManager.switchChatSession(sessionId);
    syncSessionToModernStore(sessionId);
  }

  useOverlayStore.getState().closeOverlay();
}

export function openBtwSessionInAuxPane(params: {
  childSessionId: string;
  parentSessionId: string;
  workspacePath?: string;
  expand?: boolean;
}): void {
  const content = buildBtwSessionPanelContent(
    params.childSessionId,
    params.parentSessionId,
    params.workspacePath
  );

  if (params.expand !== false) {
    window.dispatchEvent(new CustomEvent('expand-right-panel'));
  }

  createTab({
    type: content.type,
    title: content.title,
    data: content.data,
    metadata: content.metadata,
    checkDuplicate: true,
    duplicateCheckKey: content.metadata?.duplicateCheckKey,
    replaceExisting: false,
    mode: 'agent',
  });
}
