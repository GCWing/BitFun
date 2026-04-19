/**
 * TaskDetailScene — Task Management Center.
 *
 * Assistant-style split: left (majority) = search + OS sessions | opened workspaces;
 * right rail = execution sessions for opened workspaces.
 */

import React, { useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
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
  Server,
  Trash2,
  X,
} from 'lucide-react';
import { Search, FilterPill, FilterPillGroup, IconButton, confirmDanger } from '@/component-library';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import type { WorkspaceInfo } from '@/shared/types';
import {
  compareSessionsForDisplay,
} from '@/flow_chat/utils/sessionOrdering';
import { fallbackWorkspaceFolderLabel } from '@/flow_chat/utils/sessionOrdering';
import { findWorkspaceForSession } from '@/flow_chat/utils/workspaceScope';
import { openMainSession } from '@/flow_chat/services/openBtwSession';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import { useSessionCapsuleStore } from '../../stores/sessionCapsuleStore';
import { useOverlayStore } from '../../stores/overlayStore';
import { stateMachineManager } from '@/flow_chat/state-machine';
import { SessionExecutionState } from '@/flow_chat/state-machine/types';
import type { FlowChatState, Session } from '@/flow_chat/types/flow-chat';
import { createLogger } from '@/shared/utils/logger';
import { useI18n } from '@/infrastructure/i18n';
import { SSHContext } from '@/features/ssh-remote/SSHRemoteContext';
import { renderLiveAppIcon } from '@/app/scenes/apps/live-app/liveAppIcons';
import { useRunningLiveAppItems, type RunningLiveAppItem } from '@/app/scenes/apps/live-app/liveAppTaskView';
import './TaskDetailScene.scss';

const log = createLogger('TaskDetailScene');
const RECENT_WORKSPACE_LIMIT = 7;

/** Agentic (dispatcher) session title: `2007.1.2` — dotted date, no leading zeros on month/day. */
function formatAgenticDotDate(ts: number): string {
  const d = new Date(ts);
  return `${d.getFullYear()}.${d.getMonth() + 1}.${d.getDate()}`;
}

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
  /** When set (e.g. dispatcher / Agentic OS run history), replaces title + id fallback. */
  displayTitle?: string;
  /** Delete control (e.g. Agentic OS run history); click does not trigger `onOpen`. */
  onDelete?: (e: React.MouseEvent) => void;
  isHighlighted: boolean;
  statusVariant: StatusVariant;
  showMode?: boolean;
  workspaceName?: string;
  formatRelativeTime: (ts: number) => string;
  onOpen: (s: Session) => void;
}

const SessionRow: React.FC<SessionRowProps> = ({
  session,
  displayTitle,
  onDelete,
  isHighlighted,
  statusVariant,
  showMode = true,
  workspaceName,
  formatRelativeTime: formatRel,
  onOpen,
}) => {
  const { t } = useI18n('common');
  const rowTitle =
    displayTitle?.trim() ||
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

      {onDelete ? (
        <IconButton
          size="xs"
          variant="ghost"
          className="tds-row__delete-btn"
          tooltip={t('nav.sessions.delete')}
          aria-label={t('nav.sessions.delete')}
          onClick={e => {
            e.stopPropagation();
            onDelete(e);
          }}
          onPointerDown={e => e.stopPropagation()}
        >
          <Trash2 size={12} />
        </IconButton>
      ) : null}

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

// ── Right rail grouping ───────────────────────────────────────────────────────

type ExecGroupingMode = 'workspace' | 'agent';

interface ExecSessionRow {
  session: Session;
  workspace: WorkspaceInfo;
  mode: ExecMode;
}

interface ExecSessionGroup {
  id: string;
  kind: ExecGroupingMode;
  title: string;
  mode?: ExecMode;
  rows: ExecSessionRow[];
}

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
  isCurrentWorkspace: boolean;
  isOpenedWorkspace: boolean;
  onSelect: (workspaceId: string) => void;
  onClose: (e: React.MouseEvent, workspaceId: string) => void;
}

const WorkspaceRow: React.FC<WorkspaceRowProps> = ({
  workspace,
  sessionCount,
  isCurrentWorkspace,
  isOpenedWorkspace,
  onSelect,
  onClose,
}) => {
  const { t } = useI18n('common');
  return (
    <div
      className={[
        'tds-ws-row',
        isCurrentWorkspace && 'is-current',
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
          {isCurrentWorkspace && <span className="tds-ws-row__badge">{t('taskDetailScene.badgeCurrent')}</span>}
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

// ── Open workspace: local folder vs SSH (menu) ───────────────────────────────

interface TaskDetailOpenWorkspaceMenuProps {
  onOpenLocal: () => void;
  onOpenRemote: () => void;
  remoteAvailable: boolean;
}

const TaskDetailOpenWorkspaceMenu: React.FC<TaskDetailOpenWorkspaceMenuProps> = ({
  onOpenLocal,
  onOpenRemote,
  remoteAvailable,
}) => {
  const { t } = useI18n('common');
  const [open, setOpen] = useState(false);
  const anchorRef = useRef<HTMLDivElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);

  const updatePos = useCallback(() => {
    const el = anchorRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const menuWidth = 216;
    const pad = 8;
    setPos({
      top: rect.bottom + 4,
      left: Math.max(pad, Math.min(rect.left, window.innerWidth - menuWidth - pad)),
    });
  }, []);

  useEffect(() => {
    if (!open) return;
    updatePos();
    const onViewport = () => updatePos();
    window.addEventListener('resize', onViewport);
    window.addEventListener('scroll', onViewport, true);
    return () => {
      window.removeEventListener('resize', onViewport);
      window.removeEventListener('scroll', onViewport, true);
    };
  }, [open, updatePos]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (anchorRef.current?.contains(target)) return;
      if (popoverRef.current?.contains(target)) return;
      setOpen(false);
    };
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
  }, [open]);

  return (
    <div className="tds-open-ws-menu" ref={anchorRef}>
      <IconButton
        size="xs"
        variant="ghost"
        tooltip={t('taskDetailScene.openWorkspaceMenu')}
        aria-label={t('taskDetailScene.openWorkspaceMenu')}
        aria-expanded={open}
        aria-haspopup="menu"
        onClick={() => setOpen(v => !v)}
      >
        <FolderPlus size={12} />
      </IconButton>
      {open && pos
        ? createPortal(
            <div
              ref={popoverRef}
              className="tds-open-ws-popover"
              style={{ top: pos.top, left: pos.left }}
              role="menu"
            >
              <button
                type="button"
                className="tds-open-ws-popover__item"
                role="menuitem"
                onClick={() => {
                  setOpen(false);
                  onOpenLocal();
                }}
              >
                <FolderOpen size={14} className="tds-open-ws-popover__icon" aria-hidden />
                <span>{t('taskDetailScene.openWorkspaceLocal')}</span>
              </button>
              <button
                type="button"
                className="tds-open-ws-popover__item"
                role="menuitem"
                disabled={!remoteAvailable}
                title={
                  remoteAvailable ? undefined : t('taskDetailScene.openWorkspaceRemoteUnavailable')
                }
                onClick={() => {
                  if (!remoteAvailable) return;
                  setOpen(false);
                  onOpenRemote();
                }}
              >
                <Server size={14} className="tds-open-ws-popover__icon" aria-hidden />
                <span>{t('taskDetailScene.openWorkspaceRemoteSsh')}</span>
              </button>
            </div>,
            document.body
          )
        : null}
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
  const sshContext = useContext(SSHContext);
  const sshOpenAvailable =
    typeof window !== 'undefined' &&
    '__TAURI__' in window &&
    Boolean(sshContext?.setShowConnectionDialog);

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

  const dispatcherSessionTitle = useCallback(
    (s: Session) => {
      const modifiedAt = s.updatedAt ?? s.lastActiveAt;
      return t('taskDetailScene.agenticSessionTitle', {
        created: formatAgenticDotDate(s.createdAt),
        modified: formatAgenticDotDate(modifiedAt),
      });
    },
    [t]
  );

  const groupingChips = useMemo<Array<{ id: ExecGroupingMode; label: string }>>(
    () => [
      { id: 'workspace', label: t('taskDetailScene.groupByWorkspace') },
      { id: 'agent', label: t('taskDetailScene.groupByAgent') },
    ],
    [t]
  );

  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() => flowChatStore.getState());
  const [runningIds, setRunningIds] = useState<Set<string>>(new Set());
  const [execGroupingMode, setExecGroupingMode] = useState<ExecGroupingMode>('workspace');
  const [listQuery, setListQuery] = useState('');
  const [execQuery, setExecQuery] = useState('');
  const workspaceOrderRef = useRef<Map<string, number>>(new Map());

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
  const execQNorm = useMemo(() => normalizeQuery(execQuery), [execQuery]);

  const dispatcherSessions = useMemo(
    () => Array.from(flowChatState.sessions.values())
      .filter(s => s.mode?.toLowerCase() === 'dispatcher')
      .filter(s => matchesQuery(dispatcherSessionTitle(s), qNorm))
      .sort(compareSessionsForDisplay),
    [flowChatState.sessions, qNorm, dispatcherSessionTitle]
  );

  const recentWorkspaceScope = useMemo(() => {
    const scope = new Map<string, WorkspaceInfo>();
    openedWorkspacesList.forEach(workspace => {
      scope.set(workspace.id, workspace);
    });
    recentWorkspaces.slice(0, RECENT_WORKSPACE_LIMIT).forEach(workspace => {
      if (!scope.has(workspace.id)) {
        scope.set(workspace.id, workspace);
      }
    });
    const values = Array.from(scope.values());
    const order = workspaceOrderRef.current;

    values.forEach(workspace => {
      if (!order.has(workspace.id)) {
        order.set(workspace.id, order.size);
      }
    });

    return values.sort((a, b) => (order.get(a.id) ?? 0) - (order.get(b.id) ?? 0));
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
        return { session, workspace: ws ?? null, mode: resolveExecMode(session) };
      })
      .filter((row): row is ExecSessionRow & { workspace: WorkspaceInfo } => row.workspace !== null);
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

  const showWorkspaceLabelsOnSessions =
    execGroupingMode === 'agent' && recentWorkspaceScope.length > 1;

  const visibleExecRows = useMemo(() => {
    return execSessions.filter(({ session, workspace, mode }) => {
      if (!qNorm && !execQNorm) return true;
      const matchesGlobalQuery = !qNorm || (
        matchesQuery(sessionDisplayTitle(session), qNorm) ||
        matchesQuery(workspace.name, qNorm) ||
        matchesQuery(MODE_LABELS[mode], qNorm) ||
        matchesQuery(session.config.agentType ?? '', qNorm)
      );
      const matchesExecQuery = !execQNorm || (
        matchesQuery(sessionDisplayTitle(session), execQNorm) ||
        matchesQuery(workspace.name, execQNorm) ||
        matchesQuery(MODE_LABELS[mode], execQNorm) ||
        matchesQuery(session.config.agentType ?? '', execQNorm)
      );
      return matchesGlobalQuery && matchesExecQuery;
    });
  }, [execSessions, qNorm, execQNorm, sessionDisplayTitle]);

  const groupedExecSessions = useMemo<ExecSessionGroup[]>(() => {
    const groups = new Map<string, ExecSessionGroup>();

    for (const row of visibleExecRows) {
      const key = execGroupingMode === 'workspace'
        ? `workspace:${row.workspace.id}`
        : `agent:${row.mode}`;
      const group = groups.get(key);

      if (group) {
        group.rows.push(row);
        continue;
      }

      const title = execGroupingMode === 'workspace'
        ? (row.workspace.name || fallbackWorkspaceFolderLabel(row.workspace.rootPath))
        : MODE_LABELS[row.mode];

      groups.set(key, {
        id: key,
        kind: execGroupingMode,
        title,
        mode: execGroupingMode === 'agent' ? row.mode : undefined,
        rows: [row],
      });
    }

    return Array.from(groups.values());
  }, [visibleExecRows, execGroupingMode]);

  const runningExecCount = useMemo(
    () => visibleExecRows.filter(({ session }) => runningIds.has(session.sessionId)).length,
    [visibleExecRows, runningIds]
  );
  const totalRunningCount = runningExecCount + runningLiveApps.length;

  const handleWorkspacePaneSelect = useCallback(async (workspaceId: string) => {
    const targetWorkspace = recentWorkspaceScope.find(workspace => workspace.id === workspaceId);
    if (!targetWorkspace) return;

    try {
      if (openedWorkspaceIdSet.has(targetWorkspace.id)) {
        if (currentWorkspace?.id !== targetWorkspace.id) {
          await setActiveWorkspace(targetWorkspace.id);
        }
        return;
      }

      await switchWorkspace(targetWorkspace);
    } catch (e) {
      log.error('Failed to switch workspace from task detail list', e);
    }
  }, [recentWorkspaceScope, openedWorkspaceIdSet, currentWorkspace?.id, setActiveWorkspace, switchWorkspace]);

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
      await closeWorkspaceById(workspaceId);
    } catch (e) {
      log.error('Failed to close workspace', e);
    }
  }, [closeWorkspaceById]);

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

  const handleDeleteDispatcherSession = useCallback(async (session: Session, e: React.MouseEvent) => {
    e.stopPropagation();
    const label = dispatcherSessionTitle(session);
    const ok = await confirmDanger(
      t('taskDetailScene.deleteAgenticSessionTitle'),
      t('taskDetailScene.deleteAgenticSessionMessage', { label }),
      {
        confirmText: t('nav.sessions.delete'),
        cancelText: t('actions.cancel'),
      }
    );
    if (!ok) return;
    try {
      await flowChatManager.deleteChatSession(session.sessionId);
    } catch (err) {
      log.error('Failed to delete Agentic OS session', err);
    }
  }, [dispatcherSessionTitle, t]);

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
                  <TaskDetailOpenWorkspaceMenu
                    onOpenLocal={() => {
                      void handleOpenNewWorkspace();
                    }}
                    onOpenRemote={() => sshContext?.setShowConnectionDialog(true)}
                    remoteAvailable={sshOpenAvailable}
                  />
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
                        displayTitle={dispatcherSessionTitle(s)}
                        onDelete={e => {
                          void handleDeleteDispatcherSession(s, e);
                        }}
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
                        isCurrentWorkspace={currentWorkspace?.id === ws.id}
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
              <span className="tds-rail-head__count">{visibleExecRows.length}</span>

              {totalRunningCount > 0 && (
                <span className="tds-rail-head__running">
                  <Radio size={9} />
                  {t('taskDetailScene.runningCount', { count: totalRunningCount })}
                </span>
              )}

              <div className="tds-rail-head__filters">
                <Search
                  className="tds-rail-search__input"
                  size="small"
                  value={execQuery}
                  onChange={setExecQuery}
                  placeholder={t('taskDetailScene.searchSessionsPlaceholder')}
                  clearable
                />
                <FilterPillGroup>
                  {groupingChips.map(chip => (
                    <FilterPill
                      key={chip.id}
                      label={chip.label}
                      active={execGroupingMode === chip.id}
                      onClick={() => setExecGroupingMode(chip.id)}
                    />
                  ))}
                </FilterPillGroup>
              </div>
            </div>

            <div className="tds-rail-list">
              {visibleExecRows.length === 0 && runningLiveApps.length === 0 ? (
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
                  {groupedExecSessions.map(group => (
                    <div key={group.id} className="tds-rail-group">
                      <div className="tds-rail-group__head">
                        {group.kind === 'workspace' ? (
                          <FolderOpen size={12} className="tds-rail-group__icon tds-rail-group__icon--workspace" />
                        ) : (
                          <ModeIcon mode={group.mode ?? 'code'} size={12} className={`tds-rail-group__icon tds-rail-group__icon--${group.mode ?? 'code'}`} />
                        )}
                        <span className="tds-rail-group__title">{group.title}</span>
                        <span className="tds-rail-group__count">{group.rows.length}</span>
                      </div>
                      <div className="tds-rail-group__list">
                        {group.rows.map(({ session, workspace }) => (
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
                      </div>
                    </div>
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
