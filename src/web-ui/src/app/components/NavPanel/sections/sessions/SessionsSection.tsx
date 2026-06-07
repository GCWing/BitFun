/**
 * SessionsSection — inline accordion content for the "Sessions" nav item.
 *
 * Rendered inside NavPanel when the Sessions item is expanded.
 * Owns all data fetching / mutation for chat sessions.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Pencil, Trash2, Check, X, Bot, Code2, ClipboardList, Panda, MoreHorizontal, Loader2, Archive, Clock3 } from 'lucide-react';
import { IconButton, Input, Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { flowChatStore } from '../../../../../flow_chat/store/FlowChatStore';
import { flowChatManager } from '../../../../../flow_chat/services/FlowChatManager';
import type { FlowChatState, Session } from '../../../../../flow_chat/types/flow-chat';
import { useSceneStore } from '../../../../stores/sceneStore';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';
import { useAgentCanvasStore } from '@/app/components/panels/content-canvas/stores';
import {
  openBtwSessionInAuxPane,
  openMainSession,
  selectActiveBtwSessionTab,
} from '@/flow_chat/services/openBtwSession';
import {
  dispatchHistorySessionOpenIntent,
  shouldShowHistorySessionOpenIntent,
} from '@/flow_chat/services/sessionOpenIntent';
import { resolveSessionRelationship } from '@/flow_chat/utils/sessionMetadata';
import {
  compareSessionsForNavStable,
  sessionBelongsToWorkspaceNavRow,
} from '@/flow_chat/utils/sessionOrdering';
import { stateMachineManager } from '@/flow_chat/state-machine';
import { SessionExecutionState } from '@/flow_chat/state-machine/types';
import { i18nService } from '@/infrastructure/i18n';
import { resolveSessionTitle } from '@/flow_chat/utils/sessionTitle';
import { isSessionNavRowActive } from './sessionNavSelection';
import {
  deriveSessionReviewActivity,
  isReviewActivityBlocking,
} from '@/flow_chat/utils/sessionReviewActivity';
import { computeFixedPopoverPosition } from '@/shared/utils/fixedPopoverViewport';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import { confirmWarning } from '@/component-library/components/ConfirmDialog/confirmService';
import ScheduledJobsModal from '@/app/components/scheduled-jobs/ScheduledJobsModal';
import { scheduleAfterStartupPaint, scheduleAfterStartupSignal } from '@/shared/utils/startupTaskScheduling';
import {
  SESSION_METADATA_DEFERRED_FALLBACK_MS,
  SESSION_METADATA_DEFERRED_FRAME_COUNT,
  SESSION_METADATA_DEFERRED_SIGNAL,
  getDeferredSessionMetadataDelayMs,
  getInitialSessionMetadataLoadMode,
  hasStartupOverlayHandedOff,
} from './sessionMetadataStartup';
import './SessionsSection.scss';

/** Top-level parent sessions shown at each expand step (children still nest under visible parents). */
const SESSIONS_LEVEL_0 = 5;
const SESSIONS_LEVEL_1 = 10;
const log = createLogger('SessionsSection');

type SessionMode = 'code' | 'cowork' | 'claw';

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

const waitForHistoryOpenIntentPaint = (): Promise<void> =>
  new Promise(resolve => {
    if (typeof window === 'undefined' || typeof window.requestAnimationFrame !== 'function') {
      globalThis.setTimeout(resolve, 0);
      return;
    }

    window.requestAnimationFrame(() => {
      window.requestAnimationFrame(() => resolve());
    });
  });

const getChildSessionBadge = (kind: Session['sessionKind']): string => {
  const normalizedKind =
    kind === 'review' || kind === 'deep_review' || kind === 'subagent'
      ? kind
      : 'btw';
  const fallback = normalizedKind === 'deep_review'
    ? 'Deep'
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
      defaultValue: kind === 'deep_review' ? 'Deep reviewing' : 'Reviewing',
    },
  );

interface SessionsSectionProps {
  workspaceId?: string;
  workspacePath?: string;
  /** Remote SSH: same `workspacePath` on different hosts must filter by this (see Session.remoteConnectionId). */
  remoteConnectionId?: string | null;
  /** Remote SSH: disambiguates same path on different hosts; when set with matching session host, connectionId may differ. */
  remoteSshHost?: string | null;
  isActiveWorkspace?: boolean;
  showCreateActions?: boolean;
  /** When set (e.g. assistant workspace), session row tooltip includes this assistant name. */
  assistantLabel?: string;
  /** When false, hide the leading mode / running icon on each row (e.g. assistant detail page). */
  showSessionModeIcon?: boolean;
  /** Prevents startup metadata fetching while the surrounding section is collapsed. */
  isVisible?: boolean;
}

const SessionsSection: React.FC<SessionsSectionProps> = ({
  workspaceId,
  workspacePath,
  remoteConnectionId = null,
  remoteSshHost = null,
  isActiveWorkspace = true,
  assistantLabel,
  showSessionModeIcon = true,
  isVisible = true,
}) => {
  const { t } = useI18n('common');
  const { setActiveWorkspace, currentWorkspace } = useWorkspaceContext();
  const activeTabId = useSceneStore(s => s.activeTabId);
  const activeBtwSessionTab = useAgentCanvasStore(state => selectActiveBtwSessionTab(state as any));
  const activeBtwSessionData = activeBtwSessionTab?.content.data as
    | { childSessionId: string; parentSessionId: string; workspacePath?: string }
    | undefined;
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() =>
    flowChatStore.getState()
  );
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState('');
  const [expandLevel, setExpandLevel] = useState<0 | 1 | 2>(0);
  const [metadataPageState, setMetadataPageState] = useState<{
    totalTopLevelCount: number | null;
    nextCursor?: string;
    hasMore: boolean;
    isLoading: boolean;
  }>({
    totalTopLevelCount: null,
    nextCursor: undefined,
    hasMore: false,
    isLoading: false,
  });
  const [openMenuSessionId, setOpenMenuSessionId] = useState<string | null>(null);
  const [sessionMenuPosition, setSessionMenuPosition] = useState<{ top: number; left: number } | null>(null);
  const [runningSessionIds, setRunningSessionIds] = useState<Set<string>>(new Set());
  const [scheduledJobsSessionId, setScheduledJobsSessionId] = useState<string | null>(null);
  const editInputRef = useRef<HTMLInputElement>(null);
  const sessionMenuPopoverRef = useRef<HTMLDivElement>(null);
  const sessionMenuAnchorRef = useRef<HTMLButtonElement>(null);
  const metadataLoadRequestIdRef = useRef(0);
  const initialMetadataLoadKeyRef = useRef<string | null>(null);

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
        parts.push(
          `${session.sessionId}|${session.isTransient ? '1':'0'}|${session.sessionKind}|` +
          `${session.workspacePath ?? ''}|${session.mode ?? ''}|${session.needsUserAttention ? '1':'0'}|` +
          `${session.hasUnreadCompletion ? '1':'0'}|${session.title ?? ''}`
        );
      }
      return parts.join(';');
    };
    const unsub = flowChatStore.subscribeSelector(selector, (() => {
      setFlowChatState(flowChatStore.getState());
    }), { isEqual: (a, b) => a === b });
    return () => unsub();
  }, []);

  useEffect(() => {
    if (editingSessionId && editInputRef.current) {
      editInputRef.current.focus();
      editInputRef.current.select();
    }
  }, [editingSessionId]);

  useEffect(() => {
    metadataLoadRequestIdRef.current += 1;
    initialMetadataLoadKeyRef.current = null;
    setExpandLevel(0);
    setMetadataPageState({
      totalTopLevelCount: null,
      nextCursor: undefined,
      hasMore: false,
      isLoading: false,
    });
  }, [workspaceId, workspacePath, remoteConnectionId, remoteSshHost]);

  const loadMetadataPage = useCallback(
    async (limit: number, cursor: string | undefined, source: string) => {
      if (!workspacePath || limit <= 0) {
        return null;
      }

      const requestId = metadataLoadRequestIdRef.current + 1;
      metadataLoadRequestIdRef.current = requestId;
      setMetadataPageState(prev => ({ ...prev, isLoading: true }));

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
          setMetadataPageState({
            totalTopLevelCount: page.totalTopLevelCount,
            nextCursor: page.nextCursor,
            hasMore: page.hasMore,
            isLoading: false,
          });
        }
        return page;
      } catch (error) {
        if (metadataLoadRequestIdRef.current === requestId) {
          setMetadataPageState(prev => ({ ...prev, isLoading: false }));
        }
        log.warn('Failed to load visible session metadata page', { error, workspacePath, cursor, limit });
        return null;
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

  // When sessions are archived, reset stale metadata so the expand toggle
  // doesn't linger with old counts after all sessions are gone.
  useEffect(() => {
    const handler = () => {
      metadataLoadRequestIdRef.current += 1;
      setExpandLevel(0);
      setMetadataPageState({
        totalTopLevelCount: null,
        nextCursor: undefined,
        hasMore: false,
        isLoading: false,
      });
      if (isVisible && workspacePath) {
        void loadMetadataPage(SESSIONS_LEVEL_0, undefined, 'sessions_nav_post_archive');
      }
    };
    window.addEventListener('bitfun:session-archived', handler);
    return () => window.removeEventListener('bitfun:session-archived', handler);
  }, [isVisible, workspacePath, loadMetadataPage]);

  useEffect(() => {
    if (!openMenuSessionId) return;
    const handleOutside = (event: MouseEvent) => {
      if (!sessionMenuPopoverRef.current?.contains(event.target as Node)) {
        setOpenMenuSessionId(null);
        setSessionMenuPosition(null);
      }
    };
    document.addEventListener('mousedown', handleOutside);
    return () => document.removeEventListener('mousedown', handleOutside);
  }, [openMenuSessionId]);

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

    updateSessionMenuPosition();

    const handleViewportChange = () => updateSessionMenuPosition();
    window.addEventListener('resize', handleViewportChange);
    window.addEventListener('scroll', handleViewportChange, true);

    return () => {
      window.removeEventListener('resize', handleViewportChange);
      window.removeEventListener('scroll', handleViewportChange, true);
    };
  }, [openMenuSessionId, updateSessionMenuPosition]);

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
          if (workspacePath) {
            return sessionBelongsToWorkspaceNavRow(s, workspacePath, remoteConnectionId, remoteSshHost);
          }
          return !s.workspacePath;
        })
        .sort(compareSessionsForNavStable),
    [flowChatState.sessions, workspacePath, remoteConnectionId, remoteSshHost]
  );

  const { topLevelSessions, childrenByParent } = useMemo(() => {
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

    for (const [pid, list] of childMap) {
      childMap.set(pid, [...list].sort(compareSessionsForNavStable));
    }

    return {
      topLevelSessions: [...parents].sort(compareSessionsForNavStable),
      childrenByParent: childMap,
    };
  }, [sessions]);

  const sessionDisplayLimit = useMemo(() => {
    const total = topLevelSessions.length;
    if (expandLevel === 2 || total <= SESSIONS_LEVEL_0) return total;
    if (expandLevel === 1) return Math.min(total, SESSIONS_LEVEL_1);
    return SESSIONS_LEVEL_0;
  }, [topLevelSessions.length, expandLevel]);

  const totalTopLevelSessionCount = metadataPageState.totalTopLevelCount ?? topLevelSessions.length;
  const hasMoreUnloadedSessions =
    metadataPageState.hasMore || topLevelSessions.length < totalTopLevelSessionCount;
  const expandToggleAction =
    expandLevel === 0
      ? 'show-more'
      : expandLevel === 1 && totalTopLevelSessionCount > SESSIONS_LEVEL_1
        ? 'show-all'
        : 'show-less';

  const visibleItems = useMemo(() => {
    const visibleParents = topLevelSessions.slice(0, sessionDisplayLimit);
    const out: Array<{ session: Session; level: 0 | 1 }> = [];
    for (const p of visibleParents) {
      out.push({ session: p, level: 0 });
      const children = childrenByParent.get(p.sessionId) || [];
      for (const c of children) out.push({ session: c, level: 1 });
    }
    return out;
  }, [childrenByParent, sessionDisplayLimit, topLevelSessions]);

  const activeSessionId = flowChatState.activeSessionId;
  const scheduledJobsSession = scheduledJobsSessionId
    ? flowChatState.sessions.get(scheduledJobsSessionId) ?? null
    : null;
  const lastHistoryOpenIntentRef = useRef<{ sessionId: string; atMs: number } | null>(null);

  const dispatchHistoryOpenIntentForSession = useCallback(
    (session: Session): boolean => {
      const sessionId = session.sessionId;
      if (
        sessionId === activeSessionId ||
        !shouldShowHistorySessionOpenIntent(session)
      ) {
        return false;
      }

      const now = typeof performance !== 'undefined' ? performance.now() : Date.now();
      const lastIntent = lastHistoryOpenIntentRef.current;
      if (
        lastIntent &&
        lastIntent.sessionId === sessionId &&
        now - lastIntent.atMs < 250
      ) {
        return true;
      }

      lastHistoryOpenIntentRef.current = { sessionId, atMs: now };
      dispatchHistorySessionOpenIntent(sessionId, getTitle(session));
      return true;
    },
    [activeSessionId],
  );

  const handleSwitch = useCallback(
    async (sessionId: string) => {
      if (editingSessionId) return;
      try {
        const session = flowChatStore.getState().sessions.get(sessionId);
        const historyOpenIntentDispatched = session
          ? dispatchHistoryOpenIntentForSession(session)
          : false;
        if (session) {
          if (historyOpenIntentDispatched) {
            await waitForHistoryOpenIntentPaint();
          }
        }
        const relationship = resolveSessionRelationship(session);
        const parentSessionId = relationship.parentSessionId;
        const mustActivateWorkspace =
          Boolean(workspaceId) && workspaceId !== currentWorkspace?.id;
        const activateWorkspace = mustActivateWorkspace
          ? async (targetWorkspaceId: string) => {
              await setActiveWorkspace(targetWorkspaceId);
            }
          : undefined;

        if (relationship.canOpenInAuxPane && parentSessionId && session) {
          await openMainSession(parentSessionId, {
            workspaceId,
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
            workspaceId,
            activateWorkspace,
          });
          return;
        }

        await openMainSession(sessionId, {
          workspaceId,
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
      currentWorkspace?.id,
    ]
  );

  const handleSessionOpenPointerDown = useCallback(
    (event: React.PointerEvent<HTMLElement>, session: Session) => {
      if (editingSessionId || session.sessionId === activeSessionId) {
        return;
      }

      const target = event.target as HTMLElement | null;
      if (target?.closest('.bitfun-nav-panel__inline-item-actions, .bitfun-nav-panel__inline-item-edit')) {
        return;
      }

      dispatchHistoryOpenIntentForSession(session);
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
        mode === 'cowork'
          ? t('nav.sessions.newCoworkSession')
          : mode === 'claw'
            ? t('nav.sessions.newClawSession')
            : t('nav.sessions.newCodeSession');
      return `${label} ${matched[1]}`;
    },
    [t]
  );

  const handleMenuOpen = useCallback(
    (e: React.MouseEvent, sessionId: string) => {
      e.stopPropagation();
      if (openMenuSessionId === sessionId) {
        setOpenMenuSessionId(null);
        setSessionMenuPosition(null);
        return;
      }
      const btn = e.currentTarget as HTMLElement;
      const rect = btn.getBoundingClientRect();
      const { top, left } = computeFixedPopoverPosition(rect, 160, 96, 4, 8);
      setSessionMenuPosition({ top, left });
      setOpenMenuSessionId(sessionId);
    },
    [openMenuSessionId]
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
        await sessionAPI.archiveSession(sessionId, workspacePath || '', remoteConnectionId || undefined, remoteSshHost || undefined);
        // Remove from in-memory state only — do NOT delete from disk
        flowChatManager.discardLocalSession(sessionId);
        window.dispatchEvent(new CustomEvent('bitfun:session-archived'));
      } catch (err) {
        log.error('Failed to archive session', err);
      }
    },
    [workspacePath, remoteConnectionId, remoteSshHost, t]
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
        log.error('Failed to update session title', err);
      }
    }
    setEditingSessionId(null);
    setEditingTitle('');
  }, [editingSessionId, editingTitle]);

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

  if (topLevelSessions.length === 0) {
    if (metadataPageState.isLoading) {
      return (
        <div className="bitfun-nav-panel__inline-list">
          <div className="bitfun-nav-panel__inline-loading">
            <Loader2 size={12} />
            <span>{t('nav.sessions.loading')}</span>
          </div>
        </div>
      );
    }
    return null;
  }

  return (
    <div className="bitfun-nav-panel__inline-list">
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
          const sessionModeKey = resolveSessionModeType(session);
          const sessionTitle = resolveSessionTitle(session);
          const parentSessionId = relationship.parentSessionId;
          const parentSession = parentSessionId ? flowChatState.sessions.get(parentSessionId) : undefined;
          const parentTitle = parentSession ? resolveSessionTitle(parentSession) : '';
          const parentTurnIndex = relationship.origin?.parentTurnIndex;
          const trimmedAssistant = assistantLabel?.trim() ?? '';
          const showAssistantInTooltip = trimmedAssistant.length > 0;
          const showRichTooltip = showAssistantInTooltip || isChildSession;
          const tooltipContent = showRichTooltip ? (
            <div className="bitfun-nav-panel__inline-item-tooltip">
              <div className="bitfun-nav-panel__inline-item-tooltip-title">{sessionTitle}</div>
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
            </div>
          ) : (
            sessionTitle
          );
          const SessionIcon =
            sessionModeKey === 'cowork'
              ? ClipboardList
              : sessionModeKey === 'claw'
                ? showAssistantInTooltip
                  ? Panda
                  : Bot
                : Code2;
          const isRunning = runningSessionIds.has(session.sessionId);
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
          const isHighPriority = !!session.needsUserAttention;
          const row = (
            <div
              className={[
                'bitfun-nav-panel__inline-item',
                level === 1 && 'is-child',
                isChildSession && 'is-btw-child',
                isRowActive && 'is-active',
                isEditing && 'is-editing',
                openMenuSessionId === session.sessionId && 'is-menu-open',
              ]
                .filter(Boolean)
                .join(' ')}
              data-testid="session-nav-item"
              data-session-id={session.sessionId}
              data-session-title={sessionTitle}
              onPointerDown={event => handleSessionOpenPointerDown(event, session)}
              onClick={() => handleSwitch(session.sessionId)}
            >
              {showSessionModeIcon ? (
                <span className="bitfun-nav-panel__inline-item-icon-slot">
                  {isRunning ? (
                    <Loader2
                      size={14}
                      className={[
                        'bitfun-nav-panel__inline-item-icon',
                        'is-running',
                      ].join(' ')}
                    />
                  ) : (
                    <SessionIcon
                      size={14}
                      className={[
                        'bitfun-nav-panel__inline-item-icon',
                        sessionModeKey === 'cowork'
                          ? 'is-cowork'
                          : sessionModeKey === 'claw'
                            ? 'is-claw'
                            : 'is-code',
                      ].join(' ')}
                    />
                  )}
                  {attentionKind ? (
                    <span
                      className={[
                        'bitfun-nav-panel__inline-item-unread-dot',
                        attentionKind === 'error' && 'is-error',
                        attentionKind === 'interrupted' && 'is-interrupted',
                        attentionKind === 'ask_user' && 'is-ask-user',
                        attentionKind === 'tool_confirm' && 'is-tool-confirm',
                        isHighPriority && 'is-high-priority',
                      ].filter(Boolean).join(' ')}
                      aria-label={
                        attentionKind === 'error'
                          ? t('nav.sessions.unreadError')
                          : attentionKind === 'interrupted'
                            ? t('nav.sessions.unreadInterrupted')
                            : attentionKind === 'ask_user'
                              ? t('nav.sessions.needsUserInput')
                              : attentionKind === 'tool_confirm'
                                ? t('nav.sessions.needsToolConfirm')
                                : t('nav.sessions.unreadCompleted')
                      }
                    />
                  ) : null}
                </span>
              ) : null}

              {isEditing ? (
                <div className="bitfun-nav-panel__inline-item-edit" onClick={e => e.stopPropagation()}>
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
                  <span className="bitfun-nav-panel__inline-item-main">
                    <span className="bitfun-nav-panel__inline-item-label">{sessionTitle}</span>
                    {isChildSession ? (
                      <span className="bitfun-nav-panel__inline-item-btw-badge">{childSessionBadge}</span>
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
                  </span>
                  <div
                    className={`bitfun-nav-panel__inline-item-actions${openMenuSessionId === session.sessionId ? ' is-open' : ''}`}
                  >
                    <button
                      type="button"
                      ref={openMenuSessionId === session.sessionId ? sessionMenuAnchorRef : undefined}
                      className={`bitfun-nav-panel__inline-item-action-btn${openMenuSessionId === session.sessionId ? ' is-open' : ''}`}
                      onClick={e => handleMenuOpen(e, session.sessionId)}
                    >
                      <MoreHorizontal size="var(--bitfun-nav-row-action-icon-size)" />
                    </button>
                  </div>
                  {openMenuSessionId === session.sessionId && sessionMenuPosition && createPortal(
                    <div
                      ref={sessionMenuPopoverRef}
                      className="bitfun-nav-panel__inline-item-menu-popover"
                      role="menu"
                      style={{ top: `${sessionMenuPosition.top}px`, left: `${sessionMenuPosition.left}px` }}
                    >
                      <button
                        type="button"
                        className="bitfun-nav-panel__inline-item-menu-item"
                        onClick={e => { setOpenMenuSessionId(null); handleStartEdit(e, session); }}
                      >
                        <Pencil size={13} />
                        <span>{t('nav.sessions.rename')}</span>
                      </button>
                      <button
                        type="button"
                        className="bitfun-nav-panel__inline-item-menu-item"
                        onClick={e => {
                          e.stopPropagation();
                          setOpenMenuSessionId(null);
                          setScheduledJobsSessionId(session.sessionId);
                        }}
                        disabled={!workspacePath}
                      >
                        <Clock3 size={13} />
                        <span>{t('nav.scheduledJobs.open')}</span>
                      </button>
                      <button
                        type="button"
                        className="bitfun-nav-panel__inline-item-menu-item"
                        onClick={e => { setOpenMenuSessionId(null); void handleArchive(e, session.sessionId); }}
                      >
                        <Archive size={13} />
                        <span>{t('nav.sessions.archive')}</span>
                      </button>
                      <button
                        type="button"
                        className="bitfun-nav-panel__inline-item-menu-item is-danger"
                        onClick={e => { setOpenMenuSessionId(null); void handleDelete(e, session.sessionId); }}
                      >
                        <Trash2 size={13} />
                        <span>{t('nav.sessions.delete')}</span>
                      </button>
                    </div>,
                    document.body
                  )}
                </>
              )}
            </div>
          );
          return isEditing || openMenuSessionId !== null ? row : (
            <Tooltip key={session.sessionId} content={tooltipContent} placement="right" followCursor>
              {row}
            </Tooltip>
          );
        })}

      {(totalTopLevelSessionCount > SESSIONS_LEVEL_0 || hasMoreUnloadedSessions) && (
        <button
          type="button"
          className={`bitfun-nav-panel__inline-toggle${metadataPageState.isLoading ? ' is-loading' : ''}`}
          data-testid="session-nav-show-more"
          data-session-nav-toggle-action={expandToggleAction}
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
                  count: Math.max(totalTopLevelSessionCount - SESSIONS_LEVEL_0, 0),
                })}
              </span>
            </>
          ) : expandLevel === 1 && totalTopLevelSessionCount > SESSIONS_LEVEL_1 ? (
            <>
              {metadataPageState.isLoading ? (
                <Loader2 size={12} className="bitfun-nav-panel__inline-toggle-spinner" />
              ) : (
                <span className="bitfun-nav-panel__inline-toggle-dots">···</span>
              )}
              <span>
                {t('nav.sessions.showAll', {
                  count: Math.max(totalTopLevelSessionCount - SESSIONS_LEVEL_1, 0),
                })}
              </span>
            </>
          ) : (
            <span>{t('nav.sessions.showLess')}</span>
          )}
        </button>
      )}

      <ScheduledJobsModal
        isOpen={scheduledJobsSession != null}
        onClose={() => setScheduledJobsSessionId(null)}
        workspacePath={scheduledJobsSession?.workspacePath || workspacePath}
        workspaceId={scheduledJobsSession?.workspaceId || workspaceId}
        remoteConnectionId={scheduledJobsSession?.remoteConnectionId || remoteConnectionId}
        remoteSshHost={scheduledJobsSession?.remoteSshHost || remoteSshHost}
        sessionId={scheduledJobsSession?.sessionId}
        targetKind="session"
        lockSessionId
        title={t('nav.scheduledJobs.title')}
        targetLabel={scheduledJobsSession ? resolveSessionTitle(scheduledJobsSession) : undefined}
        targetDescription={scheduledJobsSession?.workspacePath || workspacePath}
      />
    </div>
  );
};

export default SessionsSection;
