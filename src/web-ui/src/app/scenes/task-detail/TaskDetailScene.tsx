/**
 * TaskDetailScene — Task Management Center.
 *
 * Assistant-style split: left (majority) = search + OS sessions | opened workspaces;
 * right rail = execution sessions for opened workspaces.
 */

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Code2,
  Brush,
  FolderOpen,
  FolderPlus,
  LayoutDashboard,
  ListTodo,
  Loader2,
  MessageSquare,
  Sparkles,
  ArrowRight,
  Clock,
  Radio,
  X,
} from 'lucide-react';
import { Search, FilterPill, FilterPillGroup, Select, IconButton } from '@/component-library';
import type { SelectOption } from '@/component-library';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import type { WorkspaceInfo } from '@/shared/types';
import {
  compareSessionsForDisplay,
} from '@/flow_chat/utils/sessionOrdering';
import { fallbackWorkspaceFolderLabel } from '@/flow_chat/utils/sessionOrdering';
import { findWorkspaceForSession } from '@/flow_chat/utils/workspaceScope';
import { openMainSession } from '@/flow_chat/services/openBtwSession';
import { useSessionCapsuleStore } from '../../stores/sessionCapsuleStore';
import { useOverlayStore } from '../../stores/overlayStore';
import { stateMachineManager } from '@/flow_chat/state-machine';
import { SessionExecutionState } from '@/flow_chat/state-machine/types';
import type { FlowChatState, Session } from '@/flow_chat/types/flow-chat';
import { createLogger } from '@/shared/utils/logger';
import { useI18n } from '@/infrastructure/i18n';
import { renderLiveAppIcon } from '@/app/scenes/apps/live-app/liveAppIcons';
import { useRunningLiveAppItems, type RunningLiveAppItem } from '@/app/scenes/apps/live-app/liveAppTaskView';
import './TaskDetailScene.scss';

const log = createLogger('TaskDetailScene');
const RECENT_WORKSPACE_LIMIT = 7;

// ── Helpers ──────────────────────────────────────────────────────────────────

type ExecMode = 'code' | 'cowork' | 'design' | 'claw';

function resolveExecMode(s: Session): ExecMode {
  const m = s.mode?.toLowerCase();
  if (m === 'cowork') return 'cowork';
  if (m === 'design') return 'design';
  if (m === 'claw') return 'claw';
  return 'code';
}

const MODE_LABELS: Record<ExecMode, string> = {
  code: 'Code',
  cowork: 'Cowork',
  design: 'Design',
  claw: 'Claw',
};

function ModeIcon({ mode, size = 13, className }: { mode: ExecMode; size?: number; className?: string }) {
  switch (mode) {
    case 'cowork': return <ListTodo size={size} className={className} />;
    case 'design': return <Brush size={size} className={className} />;
    case 'claw': return <Sparkles size={size} className={className} />;
    default: return <Code2 size={size} className={className} />;
  }
}

type StatusVariant = 'running' | 'active' | 'error' | 'idle';

function getStatusVariant(s: Session, runningIds: Set<string>): StatusVariant {
  if (runningIds.has(s.sessionId)) return 'running';
  if (s.status === 'error') return 'error';
  if (s.status === 'active') return 'active';
  return 'idle';
}

// ── Session row ───────────────────────────────────────────────────────────────

interface SessionRowProps {
  session: Session;
  isHighlighted: boolean;
  statusVariant: StatusVariant;
  showMode?: boolean;
  workspaceName?: string;
  formatRelativeTime: (ts: number) => string;
  onOpen: (s: Session) => void;
}

const SessionRow: React.FC<SessionRowProps> = ({
  session,
  isHighlighted,
  statusVariant,
  showMode = true,
  workspaceName,
  formatRelativeTime: formatRel,
  onOpen,
}) => {
  const { t } = useI18n('common');
  const rowTitle =
    session.title?.trim() ||
    t('taskDetailScene.fallbackTaskTitle', { id: session.sessionId.slice(0, 6) });
  const isRunning = statusVariant === 'running';
  const isDispatcher = session.mode?.toLowerCase() === 'dispatcher';
  const mode = resolveExecMode(session);

  return (
    <div
      className={[
        'tds-row',
        isHighlighted && 'is-highlighted',
        isRunning && 'is-running',
      ].filter(Boolean).join(' ')}
      role="button"
      tabIndex={0}
      onClick={() => onOpen(session)}
      onKeyDown={e => e.key === 'Enter' && onOpen(session)}
    >
      <span className={`tds-row__dot tds-row__dot--${statusVariant}`} />

      <span className="tds-row__icon-wrap">
        {isRunning ? (
          <Loader2 size={13} className="tds-row__icon tds-row__icon--spin" />
        ) : isDispatcher ? (
          <LayoutDashboard size={13} className="tds-row__icon tds-row__icon--dispatcher" />
        ) : (
          <ModeIcon mode={mode} size={13} className={`tds-row__icon tds-row__icon--${mode}`} />
        )}
      </span>

      <span className="tds-row__body">
        <span className="tds-row__title">{rowTitle}</span>
        <span className="tds-row__meta">
          {showMode && !isDispatcher && (
            <span className={`tds-row__badge tds-row__badge--${mode}`}>{MODE_LABELS[mode]}</span>
          )}
          {workspaceName && (
            <span className="tds-row__badge tds-row__badge--ws">
              <FolderOpen size={9} />
              {workspaceName}
            </span>
          )}
          <span className="tds-row__meta-dot">·</span>
          <span className="tds-row__meta-item"><Clock size={9} />{formatRel(session.lastActiveAt)}</span>
          <span className="tds-row__meta-dot">·</span>
          <span className="tds-row__meta-item"><MessageSquare size={9} />{session.dialogTurns.length}</span>
        </span>
      </span>

      <ArrowRight size={12} className="tds-row__arrow" />
    </div>
  );
};

interface LiveAppRowProps {
  app: RunningLiveAppItem;
  isHighlighted: boolean;
  formatRelativeTime: (ts: number) => string;
  onOpen: (appId: string) => void;
}

const LiveAppRow: React.FC<LiveAppRowProps> = ({
  app,
  isHighlighted,
  formatRelativeTime,
  onOpen,
}) => (
  <div
    className={[
      'tds-row',
      'tds-row--live-app',
      isHighlighted && 'is-highlighted',
      'is-running',
    ].filter(Boolean).join(' ')}
    role="button"
    tabIndex={0}
    onClick={() => onOpen(app.id)}
    onKeyDown={e => e.key === 'Enter' && onOpen(app.id)}
  >
    <span className="tds-row__dot tds-row__dot--running" />

    <span className="tds-row__icon-wrap">
      <span className="tds-row__live-app-icon">
        {renderLiveAppIcon(app.icon, 13)}
      </span>
    </span>

    <span className="tds-row__body">
      <span className="tds-row__title">{app.title}</span>
      <span className="tds-row__meta">
        <span className="tds-row__badge tds-row__badge--live-app">Live App</span>
        <span className="tds-row__meta-dot">·</span>
        <span className="tds-row__meta-item"><Clock size={9} />{formatRelativeTime(app.updatedAt)}</span>
      </span>
    </span>

    <ArrowRight size={12} className="tds-row__arrow" />
  </div>
);

// ── Mode filter type ──────────────────────────────────────────────────────────

type ModeFilter = 'all' | ExecMode;

function normalizeQuery(q: string): string {
  return q.trim().toLowerCase();
}

function matchesQuery(haystack: string, q: string): boolean {
  if (!q) return true;
  return haystack.toLowerCase().includes(q);
}

// ── Workspace row (opened workspaces pane) ───────────────────────────────────

interface WorkspaceRowProps {
  workspace: WorkspaceInfo;
  sessionCount: number;
  isActiveWorkspace: boolean;
  isFilterSelected: boolean;
  isOpenedWorkspace: boolean;
  onSelect: (workspaceId: string) => void;
  onClose: (e: React.MouseEvent, workspaceId: string) => void;
}

const WorkspaceRow: React.FC<WorkspaceRowProps> = ({
  workspace,
  sessionCount,
  isActiveWorkspace,
  isFilterSelected,
  isOpenedWorkspace,
  onSelect,
  onClose,
}) => {
  const { t } = useI18n('common');
  return (
  <div
    className={[
      'tds-ws-row',
      isFilterSelected && 'is-filter',
      isActiveWorkspace && 'is-current',
    ].filter(Boolean).join(' ')}
    role="button"
    tabIndex={0}
    onClick={() => onSelect(workspace.id)}
    onKeyDown={e => e.key === 'Enter' && onSelect(workspace.id)}
  >
    <span className="tds-ws-row__icon-wrap">
      <FolderOpen size={13} className="tds-ws-row__icon" />
    </span>
    <span className="tds-ws-row__body">
      <span className="tds-ws-row__title">{workspace.name}</span>
      <span className="tds-ws-row__meta">
        {isActiveWorkspace && <span className="tds-ws-row__badge">{t('taskDetailScene.badgeCurrent')}</span>}
        {!isOpenedWorkspace && <span className="tds-ws-row__badge">{t('taskDetailScene.badgeRecent')}</span>}
        <span className="tds-ws-row__meta-item">
          <MessageSquare size={9} />
          {sessionCount}
        </span>
      </span>
    </span>
    {isOpenedWorkspace ? (
      <IconButton
        size="xs"
        variant="ghost"
        className="tds-ws-row__close"
        tooltip={t('taskDetailScene.closeWorkspace')}
        onClick={e => onClose(e, workspace.id)}
        aria-label={t('taskDetailScene.closeWorkspace')}
      >
        <X size={11} />
      </IconButton>
    ) : null}
  </div>
  );
};

// ── Main Scene ────────────────────────────────────────────────────────────────

const TaskDetailScene: React.FC = () => {
  const { t, formatDate } = useI18n('common');
  const taskDetailSessionId = useSessionCapsuleStore(s => s.taskDetailSessionId);
  const closeTaskDetail = useSessionCapsuleStore(s => s.closeTaskDetail);
  const closeOverlay = useOverlayStore(s => s.closeOverlay);
  const openOverlay = useOverlayStore(s => s.openOverlay);
  const activeOverlay = useOverlayStore(s => s.activeOverlay);
  const runningLiveApps = useRunningLiveAppItems();

  const {
    openedWorkspacesList,
    recentWorkspaces,
    setActiveWorkspace,
    switchWorkspace,
    currentWorkspace,
    openWorkspace,
    closeWorkspaceById,
  } = useWorkspaceContext();

  const formatRelativeTime = useCallback(
    (ts: number) => {
      const diff = Date.now() - ts;
      if (diff < 60_000) return t('taskDetailScene.relativeJustNow');
      if (diff < 3_600_000) {
        return t('taskDetailScene.relativeMinutesAgo', { count: Math.floor(diff / 60_000) });
      }
      if (diff < 86_400_000) {
        return t('taskDetailScene.relativeHoursAgo', { count: Math.floor(diff / 3_600_000) });
      }
      if (diff < 7 * 86_400_000) {
        return t('taskDetailScene.relativeDaysAgo', { count: Math.floor(diff / 86_400_000) });
      }
      return formatDate(new Date(ts), { month: 'short', day: 'numeric' });
    },
    [t, formatDate]
  );

  const sessionDisplayTitle = useCallback(
    (s: Session) =>
      s.title?.trim() || t('taskDetailScene.fallbackTaskTitle', { id: s.sessionId.slice(0, 6) }),
    [t]
  );

  const modeChips = useMemo<Array<{ id: ModeFilter; label: string }>>(
    () => [
      { id: 'all', label: t('taskDetailScene.filterAll') },
      { id: 'code', label: 'Code' },
      { id: 'cowork', label: 'Cowork' },
      { id: 'claw', label: 'Claw' },
    ],
    [t]
  );

  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() => flowChatStore.getState());
  const [runningIds, setRunningIds] = useState<Set<string>>(new Set());
  const [wsFilter, setWsFilter] = useState<string>('all');
  const [modeFilter, setModeFilter] = useState<ModeFilter>('all');
  const [listQuery, setListQuery] = useState('');

  useEffect(() => {
    setFlowChatState(flowChatStore.getState());
    return flowChatStore.subscribe(s => setFlowChatState(s));
  }, []);

  useEffect(() => {
    const update = () => {
      const running = new Set<string>();
      for (const s of flowChatState.sessions.values()) {
        const m = stateMachineManager.get(s.sessionId);
        if (m && (
          m.getCurrentState() === SessionExecutionState.PROCESSING ||
          m.getCurrentState() === SessionExecutionState.FINISHING
        )) running.add(s.sessionId);
      }
      setRunningIds(running);
    };
    update();
    return stateMachineManager.subscribeGlobal(update);
  }, [flowChatState.sessions]);

  // Dispatcher (Agentic OS) sessions — not tied to a project workspace
  const qNorm = useMemo(() => normalizeQuery(listQuery), [listQuery]);

  const dispatcherSessions = useMemo(
    () => Array.from(flowChatState.sessions.values())
      .filter(s => s.mode?.toLowerCase() === 'dispatcher')
      .filter(s => matchesQuery(sessionDisplayTitle(s), qNorm))
      .sort(compareSessionsForDisplay),
    [flowChatState.sessions, qNorm, sessionDisplayTitle]
  );

  const recentWorkspaceScope = useMemo(() => {
    const scope = new Map<string, WorkspaceInfo>();
    recentWorkspaces.slice(0, RECENT_WORKSPACE_LIMIT).forEach(workspace => {
      scope.set(workspace.id, workspace);
    });
    openedWorkspacesList.forEach(workspace => {
      scope.set(workspace.id, workspace);
    });
    return Array.from(scope.values());
  }, [recentWorkspaces, openedWorkspacesList]);

  const openedWorkspaceIdSet = useMemo(
    () => new Set(openedWorkspacesList.map(workspace => workspace.id)),
    [openedWorkspacesList]
  );

  // Execution sessions tied to recent/opened workspaces.
  const execSessions = useMemo(() => {
    return Array.from(flowChatState.sessions.values())
      .filter(s => s.mode?.toLowerCase() !== 'dispatcher')
      .sort(compareSessionsForDisplay)
      .map(session => {
        const ws = findWorkspaceForSession(session, recentWorkspaceScope);
        return { session, workspace: ws ?? null };
      })
      .filter((row): row is { session: Session; workspace: WorkspaceInfo } => row.workspace !== null);
  }, [flowChatState.sessions, recentWorkspaceScope]);

  const sessionCountByWorkspaceId = useMemo(() => {
    const m = new Map<string, number>();
    for (const { workspace } of execSessions) {
      m.set(workspace.id, (m.get(workspace.id) ?? 0) + 1);
    }
    return m;
  }, [execSessions]);

  const filteredWorkspaces = useMemo(() => {
    return recentWorkspaceScope.filter(ws =>
      matchesQuery(ws.name, qNorm) || matchesQuery(ws.rootPath ?? '', qNorm)
    );
  }, [recentWorkspaceScope, qNorm]);

  const wsSelectOptions = useMemo<SelectOption[]>(
    () => [
      { label: t('taskDetailScene.allWorkspaces'), value: 'all' },
      ...recentWorkspaceScope.map(ws => ({ label: ws.name, value: ws.id })),
    ],
    [recentWorkspaceScope, t]
  );

  const showWorkspaceLabelsOnSessions = recentWorkspaceScope.length > 1;

  // Filtered exec sessions (opened workspaces only)
  const filteredExec = useMemo(() => {
    return execSessions.filter(({ session, workspace }) => {
      if (modeFilter !== 'all' && resolveExecMode(session) !== modeFilter) return false;
      if (wsFilter !== 'all' && workspace.id !== wsFilter) return false;
      if (
        qNorm &&
        !matchesQuery(sessionDisplayTitle(session), qNorm) &&
        !matchesQuery(workspace.name, qNorm)
      ) {
        return false;
      }
      return true;
    });
  }, [execSessions, modeFilter, wsFilter, qNorm, sessionDisplayTitle]);

  const runningExecCount = useMemo(
    () => filteredExec.filter(({ session }) => runningIds.has(session.sessionId)).length,
    [filteredExec, runningIds]
  );
  const totalRunningCount = runningExecCount + runningLiveApps.length;

  const handleWorkspacePaneSelect = useCallback((workspaceId: string) => {
    setWsFilter(prev => (prev === workspaceId ? 'all' : workspaceId));
  }, []);

  const handleOpenNewWorkspace = useCallback(async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ directory: true, multiple: false });
      if (selected && typeof selected === 'string') {
        await openWorkspace(selected);
      }
    } catch (e) {
      log.error('Failed to open workspace', e);
    }
  }, [openWorkspace]);

  const handleCloseWorkspace = useCallback(async (e: React.MouseEvent, workspaceId: string) => {
    e.stopPropagation();
    try {
      if (wsFilter === workspaceId) setWsFilter('all');
      await closeWorkspaceById(workspaceId);
    } catch (e) {
      log.error('Failed to close workspace', e);
    }
  }, [closeWorkspaceById, wsFilter]);

  const handleOpenSession = useCallback(async (session: Session) => {
    try {
      const ws = findWorkspaceForSession(session, recentWorkspaceScope);
      if (ws && !openedWorkspaceIdSet.has(ws.id)) {
        await switchWorkspace(ws);
      }
      const mustActivate = ws && ws.id !== currentWorkspace?.id;
      await openMainSession(session.sessionId, {
        workspaceId: ws?.id,
        activateWorkspace: mustActivate ? setActiveWorkspace : undefined,
      });
      closeTaskDetail();
      closeOverlay();
    } catch (e) {
      log.error('Failed to open session', e);
    }
  }, [
    recentWorkspaceScope,
    openedWorkspaceIdSet,
    switchWorkspace,
    currentWorkspace?.id,
    setActiveWorkspace,
    closeTaskDetail,
    closeOverlay,
  ]);

  const handleOpenLiveApp = useCallback((appId: string) => {
    openOverlay(`live-app:${appId}`);
    closeTaskDetail();
  }, [closeTaskDetail, openOverlay]);

  return (
    <div className="tds">
      <div className="tds-layout">

        {/* ── Left: title + search + dual panes (2×2 grid aligns OS | workspace headers) ─ */}
        <div className="tds-layout__left">
          <div className="tds-left-header">
            <h1 className="tds-left-header__title">{t('taskDetailScene.pageTitle')}</h1>
            <p className="tds-left-header__subtitle">{t('taskDetailScene.pageSubtitle')}</p>
          </div>

          <div className="tds-search-wrap">
            <Search
              className="tds-search"
              size="large"
              value={listQuery}
              onChange={setListQuery}
              placeholder={t('taskDetailScene.searchPlaceholder')}
              clearable
            />
          </div>

          <div className="tds-left-split-wrap">
            <div className="tds-left-split">
              {/* Row 1 — headers share one grid row so bottom borders align */}
              <div className="tds-left-pane tds-left-pane--os tds-left-pane--head">
                <div className="tds-pane-head">
                  <LayoutDashboard size={12} className="tds-pane-head__icon tds-pane-head__icon--dispatcher" />
                  <span className="tds-pane-head__title">{t('taskDetailScene.runHistoryTitle')}</span>
                  <span className="tds-pane-head__count">{dispatcherSessions.length}</span>
                </div>
              </div>
              <div className="tds-left-pane tds-left-pane--ws tds-left-pane--head">
                <div className="tds-pane-head">
                  <FolderOpen size={12} className="tds-pane-head__icon tds-pane-head__icon--ws" />
                  <span className="tds-pane-head__title">{t('taskDetailScene.recentWorkspacesTitle')}</span>
                  <span className="tds-pane-head__count">{filteredWorkspaces.length}</span>
                  <FilterPill
                    label={t('taskDetailScene.filterAll')}
                    active={wsFilter === 'all'}
                    onClick={() => setWsFilter('all')}
                    className="tds-pane-head__chip"
                  />
                  <IconButton
                    size="xs"
                    variant="ghost"
                    tooltip={t('taskDetailScene.openWorkspace')}
                    onClick={handleOpenNewWorkspace}
                    aria-label={t('taskDetailScene.openWorkspace')}
                  >
                    <FolderPlus size={12} />
                  </IconButton>
                </div>
              </div>
              {/* Row 2 — scrollable lists */}
              <div className="tds-left-pane tds-left-pane--os tds-left-pane--list">
                <div className="tds-pane-list">
                  {dispatcherSessions.length === 0 ? (
                    <div className="tds-empty tds-empty--compact">
                      <LayoutDashboard size={26} />
                      <p>{qNorm ? t('taskDetailScene.emptyRunHistoryFiltered') : t('taskDetailScene.emptyRunHistory')}</p>
                    </div>
                  ) : (
                    dispatcherSessions.map(s => (
                      <SessionRow
                        key={s.sessionId}
                        session={s}
                        isHighlighted={s.sessionId === taskDetailSessionId}
                        statusVariant={getStatusVariant(s, runningIds)}
                        showMode={false}
                        formatRelativeTime={formatRelativeTime}
                        onOpen={handleOpenSession}
                      />
                    ))
                  )}
                </div>
              </div>
              <div className="tds-left-pane tds-left-pane--ws tds-left-pane--list">
                <div className="tds-pane-list">
                  {filteredWorkspaces.length === 0 ? (
                    <div className="tds-empty tds-empty--compact">
                      <FolderOpen size={26} />
                      <p>{qNorm ? t('taskDetailScene.emptyWorkspacesFiltered') : t('taskDetailScene.emptyWorkspaces')}</p>
                    </div>
                  ) : (
                    filteredWorkspaces.map(ws => (
                      <WorkspaceRow
                        key={ws.id}
                        workspace={ws}
                        sessionCount={sessionCountByWorkspaceId.get(ws.id) ?? 0}
                        isActiveWorkspace={currentWorkspace?.id === ws.id}
                        isFilterSelected={wsFilter === ws.id}
                        isOpenedWorkspace={openedWorkspaceIdSet.has(ws.id)}
                        onSelect={handleWorkspacePaneSelect}
                        onClose={handleCloseWorkspace}
                      />
                    ))
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* ── Right rail: workspace execution sessions ─────────────────── */}
        <div className="tds-layout__right">
          <div className="tds-rail-shell">
            <div className="tds-rail-head">
              <Code2 size={13} className="tds-rail-head__icon" />
              <span className="tds-rail-head__title">{t('taskDetailScene.workspaceSessionsTitle')}</span>
              <span className="tds-rail-head__count">{filteredExec.length}</span>

              {totalRunningCount > 0 && (
                <span className="tds-rail-head__running">
                  <Radio size={9} />
                  {t('taskDetailScene.runningCount', { count: totalRunningCount })}
                </span>
              )}

              <div className="tds-rail-head__filters">
                {showWorkspaceLabelsOnSessions && (
                  <Select
                    className="tds-rail-select"
                    size="small"
                    options={wsSelectOptions}
                    value={wsFilter}
                    onChange={v => setWsFilter(String(v))}
                  />
                )}
                <FilterPillGroup>
                  {modeChips.map(chip => (
                    <FilterPill
                      key={chip.id}
                      label={chip.label}
                      active={modeFilter === chip.id}
                      onClick={() => setModeFilter(chip.id)}
                    />
                  ))}
                </FilterPillGroup>
              </div>
            </div>

            <div className="tds-rail-list">
              {filteredExec.length === 0 && runningLiveApps.length === 0 ? (
                <div className="tds-empty">
                  <Code2 size={32} />
                  <p>{execSessions.length === 0 ? t('taskDetailScene.emptyWorkspaceSessions') : t('taskDetailScene.emptySessionsFiltered')}</p>
                </div>
              ) : (
                <>
                  {runningLiveApps.map(app => (
                    <LiveAppRow
                      key={app.id}
                      app={app}
                      isHighlighted={activeOverlay === app.overlayId}
                      formatRelativeTime={formatRelativeTime}
                      onOpen={handleOpenLiveApp}
                    />
                  ))}
                  {filteredExec.map(({ session, workspace }) => (
                    <SessionRow
                      key={session.sessionId}
                      session={session}
                      isHighlighted={session.sessionId === taskDetailSessionId}
                      statusVariant={getStatusVariant(session, runningIds)}
                      showMode
                      workspaceName={
                        showWorkspaceLabelsOnSessions
                          ? openedWorkspaceIdSet.has(workspace.id)
                            ? workspace.name
                            : `${workspace.name || fallbackWorkspaceFolderLabel(workspace.rootPath)} · ${t('taskDetailScene.badgeRecent')}`
                          : undefined
                      }
                      formatRelativeTime={formatRelativeTime}
                      onOpen={handleOpenSession}
                    />
                  ))}
                </>
              )}
            </div>
          </div>
        </div>

      </div>
    </div>
  );
};

export default TaskDetailScene;
