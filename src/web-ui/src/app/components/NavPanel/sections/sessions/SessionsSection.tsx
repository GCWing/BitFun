/**
 * SessionsSection — inline accordion content for the "Sessions" nav item.
 *
 * Rendered inside NavPanel when the Sessions item is expanded.
 * Owns all data fetching / mutation for chat sessions.
 */

import React, { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Pencil, Trash2, Check, X, Bot, MoreHorizontal, Loader2, Archive, Clock3, Copy, FileDown, ChevronLeft } from 'lucide-react';
import { IconButton, Input, Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { flowChatStore } from '../../../../../flow_chat/store/FlowChatStore';
import { flowChatManager } from '../../../../../flow_chat/services/FlowChatManager';
import type { FlowChatState, Session } from '../../../../../flow_chat/types/flow-chat';
import { hasPendingAskUserQuestion, resolveTrackedTurn } from '../../../../../flow_chat/utils/askUserQuestionState';
import { useSceneStore } from '../../../../stores/sceneStore';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useAgentCanvasStore } from '@/app/components/panels/content-canvas/stores';
import {
  openBtwSessionInAuxPane,
  selectActiveBtwSessionTab,
} from '@/flow_chat/services/btwSessionPane';
import { openMainSession } from '@/flow_chat/services/sessionActivation';
import {
  dispatchHistorySessionOpenIntent,
  shouldShowHistorySessionOpenIntent,
} from '@/flow_chat/services/sessionOpenIntent';
import { recordHistorySessionDiagnosticEvent } from '@/flow_chat/services/historySessionDiagnostics';
import { resolveSessionRelationship } from '@/flow_chat/utils/sessionMetadata';
import {
  sessionBelongsToWorkspaceNavRow,
} from '@/flow_chat/utils/sessionOrdering';
import {
  compareWorkspaceNavSessions,
  DEFAULT_WORKSPACE_SESSION_FILTERS,
  hasWorkspaceSessionFilters,
  matchesWorkspaceSessionView,
  useWorkspaceSessionViewStore,
} from '../../workspaceSessionView';
import { stateMachineManager } from '@/flow_chat/state-machine';
import { SessionExecutionState } from '@/flow_chat/state-machine/types';
import { i18nService } from '@/infrastructure/i18n';
import { resolveSessionTitle } from '@/flow_chat/utils/sessionTitle';
import { isSessionNavRowActive } from './sessionNavSelection';
import {
  deriveSessionReviewActivity,
  isReviewActivityBlocking,
} from '@/flow_chat/utils/sessionReviewActivity';
import { useBackgroundSubagentActivityStore } from '@/flow_chat/store/backgroundSubagentActivityStore';
import type {
  BackgroundSubagentActivity,
  BackgroundSubagentActivityItem,
} from '@/flow_chat/utils/backgroundSubagentActivity';
import { computeFixedPopoverPosition } from '@/shared/utils/fixedPopoverViewport';
import { exportSessionToMarkdown } from '@/flow_chat/services/sessionMarkdownExport';
import type { TranscriptExportScope } from '@/flow_chat/utils/dialogTranscriptExport';
import { confirmWarning } from '@/component-library/components/ConfirmDialog/confirmService';
import {
  AssistantAvatar,
  type AssistantAvatarStatus,
} from '@/app/components/AssistantAvatar';
import { notificationService } from '@/shared/notification-system';
import { copyTextToClipboard } from '@/shared/utils/textSelection';
import { isOutcomeUnknownError } from '@/infrastructure/api/errors/TauriCommandError';
import { scheduleAfterStartupPaint, scheduleAfterStartupSignal } from '@/shared/utils/startupTaskScheduling';
import {
  isNonLocalDispatchTarget,
  type DispatchJobState,
} from '@/features/dispatch/types';
import { useDispatchJobStore } from '@/features/dispatch/dispatchJobStore';
import { resolveDispatchNavPresentation } from '@/features/dispatch/dispatchNavPresentation';
import {
  SESSION_METADATA_DEFERRED_FALLBACK_MS,
  SESSION_METADATA_DEFERRED_FRAME_COUNT,
  SESSION_METADATA_DEFERRED_SIGNAL,
  getDeferredSessionMetadataDelayMs,
  getInitialSessionMetadataLoadMode,
  hasStartupOverlayHandedOff,
} from './sessionMetadataStartup';
import {
  getEffectiveTopLevelSessionCount,
  getSessionBufferPrefetchLimit,
  getSessionExpandToggleState,
  SESSIONS_LEVEL_0,
  SESSIONS_LEVEL_1,
} from './sessionNavExpand';
import { useSessionRowRemovalTransition } from './sessionRowShift';
import './SessionsSection.scss';

const log = createLogger('SessionsSection');
const ScheduledJobsModal = lazy(() => import('@/app/components/scheduled-jobs/ScheduledJobsModal'));

type SessionMode = 'code' | 'cowork' | 'claw';
type HistoryOpenIntentDispatchResult = 'none' | 'dispatched' | 'already-pending';

/** Page size for the fully-expanded (level 2) session list. */
const SESSIONS_LEVEL_2_PAGE = 200;

/**
 * Delay before topping the off-screen row buffer back up. Keeps the refill out
 * of the startup burst and coalesces the repeated deletes of a cleanup pass.
 */
const SESSIONS_BUFFER_PREFETCH_DELAY_MS = 800;

const escapeRegExp = (value: string): string =>
  value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

const resolveSessionModeType = (session: Session): SessionMode => {
  const normalizedMode = session.mode?.toLowerCase();
  if (normalizedMode === 'cowork') return 'cowork';
  if (normalizedMode === 'claw') return 'claw';
  return 'code';
};

const getTitle = (session: Session): string =>
  resolveSessionTitle(session, (key, options) => i18nService.t(key, options));

const countTopLevelSessionsInScope = (
  sessions: Iterable<Session>,
  workspacePath?: string,
  remoteConnectionId?: string | null,
  remoteSshHost?: string | null,
): number => {
  const scopedSessions = Array.from(sessions).filter((session: Session) => {
    if (session.isTransient || session.sessionKind === 'subagent') {
      return false;
    }
    if (workspacePath) {
      return sessionBelongsToWorkspaceNavRow(session, workspacePath, remoteConnectionId, remoteSshHost);
    }
    return !session.workspacePath;
  });

  const knownIds = new Set(scopedSessions.map(session => session.sessionId));
  return scopedSessions.reduce((count, session) => {
    const parentSessionId = resolveSessionRelationship(session).parentSessionId;
    if (parentSessionId && knownIds.has(parentSessionId)) {
      return count;
    }
    return count + 1;
  }, 0);
};

const getChildSessionBadge = (kind: Session['sessionKind']): string => {
  const normalizedKind =
    kind === 'review' || kind === 'deep_review' || kind === 'subagent'
      ? kind
      : 'btw';
  const fallback = normalizedKind === 'deep_review'
    ? 'Strict'
    : normalizedKind === 'review'
      ? 'Review'
      : normalizedKind === 'subagent'
        ? 'Agent'
      : 'btw';
  return i18nService.t(`flow-chat:childSession.kinds.${normalizedKind}.short`, {
    defaultValue: fallback,
  });
};

const getReviewActivityBadge = (kind: 'review' | 'deep_review'): string =>
  i18nService.t(
    kind === 'deep_review'
      ? 'common:nav.sessions.deepReviewRunning'
      : 'common:nav.sessions.reviewRunning',
    {
      defaultValue: 'Reviewing',
    },
  );

export interface AssistantSessionPresentation {
  kind: 'assistant';
  assistant: {
    id: string;
    name: string;
    avatar?: string | null;
    emoji?: string | null;
  };
}

interface SessionsSectionProps {
  workspaceId?: string;
  workspacePath?: string;
  /** Remote SSH: same `workspacePath` on different hosts must filter by this (see Session.remoteConnectionId). */
  remoteConnectionId?: string | null;
  /** Remote SSH: disambiguates same path on different hosts; when set with matching session host, connectionId may differ. */
  remoteSshHost?: string | null;
  isActiveWorkspace?: boolean;
  showCreateActions?: boolean;
  /** Product presentation for assistant-owned sessions. Project sessions use the compact row. */
  presentation?: AssistantSessionPresentation;
  /** Prevents startup metadata fetching while the surrounding section is collapsed. */
  isVisible?: boolean;
  /** Apply the Workspace section's shared sorting and display preferences. */
  useWorkspaceViewPreferences?: boolean;
  /** Multiple project scopes used by the flat "all sessions" projection. */
  workspaceScopes?: WorkspaceSessionScope[];
  /** Navigation geometry for nested workspace rows versus the flat aggregate list. */
  layout?: 'nested' | 'flat';
}

export interface WorkspaceSessionScope {
  workspaceId: string;
  workspaceName: string;
  workspacePath: string;
  remoteConnectionId?: string | null;
  remoteSshHost?: string | null;
}

const SessionsSection: React.FC<SessionsSectionProps> = ({
  workspaceId,
  workspacePath,
  remoteConnectionId = null,
  remoteSshHost = null,
  isActiveWorkspace = true,
  presentation,
  isVisible = true,
  useWorkspaceViewPreferences = false,
  workspaceScopes,
  layout = 'nested',
}) => {
  const { t } = useI18n('common');
  const storedSessionOrdering = useWorkspaceSessionViewStore(state => state.ordering);
  const storedSessionShow = useWorkspaceSessionViewStore(state => state.show);
  const storedSessionFilters = useWorkspaceSessionViewStore(state => state.filters);
  const sessionOrdering = useWorkspaceViewPreferences ? storedSessionOrdering : 'created';
  const sessionShow = useWorkspaceViewPreferences ? storedSessionShow : 'all';
  const sessionFilters = useWorkspaceViewPreferences
    ? storedSessionFilters
    : DEFAULT_WORKSPACE_SESSION_FILTERS;
  const hasActiveSessionFilter = sessionShow !== 'all' || hasWorkspaceSessionFilters(sessionFilters);
  const sessionListClassName = `bitfun-nav-panel__inline-list${layout === 'flat' ? ' is-flat-workspace-view' : ''}`;
  const { setActiveWorkspace, currentWorkspace } = useWorkspaceContext();
  const activeTabId = useSceneStore(s => s.activeTabId);
  const activeBtwSessionTab = useAgentCanvasStore(state => selectActiveBtwSessionTab(state as any));
  const activeBtwSessionData = activeBtwSessionTab?.content.data as
    | { childSessionId: string; parentSessionId: string; workspacePath?: string }
    | undefined;
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() =>
    flowChatStore.getState()
  );
  const backgroundSubagentActivities = useBackgroundSubagentActivityStore(state => state.activities);
  const dispatchTransportByJobId = useDispatchJobStore(state => state.transportByJobId);
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState('');
  const [expandLevel, setExpandLevel] = useState<0 | 1 | 2>(0);
  // Level-2 ("show all") renders in pages of 200 rows so a huge session
  // history cannot mount thousands of un-virtualized rows at once.
  const [level2DisplayCount, setLevel2DisplayCount] = useState(SESSIONS_LEVEL_2_PAGE);

  useEffect(() => {
    if (expandLevel !== 2) {
      setLevel2DisplayCount(SESSIONS_LEVEL_2_PAGE);
    }
  }, [expandLevel]);
  const [metadataPageState, setMetadataPageState] = useState<{
    totalTopLevelCount: number | null;
    syncedTopLevelCount: number | null;
    nextCursor?: string;
    hasMore: boolean;
    isLoading: boolean;
    loadError: boolean;
  }>({
    totalTopLevelCount: null,
    syncedTopLevelCount: null,
    nextCursor: undefined,
    hasMore: false,
    isLoading: false,
    loadError: false,
  });
  const [openMenuSessionId, setOpenMenuSessionId] = useState<string | null>(null);
  const [sessionMenuPosition, setSessionMenuPosition] = useState<{ top: number; left: number } | null>(null);
  /** Second level of the session menu: pick what a Markdown export includes. */
  const [isExportScopeMenu, setIsExportScopeMenu] = useState(false);
  const [exportingSessionId, setExportingSessionId] = useState<string | null>(null);
  const [runningSessionIds, setRunningSessionIds] = useState<Set<string>>(new Set());
  const [scheduledJobsSessionId, setScheduledJobsSessionId] = useState<string | null>(null);
  const editInputRef = useRef<HTMLInputElement>(null);
  const sessionMenuPopoverRef = useRef<HTMLDivElement>(null);
  const sessionMenuAnchorRef = useRef<HTMLButtonElement>(null);
  const metadataLoadRequestIdRef = useRef(0);
  /** User-driven metadata loads still running; background loads yield to them. */
  const foregroundLoadCountRef = useRef(0);
  const initialMetadataLoadKeyRef = useRef<string | null>(null);
  const sessionListRef = useRef<HTMLDivElement>(null);
  /** Last (scope, live, synced) triple a background reconcile ran for. */
  const liveReconcileSignatureRef = useRef<string | null>(null);
  /** Last (scope, cursor, size) triple a buffer prefetch ran for. */
  const bufferPrefetchSignatureRef = useRef<string | null>(null);

  // Subscribe to state machine changes for running status
  useEffect(() => {
    const updateRunningSessions = () => {
      const running = new Set<string>();
      for (const session of flowChatState.sessions.values()) {
        const machine = stateMachineManager.get(session.sessionId);
        if (
          machine &&
          (machine.getCurrentState() === SessionExecutionState.PROCESSING ||
            machine.getCurrentState() === SessionExecutionState.FINISHING)
        ) {
          running.add(session.sessionId);
        }
      }
      setRunningSessionIds(running);
    };

    updateRunningSessions();
    const unsubscribe = stateMachineManager.subscribeGlobal(() => {
      updateRunningSessions();
    });
    return () => unsubscribe();
  }, [flowChatState.sessions]);

  useEffect(() => {
    const selector = (s: FlowChatState): string => {
      const parts: string[] = [s.activeSessionId ?? ''];
      for (const session of s.sessions.values()) {
        const latestTurn = session.dialogTurns[session.dialogTurns.length - 1];
        const trackedTurn = resolveTrackedTurn(session);
        const hasAskUser = hasPendingAskUserQuestion(trackedTurn);
        const dispatchTarget = session.config.dispatchTarget;
        const dispatchTargetSnapshot = dispatchTarget?.kind === 'ssh'
          ? `ssh:${dispatchTarget.connectionId}:${dispatchTarget.workspacePath}:${dispatchTarget.displayName}`
          : dispatchTarget?.kind === 'device'
            ? `device:${dispatchTarget.deviceId}:${dispatchTarget.workspacePath}:${dispatchTarget.displayName}`
            : 'local';
        parts.push(
          `${session.sessionId}|${session.isTransient ? '1':'0'}|${session.sessionKind}|` +
          `${session.parentSessionId ?? ''}|${session.parentToolCallId ?? ''}|${session.subagentType ?? ''}|` +
          `${session.workspacePath ?? ''}|${session.mode ?? ''}|${session.needsUserAttention ? '1':'0'}|` +
          `${session.hasUnreadCompletion ? '1':'0'}|${latestTurn?.status ?? ''}|${hasAskUser ? '1':'0'}|${trackedTurn?.id ?? ''}|` +
          `${session.title ?? ''}|${dispatchTargetSnapshot}|${session.config.dispatchJobState ?? ''}`
        );
      }
      return parts.join(';');
    };
    const unsub = flowChatStore.subscribeSelector(selector, (() => {
      setFlowChatState(flowChatStore.getState());
    }), { isEqual: (a, b) => a === b });
    return () => unsub();
  }, []);

  const backgroundSubagentActivityByParent = useMemo(() => {
    const itemsByParent = new Map<string, BackgroundSubagentActivityItem[]>();
    for (const item of Object.values(backgroundSubagentActivities)) {
      const items = itemsByParent.get(item.parentSessionId) ?? [];
      items.push(item);
      itemsByParent.set(item.parentSessionId, items);
    }

    const activityByParent = new Map<string, BackgroundSubagentActivity>();
    for (const [parentSessionId, items] of itemsByParent) {
      const sortedItems = [...items].sort((left, right) => (
        left.createdAt - right.createdAt || left.sessionId.localeCompare(right.sessionId)
      ));
      activityByParent.set(parentSessionId, {
        runningCount: sortedItems.filter(item => item.status === 'processing').length,
        finishingCount: sortedItems.filter(item => item.status === 'finishing').length,
        totalCount: sortedItems.length,
        items: sortedItems,
      });
    }

    return activityByParent;
  }, [backgroundSubagentActivities]);

  useEffect(() => {
    if (editingSessionId && editInputRef.current) {
      editInputRef.current.focus();
      editInputRef.current.select();
    }
  }, [editingSessionId]);

  useEffect(() => {
    metadataLoadRequestIdRef.current += 1;
    initialMetadataLoadKeyRef.current = null;
    liveReconcileSignatureRef.current = null;
    bufferPrefetchSignatureRef.current = null;
    setExpandLevel(0);
    setMetadataPageState({
      totalTopLevelCount: null,
      syncedTopLevelCount: null,
      nextCursor: undefined,
      hasMore: false,
      isLoading: false,
      loadError: false,
    });
  }, [workspaceId, workspacePath, remoteConnectionId, remoteSshHost]);

  const workspaceScopesKey = useMemo(
    () => workspaceScopes?.map(scope => [
      scope.workspaceId,
      scope.workspacePath,
      scope.remoteConnectionId ?? '',
      scope.remoteSshHost ?? '',
    ].join(':')).join('|') ?? '',
    [workspaceScopes],
  );

  useEffect(() => {
    if (!isVisible || !workspaceScopes?.length) return;
    let cancelled = false;
    const loadAllScopes = async () => {
      for (const scope of workspaceScopes) {
        let cursor: string | undefined;
        do {
          const page = await flowChatStore.loadSessionMetadataPage(
            scope.workspacePath,
            SESSIONS_LEVEL_2_PAGE,
            cursor,
            scope.remoteConnectionId || undefined,
            scope.remoteSshHost || undefined,
            'sessions_nav_all_grouping',
          );
          if (cancelled) return;
          cursor = page.hasMore ? page.nextCursor : undefined;
        } while (cursor);
      }
    };
    void loadAllScopes().catch(error => {
      if (!cancelled) log.warn('Failed to load all-session workspace projection', { error });
    });
    return () => { cancelled = true; };
  }, [isVisible, workspaceScopes, workspaceScopesKey]);

  useEffect(() => {
    if (!isVisible || sessionFilters.hideArchived) return;
    const scopes = workspaceScopes?.length
      ? workspaceScopes
      : workspacePath
        ? [{ workspacePath, remoteConnectionId, remoteSshHost }]
        : [];
    void Promise.all(scopes.map(scope => flowChatStore.loadArchivedSessionMetadata(
      scope.workspacePath,
      scope.remoteConnectionId || undefined,
      scope.remoteSshHost || undefined,
    ))).catch(error => {
      log.warn('Failed to load archived session projection', { error });
    });
  }, [
    isVisible,
    remoteConnectionId,
    remoteSshHost,
    sessionFilters.hideArchived,
    workspacePath,
    workspaceScopes,
    workspaceScopesKey,
  ]);

  const loadMetadataPage = useCallback(
    async (
      limit: number,
      cursor: string | undefined,
      source: string,
      options?: { background?: boolean },
    ) => {
      if (!workspacePath || limit <= 0) {
        return null;
      }

      // A background refresh only re-syncs counts for a list that is already on
      // screen. Surfacing its spinner (or its retry state) would make routine
      // upkeep — such as replacing the row a delete consumed — flash the list.
      // It also leaves the request id alone so it cannot strand a user-driven
      // load (which would otherwise never clear its spinner); instead it drops
      // its own result if anything else claimed the list meanwhile.
      const isBackgroundLoad = options?.background === true;
      if (isBackgroundLoad && foregroundLoadCountRef.current > 0) {
        return null;
      }

      const requestId = isBackgroundLoad
        ? metadataLoadRequestIdRef.current
        : metadataLoadRequestIdRef.current + 1;
      if (!isBackgroundLoad) {
        metadataLoadRequestIdRef.current = requestId;
        foregroundLoadCountRef.current += 1;
        setMetadataPageState(prev => ({
          ...prev,
          isLoading: true,
          loadError: false,
        }));
      }

      try {
        const page = await flowChatStore.loadSessionMetadataPage(
          workspacePath,
          limit,
          cursor,
          remoteConnectionId || undefined,
          remoteSshHost || undefined,
          source
        );
        if (metadataLoadRequestIdRef.current === requestId) {
          const syncedTopLevelCount = countTopLevelSessionsInScope(
            flowChatStore.getState().sessions.values(),
            workspacePath,
            remoteConnectionId,
            remoteSshHost,
          );
          setMetadataPageState({
            totalTopLevelCount: page.totalTopLevelCount,
            syncedTopLevelCount,
            nextCursor: page.nextCursor,
            hasMore: page.hasMore,
            isLoading: false,
            loadError: false,
          });
        }
        return page;
      } catch (error) {
        if (metadataLoadRequestIdRef.current === requestId && !isBackgroundLoad) {
          setMetadataPageState(prev => ({
            ...prev,
            isLoading: false,
            loadError: true,
          }));
        }
        log.warn('Failed to load visible session metadata page', { error, workspacePath, cursor, limit });
        return null;
      } finally {
        if (!isBackgroundLoad) {
          foregroundLoadCountRef.current = Math.max(foregroundLoadCountRef.current - 1, 0);
        }
      }
    },
    [workspacePath, remoteConnectionId, remoteSshHost]
  );

  const initialMetadataKey = useMemo(
    () => [
      workspacePath ?? '',
      remoteConnectionId ?? '',
      remoteSshHost ?? '',
    ].join('\n'),
    [workspacePath, remoteConnectionId, remoteSshHost],
  );

  const loadInitialMetadataPage = useCallback(
    async (source: string) => {
      if (!workspacePath) {
        return;
      }
      if (initialMetadataLoadKeyRef.current === initialMetadataKey) {
        return;
      }

      initialMetadataLoadKeyRef.current = initialMetadataKey;
      const page = await loadMetadataPage(SESSIONS_LEVEL_0, undefined, source);
      if (!page && initialMetadataLoadKeyRef.current === initialMetadataKey) {
        initialMetadataLoadKeyRef.current = null;
      }
    },
    [initialMetadataKey, loadMetadataPage, workspacePath],
  );

  useEffect(() => {
    if (!isVisible || !workspacePath) {
      return;
    }

    const loadMode = getInitialSessionMetadataLoadMode({
      hasWorkspacePath: Boolean(workspacePath),
      isActiveWorkspace,
      isVisible,
      startupOverlayHandedOff: hasStartupOverlayHandedOff(),
    });

    if (loadMode === 'skip') {
      return;
    }

    if (loadMode === 'immediate') {
      void loadInitialMetadataPage('sessions_nav_initial_active');
      return;
    }

    let cancelled = false;
    let delayTimer: number | null = null;
    const scheduleDeferredMetadataLoad = () => {
      if (cancelled) {
        return;
      }
      const delayMs = getDeferredSessionMetadataDelayMs(workspaceId ?? workspacePath);
      const runDeferredLoad = () => {
        delayTimer = null;
        if (!cancelled) {
          void loadInitialMetadataPage('sessions_nav_initial_deferred');
        }
      };

      if (delayMs > 0) {
        delayTimer = window.setTimeout(runDeferredLoad, delayMs);
        return;
      }
      runDeferredLoad();
    };
    const cancelStartupSchedule = loadMode === 'after-startup-paint'
      ? scheduleAfterStartupPaint(scheduleDeferredMetadataLoad, {
          frameCount: SESSION_METADATA_DEFERRED_FRAME_COUNT,
        })
      : scheduleAfterStartupSignal(scheduleDeferredMetadataLoad, {
          signalName: SESSION_METADATA_DEFERRED_SIGNAL,
          fallbackTimeoutMs: SESSION_METADATA_DEFERRED_FALLBACK_MS,
          frameCount: SESSION_METADATA_DEFERRED_FRAME_COUNT,
        });

    return () => {
      cancelled = true;
      cancelStartupSchedule();
      if (delayTimer !== null) {
        window.clearTimeout(delayTimer);
      }
    };
  }, [
    isActiveWorkspace,
    isVisible,
    loadInitialMetadataPage,
    workspaceId,
    workspacePath,
  ]);

  useEffect(() => {
    const needsExpandedDataset = sessionOrdering !== 'updated' || hasActiveSessionFilter;
    if (!needsExpandedDataset || !isVisible || !workspacePath) {
      return;
    }

    setExpandLevel(2);
    setLevel2DisplayCount(SESSIONS_LEVEL_2_PAGE);
    void loadMetadataPage(
      SESSIONS_LEVEL_2_PAGE,
      undefined,
      'sessions_nav_view_preferences',
    );
  }, [
    isVisible,
    loadMetadataPage,
    sessionOrdering,
    hasActiveSessionFilter,
    workspacePath,
  ]);

  // When sessions are archived, reset stale metadata so the expand toggle
  // doesn't linger with old counts after all sessions are gone.
  useEffect(() => {
    const handler = () => {
      metadataLoadRequestIdRef.current += 1;
      liveReconcileSignatureRef.current = null;
      bufferPrefetchSignatureRef.current = null;
      setExpandLevel(0);
      setMetadataPageState({
        totalTopLevelCount: null,
        syncedTopLevelCount: null,
        nextCursor: undefined,
        hasMore: false,
        isLoading: false,
        loadError: false,
      });
      if (isVisible && workspacePath) {
        void loadMetadataPage(SESSIONS_LEVEL_0, undefined, 'sessions_nav_post_archive');
      }
    };
    window.addEventListener('bitfun:session-archived', handler);
    return () => window.removeEventListener('bitfun:session-archived', handler);
  }, [isVisible, workspacePath, loadMetadataPage]);

  const closeSessionMenu = useCallback(() => {
    setOpenMenuSessionId(null);
    setSessionMenuPosition(null);
    setIsExportScopeMenu(false);
  }, []);

  useEffect(() => {
    if (!openMenuSessionId) return;
    const handleOutside = (event: MouseEvent) => {
      if (!sessionMenuPopoverRef.current?.contains(event.target as Node)) {
        closeSessionMenu();
      }
    };
    document.addEventListener('mousedown', handleOutside);
    return () => document.removeEventListener('mousedown', handleOutside);
  }, [closeSessionMenu, openMenuSessionId]);

  const updateSessionMenuPosition = useCallback(() => {
    const anchor = sessionMenuAnchorRef.current;
    if (!anchor || !openMenuSessionId) return;
    const rect = anchor.getBoundingClientRect();
    const viewportPadding = 8;
    const gap = 4;
    const fallbackWidth = 160;
    const fallbackHeight = 96;

    const apply = () => {
      const menuEl = sessionMenuPopoverRef.current;
      const w = menuEl?.offsetWidth ?? fallbackWidth;
      const h = menuEl?.offsetHeight ?? fallbackHeight;
      setSessionMenuPosition(computeFixedPopoverPosition(rect, w, h, gap, viewportPadding));
    };

    apply();
    requestAnimationFrame(apply);
  }, [openMenuSessionId]);

  useEffect(() => {
    if (!openMenuSessionId) return;

    // The second menu level has a different height; re-anchor on switch.
    updateSessionMenuPosition();

    const handleViewportChange = () => updateSessionMenuPosition();
    window.addEventListener('resize', handleViewportChange);
    window.addEventListener('scroll', handleViewportChange, true);

    return () => {
      window.removeEventListener('resize', handleViewportChange);
      window.removeEventListener('scroll', handleViewportChange, true);
    };
  }, [isExportScopeMenu, openMenuSessionId, updateSessionMenuPosition]);

  // Clear unread completion mark after the switched session renders
  useEffect(() => {
    const handleSessionSwitched = (e: Event) => {
      const { sessionId } = (e as CustomEvent).detail;
      if (!sessionId) return;
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          flowChatStore.clearSessionUnreadCompletion(sessionId);
          flowChatStore.clearSessionNeedsAttention(sessionId);
        });
      });
    };

    window.addEventListener('bitfun:session-switched', handleSessionSwitched);
    return () => window.removeEventListener('bitfun:session-switched', handleSessionSwitched);
  }, []);

  const sessions = useMemo(
    () =>
      Array.from(flowChatState.sessions.values())
        .filter((s: Session) => {
          if (s.isTransient) {
            return false;
          }
          if (s.sessionKind === 'subagent') {
            return false;
          }
          if (workspaceScopes?.length) {
            return workspaceScopes.some(scope => sessionBelongsToWorkspaceNavRow(
              s,
              scope.workspacePath,
              scope.remoteConnectionId,
              scope.remoteSshHost,
            ));
          }
          if (workspacePath) {
            return sessionBelongsToWorkspaceNavRow(s, workspacePath, remoteConnectionId, remoteSshHost);
          }
          return !s.workspacePath;
        }),
    [flowChatState.sessions, workspacePath, remoteConnectionId, remoteSshHost, workspaceScopes]
  );

  const { topLevelSessions: allTopLevelSessions, childrenByParent } = useMemo(() => {
    const childMap = new Map<string, Session[]>();
    const parents: Session[] = [];

    const knownIds = new Set(sessions.map(s => s.sessionId));

    for (const s of sessions) {
      const pid = resolveSessionRelationship(s).parentSessionId;
      if (pid && typeof pid === 'string' && pid.trim() && knownIds.has(pid)) {
        const list = childMap.get(pid) || [];
        list.push(s);
        childMap.set(pid, list);
      } else {
        parents.push(s);
      }
    }

    const compareForCurrentView = (left: Session, right: Session) =>
      compareWorkspaceNavSessions(
        left,
        right,
        sessionOrdering,
        getTitle,
        session => runningSessionIds.has(session.sessionId),
      );

    for (const [pid, list] of childMap) {
      childMap.set(pid, [...list].sort(compareForCurrentView));
    }

    return {
      topLevelSessions: [...parents].sort(compareForCurrentView),
      childrenByParent: childMap,
    };
  }, [runningSessionIds, sessionOrdering, sessions]);

  const topLevelSessions = useMemo(
    () => allTopLevelSessions.filter(session => matchesWorkspaceSessionView(
      session,
      sessionShow,
      sessionFilters,
      runningSessionIds.has(session.sessionId),
    )),
    [allTopLevelSessions, runningSessionIds, sessionFilters, sessionShow],
  );

  const visibleChildrenByParent = useMemo(() => {
    if (!hasActiveSessionFilter) return childrenByParent;

    const filtered = new Map<string, Session[]>();
    for (const [parentId, children] of childrenByParent) {
      filtered.set(parentId, children.filter(session => matchesWorkspaceSessionView(
        session,
        sessionShow,
        sessionFilters,
        runningSessionIds.has(session.sessionId),
      )));
    }
    return filtered;
  }, [childrenByParent, hasActiveSessionFilter, runningSessionIds, sessionFilters, sessionShow]);

  const sessionDisplayLimit = useMemo(() => {
    const total = topLevelSessions.length;
    if (expandLevel === 2) return Math.min(total, level2DisplayCount);
    if (total <= SESSIONS_LEVEL_0) return total;
    if (expandLevel === 1) return Math.min(total, SESSIONS_LEVEL_1);
    return SESSIONS_LEVEL_0;
  }, [topLevelSessions.length, expandLevel, level2DisplayCount]);

  const totalTopLevelSessionCount = !hasActiveSessionFilter && !workspaceScopes?.length
    ? getEffectiveTopLevelSessionCount(
        metadataPageState.totalTopLevelCount,
        metadataPageState.syncedTopLevelCount,
        allTopLevelSessions.length,
        metadataPageState.isLoading,
      )
    : topLevelSessions.length;
  const hasMoreUnloadedSessions =
    !hasActiveSessionFilter && !workspaceScopes?.length && allTopLevelSessions.length < totalTopLevelSessionCount;
  const expandToggleState = getSessionExpandToggleState(totalTopLevelSessionCount, expandLevel);

  useEffect(() => {
    if (
      !isVisible ||
      !workspacePath ||
      metadataPageState.isLoading ||
      metadataPageState.totalTopLevelCount === null ||
      metadataPageState.syncedTopLevelCount === null ||
      allTopLevelSessions.length === metadataPageState.syncedTopLevelCount
    ) {
      return;
    }

    // Re-running for a scope whose counts have not moved would spin on a
    // failing backend, since a background load leaves the state untouched.
    const signature = [
      initialMetadataKey,
      allTopLevelSessions.length,
      metadataPageState.syncedTopLevelCount,
    ].join('\n');
    if (liveReconcileSignatureRef.current === signature) {
      return;
    }
    liveReconcileSignatureRef.current = signature;

    void loadMetadataPage(SESSIONS_LEVEL_0, undefined, 'sessions_nav_live_reconcile', {
      background: true,
    });
  }, [
    initialMetadataKey,
    isVisible,
    loadMetadataPage,
    metadataPageState.isLoading,
    metadataPageState.syncedTopLevelCount,
    metadataPageState.totalTopLevelCount,
    allTopLevelSessions.length,
    workspacePath,
  ]);

  // Keep a few rows loaded past the visible slice. Deleting a session then
  // promotes an already-loaded row in the same commit instead of leaving a gap
  // until a metadata round trip lands.
  useEffect(() => {
    if (
      !isVisible ||
      !workspacePath ||
      metadataPageState.isLoading ||
      metadataPageState.loadError ||
      metadataPageState.totalTopLevelCount === null ||
      !metadataPageState.nextCursor
    ) {
      return;
    }

    const prefetchLimit = getSessionBufferPrefetchLimit({
      expandLevel,
      loadedTopLevelCount: allTopLevelSessions.length,
      totalTopLevelCount: totalTopLevelSessionCount,
      hasMore: metadataPageState.hasMore,
    });
    if (prefetchLimit <= 0) {
      return;
    }

    const signature = [initialMetadataKey, metadataPageState.nextCursor, prefetchLimit].join('\n');
    if (bufferPrefetchSignatureRef.current === signature) {
      return;
    }

    const cursor = metadataPageState.nextCursor;
    const timer = window.setTimeout(() => {
      bufferPrefetchSignatureRef.current = signature;
      void loadMetadataPage(prefetchLimit, cursor, 'sessions_nav_buffer_prefetch', {
        background: true,
      });
    }, SESSIONS_BUFFER_PREFETCH_DELAY_MS);

    return () => window.clearTimeout(timer);
  }, [
    allTopLevelSessions.length,
    expandLevel,
    initialMetadataKey,
    isVisible,
    loadMetadataPage,
    metadataPageState.hasMore,
    metadataPageState.isLoading,
    metadataPageState.loadError,
    metadataPageState.nextCursor,
    metadataPageState.totalTopLevelCount,
    totalTopLevelSessionCount,
    workspacePath,
  ]);

  const visibleItems = useMemo(() => {
    const visibleParents = topLevelSessions.slice(0, sessionDisplayLimit);
    const out: Array<{ session: Session; level: 0 | 1 }> = [];
    for (const p of visibleParents) {
      out.push({ session: p, level: 0 });
      const children = visibleChildrenByParent.get(p.sessionId) || [];
      for (const c of children) out.push({ session: c, level: 1 });
    }
    return out;
  }, [sessionDisplayLimit, topLevelSessions, visibleChildrenByParent]);

  const visibleRowSignature = useMemo(
    () => visibleItems.map(item => item.session.sessionId).join('|'),
    [visibleItems],
  );
  useSessionRowRemovalTransition(sessionListRef, visibleRowSignature);

  const activeSessionId = flowChatState.activeSessionId;
  const scheduledJobsSession = scheduledJobsSessionId
    ? flowChatState.sessions.get(scheduledJobsSessionId) ?? null
    : null;
  const lastHistoryOpenIntentRef = useRef<{ sessionId: string; atMs: number } | null>(null);

  const dispatchHistoryOpenIntentForSession = useCallback(
    (session: Session, source: 'pointerdown' | 'switch'): HistoryOpenIntentDispatchResult => {
      const sessionId = session.sessionId;
      if (
        sessionId === activeSessionId ||
        !shouldShowHistorySessionOpenIntent(session, {
          isRunning: runningSessionIds.has(sessionId),
        })
      ) {
        return 'none';
      }

      const now = typeof performance !== 'undefined' ? performance.now() : Date.now();
      const lastIntent = lastHistoryOpenIntentRef.current;
      if (
        lastIntent &&
        lastIntent.sessionId === sessionId &&
        now - lastIntent.atMs < 250
      ) {
        recordHistorySessionDiagnosticEvent(sessionId, 'history_open_intent_deduped', {
          source,
          ageMs: Math.round(now - lastIntent.atMs),
        });
        return 'already-pending';
      }

      lastHistoryOpenIntentRef.current = { sessionId, atMs: now };
      dispatchHistorySessionOpenIntent(sessionId, getTitle(session));
      recordHistorySessionDiagnosticEvent(sessionId, 'history_open_intent_source', {
        source,
      });
      return 'dispatched';
    },
    [activeSessionId, runningSessionIds],
  );

  const handleSwitch = useCallback(
    async (sessionId: string) => {
      if (editingSessionId) return;
      try {
        const session = flowChatStore.getState().sessions.get(sessionId);
        const historyOpenIntentDispatch = session
          ? dispatchHistoryOpenIntentForSession(session, 'switch')
          : 'none';
        if (session && historyOpenIntentDispatch !== 'none') {
          flowChatManager.preloadHistoricalSessionForOpen(sessionId);
        }
        const relationship = resolveSessionRelationship(session);
        const parentSessionId = relationship.parentSessionId;
        const matchingScope = session && workspaceScopes?.find(scope => sessionBelongsToWorkspaceNavRow(
          session,
          scope.workspacePath,
          scope.remoteConnectionId,
          scope.remoteSshHost,
        ));
        const targetWorkspaceId = matchingScope?.workspaceId ?? workspaceId;
        const mustActivateWorkspace =
          Boolean(targetWorkspaceId) && targetWorkspaceId !== currentWorkspace?.id;
        const activateWorkspace = mustActivateWorkspace
          ? async (targetWorkspaceId: string) => {
              await setActiveWorkspace(targetWorkspaceId);
            }
          : undefined;

        const parentSession = parentSessionId
          ? flowChatStore.getState().sessions.get(parentSessionId)
          : undefined;
        if (relationship.canOpenInAuxPane && parentSessionId && parentSession && session) {
          await openMainSession(parentSessionId, {
            workspaceId: targetWorkspaceId,
            activateWorkspace,
          });
          openBtwSessionInAuxPane({
            childSessionId: sessionId,
            parentSessionId,
            workspacePath: session.workspacePath,
          });
          return;
        }

        if (sessionId === activeSessionId) {
          await openMainSession(sessionId, {
            workspaceId: targetWorkspaceId,
            activateWorkspace,
          });
          return;
        }

        await openMainSession(sessionId, {
          workspaceId: targetWorkspaceId,
          activateWorkspace,
        });
        window.dispatchEvent(
          new CustomEvent('flowchat:switch-session', { detail: { sessionId } })
        );
      } catch (err) {
        log.error('Failed to switch session', err);
      }
    },
    [
      activeSessionId,
      dispatchHistoryOpenIntentForSession,
      editingSessionId,
      setActiveWorkspace,
      workspaceId,
      workspaceScopes,
      currentWorkspace?.id,
    ]
  );

  const handleSessionOpenPointerDown = useCallback(
    (event: React.PointerEvent<HTMLElement>, session: Session) => {
      if (editingSessionId || session.sessionId === activeSessionId) {
        return;
      }
      if (event.button !== 0) {
        return;
      }

      const target = event.target as HTMLElement | null;
      if (target?.closest('.bitfun-nav-panel__inline-item-actions, .bitfun-nav-panel__inline-item-edit')) {
        return;
      }

      const historyOpenIntentDispatch = dispatchHistoryOpenIntentForSession(session, 'pointerdown');
      if (historyOpenIntentDispatch !== 'none') {
        flowChatManager.preloadHistoricalSessionForOpen(session.sessionId);
      }
    },
    [activeSessionId, dispatchHistoryOpenIntentForSession, editingSessionId],
  );

  const resolveSessionTitle = useCallback(
    (session: Session): string => {
      const rawTitle = getTitle(session);
      const newSessionPrefixes = Array.from(
        new Set([
          t('nav.sessions.newSession'),
          i18nService.t('nav.sessions.newSession', { lng: 'en-US' }),
          i18nService.t('nav.sessions.newSession', { lng: 'zh-CN' }),
          i18nService.t('nav.sessions.newSession', { lng: 'zh-TW' }),
        ].filter((value): value is string => Boolean(value)))
      );
      const matched = rawTitle.match(
        new RegExp(`^(?:${newSessionPrefixes.map(escapeRegExp).join('|')})\\s*(\\d+)$`, 'i')
      );
      if (!matched) return rawTitle;

      const mode = resolveSessionModeType(session);
      const label =
        mode === 'claw'
          ? t('nav.sessions.newClawSession')
          : t('nav.sessions.newSession');
      return `${label} ${matched[1]}`;
    },
    [t]
  );

  const handleMenuOpen = useCallback(
    (e: React.MouseEvent, sessionId: string) => {
      e.stopPropagation();
      if (openMenuSessionId === sessionId) {
        closeSessionMenu();
        return;
      }
      const btn = e.currentTarget as HTMLElement;
      const rect = btn.getBoundingClientRect();
      const { top, left } = computeFixedPopoverPosition(rect, 160, 120, 4, 8);
      setSessionMenuPosition({ top, left });
      setIsExportScopeMenu(false);
      setOpenMenuSessionId(sessionId);
    },
    [closeSessionMenu, openMenuSessionId]
  );

  const handleExportMarkdown = useCallback(
    async (e: React.MouseEvent, session: Session, scope: TranscriptExportScope) => {
      e.stopPropagation();
      closeSessionMenu();
      if (exportingSessionId) return;

      setExportingSessionId(session.sessionId);
      try {
        await exportSessionToMarkdown(
          {
            sessionId: session.sessionId,
            title: resolveSessionTitle(session),
            workspacePath:
              session.projectWorkspacePath
              || session.config.projectWorkspacePath
              || session.workspacePath
              || workspacePath,
            // The nav row carries the workspace's current connection; a session's
            // stored ids can be stale (e.g. after an SSH port change).
            remoteConnectionId: remoteConnectionId ?? session.remoteConnectionId,
            remoteSshHost: remoteSshHost ?? session.remoteSshHost,
          },
          scope
        );
      } finally {
        setExportingSessionId(null);
      }
    },
    [
      closeSessionMenu,
      exportingSessionId,
      remoteConnectionId,
      remoteSshHost,
      resolveSessionTitle,
      workspacePath,
    ]
  );

  const handleDelete = useCallback(
    async (e: React.MouseEvent, sessionId: string) => {
      e.stopPropagation();
      try {
        await flowChatManager.deleteChatSession(sessionId);
      } catch (err) {
        log.error('Failed to delete session', err);
      }
    },
    []
  );

  const handleArchive = useCallback(
    async (e: React.MouseEvent, sessionId: string) => {
      e.stopPropagation();
      const confirmed = await confirmWarning(
        t('nav.sessions.archiveConfirmTitle'),
        t('nav.sessions.archiveConfirmMessage')
      );
      if (!confirmed) return;
      try {
        await flowChatManager.archiveChatSession(sessionId);
        window.dispatchEvent(new CustomEvent('bitfun:session-archived'));
      } catch (err) {
        log.error('Failed to archive session', err);
      }
    },
    [t]
  );

  const handleCopySessionId = useCallback(
    async (e: React.MouseEvent, sessionId: string) => {
      e.stopPropagation();
      const copied = await copyTextToClipboard(sessionId);
      if (copied) {
        notificationService.success(t('nav.sessions.copySessionIdSuccess'), { duration: 2000 });
      } else {
        notificationService.error(t('nav.sessions.copySessionIdFailed'), { duration: 3000 });
      }
    },
    [t]
  );

  const handleStartEdit = useCallback(
    (e: React.MouseEvent, session: Session) => {
      e.stopPropagation();
      setEditingSessionId(session.sessionId);
      setEditingTitle(resolveSessionTitle(session));
    },
    [resolveSessionTitle]
  );

  const handleConfirmEdit = useCallback(async () => {
    if (!editingSessionId) return;
    const trimmed = editingTitle.trim();
    if (trimmed) {
      try {
        await flowChatManager.renameChatSessionTitle(editingSessionId, trimmed);
      } catch (err) {
        log.error('Failed to update session title', { sessionId: editingSessionId, error: err });
        if (isOutcomeUnknownError(err)) {
          notificationService.warning(t('nav.sessions.renameOutcomeUnknown'), { duration: 6000 });
          try {
            await flowChatManager.reloadSessionTitle(editingSessionId);
          } catch (refreshError) {
            log.error('Failed to reload the session title after an unknown rename outcome', {
              sessionId: editingSessionId,
              error: refreshError,
            });
          }
        }
      }
    }
    setEditingSessionId(null);
    setEditingTitle('');
  }, [editingSessionId, editingTitle, t]);

  const handleCancelEdit = useCallback(() => {
    setEditingSessionId(null);
    setEditingTitle('');
  }, []);

  const handleEditKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        handleConfirmEdit();
      } else if (e.key === 'Escape') {
        e.preventDefault();
        handleCancelEdit();
      }
    },
    [handleConfirmEdit, handleCancelEdit]
  );

  const handleExpandToggle = useCallback(async () => {
    if (metadataPageState.isLoading) {
      return;
    }

    const loadedTopLevelCount = topLevelSessions.length;
    const total = totalTopLevelSessionCount;

    if (expandLevel === 0) {
      const targetCount = Math.min(total, SESSIONS_LEVEL_1);
      if (
        loadedTopLevelCount < targetCount &&
        hasMoreUnloadedSessions &&
        metadataPageState.nextCursor
      ) {
        await loadMetadataPage(
          targetCount - loadedTopLevelCount,
          metadataPageState.nextCursor,
          'sessions_nav_expand_level_1'
        );
      }
      setExpandLevel(1);
      return;
    }

    if (expandLevel === 1 && total > SESSIONS_LEVEL_1) {
      if (
        loadedTopLevelCount < total &&
        hasMoreUnloadedSessions &&
        metadataPageState.nextCursor
      ) {
        await loadMetadataPage(
          total - loadedTopLevelCount,
          metadataPageState.nextCursor,
          'sessions_nav_expand_all'
        );
      }
      setExpandLevel(2);
      return;
    }

    setExpandLevel(0);
  }, [
    expandLevel,
    hasMoreUnloadedSessions,
    loadMetadataPage,
    metadataPageState.isLoading,
    metadataPageState.nextCursor,
    topLevelSessions.length,
    totalTopLevelSessionCount,
  ]);

  if (allTopLevelSessions.length === 0) {
    if (metadataPageState.isLoading) {
      return (
        <div data-bf-component="sessions-section" data-bf-part="root" className={sessionListClassName}>
          <div className="bitfun-nav-panel__inline-loading" data-bf-component="sessions-section" data-bf-part="loading" data-bf-state="loading">
            <Loader2 size={12} />
            <span>{t('nav.sessions.loading')}</span>
          </div>
        </div>
      );
    }
    if (metadataPageState.loadError) {
      return (
        <div data-bf-component="sessions-section" data-bf-part="root" className={sessionListClassName}>
          <button
            type="button"
            className="bitfun-nav-panel__inline-action"
            data-bf-component="sessions-section"
            data-bf-part="retry"
            onClick={() => {
              void loadInitialMetadataPage('sessions_nav_manual_retry');
            }}
          >
            <span>{t('nav.sessions.loadFailedRetry')}</span>
          </button>
        </div>
      );
    }
    return (
      <div className={sessionListClassName}>
        {presentation?.kind === 'assistant' ? (
          <div className="bitfun-nav-panel__inline-empty is-assistant" aria-disabled="true">
            <AssistantAvatar
              presetId={presentation.assistant.avatar}
              emoji={presentation.assistant.emoji}
              stableKey={presentation.assistant.id}
              name={presentation.assistant.name}
              size={26}
            />
            <span className="bitfun-nav-panel__inline-empty-copy">
              <span className="bitfun-nav-panel__inline-empty-name">{presentation.assistant.name}</span>
              <span>{t('nav.sessions.noSessions')}</span>
            </span>
          </div>
        ) : (
          <div className="bitfun-nav-panel__inline-empty" aria-disabled="true">
            {t('nav.sessions.noSessions')}
          </div>
        )}
      </div>
    );
  }

  if (topLevelSessions.length === 0) {
    return (
      <div className={sessionListClassName}>
        <div className="bitfun-nav-panel__inline-empty" aria-disabled="true">
          {t('nav.sessions.viewMenu.noMatches')}
        </div>
      </div>
    );
  }

  return (
    <div className={sessionListClassName} ref={sessionListRef}>
      {visibleItems.map(({ session, level }) => {
          const isEditing = editingSessionId === session.sessionId;
          const relationship = resolveSessionRelationship(session);
          const isChildSession = level === 1 && relationship.displayAsChild;
          const childSessionBadge = getChildSessionBadge(relationship.kind);
          const parentReviewActivity = deriveSessionReviewActivity(
            flowChatState,
            session.sessionId,
            id => stateMachineManager.getCurrentState(id),
          );
          const showParentReviewActivity = !isChildSession && isReviewActivityBlocking(parentReviewActivity);
          const showChildReviewActivity =
            isChildSession && relationship.isReview && runningSessionIds.has(session.sessionId);
          const reviewActivityKind =
            showParentReviewActivity
              ? parentReviewActivity!.kind
              : showChildReviewActivity && (relationship.kind === 'review' || relationship.kind === 'deep_review')
                ? relationship.kind
                : null;
          const sessionTitle = resolveSessionTitle(session);
          const sessionWorkspaceScope = workspaceScopes?.find(scope => sessionBelongsToWorkspaceNavRow(
            session,
            scope.workspacePath,
            scope.remoteConnectionId,
            scope.remoteSshHost,
          ));
          const isRunning = runningSessionIds.has(session.sessionId);
          const isWaitingForUserAnswer = isRunning && hasPendingAskUserQuestion(
            resolveTrackedTurn(session),
          );
          const backgroundSubagentActivity = !isChildSession
            ? backgroundSubagentActivityByParent.get(session.sessionId)
            : undefined;
          const backgroundSubagentActivityCount = backgroundSubagentActivity?.totalCount ?? 0;
          const showBackgroundSubagentActivity = !isChildSession && backgroundSubagentActivityCount > 0;
          const parentSessionId = relationship.parentSessionId;
          const parentSession = parentSessionId ? flowChatState.sessions.get(parentSessionId) : undefined;
          const parentTitle = parentSession ? resolveSessionTitle(parentSession) : '';
          const parentTurnIndex = relationship.origin?.parentTurnIndex;
          const assistantIdentity = presentation?.kind === 'assistant'
            ? presentation.assistant
            : null;
          const trimmedAssistant = assistantIdentity?.name.trim() ?? '';
          const showAssistantInTooltip = trimmedAssistant.length > 0;
          const dispatchTarget = session.config.dispatchTarget;
          const isDispatched = isNonLocalDispatchTarget(dispatchTarget);
          const dispatchTargetLabel =
            dispatchTarget?.kind === 'ssh' || dispatchTarget?.kind === 'device'
              ? dispatchTarget.displayName
              : '';
          const dispatchState = session.config.dispatchJobState ?? 'submitting';
          const dispatchStateLabel = {
            submitting: t('nav.sessions.dispatchStates.submitting'),
            submission_unknown: t('nav.sessions.dispatchStates.submission_unknown'),
            queued: t('nav.sessions.dispatchStates.queued'),
            running: t('nav.sessions.dispatchStates.running'),
            succeeded: t('nav.sessions.dispatchStates.succeeded'),
            failed: t('nav.sessions.dispatchStates.failed'),
            cancelled: t('nav.sessions.dispatchStates.cancelled'),
          } satisfies Record<DispatchJobState, string>;
          const dispatchTransport = session.config.dispatchJobId
            ? dispatchTransportByJobId[session.config.dispatchJobId]
            : undefined;
          const dispatchPresentation = isDispatched
            ? resolveDispatchNavPresentation({
                targetLabel: dispatchTargetLabel,
                state: dispatchState,
                reachability: dispatchTransport?.reachability,
                runningSummary: t('nav.sessions.dispatchRunningOn', {
                  target: dispatchTargetLabel,
                  state: dispatchStateLabel[dispatchState],
                }),
                unreachableLabel: t('nav.sessions.dispatchUnreachable'),
                unreachableSummary: t('nav.sessions.dispatchUnreachableDetails', {
                  target: dispatchTargetLabel,
                }),
              })
            : null;
          const showRichTooltip =
            Boolean(sessionWorkspaceScope) ||
            showAssistantInTooltip ||
            isChildSession ||
            showBackgroundSubagentActivity ||
            isDispatched;
          const tooltipContent = showRichTooltip ? (
            <div className="bitfun-nav-panel__inline-item-tooltip">
              <div className="bitfun-nav-panel__inline-item-tooltip-title">{sessionTitle}</div>
              {sessionWorkspaceScope ? (
                <div className="bitfun-nav-panel__inline-item-tooltip-meta">
                  {sessionWorkspaceScope.workspaceName}
                </div>
              ) : null}
              {showAssistantInTooltip ? (
                <div className="bitfun-nav-panel__inline-item-tooltip-meta">
                  {t('nav.sessions.assistantOwner', { name: trimmedAssistant })}
                </div>
              ) : null}
              {isChildSession ? (
                <div className="bitfun-nav-panel__inline-item-tooltip-meta">
                  {parentTurnIndex
                    ? t('nav.sessions.childSourceWithTurn', {
                        parentTitle: parentTitle || t('nav.sessions.parentSession'),
                        turnIndex: parentTurnIndex,
                      })
                    : t('nav.sessions.childSourceWithoutTurn', {
                        parentTitle: parentTitle || t('nav.sessions.parentSession'),
                  })}
                </div>
              ) : null}
              {isDispatched ? (
                <div className="bitfun-nav-panel__inline-item-tooltip-meta">
                  {dispatchPresentation?.summary}
                </div>
              ) : null}
              {showBackgroundSubagentActivity && backgroundSubagentActivity ? (
                <>
                  <div className="bitfun-nav-panel__inline-item-tooltip-meta">
                    {t('nav.sessions.backgroundSubagentsRunning', {
                      count: backgroundSubagentActivityCount,
                    })}
                  </div>
                  {backgroundSubagentActivity.items.length > 0 ? (
                    <div className="bitfun-nav-panel__inline-item-tooltip-meta">
                      {backgroundSubagentActivity.items
                        .slice(0, 2)
                        .map(item => item.title)
                        .join(' · ')}
                    </div>
                  ) : null}
                </>
              ) : null}
            </div>
          ) : (
            sessionTitle
          );
          const isRowActive = isSessionNavRowActive({
            rowSessionId: session.sessionId,
            activeTabId,
            activeSessionId,
            activeChildSessionId: activeBtwSessionData?.childSessionId,
            activeChildParentSessionId: activeBtwSessionData?.parentSessionId,
          });
          // Determine the notification state for this session row.
          // Priority: needsUserAttention > hasUnreadCompletion.
          const attentionKind = !isRunning && !isRowActive
            ? (session.needsUserAttention || session.hasUnreadCompletion || undefined)
            : undefined;
          const hiddenAttentionLabel =
            attentionKind === 'error'
              ? t('nav.sessions.unreadError')
              : attentionKind === 'interrupted'
                ? t('nav.sessions.unreadInterrupted')
                : attentionKind && attentionKind !== 'ask_user' && attentionKind !== 'tool_confirm'
                  ? t('nav.sessions.unreadCompleted')
                  : null;
          const assistantAvatarStatus: AssistantAvatarStatus = isRunning
            ? isWaitingForUserAnswer ? 'attention' : 'running'
            : attentionKind === 'error' || attentionKind === 'interrupted'
              ? 'error'
              : attentionKind === 'ask_user' || attentionKind === 'tool_confirm'
                ? 'attention'
                : attentionKind
                  ? 'unread'
                  : 'idle';
          const showAssistantIdentity = Boolean(assistantIdentity && !isChildSession);
          const row = (
            <div
              className={[
                'bitfun-nav-panel__inline-item',
                showAssistantIdentity && 'is-assistant-session',
                level === 1 && 'is-child',
                isChildSession && 'is-btw-child',
                isRowActive && 'is-active',
                isEditing && 'is-editing',
                openMenuSessionId === session.sessionId && 'is-menu-open',
              ]
                .filter(Boolean)
                .join(' ')}
              data-bf-component="sessions-section"
              data-bf-part="row"
              data-bf-state={[
                isRowActive && 'active',
                isEditing && 'editing',
                openMenuSessionId === session.sessionId && 'menuOpen',
              ].filter(Boolean).join(' ') || undefined}
              data-testid="nav-session-item"
              data-session-id={session.sessionId}
              data-session-kind={relationship.kind}
              data-session-level={String(level)}
              data-session-active={isRowActive ? 'true' : 'false'}
              onPointerDown={event => handleSessionOpenPointerDown(event, session)}
              onClick={() => handleSwitch(session.sessionId)}
            >
              {showAssistantIdentity && assistantIdentity ? (
                <span className="bitfun-nav-panel__inline-item-avatar" data-bf-component="sessions-section" data-bf-part="assistantAvatar">
                  <AssistantAvatar
                    presetId={assistantIdentity.avatar}
                    emoji={assistantIdentity.emoji}
                    stableKey={assistantIdentity.id}
                    name={assistantIdentity.name}
                    size={26}
                    status={assistantAvatarStatus}
                    active={isRowActive}
                  />
                </span>
              ) : null}

              {isWaitingForUserAnswer ? (
                <span className="sr-only">{t('nav.sessions.needsUserInput')}</span>
              ) : null}
              {hiddenAttentionLabel ? (
                <span className="sr-only">{hiddenAttentionLabel}</span>
              ) : null}

              {isEditing ? (
                <div className="bitfun-nav-panel__inline-item-edit" data-bf-component="sessions-section" data-bf-part="edit" onClick={e => e.stopPropagation()}>
                  <Input
                    ref={editInputRef}
                    className="bitfun-nav-panel__inline-item-edit-field"
                    variant="default"
                    inputSize="small"
                    value={editingTitle}
                    onChange={e => setEditingTitle(e.target.value)}
                    onKeyDown={handleEditKeyDown}
                    onBlur={handleConfirmEdit}
                  />
                  <IconButton
                    variant="success"
                    size="xs"
                    className="bitfun-nav-panel__inline-item-edit-btn confirm"
                    onClick={e => { e.stopPropagation(); handleConfirmEdit(); }}
                    tooltip={t('nav.sessions.confirmEdit')}
                    tooltipPlacement="top"
                  >
                    <Check size={11} />
                  </IconButton>
                  <IconButton
                    variant="default"
                    size="xs"
                    className="bitfun-nav-panel__inline-item-edit-btn cancel"
                    onMouseDown={e => { e.preventDefault(); e.stopPropagation(); handleCancelEdit(); }}
                    tooltip={t('nav.sessions.cancelEdit')}
                    tooltipPlacement="top"
                  >
                    <X size={11} />
                  </IconButton>
                </div>
              ) : (
                <>
                  <span className="bitfun-nav-panel__inline-item-main" data-bf-component="sessions-section" data-bf-part="rowMain">
                    <span className="bitfun-nav-panel__inline-item-copy">
                      <span className="bitfun-nav-panel__inline-item-primary">
                        <span className="bitfun-nav-panel__inline-item-label">{sessionTitle}</span>
                    {isChildSession ? (
                      <span className="bitfun-nav-panel__inline-item-btw-badge">{childSessionBadge}</span>
                    ) : null}
                    {isDispatched ? (
                      <span
                        className="bitfun-nav-panel__inline-item-dispatch-badge"
                        data-state={dispatchPresentation?.visualState}
                        title={dispatchPresentation?.summary}
                      >
                        {dispatchPresentation?.badgeLabel}
                      </span>
                    ) : null}
                    {attentionKind === 'ask_user' || attentionKind === 'tool_confirm' ? (
                      <span className="bitfun-nav-panel__inline-item-attention-badge">
                        {attentionKind === 'ask_user'
                          ? t('nav.sessions.badgeNeedsInput')
                          : t('nav.sessions.badgeNeedsConfirm')}
                      </span>
                    ) : null}
                    {reviewActivityKind ? (
                      <span className="bitfun-nav-panel__inline-item-review-badge">
                        <Loader2 size={9} aria-hidden />
                        {getReviewActivityBadge(reviewActivityKind)}
                      </span>
                    ) : null}
                        {showBackgroundSubagentActivity ? (
                      <span
                        className="bitfun-nav-panel__inline-item-background-subagent-badge"
                        aria-label={t('nav.sessions.backgroundSubagentsRunning', {
                          count: backgroundSubagentActivityCount,
                        })}
                      >
                        <Bot
                          className="bitfun-nav-panel__inline-item-background-subagent-icon is-bot"
                          size={10}
                          aria-hidden
                        />
                        <Loader2
                          className="bitfun-nav-panel__inline-item-background-subagent-icon is-loader"
                          size={10}
                          aria-hidden
                        />
                      </span>
                        ) : null}
                        {sessionWorkspaceScope && !isChildSession ? (
                          <span className="bitfun-nav-panel__inline-item-workspace-name">
                            {sessionWorkspaceScope.workspaceName}
                          </span>
                        ) : null}
                      </span>
                      {showAssistantIdentity ? (
                        <span className="bitfun-nav-panel__inline-item-assistant-name">{trimmedAssistant}</span>
                      ) : null}
                    </span>
                  </span>
                  <div
                    className={`bitfun-nav-panel__inline-item-actions${openMenuSessionId === session.sessionId ? ' is-open' : ''}`}
                    data-bf-component="sessions-section"
                    data-bf-part="actions"
                    data-bf-state={openMenuSessionId === session.sessionId ? 'menuOpen' : undefined}
                  >
                    <button
                      type="button"
                      ref={openMenuSessionId === session.sessionId ? sessionMenuAnchorRef : undefined}
                      className={`bitfun-nav-panel__inline-item-action-btn${openMenuSessionId === session.sessionId ? ' is-open' : ''}`}
                      onClick={e => handleMenuOpen(e, session.sessionId)}
                      data-testid="nav-session-menu-btn"
                      data-session-id={session.sessionId}
                    >
                      <MoreHorizontal size={13} />
                    </button>
                  </div>
                  {openMenuSessionId === session.sessionId && sessionMenuPosition && createPortal(
                    <div
                      ref={sessionMenuPopoverRef}
                      className="bitfun-nav-panel__inline-item-menu-popover"
                      data-bf-component="sessions-section"
                      data-bf-part="menu"
                      data-bf-state="menuOpen"
                      role="menu"
                      style={{ top: `${sessionMenuPosition.top}px`, left: `${sessionMenuPosition.left}px` }}
                      data-testid="nav-session-menu"
                      data-session-id={session.sessionId}
                    >
                      {isExportScopeMenu ? (
                        <>
                          <button
                            type="button"
                            className="bitfun-nav-panel__inline-item-menu-item"
                            onClick={e => {
                              e.stopPropagation();
                              setIsExportScopeMenu(false);
                            }}
                            data-testid="nav-session-menu-export-back"
                            data-session-id={session.sessionId}
                          >
                            <ChevronLeft size={13} />
                            <span>{t('nav.sessions.exportMarkdown')}</span>
                          </button>
                          <button
                            type="button"
                            className="bitfun-nav-panel__inline-item-menu-item"
                            onClick={e => { void handleExportMarkdown(e, session, 'full'); }}
                            data-testid="nav-session-menu-export-full"
                            data-session-id={session.sessionId}
                          >
                            <FileDown size={13} />
                            <span>{t('nav.sessions.exportMarkdownFull')}</span>
                          </button>
                          <button
                            type="button"
                            className="bitfun-nav-panel__inline-item-menu-item"
                            onClick={e => { void handleExportMarkdown(e, session, 'result'); }}
                            data-testid="nav-session-menu-export-result"
                            data-session-id={session.sessionId}
                          >
                            <FileDown size={13} />
                            <span>{t('nav.sessions.exportMarkdownResult')}</span>
                          </button>
                        </>
                      ) : (
                        <>
                          <button
                            type="button"
                            className="bitfun-nav-panel__inline-item-menu-item"
                            onClick={e => { closeSessionMenu(); handleStartEdit(e, session); }}
                            data-testid="nav-session-menu-rename"
                            data-session-id={session.sessionId}
                          >
                            <Pencil size={13} />
                            <span>{t('nav.sessions.rename')}</span>
                          </button>
                          <button
                            type="button"
                            className="bitfun-nav-panel__inline-item-menu-item"
                            onClick={e => { closeSessionMenu(); void handleCopySessionId(e, session.sessionId); }}
                            data-testid="nav-session-menu-copy-id"
                            data-session-id={session.sessionId}
                          >
                            <Copy size={13} />
                            <span>{t('nav.sessions.copySessionId')}</span>
                          </button>
                          <button
                            type="button"
                            className="bitfun-nav-panel__inline-item-menu-item"
                            onClick={e => {
                              e.stopPropagation();
                              setIsExportScopeMenu(true);
                            }}
                            disabled={exportingSessionId === session.sessionId}
                            data-testid="nav-session-menu-export-markdown"
                            data-session-id={session.sessionId}
                          >
                            {exportingSessionId === session.sessionId
                              ? <Loader2 size={13} className="bitfun-nav-panel__inline-toggle-spinner" />
                              : <FileDown size={13} />}
                            <span>{t('nav.sessions.exportMarkdown')}</span>
                          </button>
                          <button
                            type="button"
                            className="bitfun-nav-panel__inline-item-menu-item"
                            onClick={e => {
                              e.stopPropagation();
                              closeSessionMenu();
                              setScheduledJobsSessionId(session.sessionId);
                            }}
                            disabled={!workspacePath}
                            data-testid="nav-session-menu-scheduled-jobs"
                            data-session-id={session.sessionId}
                          >
                            <Clock3 size={13} />
                            <span>{t('nav.scheduledJobs.open')}</span>
                          </button>
                          <button
                            type="button"
                            className="bitfun-nav-panel__inline-item-menu-item"
                            onClick={e => { closeSessionMenu(); void handleArchive(e, session.sessionId); }}
                            data-testid="nav-session-menu-archive"
                            data-session-id={session.sessionId}
                          >
                            <Archive size={13} />
                            <span>{t('nav.sessions.archive')}</span>
                          </button>
                          <button
                            type="button"
                            className="bitfun-nav-panel__inline-item-menu-item is-danger"
                            onClick={e => { closeSessionMenu(); void handleDelete(e, session.sessionId); }}
                            data-testid="nav-session-menu-delete"
                            data-session-id={session.sessionId}
                          >
                            <Trash2 size={13} />
                            <span>{t('nav.sessions.delete')}</span>
                          </button>
                        </>
                      )}
                    </div>,
                    getAppearanceOverlayHost()
                  )}
                </>
              )}
            </div>
          );
          // Always wrapped, even while editing or with a row menu open: swapping
          // the wrapper out would change every row's element type, remounting
          // the whole list (and flashing it) on each menu open/close.
          return (
            <Tooltip
              key={session.sessionId}
              content={tooltipContent}
              placement="right"
              followCursor
              disabled={isEditing || openMenuSessionId !== null}
            >
              {row}
            </Tooltip>
          );
        })}

      {expandLevel === 2 && topLevelSessions.length > sessionDisplayLimit && (
        <button
          type="button"
          className="bitfun-nav-panel__inline-toggle"
          data-testid="nav-session-list-load-more"
          onClick={() => setLevel2DisplayCount(prev => prev + SESSIONS_LEVEL_2_PAGE)}
        >
          <span className="bitfun-nav-panel__inline-toggle-dots">···</span>
          <span>
            {t('nav.sessions.showMore', {
              count: topLevelSessions.length - sessionDisplayLimit,
            })}
          </span>
        </button>
      )}

      {expandToggleState.shouldRender && (
        <button
          type="button"
          className={`bitfun-nav-panel__inline-toggle${metadataPageState.isLoading ? ' is-loading' : ''}`}
          data-bf-component="sessions-section"
          data-bf-part="toggle"
          data-bf-state={metadataPageState.isLoading ? 'loading' : undefined}
          data-testid="nav-session-list-toggle"
          data-session-nav-toggle-action={expandToggleState.action}
          disabled={metadataPageState.isLoading}
          onClick={() => { void handleExpandToggle(); }}
        >
          {expandLevel === 0 ? (
            <>
              {metadataPageState.isLoading ? (
                <Loader2 size={12} className="bitfun-nav-panel__inline-toggle-spinner" />
              ) : (
                <span className="bitfun-nav-panel__inline-toggle-dots">···</span>
              )}
              <span>
                {t('nav.sessions.showMore', {
                  count: expandToggleState.collapsedRemainingCount,
                })}
              </span>
            </>
          ) : expandLevel === 1 && expandToggleState.expandedRemainingCount > 0 ? (
            <>
              {metadataPageState.isLoading ? (
                <Loader2 size={12} className="bitfun-nav-panel__inline-toggle-spinner" />
              ) : (
                <span className="bitfun-nav-panel__inline-toggle-dots">···</span>
              )}
              <span>
                {t('nav.sessions.showAll', {
                  count: expandToggleState.expandedRemainingCount,
                })}
              </span>
            </>
          ) : (
            <span>{t('nav.sessions.showLess')}</span>
          )}
        </button>
      )}

      {scheduledJobsSession && (
        <Suspense fallback={null}>
          <ScheduledJobsModal
            isOpen={scheduledJobsSession != null}
            onClose={() => setScheduledJobsSessionId(null)}
            workspacePath={scheduledJobsSession.workspacePath || workspacePath}
            workspaceId={scheduledJobsSession.workspaceId || workspaceId}
            remoteConnectionId={scheduledJobsSession.remoteConnectionId || remoteConnectionId}
            remoteSshHost={scheduledJobsSession.remoteSshHost || remoteSshHost}
            sessionId={scheduledJobsSession.sessionId}
            targetKind="session"
            lockSessionId
            title={t('nav.scheduledJobs.title')}
            targetLabel={resolveSessionTitle(scheduledJobsSession)}
            targetDescription={scheduledJobsSession.workspacePath || workspacePath}
          />
        </Suspense>
      )}
    </div>
  );
};

export default SessionsSection;
