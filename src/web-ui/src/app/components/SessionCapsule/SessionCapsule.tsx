/**
 * SessionCapsule — floating vertical capsule for session navigation.
 *
 * Replaces the former left sidebar session list (NavPanel + session list).
 *
 * States:
 *   Collapsed — a small rounded pill on the left edge, vertically centered.
 *               No running tasks: list icon + session count badge (click expands).
 *               With running tasks: every running session shows a mode-colored avatar; click switches.
 *               Below avatars: compact button to expand the full list.
 *   Expanded  — a tall rounded rectangle (capsule) containing the session list.
 *
 * The panel is position:fixed so it floats over all content.
 * Collapse/expand state is persisted in localStorage.
 *
 * The capsule stays visible over overlay scenes; UnifiedTopBar "view all tasks" expands
 * this panel instead of opening a separate modal.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Brush, Code2, ListChecks, LayoutDashboard, LayoutGrid, ListTodo, Pin, Plus, Sparkles, Square } from 'lucide-react';
import { Search, Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { flowChatStore } from '../../../flow_chat/store/FlowChatStore';
import type { FlowChatState, Session } from '../../../flow_chat/types/flow-chat';
import { stateMachineManager } from '../../../flow_chat/state-machine';
import { SessionExecutionState } from '../../../flow_chat/state-machine/types';
import {
  openBtwSessionInAuxPane,
  openMainSession,
  selectActiveBtwSessionTab,
} from '../../../flow_chat/services/openBtwSession';
import { resolveSessionRelationship } from '../../../flow_chat/utils/sessionMetadata';
import { compareSessionsForDisplay, findOpenedWorkspaceForSession } from '../../../flow_chat/utils/sessionOrdering';
import { useAgentCanvasStore } from '@/app/components/panels/content-canvas/stores';
import { createLogger } from '@/shared/utils/logger';
import { renderLiveAppIcon } from '@/app/scenes/apps/live-app/liveAppIcons';
import {
  resolveActiveRunningLiveAppId,
  useRunningLiveAppItems,
  type RunningLiveAppItem,
} from '@/app/scenes/apps/live-app/liveAppTaskView';
import { useOverlayStore } from '../../stores/overlayStore';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import { liveAppAPI } from '@/infrastructure/api/service-api/LiveAppAPI';
import { useLiveAppStore } from '@/app/scenes/apps/live-app/liveAppStore';
import { useSessionCapsuleStore } from '../../stores/sessionCapsuleStore';
import SessionList from '../SessionList/SessionList';
import { NewSessionDialog } from './NewSessionDialog';
import './SessionCapsule.scss';

const log = createLogger('SessionCapsule');
const AGENT_SCENE = 'session' as const;
/** Default visible rows in the expanded capsule; search still filters within this slice. */
const RECENT_SESSION_LIMIT = 7;

type SessionMode = 'code' | 'cowork' | 'design' | 'claw';

const resolveSessionModeType = (session: Session): SessionMode => {
  const normalizedMode = session.mode?.toLowerCase();
  if (normalizedMode === 'cowork') return 'cowork';
  if (normalizedMode === 'design') return 'design';
  if (normalizedMode === 'claw') return 'claw';
  return 'code';
};

const getSessionListTitle = (session: Session): string =>
  session.title?.trim() || `Task ${session.sessionId.slice(0, 6)}`;

const STORAGE_KEY = 'bitfun.sessionCapsule.expanded';
const STORAGE_PINNED = 'bitfun.sessionCapsule.pinned';

function readExpandedFromStorage(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) !== 'false';
  } catch {
    return true;
  }
}

function writeExpandedToStorage(value: boolean): void {
  try {
    localStorage.setItem(STORAGE_KEY, String(value));
  } catch { /* ignore */ }
}

function readPinnedFromStorage(): boolean {
  try {
    return localStorage.getItem(STORAGE_PINNED) === 'true';
  } catch {
    return false;
  }
}

function writePinnedToStorage(value: boolean): void {
  try {
    localStorage.setItem(STORAGE_PINNED, String(value));
  } catch { /* ignore */ }
}

const SessionCapsule: React.FC = () => {
  const { t } = useI18n('common');
  const activeOverlay = useOverlayStore((s) => s.activeOverlay);
  const openOverlay = useOverlayStore((s) => s.openOverlay);
  const closeOverlay = useOverlayStore((s) => s.closeOverlay);
  const markWorkerStopped = useLiveAppStore((s) => s.markWorkerStopped);
  const openTaskDetail = useSessionCapsuleStore((s) => s.openTaskDetail);
  const sessionListExpandNonce = useSessionCapsuleStore((s) => s.sessionListExpandNonce);
  const { openedWorkspacesList, setActiveWorkspace, currentWorkspace } = useWorkspaceContext();
  const activeBtwSessionTab = useAgentCanvasStore((state) => selectActiveBtwSessionTab(state as any));
  const activeBtwSessionData = activeBtwSessionTab?.content.data as
    | { childSessionId: string; parentSessionId: string; workspacePath?: string }
    | undefined;

  const [expanded, setExpanded] = useState<boolean>(readExpandedFromStorage);
  const [pinned, setPinned] = useState<boolean>(readPinnedFromStorage);
  const [overlayExpanded, setOverlayExpanded] = useState(false);
  const [listFilterQuery, setListFilterQuery] = useState('');
  const [newSessionDialogOpen, setNewSessionDialogOpen] = useState(false);
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() => flowChatStore.getState());
  const [runningSessionIds, setRunningSessionIds] = useState<Set<string>>(() => new Set());
  const [hoverExpanded, setHoverExpanded] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);
  const runningLiveApps = useRunningLiveAppItems();
  const activeLiveAppId = resolveActiveRunningLiveAppId(activeOverlay);

  useEffect(() => {
    const unsub = flowChatStore.subscribe((s) => setFlowChatState(s));
    return () => unsub();
  }, []);

  const updateRunningSessions = useCallback(() => {
    const running = new Set<string>();
    for (const session of flowChatStore.getState().sessions.values()) {
      if (session.mode === 'Dispatcher') continue;
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
  }, []);

  useEffect(() => {
    updateRunningSessions();
    const unsubMachine = stateMachineManager.subscribeGlobal(updateRunningSessions);
    return () => unsubMachine();
  }, [updateRunningSessions, flowChatState.sessions]);

  const activeSessionId = flowChatState.activeSessionId;
  const activeTabId = activeOverlay ?? AGENT_SCENE;

  const isSessionUiFocused = useCallback(
    (session: Session | undefined): boolean => {
      if (!session) return false;
      const relationship = resolveSessionRelationship(session);
      if (relationship.isBtw && relationship.canOpenInAuxPane) {
        return activeBtwSessionData?.childSessionId === session.sessionId;
      }
      return activeTabId === AGENT_SCENE && session.sessionId === activeSessionId;
    },
    [activeBtwSessionData?.childSessionId, activeSessionId, activeTabId]
  );

  const runningSessionsOrdered = useMemo((): Session[] => {
    if (runningSessionIds.size === 0) return [];
    return Array.from(flowChatState.sessions.values())
      .filter((s) => runningSessionIds.has(s.sessionId))
      .sort(compareSessionsForDisplay);
  }, [runningSessionIds, flowChatState.sessions]);

  const runningItems = useMemo(
    (): Array<
      | { kind: 'session'; session: Session }
      | { kind: 'live-app'; app: RunningLiveAppItem }
    > => [
      ...runningLiveApps.map(app => ({ kind: 'live-app' as const, app })),
      ...runningSessionsOrdered.map(session => ({ kind: 'session' as const, session })),
    ],
    [runningLiveApps, runningSessionsOrdered]
  );

  const handleSwitchToSession = useCallback(
    async (sessionId: string) => {
      try {
        const session = flowChatStore.getState().sessions.get(sessionId);
        const relationship = resolveSessionRelationship(session);
        const parentSessionId = relationship.parentSessionId;
        const resolvedWorkspaceId = session
          ? findOpenedWorkspaceForSession(session, openedWorkspacesList)?.id
          : undefined;
        const mustActivateWorkspace =
          Boolean(resolvedWorkspaceId) && resolvedWorkspaceId !== currentWorkspace?.id;
        const activateWorkspace = mustActivateWorkspace
          ? async (targetWorkspaceId: string) => {
              await setActiveWorkspace(targetWorkspaceId);
            }
          : undefined;

        if (relationship.canOpenInAuxPane && parentSessionId && session) {
          await openMainSession(parentSessionId, {
            workspaceId: resolvedWorkspaceId,
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
            workspaceId: resolvedWorkspaceId,
            activateWorkspace,
          });
          return;
        }

        await openMainSession(sessionId, {
          workspaceId: resolvedWorkspaceId,
          activateWorkspace,
        });
      } catch (err) {
        log.error('Failed to switch session from capsule', err);
      }
    },
    [activeSessionId, currentWorkspace?.id, openedWorkspacesList, setActiveWorkspace]
  );

  const handleOpenTaskDetail = useCallback(() => {
    const state = flowChatStore.getState();
    const targetId =
      state.activeSessionId ??
      Array.from(state.sessions.values())
        .filter(session => session.mode?.toLowerCase() !== 'dispatcher')
        .sort(compareSessionsForDisplay)[0]?.sessionId;
    if (!targetId) return;
    openTaskDetail(targetId);
    openOverlay('task-detail');
  }, [openTaskDetail, openOverlay]);

  const handleOpenLiveApp = useCallback((appId: string) => {
    openOverlay(`live-app:${appId}`);
  }, [openOverlay]);

  const handleCancelSessionTask = useCallback((event: React.MouseEvent, sessionId: string) => {
    event.stopPropagation();
    void flowChatManager.cancelTaskForSession(sessionId);
  }, []);

  const handleStopLiveApp = useCallback(
    async (event: React.MouseEvent, appId: string) => {
      event.stopPropagation();
      try {
        await liveAppAPI.workerStop(appId);
      } catch (error) {
        log.warn('Failed to stop live app worker', { appId, error });
      } finally {
        markWorkerStopped(appId);
        if (activeOverlay === `live-app:${appId}`) {
          closeOverlay();
        }
      }
    },
    [activeOverlay, closeOverlay, markWorkerStopped]
  );

  const toggle = useCallback(() => {
    setExpanded((v) => {
      const next = !v;
      writeExpandedToStorage(next);
      return next;
    });
  }, []);

  const togglePinned = useCallback(() => {
    setPinned((v) => {
      const next = !v;
      writePinnedToStorage(next);
      return next;
    });
  }, []);

  useEffect(() => {
    if (!expanded) setListFilterQuery('');
  }, [expanded]);

  useEffect(() => {
    if (!overlayExpanded) setListFilterQuery('');
  }, [overlayExpanded]);

  const runningCount = runningItems.length;
  const canHoverExpand = activeOverlay === null && !expanded && runningCount > 0;
  const showExpandedPanel = activeOverlay !== null
    ? overlayExpanded
    : (expanded || hoverExpanded || newSessionDialogOpen);
  const liftAboveOverlayScene = activeOverlay !== null;
  const showCollapsedCapsule = activeOverlay === null;

  const collapseCapsule = useCallback(() => {
    if (activeOverlay !== null) {
      setOverlayExpanded(false);
      return;
    }
    setExpanded(false);
    writeExpandedToStorage(false);
  }, [activeOverlay]);

  useEffect(() => {
    if (expanded || runningSessionIds.size === 0) {
      setHoverExpanded(false);
    }
  }, [expanded, runningSessionIds.size, runningLiveApps.length]);

  // Collapse when clicking outside the capsule (expanded only).
  // Ignore portaled UI that belongs to the session list (see SessionList).
  useEffect(() => {
    if (!showExpandedPanel || pinned || newSessionDialogOpen) return;
    const handler = (e: PointerEvent) => {
      const target = e.target;
      if (!(target instanceof Node)) return;
      if (panelRef.current?.contains(target)) return;
      const root = target instanceof Element ? target : target.parentElement;
      if (root?.closest?.('[data-bitfun-ignore-session-capsule-outside]')) return;
      if (root?.closest?.('.modal-overlay')) return;
      collapseCapsule();
    };
    document.addEventListener('pointerdown', handler);
    return () => document.removeEventListener('pointerdown', handler);
  }, [collapseCapsule, newSessionDialogOpen, pinned, showExpandedPanel]);

  const lastExpandNonceRef = useRef(sessionListExpandNonce);
  useEffect(() => {
    if (sessionListExpandNonce === lastExpandNonceRef.current) return;
    lastExpandNonceRef.current = sessionListExpandNonce;
    if (activeOverlay !== null) {
      setOverlayExpanded(true);
      setHoverExpanded(false);
      return;
    }
    setExpanded(true);
    writeExpandedToStorage(true);
    setHoverExpanded(false);
  }, [activeOverlay, sessionListExpandNonce]);

  return (
    !showExpandedPanel && !showCollapsedCapsule ? null : (
    <div
      ref={panelRef}
      className={[
        'session-capsule',
        showExpandedPanel ? 'session-capsule--expanded' : '',
        !showExpandedPanel && runningCount > 0 ? 'session-capsule--running' : '',
        liftAboveOverlayScene ? 'session-capsule--above-scene-chrome' : '',
      ].filter(Boolean).join(' ')}
      aria-label={t('nav.sections.sessions')}
      onMouseEnter={canHoverExpand ? () => setHoverExpanded(true) : undefined}
      onMouseLeave={canHoverExpand ? () => {
        if (!newSessionDialogOpen) {
          setHoverExpanded(false);
        }
      } : undefined}
      onFocus={canHoverExpand ? () => setHoverExpanded(true) : undefined}
      onBlur={canHoverExpand ? (event) => {
        if (!newSessionDialogOpen && !event.currentTarget.contains(event.relatedTarget as Node | null)) {
          setHoverExpanded(false);
        }
      } : undefined}
    >
      {showExpandedPanel ? (
        <>
          {/* Row 1: search field only */}
          <div className="session-capsule__header">
            <Search
              className="session-capsule__search-input session-capsule__search--pill"
              placeholder={t('nav.sessionCapsule.searchPlaceholder')}
              value={listFilterQuery}
              onChange={setListFilterQuery}
              onClear={() => setListFilterQuery('')}
              clearable
              size="small"
              enterToSearch={false}
              inputAriaLabel={t('nav.sessionCapsule.searchPlaceholder')}
            />
          </div>

          {/* Task list */}
          <div className="session-capsule__list">
            <SessionList
              listAllSessions
              listFilterQuery={listFilterQuery}
              maxSessions={RECENT_SESSION_LIMIT}
            />
          </div>

          {/* Footer: new session + details + pin expand */}
          <div className="session-capsule__footer">
            <Tooltip content={t('nav.sessionCapsule.newSessionButton')} placement="top">
              <button
                type="button"
                className="session-capsule__icon-btn"
                onClick={() => setNewSessionDialogOpen(true)}
                aria-label={t('nav.sessionCapsule.newSessionButton')}
              >
                <Plus size={13} strokeWidth={2.25} />
              </button>
            </Tooltip>
            <Tooltip content={t('nav.sessionCapsule.viewDetails')} placement="top">
              <button
                type="button"
                className="session-capsule__icon-btn"
                aria-label={t('nav.sessionCapsule.viewDetails')}
                onClick={handleOpenTaskDetail}
              >
                <LayoutDashboard size={13} strokeWidth={2.25} />
              </button>
            </Tooltip>
            <Tooltip
              content={pinned ? t('nav.sessionCapsule.unpinKeepOpen') : t('nav.sessionCapsule.pinKeepOpen')}
              placement="top"
            >
              <button
                type="button"
                className={`session-capsule__icon-btn${pinned ? ' is-pinned' : ''}`}
                onClick={togglePinned}
                aria-label={pinned ? t('nav.sessionCapsule.unpinKeepOpen') : t('nav.sessionCapsule.pinKeepOpen')}
                aria-pressed={pinned}
              >
                <Pin size={13} strokeWidth={2.25} />
              </button>
            </Tooltip>
          </div>
          <NewSessionDialog open={newSessionDialogOpen} onClose={() => setNewSessionDialogOpen(false)} />
        </>
      ) : runningItems.length > 0 ? (
        <div
          className="session-capsule__running-panel"
          role="group"
          aria-label={t('nav.sessionCapsule.runningSessionsGroupLabel')}
        >
          <div className="session-capsule__running-hd">
            <span className="session-capsule__running-count">
              {runningItems.length}
            </span>
          </div>

          <div className="session-capsule__running-rows">
            {runningItems.map(item => {
              if (item.kind === 'live-app') {
                const { app } = item;
                const focused = activeLiveAppId === app.id;
                return (
                  <div key={app.id} className="session-capsule__running-row-wrap">
                    <Tooltip
                      content={t('nav.sessionCapsule.runningLiveAppTooltip', { title: app.title })}
                      placement="right"
                    >
                      <button
                        type="button"
                        className={`session-capsule__running-row${focused ? ' is-active' : ''}`}
                        onClick={() => handleOpenLiveApp(app.id)}
                        aria-label={t('nav.sessionCapsule.runningLiveAppTooltip', { title: app.title })}
                      >
                        <span
                          className={[
                            'session-capsule__mode-avatar',
                            'is-live-app',
                            focused ? 'is-focused' : '',
                          ].filter(Boolean).join(' ')}
                          aria-hidden
                        >
                          {renderLiveAppIcon(app.icon, 12)}
                        </span>
                        <span className="session-capsule__running-row-title">{app.title}</span>
                        <span className="session-capsule__running-row-badge" aria-hidden>
                          <LayoutGrid size={10} />
                        </span>
                      </button>
                    </Tooltip>
                    <Tooltip content={t('nav.sessionCapsule.stopRunningLiveApp')} placement="right">
                      <button
                        type="button"
                        className="session-capsule__running-row-cancel"
                        onClick={event => void handleStopLiveApp(event, app.id)}
                        aria-label={t('nav.sessionCapsule.stopRunningLiveApp')}
                      >
                        <Square className="session-capsule__running-row-cancel-icon" size={10} strokeWidth={2.25} aria-hidden />
                      </button>
                    </Tooltip>
                  </div>
                );
              }

              const { session } = item;
              const mode = resolveSessionModeType(session);
              const ModeIcon =
                mode === 'cowork'
                  ? ListTodo
                  : mode === 'design'
                    ? Brush
                    : mode === 'claw'
                      ? Sparkles
                      : Code2;
              const focused = isSessionUiFocused(session);
              const title = getSessionListTitle(session);
              return (
                <div key={session.sessionId} className="session-capsule__running-row-wrap">
                  <Tooltip
                    content={t('nav.sessionCapsule.runningSwitchTooltip', { title })}
                    placement="right"
                  >
                    <button
                      type="button"
                      className={`session-capsule__running-row${focused ? ' is-active' : ''}`}
                      onClick={() => void handleSwitchToSession(session.sessionId)}
                      aria-label={t('nav.sessionCapsule.runningSwitchTooltip', { title })}
                    >
                      <span
                        className={[
                          'session-capsule__mode-avatar',
                          `is-${mode}`,
                          focused ? 'is-focused' : '',
                        ].filter(Boolean).join(' ')}
                        aria-hidden
                      >
                        <ModeIcon size={12} strokeWidth={2.4} />
                      </span>
                      <span className="session-capsule__running-row-title">{title}</span>
                    </button>
                  </Tooltip>
                  <Tooltip content={t('nav.sessionCapsule.cancelRunningAgentTask')} placement="right">
                    <button
                      type="button"
                      className="session-capsule__running-row-cancel"
                      onClick={event => handleCancelSessionTask(event, session.sessionId)}
                      aria-label={t('nav.sessionCapsule.cancelRunningAgentTask')}
                    >
                      <Square className="session-capsule__running-row-cancel-icon" size={10} strokeWidth={2.25} aria-hidden />
                    </button>
                  </Tooltip>
                </div>
              );
            })}
          </div>
        </div>
      ) : (
        <Tooltip content={t('nav.sections.sessions')} placement="right">
          <button
            type="button"
            className="session-capsule__trigger"
            onClick={toggle}
            aria-label={t('nav.sections.sessions')}
            aria-expanded={false}
          >
            <ListChecks size={15} />
          </button>
        </Tooltip>
      )}
    </div>
    )
  );
};

export default SessionCapsule;
