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
 * The floating capsule is only visible on the base Agent scene (activeOverlay === null).
 * When any overlay is active (settings, tools, shell, live apps, ...) the capsule hides
 * entirely — the UnifiedTopBar's task-list icon opens a centered SessionListDialog instead.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Code2, ListChecks, LayoutDashboard, ListTodo, Pin, Plus, Sparkles } from 'lucide-react';
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
import { useOverlayStore } from '../../stores/overlayStore';
import { useSessionCapsuleStore } from '../../stores/sessionCapsuleStore';
import SessionList from '../SessionList/SessionList';
import { NewSessionDialog } from './NewSessionDialog';
import './SessionCapsule.scss';

const log = createLogger('SessionCapsule');
const AGENT_SCENE = 'session' as const;

type SessionMode = 'code' | 'cowork' | 'claw';

const resolveSessionModeType = (session: Session): SessionMode => {
  const normalizedMode = session.mode?.toLowerCase();
  if (normalizedMode === 'cowork') return 'cowork';
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
  const openTaskDetail = useSessionCapsuleStore((s) => s.openTaskDetail);
  const { openedWorkspacesList, setActiveWorkspace, currentWorkspace } = useWorkspaceContext();
  const activeBtwSessionTab = useAgentCanvasStore((state) => selectActiveBtwSessionTab(state as any));
  const activeBtwSessionData = activeBtwSessionTab?.content.data as
    | { childSessionId: string; parentSessionId: string; workspacePath?: string }
    | undefined;

  const [expanded, setExpanded] = useState<boolean>(readExpandedFromStorage);
  const [pinned, setPinned] = useState<boolean>(readPinnedFromStorage);
  const [listFilterQuery, setListFilterQuery] = useState('');
  const [newSessionDialogOpen, setNewSessionDialogOpen] = useState(false);
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() => flowChatStore.getState());
  const [runningSessionIds, setRunningSessionIds] = useState<Set<string>>(() => new Set());
  const panelRef = useRef<HTMLDivElement>(null);

  // Capsule is only visible on the base Agent scene; every overlay uses SessionListDialog instead.
  const usesDialog = activeOverlay !== null;

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

  /** All running sessions (same order as the list), shown in full inside the collapsed capsule */
  const runningSessionsOrdered = useMemo((): Session[] => {
    if (runningSessionIds.size === 0) return [];
    return Array.from(flowChatState.sessions.values())
      .filter((s) => runningSessionIds.has(s.sessionId))
      .sort(compareSessionsForDisplay);
  }, [runningSessionIds, flowChatState.sessions]);

  /** Exclude Agentic OS Dispatcher sessions — same filter as SessionList / SessionListDialog. */
  const sessionCount = useMemo(
    () =>
      Array.from(flowChatState.sessions.values()).filter(
        (s) => s.mode?.toLowerCase() !== 'dispatcher'
      ).length,
    [flowChatState.sessions]
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
      Array.from(state.sessions.values()).sort(compareSessionsForDisplay)[0]?.sessionId;
    if (!targetId) return;
    openTaskDetail(targetId);
    openOverlay('task-detail');
  }, [openTaskDetail, openOverlay]);

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

  // Collapse when clicking outside the capsule (expanded only).
  // Ignore portaled UI that belongs to the session list (see SessionList).
  useEffect(() => {
    if (!expanded || pinned) return;
    const handler = (e: PointerEvent) => {
      const target = e.target;
      if (!(target instanceof Node)) return;
      if (panelRef.current?.contains(target)) return;
      const root = target instanceof Element ? target : target.parentElement;
      if (root?.closest?.('[data-bitfun-ignore-session-capsule-outside]')) return;
      if (root?.closest?.('.modal-overlay')) return;
      setExpanded(false);
      writeExpandedToStorage(false);
    };
    document.addEventListener('pointerdown', handler);
    return () => document.removeEventListener('pointerdown', handler);
  }, [expanded, pinned]);

  // When switching to an overlay that uses the dialog, hide the capsule.
  useEffect(() => {
    if (usesDialog) {
      setExpanded(false);
      writeExpandedToStorage(false);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeOverlay]);

  // In any overlay scene: session list is opened via SessionListDialog — hide capsule entirely.
  if (usesDialog) return null;

  const hasRunning = !expanded && runningSessionsOrdered.length > 0;

  return (
    <div
      ref={panelRef}
      className={[
        'session-capsule',
        expanded ? 'session-capsule--expanded' : '',
        hasRunning ? 'session-capsule--running' : '',
      ].filter(Boolean).join(' ')}
      aria-label={t('nav.sections.sessions')}
    >
      {expanded ? (
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
            <SessionList listAllSessions listFilterQuery={listFilterQuery} />
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
      ) : runningSessionsOrdered.length > 0 ? (
        /* ── Running sessions card — wider panel showing each active task ── */
        <div
          className="session-capsule__running-panel"
          role="group"
          aria-label={t('nav.sessionCapsule.runningSessionsGroupLabel')}
        >
          {/* Header row: label + running count */}
          <div className="session-capsule__running-hd">
            <span className="session-capsule__running-hd-label">
              {t('nav.sessionCapsule.runningSessionsGroupLabel')}
            </span>
            <span className="session-capsule__running-count">
              {runningSessionsOrdered.length}
            </span>
          </div>

          {/* One row per running session */}
          <div className="session-capsule__running-rows">
            {runningSessionsOrdered.map((session) => {
              const mode = resolveSessionModeType(session);
              const ModeIcon = mode === 'cowork' ? ListTodo : mode === 'claw' ? Sparkles : Code2;
              const focused = isSessionUiFocused(session);
              const title = getSessionListTitle(session);
              return (
                <Tooltip
                  key={session.sessionId}
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
              );
            })}
          </div>

          {/* Footer: expand full task list */}
          <div className="session-capsule__running-ft">
            <button
              type="button"
              className="session-capsule__open-list-btn"
              onClick={toggle}
              aria-label={t('nav.sessionCapsule.openTaskList')}
              aria-expanded={false}
            >
              <ListChecks size={11} strokeWidth={2.3} />
              <span>{t('nav.sessionCapsule.openTaskList')}</span>
            </button>
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
            {sessionCount > 0 && (
              <span className="session-capsule__badge">
                {sessionCount > 99 ? '99+' : sessionCount}
              </span>
            )}
          </button>
        </Tooltip>
      )}
    </div>
  );
};

export default SessionCapsule;
