import { i18nService } from '@/infrastructure/i18n';
import { createTab } from '@/shared/utils/tabUtils';
import { createLogger } from '@/shared/utils/logger';
import type { PanelContent } from '@/app/components/panels/base/types';
import { useAgentCanvasStore } from '@/app/components/panels/content-canvas/stores';
import type { CanvasTab } from '@/app/components/panels/content-canvas/types';
import type { Session } from '../types/flow-chat';
import { flowChatStore, isSessionConfirmedDeleted } from '../store/FlowChatStore';
import { flowChatManager } from './FlowChatManager';

export const BTW_SESSION_PANEL_TYPE = 'btw-session' as const;

const log = createLogger('btwSessionPane');

export type BtwSessionViewKind = 'review-check';

export interface BtwSessionPanelData {
  childSessionId: string;
  parentSessionId: string;
  workspacePath?: string;
  viewKind?: BtwSessionViewKind;
  displayTitle?: string;
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

export interface LoadBtwSessionHistoryParams {
  childSessionId: string;
  workspacePath?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

type AgentCanvasState = ReturnType<typeof useAgentCanvasStore.getState>;

export function getBtwSessionDuplicateKey(childSessionId: string): string {
  return `btw-session-${childSessionId}`;
}

const BTW_PLACEHOLDER_TITLE_TEXT_KEYS = ['flow-chat:btw.threadLabel', 'flow-chat:btw.deletedThreadLabel'] as const;

/** Timeout guard that stops watching a tab title if no real title ever arrives. */
const BTW_TAB_TITLE_REFRESH_TIMEOUT_MS = 5 * 60 * 1000;

const isBtwPlaceholderTitleText = (title: string | null | undefined): boolean =>
  Boolean(
    title?.trim() &&
    BTW_PLACEHOLDER_TITLE_TEXT_KEYS.some(key => title.trim() === i18nService.t(key)),
  );

/**
 * Resolve the child session's real title, ignoring generic placeholder titles
 * (for example a freshly created shell that has not been hydrated yet).
 */
const resolveBtwSessionTitleText = (session: Session | undefined): string | null => {
  if (!session) {
    return null;
  }
  const rawTitle =
    session.titleSource === 'i18n' && session.titleI18nKey
      ? i18nService.t(session.titleI18nKey, session.titleI18nParams)
      : session.title;
  const title = typeof rawTitle === 'string' ? rawTitle.trim() : '';
  if (!title || isBtwPlaceholderTitleText(title)) {
    return null;
  }
  return title;
};

const resolveBtwSessionTitle = (childSessionId: string): string => {
  const session = flowChatStore.getState().sessions.get(childSessionId);
  if (!session) {
    return i18nService.t('flow-chat:btw.deletedThreadLabel');
  }
  return resolveBtwSessionTitleText(session) || i18nService.t('flow-chat:btw.threadLabel');
};

const activeTabTitleWatchers = new Set<string>();

/**
 * Keeps a btw-session tab title in sync with the child session: once the real
 * session name arrives (history hydration metadata or title generation) the
 * generic placeholder title is replaced, and a session that disappears before
 * any real title arrived is marked as deleted. Explicit display titles win and
 * are never overwritten (callers only subscribe when the title is a
 * placeholder).
 */
const subscribeBtwSessionTabTitleRefresh = (params: {
  duplicateCheckKey: string;
  childSessionId: string;
}): void => {
  const resolveRealTitle = (): string | null => {
    const session = flowChatStore.getState().sessions.get(params.childSessionId);
    return session ? resolveBtwSessionTitleText(session) : null;
  };
  if (resolveRealTitle() || activeTabTitleWatchers.has(params.duplicateCheckKey)) {
    return;
  }

  let disposed = false;
  let unsubscribe: (() => void) | null = null;
  const cleanupTimer: { current?: ReturnType<typeof setTimeout> } = {};
  const dispose = (): void => {
    if (disposed) {
      return;
    }
    disposed = true;
    activeTabTitleWatchers.delete(params.duplicateCheckKey);
    if (cleanupTimer.current !== undefined) {
      clearTimeout(cleanupTimer.current);
    }
    unsubscribe?.();
  };

  activeTabTitleWatchers.add(params.duplicateCheckKey);
  unsubscribe = flowChatStore.subscribe(() => {
    if (disposed) {
      return;
    }
    const canvasStore = useAgentCanvasStore.getState();
    const existing = canvasStore.findTabByMetadata({ duplicateCheckKey: params.duplicateCheckKey });
    if (!existing || !isBtwPlaceholderTitleText(existing.tab.title)) {
      return;
    }

    const session = flowChatStore.getState().sessions.get(params.childSessionId);
    if (!session) {
      // Session disappeared before a real title arrived; mark as deleted.
      const deletedTitle = i18nService.t('flow-chat:btw.deletedThreadLabel');
      if (existing.tab.title !== deletedTitle) {
        canvasStore.updateTabContent(existing.tab.id, existing.groupId, {
          ...existing.tab.content,
          title: deletedTitle,
        });
      }
      return;
    }

    const realTitle = resolveRealTitle();
    if (!realTitle) {
      return;
    }
    dispose();
    if (existing.tab.title !== realTitle) {
      const content = existing.tab.content;
      const data = content.data && typeof content.data === 'object'
        ? { ...content.data, displayTitle: undefined }
        : content.data;
      canvasStore.updateTabContent(existing.tab.id, existing.groupId, {
        ...content,
        title: realTitle,
        data,
      });
    }
  });
  cleanupTimer.current = setTimeout(dispose, BTW_TAB_TITLE_REFRESH_TIMEOUT_MS);
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
  workspacePath?: string,
  viewKind?: BtwSessionViewKind,
  displayTitle?: string,
): PanelContent => ({
  type: BTW_SESSION_PANEL_TYPE,
  title: displayTitle?.trim() || resolveBtwSessionTitle(childSessionId),
  data: {
    childSessionId,
    parentSessionId,
    workspacePath,
    ...(viewKind ? { viewKind } : {}),
    ...(displayTitle?.trim() ? { displayTitle: displayTitle.trim() } : {}),
  } satisfies BtwSessionPanelData,
  metadata: {
    duplicateCheckKey: getBtwSessionDuplicateKey(childSessionId),
    childSessionId,
    parentSessionId,
    contentRole: 'btw-session',
  } satisfies BtwSessionPanelMetadata,
});

export const selectActiveAgentTab = (state: AgentCanvasState) => {
  // Resolve the active group. In grid9 mode the active group may be any of the
  // 16 editor groups (slot4..slot16), so fall through to scanning all group
  // state keys instead of assuming primary/secondary/tertiary (d7-P1-2).
  let activeGroupId = state.activeGroupId;
  const isKnownGroup = (
    activeGroupId === 'primary' || activeGroupId === 'secondary' || activeGroupId === 'tertiary'
    || /^slot(1[0-6]|[4-9])$/.test(activeGroupId)
  );
  if (!isKnownGroup) {
    activeGroupId = state.primaryGroup.activeTabId
      ? 'primary'
      : state.secondaryGroup.activeTabId
        ? 'secondary'
        : 'tertiary';
  }
  const groupKey: keyof AgentCanvasState =
    activeGroupId === 'primary' ? 'primaryGroup'
      : activeGroupId === 'secondary' ? 'secondaryGroup'
        : activeGroupId === 'tertiary' ? 'tertiaryGroup'
          : `${activeGroupId}Group` as keyof AgentCanvasState;
  const activeGroup = (state[groupKey] ?? state.tertiaryGroup) as { activeTabId: string | null; tabs: CanvasTab[] };
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

export async function loadBtwSessionHistory(params: LoadBtwSessionHistoryParams): Promise<void> {
  const location = params.workspacePath
    ? {
        workspacePath: params.workspacePath,
        remoteConnectionId: params.remoteConnectionId,
        remoteSshHost: params.remoteSshHost,
      }
    : undefined;
  const hydrate = (): Promise<void> => {
    if (location) {
      return flowChatManager.hydrateSessionHistoryForDetail(params.childSessionId, location);
    }
    return flowChatManager.hydrateSessionHistoryForDetail(params.childSessionId);
  };
  try {
    await hydrate();
  } catch (error) {
    // Automatic retry with the same parameters. If the second attempt also
    // fails the error propagates (the store marks historyState 'failed'), so
    // the panel can surface a visible retry entry instead of a silent empty
    // conversation.
    log.warn('Session history hydration failed, retrying once', {
      childSessionId: params.childSessionId,
      error,
    });
    await hydrate();
  }
}

export function ensureBtwSessionAvailable(params: EnsureBtwSessionAvailableParams): void {
  // A session whose deletion was confirmed must not be re-created as a
  // placeholder shell (nor hydrated) when its panel is requested again; the
  // panel already renders the deleted-thread placeholder title.
  if (isSessionConfirmedDeleted(params.childSessionId)) {
    log.warn('ensureBtwSessionAvailable: ignoring confirmed deleted session', {
      childSessionId: params.childSessionId,
    });
    return;
  }

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
      params.sessionTitle || i18nService.t('flow-chat:btw.threadLabel'),
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
  const hasLoadedDialogTurns = Boolean(sessionToHydrate?.dialogTurns?.length);
  const shouldHydrateMissingSubagentModel =
    Boolean(
      sessionToHydrate &&
      (params.sessionKind === 'subagent' || sessionToHydrate.sessionKind === 'subagent') &&
      !sessionToHydrate.config?.modelName &&
      !hasLoadedDialogTurns
    );
  // Relaxed: hydrate whenever a session exists with empty content and has not
  // reached a renderable ('ready') or in-flight ('hydrating') state, so
  // event-created placeholder shells (e.g. subagents) load automatically.
  // 'failed' stays eligible here (open-panel retry); the panel itself leaves
  // 'failed' for manual retry to avoid looping.
  const sessionHasEmptyUnreadyContent = Boolean(
    sessionToHydrate &&
    !hasLoadedDialogTurns &&
    sessionToHydrate.historyState !== 'ready' &&
    sessionToHydrate.historyState !== 'hydrating'
  );
  const shouldHydrate =
    !existingSession ||
    shouldHydrateMissingSubagentModel ||
    sessionHasEmptyUnreadyContent;

  const workspacePath = resolvedWorkspacePath || sessionToHydrate?.workspacePath;
  if (!shouldHydrate || !workspacePath) {
    return;
  }

  void loadBtwSessionHistory({
    childSessionId: params.childSessionId,
    ...(!sessionToHydrate?.workspacePath
      ? {
          workspacePath,
          remoteConnectionId: resolvedRemoteConnectionId,
          remoteSshHost: resolvedRemoteSshHost,
        }
      : {}),
  }).catch(error => {
    // Surface hydration failures in logs; the session panel also shows a
    // visible retry entry once historyState becomes 'failed'.
    log.warn('Failed to hydrate btw session history', {
      childSessionId: params.childSessionId,
      error,
    });
  });
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
  viewKind?: BtwSessionViewKind;
}): void {
  // Resolve the panel title before ensureBtwSessionAvailable may create an
  // on-demand shell, so a missing (deleted) child session gets the deleted
  // placeholder instead of the generic thread label.
  const content = buildBtwSessionPanelContent(
    params.childSessionId,
    params.parentSessionId,
    params.workspacePath,
    params.viewKind,
    params.sessionTitle,
  );

  ensureBtwSessionAvailable(params);

  const duplicateCheckKey = content.metadata?.duplicateCheckKey;
  const canvasStore = useAgentCanvasStore.getState();
  if (duplicateCheckKey) {
    const existing = canvasStore.findTabByMetadata({ duplicateCheckKey });
    if (existing) {
      if (params.expand !== false && isRightPanelCollapsed()) {
        requestRightPanelExpansion();
      }
      canvasStore.updateTabContent(existing.tab.id, existing.groupId, content);
      canvasStore.switchToTab(existing.tab.id, existing.groupId);
      clearSessionUnreadCompletionAfterRender(params.childSessionId);
      if (!params.sessionTitle?.trim() || isBtwPlaceholderTitleText(params.sessionTitle)) {
        subscribeBtwSessionTabTitleRefresh({
          duplicateCheckKey,
          childSessionId: params.childSessionId,
        });
      }
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
  if (duplicateCheckKey) {
    if (!params.sessionTitle?.trim() || isBtwPlaceholderTitleText(params.sessionTitle)) {
      subscribeBtwSessionTabTitleRefresh({
        duplicateCheckKey,
        childSessionId: params.childSessionId,
      });
    }
  }
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
