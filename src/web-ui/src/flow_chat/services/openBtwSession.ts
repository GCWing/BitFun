import { i18nService } from '@/infrastructure/i18n';
import { appManager } from '@/app/services/AppManager';
import { useSceneStore } from '@/app/stores/sceneStore';
import { createTab } from '@/shared/utils/tabUtils';
import type { PanelContent } from '@/app/components/panels/base/types';
import { useAgentCanvasStore } from '@/app/components/panels/content-canvas/stores';
import type { CanvasTab } from '@/app/components/panels/content-canvas/types';
import { flowChatStore } from '../store/FlowChatStore';
import { flowChatManager } from './FlowChatManager';
import { syncSessionToModernStore } from './storeSync';
import { resolveSessionTitle } from '../utils/sessionTitle';

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

export interface EnsureBtwSessionAvailableParams {
  childSessionId: string;
  parentSessionId: string;
  workspacePath?: string;
  sessionKind?: 'btw' | 'review' | 'deep_review' | 'miniapp' | 'subagent';
  sessionTitle?: string;
  agentType?: string;
  parentToolCallId?: string;
  subagentType?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
  includeInternal?: boolean;
}

type AgentCanvasState = ReturnType<typeof useAgentCanvasStore.getState>;

export const getBtwSessionDuplicateKey = (childSessionId: string) => `btw-session-${childSessionId}`;

const resolveBtwSessionTitle = (childSessionId: string): string => {
  const session = flowChatStore.getState().sessions.get(childSessionId);
  const title = session
    ? resolveSessionTitle(session, (key, options) => i18nService.t(key, options))
    : undefined;
  if (title) return title;
  return i18nService.t('flow-chat:btw.threadLabel');
};

const scheduleFrame = (callback: FrameRequestCallback): void => {
  if (typeof globalThis.requestAnimationFrame === 'function') {
    globalThis.requestAnimationFrame(callback);
    return;
  }
  setTimeout(() => callback(Date.now()), 0);
};

const clearSessionUnreadCompletionAfterRender = (sessionId: string): void => {
  scheduleFrame(() => {
    scheduleFrame(() => {
      flowChatStore.clearSessionUnreadCompletion(sessionId);
    });
  });
};

export const isBtwSessionPanelContent = (content: PanelContent | null | undefined): boolean =>
  content?.type === BTW_SESSION_PANEL_TYPE;

const isRightPanelCollapsed = (): boolean => {
  try {
    if (typeof window === 'undefined') {
      return false;
    }
    const layoutState = (window as unknown as {
      __BITFUN_LAYOUT_STATE__?: { rightPanelCollapsed?: boolean };
    }).__BITFUN_LAYOUT_STATE__;
    return layoutState?.rightPanelCollapsed ?? false;
  } catch {
    return false;
  }
};

const requestRightPanelExpansion = (): void => {
  if (typeof window !== 'undefined') {
    window.dispatchEvent(new window.CustomEvent('expand-right-panel'));
  }
};

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

export function ensureBtwSessionAvailable(params: EnsureBtwSessionAvailableParams): void {
  const existingSession = flowChatStore.getState().sessions.get(params.childSessionId);
  const parentSession = flowChatStore.getState().sessions.get(params.parentSessionId);
  const resolvedWorkspacePath = params.workspacePath || parentSession?.workspacePath;
  const resolvedRemoteConnectionId =
    params.remoteConnectionId || existingSession?.remoteConnectionId || parentSession?.remoteConnectionId;
  const resolvedRemoteSshHost =
    params.remoteSshHost || existingSession?.remoteSshHost || parentSession?.remoteSshHost;

  if (
    existingSession &&
    (params.sessionKind === 'subagent' || existingSession.sessionKind === 'subagent')
  ) {
    flowChatStore.updateSessionRelationship(params.childSessionId, {
      parentSessionId: params.parentSessionId,
      sessionKind: params.sessionKind || existingSession.sessionKind,
      parentToolCallId: params.parentToolCallId,
      subagentType: params.subagentType,
    });
  }

  if (!existingSession) {
    flowChatStore.addExternalSession(
      params.childSessionId,
      params.sessionTitle || resolveBtwSessionTitle(params.childSessionId),
      params.agentType || parentSession?.mode || 'agentic',
      resolvedWorkspacePath,
      {
        parentSessionId: params.parentSessionId,
        sessionKind: params.sessionKind || 'btw',
        parentToolCallId: params.parentToolCallId,
        subagentType: params.subagentType,
      },
      resolvedRemoteConnectionId,
      resolvedRemoteSshHost,
    );
  }

  const sessionToHydrate = flowChatStore.getState().sessions.get(params.childSessionId);
  const shouldHydrate =
    !existingSession ||
    Boolean(
      sessionToHydrate?.isHistorical &&
      (sessionToHydrate.historyState === 'metadata-only' || sessionToHydrate.historyState === 'failed')
    );

  const workspacePath = resolvedWorkspacePath || sessionToHydrate?.workspacePath;
  if (!shouldHydrate || !workspacePath) {
    return;
  }

  void flowChatStore.loadSessionHistory(
    params.childSessionId,
    workspacePath,
    undefined,
    resolvedRemoteConnectionId,
    resolvedRemoteSshHost,
    { includeInternal: params.includeInternal },
  );
}

export async function openMainSession(
  sessionId: string,
  options?: {
    workspaceId?: string;
    activateWorkspace?: (workspaceId: string) => void | Promise<unknown>;
  }
): Promise<void> {
  appManager.updateLayout({
    leftPanelActiveTab: 'sessions',
    leftPanelCollapsed: false,
  });

  if (options?.workspaceId && options.activateWorkspace) {
    await options.activateWorkspace(options.workspaceId);
  }

  const isTargetActive = () => flowChatStore.getState().activeSessionId === sessionId;

  if (isTargetActive()) {
    syncSessionToModernStore(sessionId);
  } else {
    await flowChatManager.switchChatSession(sessionId);
    if (!isTargetActive()) {
      return;
    }
    syncSessionToModernStore(sessionId);
  }

  useSceneStore.getState().openScene('session');
}

export function openBtwSessionInAuxPane(params: {
  childSessionId: string;
  parentSessionId: string;
  workspacePath?: string;
  expand?: boolean;
  sessionKind?: 'btw' | 'review' | 'deep_review' | 'miniapp' | 'subagent';
  sessionTitle?: string;
  agentType?: string;
  parentToolCallId?: string;
  subagentType?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
  includeInternal?: boolean;
}): void {
  ensureBtwSessionAvailable(params);

  const content = buildBtwSessionPanelContent(
    params.childSessionId,
    params.parentSessionId,
    params.workspacePath
  );

  const duplicateCheckKey = content.metadata?.duplicateCheckKey;
  const canvasStore = useAgentCanvasStore.getState();
  if (duplicateCheckKey) {
    const existing = canvasStore.findTabByMetadata({ duplicateCheckKey });
    if (existing) {
      if (params.expand !== false && isRightPanelCollapsed()) {
        requestRightPanelExpansion();
      }
      canvasStore.switchToTab(existing.tab.id, existing.groupId);
      clearSessionUnreadCompletionAfterRender(params.childSessionId);
      return;
    }
  }

  if (params.expand !== false) {
    requestRightPanelExpansion();
  }

  createTab({
    type: content.type,
    title: content.title,
    data: content.data,
    metadata: content.metadata,
    checkDuplicate: true,
    duplicateCheckKey,
    replaceExisting: false,
    mode: 'agent',
  });
  clearSessionUnreadCompletionAfterRender(params.childSessionId);
}

export function closeBtwSessionInAuxPane(childSessionId: string): boolean {
  const duplicateCheckKey = getBtwSessionDuplicateKey(childSessionId);
  const canvasStore = useAgentCanvasStore.getState();
  const result = canvasStore.findTabByMetadata({ duplicateCheckKey });
  if (!result) {
    return false;
  }

  canvasStore.closeTab(result.tab.id, result.groupId, { forceRemove: true });
  return true;
}
