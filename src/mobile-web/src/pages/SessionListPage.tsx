import React, { useEffect, useRef, useCallback, useState } from 'react';
import LanguageToggleButton from '../components/LanguageToggleButton';
import { useI18n } from '../i18n';
import { RemoteSessionManager, type RecentWorkspaceEntry, type SessionInfo } from '../services/RemoteSessionManager';
import { useMobileStore } from '../services/store';
import { useTheme } from '../theme';
import logoIcon from '../assets/Logo-ICON.png';

const PAGE_SIZE = 30;

type DisplayMode = 'pro' | 'assistant';

interface SessionListPageProps {
  sessionMgr: RemoteSessionManager;
  onSelectSession: (sessionId: string, sessionName?: string, isNew?: boolean) => void;
  onOpenWorkspace: () => void;
  onDisconnect: () => void;
  onOpenDevices?: () => void;
}

function formatTime(
  unixStr: string,
  formatDate: (date: Date | number, options?: Intl.DateTimeFormatOptions) => string,
  t: (key: string, params?: Record<string, string | number>) => string,
): string {
  const ts = parseInt(unixStr, 10);
  if (!ts || isNaN(ts)) return '';
  const date = new Date(ts * 1000);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 1) return t('common.justNow');
  if (diffMin < 60) return t('common.minutesAgo', { count: diffMin });
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return t('common.hoursAgo', { count: diffHr });
  const diffDay = Math.floor(diffHr / 24);
  if (diffDay < 7) return t('common.daysAgo', { count: diffDay });
  return formatDate(date);
}

function agentLabel(agentType: string, t: (key: string) => string): string {
  switch (agentType) {
    case 'code':
    case 'agentic':
      return t('sessions.agentCode');
    case 'cowork':
    case 'Cowork':
      return t('sessions.agentCowork');
    case 'claw':
    case 'Claw':
      return t('shared.agents.claw');
    default:
      return agentType || t('sessions.agentDefault');
  }
}

function isCoworkAgent(agentType: string): boolean {
  return agentType === 'cowork' || agentType === 'Cowork';
}

function isClawAgent(agentType: string): boolean {
  return agentType === 'claw' || agentType === 'Claw';
}

/** Pick first workspace suitable for Expert mode (exclude Claw assistant roots when kind is known). */
function pickFirstProWorkspace(list: RecentWorkspaceEntry[]): RecentWorkspaceEntry | undefined {
  if (list.length === 0) return undefined;
  const anyKind = list.some((w) => w.workspace_kind != null);
  if (anyKind) {
    return list.find((w) => w.workspace_kind !== 'assistant');
  }
  return list[0];
}

function truncateMiddle(str: string, maxLen: number): string {
  if (!str || str.length <= maxLen) return str;
  const keep = maxLen - 3;
  const head = Math.ceil(keep * 0.6);
  const tail = keep - head;
  return str.slice(0, head) + '...' + str.slice(-tail);
}

function SessionTypeIcon({ agentType }: { agentType: string }) {
  if (isCoworkAgent(agentType)) {
    return (
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
        <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />
        <circle cx="9" cy="7" r="4" />
        <path d="M22 21v-2a4 4 0 0 0-3-3.87" />
        <path d="M16 3.13a4 4 0 0 1 0 7.75" />
      </svg>
    );
  }

  if (isClawAgent(agentType)) {
    return (
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
        <rect width="20" height="14" x="2" y="5" rx="2" />
        <path d="M2 10h20" />
      </svg>
    );
  }

  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
    </svg>
  );
}

/* Mode Selection Icons */
const ProModeIcon = () => (
  <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="4 17 10 11 4 5" />
    <line x1="12" y1="19" x2="20" y2="19" />
  </svg>
);

const AssistantModeIcon = () => (
  <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
    <path d="M12 8V4H8" />
    <rect width="16" height="12" x="4" y="8" rx="2" />
    <path d="M2 14h2" />
    <path d="M20 14h2" />
    <path d="M15 13v2" />
    <path d="M9 13v2" />
  </svg>
);

const WorkspaceIcon = () => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
    <path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.93a2 2 0 0 1 1.66.9l.82 1.2a2 2 0 0 0 1.66.9H18a2 2 0 0 1 2 2v2"/>
  </svg>
);

const ThemeToggleIcon: React.FC<{ isDark: boolean }> = ({ isDark }) => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
    {isDark ? (
      <path d="M8 1.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM3 8a5 5 0 0 1 5-5v10a5 5 0 0 1-5-5Z" fill="currentColor"/>
    ) : (
      <path d="M8 1a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-1 0v-1A.5.5 0 0 1 8 1Zm0 11a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-1 0v-1A.5.5 0 0 1 8 12Zm7-4a.5.5 0 0 1-.5.5h-1a.5.5 0 0 1 0-1h1A.5.5 0 0 1 15 8ZM3 8a.5.5 0 0 1-.5.5h-1a.5.5 0 0 1 0-1h1A.5.5 0 0 1 3 8Zm9.95-3.54a.5.5 0 0 1 0 .71l-.71.7a.5.5 0 1 1-.7-.7l.7-.71a.5.5 0 0 1 .71 0ZM5.46 11.24a.5.5 0 0 1 0 .71l-.7.71a.5.5 0 0 1-.71-.71l.7-.71a.5.5 0 0 1 .71 0Zm7.08 1.42a.5.5 0 0 1-.7 0l-.71-.71a.5.5 0 0 1 .7-.7l.71.7a.5.5 0 0 1 0 .71ZM5.46 4.76a.5.5 0 0 1-.71 0l-.71-.7a.5.5 0 0 1 .71-.71l.7.7a.5.5 0 0 1 0 .71ZM8 5a3 3 0 1 1 0 6 3 3 0 0 1 0-6Z" fill="currentColor"/>
    )}
  </svg>
);

const SessionListPage: React.FC<SessionListPageProps> = ({ sessionMgr, onSelectSession, onOpenWorkspace, onDisconnect, onOpenDevices }) => {
  const { t, formatDate } = useI18n();
  const {
    sessions,
    setSessions,
    appendSessions,
    setError,
    currentWorkspace,
    setCurrentWorkspace,
    currentAssistant,
    setCurrentAssistant,
    setPairedDisplayMode,
    authenticatedUserId,
    connectionHealth,
    controlTarget,
  } = useMobileStore();
  const { isDark, toggleTheme } = useTheme();
  const [creating, setCreating] = useState(false);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [hasMore, setHasMore] = useState(false);
  const [displayMode, setDisplayMode] = useState<DisplayMode>(() => {
    const hint = useMobileStore.getState().pairedDisplayMode;
    if (hint === 'assistant' || hint === 'pro') return hint;
    return 'pro';
  });

  const [assistantList, setAssistantList] = useState<Array<{ path: string; name: string; assistant_id?: string }>>([]);
  const [showAssistantPicker, setShowAssistantPicker] = useState(false);
  const [workspaceList, setWorkspaceList] = useState<Array<{
    path: string;
    name: string;
    last_opened: string;
    workspace_kind?: 'normal' | 'assistant' | 'remote';
    remote_connection_id?: string;
    remote_ssh_host?: string;
  }>>([]);
  const [showWorkspacePicker, setShowWorkspacePicker] = useState(false);

  // Search, rename & delete state
  const [searchQuery, setSearchQuery] = useState('');
  const [menuSession, setMenuSession] = useState<SessionInfo | null>(null);
  const [renameTarget, setRenameTarget] = useState<SessionInfo | null>(null);
  const [renameValue, setRenameValue] = useState('');
  const [deleteConfirmTarget, setDeleteConfirmTarget] = useState<SessionInfo | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [actionToast, setActionToast] = useState<string | null>(null);

  const [showDisconnectConfirm, setShowDisconnectConfirm] = useState(false);

  const longPressTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const longPressPosRef = useRef({ x: 0, y: 0 });
  const longPressTriggeredRef = useRef(false);
  const toastTimerRef = useRef<ReturnType<typeof setTimeout>>();

  const hasSearchQuery = searchQuery.trim().length > 0;
  // Show the resume card as soon as session data is available — don't gate it
  // behind `loading`, otherwise a background refresh hides the card and makes it
  // pop back in after the network round-trip, lagging behind the rest of the UI.
  const showResumeCard = sessions.length > 0 && !hasSearchQuery;

  // ── Long-press context menu ─────────────────────────────────────
  const clearLongPressTimer = () => {
    if (longPressTimerRef.current) {
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = undefined;
    }
  };

  const handleSessionTouchStart = useCallback((s: SessionInfo, e: React.TouchEvent) => {
    if (deleting || renaming) return;
    clearLongPressTimer();
    longPressTriggeredRef.current = false;
    longPressPosRef.current = { x: e.touches[0].clientX, y: e.touches[0].clientY };
    longPressTimerRef.current = setTimeout(() => {
      longPressTriggeredRef.current = true;
      setMenuSession(s);
      longPressTimerRef.current = undefined;
    }, 500);
  }, [deleting, renaming]);

  const handleSessionTouchMove = useCallback((e: React.TouchEvent) => {
    const dx = Math.abs(e.touches[0].clientX - longPressPosRef.current.x);
    const dy = Math.abs(e.touches[0].clientY - longPressPosRef.current.y);
    if (dx > 10 || dy > 10) {
      clearLongPressTimer();
    }
  }, []);

  const handleSessionTouchEnd = useCallback(() => {
    clearLongPressTimer();
  }, []);

  const handleSessionClick = useCallback((s: SessionInfo, e: React.MouseEvent) => {
    if (longPressTriggeredRef.current) {
      e.preventDefault();
      e.stopPropagation();
      longPressTriggeredRef.current = false;
      return;
    }
    onSelectSession(s.session_id, s.name);
  }, [onSelectSession]);

  // ── Session actions ─────────────────────────────────────────────
  const showToast = useCallback((msg: string) => {
    if (toastTimerRef.current) clearTimeout(toastTimerRef.current);
    setActionToast(msg);
    toastTimerRef.current = setTimeout(() => setActionToast(null), 2500);
  }, []);

  // Cleanup timers on unmount
  useEffect(() => {
    return () => {
      clearLongPressTimer();
      if (toastTimerRef.current) clearTimeout(toastTimerRef.current);
    };
  }, []);

  const handleRename = useCallback(async () => {
    if (!renameTarget || !renameValue.trim()) return;
    setRenaming(true);
    try {
      await sessionMgr.renameSession(renameTarget.session_id, renameValue.trim());
      useMobileStore.getState().updateSessionName(renameTarget.session_id, renameValue.trim());
      setRenameTarget(null);
      setMenuSession(null);
    } catch (e: any) {
      showToast(e.message || t('sessions.renameFailed'));
    } finally {
      setRenaming(false);
    }
  }, [renameTarget, renameValue, sessionMgr, showToast, t]);

  const handleDelete = useCallback(async () => {
    if (!deleteConfirmTarget) return;
    setDeleting(true);
    try {
      await sessionMgr.deleteSession(deleteConfirmTarget.session_id);
      useMobileStore.getState().removeSession(deleteConfirmTarget.session_id);
      setDeleteConfirmTarget(null);
      setMenuSession(null);
      showToast(t('sessions.deleted'));
    } catch (e: any) {
      showToast(e.message || t('sessions.deleteFailed'));
    } finally {
      setDeleting(false);
    }
  }, [deleteConfirmTarget, sessionMgr, showToast, t]);

  const [pullDistance, setPullDistance] = useState(0);
  const [refreshing, setRefreshing] = useState(false);
  const offsetRef = useRef(0);
  const listRef = useRef<HTMLDivElement>(null);
  const listRequestSeqRef = useRef(0);
  const initLoadedPathRef = useRef<string | undefined>(undefined);
  const touchStartY = useRef(0);
  const isPulling = useRef(false);

  // Load assistant list when entering assistant mode
  const loadAssistantList = useCallback(async () => {
    try {
      const assistants = await sessionMgr.listAssistants();
      setAssistantList(assistants);
      // Set default assistant if none selected
      if (!currentAssistant && assistants.length > 0) {
        const defaultAssistant = assistants.find(a => !a.assistant_id) || assistants[0];
        setCurrentAssistant(defaultAssistant);
        return defaultAssistant.path;
      }
      return currentAssistant?.path;
    } catch (e: any) {
      setError(e.message);
      return undefined;
    }
  }, [sessionMgr, currentAssistant, setCurrentAssistant, setError]);

  const loadFirstPage = useCallback(async (
    workspacePath: string | undefined,
    query = '',
    identity?: { remoteConnectionId?: string; remoteSshHost?: string },
  ) => {
    const requestSeq = ++listRequestSeqRef.current;
    setLoading(true);
    offsetRef.current = 0;
    try {
      const resp = await sessionMgr.listSessions(
        workspacePath,
        PAGE_SIZE,
        0,
        query,
        identity,
      );
      if (requestSeq !== listRequestSeqRef.current) return;
      setSessions(resp.sessions);
      setHasMore(resp.has_more);
      offsetRef.current = resp.sessions.length;
    } catch (e: any) {
      if (requestSeq !== listRequestSeqRef.current) return;
      setError(e.message);
    } finally {
      if (requestSeq === listRequestSeqRef.current) {
        setLoading(false);
      }
    }
  }, [sessionMgr, setSessions, setError]);

  // Load workspace list for Pro mode picker
  const loadWorkspaceList = useCallback(async () => {
    try {
      const workspaces = await sessionMgr.listRecentWorkspaces();
      setWorkspaceList(workspaces);
    } catch (e: any) {
      setError(e.message);
    }
  }, [sessionMgr, setError]);

  const handleSelectWorkspace = useCallback(async (workspace: {
    path: string;
    name: string;
    remote_connection_id?: string;
    remote_ssh_host?: string;
  }) => {
    try {
      const result = await sessionMgr.setWorkspace(workspace.path, {
        remoteConnectionId: workspace.remote_connection_id,
        remoteSshHost: workspace.remote_ssh_host,
      });
      if (result.success) {
        const path = result.path || workspace.path;
        const remoteConnectionId =
          result.remote_connection_id ?? workspace.remote_connection_id;
        const remoteSshHost = result.remote_ssh_host ?? workspace.remote_ssh_host;
        const identity = { remoteConnectionId, remoteSshHost };
        setCurrentWorkspace({
          has_workspace: true,
          path,
          project_name: result.project_name || workspace.name,
          workspace_kind: remoteConnectionId || remoteSshHost
            ? 'remote'
            : undefined,
          remote_connection_id: remoteConnectionId,
          remote_ssh_host: remoteSshHost,
        });
        setShowWorkspacePicker(false);
        loadFirstPage(path, searchQuery, identity);
      } else {
        setError(result.error || 'Failed to set workspace');
      }
    } catch (e: any) {
      setError(e.message);
    }
  }, [sessionMgr, setCurrentWorkspace, setError, loadFirstPage, searchQuery]);

  const trySelectFirstProWorkspace = useCallback(async (): Promise<boolean> => {
    try {
      const list = await sessionMgr.listRecentWorkspaces();
      const candidate = pickFirstProWorkspace(list);
      if (!candidate) return false;
      const result = await sessionMgr.setWorkspace(candidate.path, {
        remoteConnectionId: candidate.remote_connection_id,
        remoteSshHost: candidate.remote_ssh_host,
      });
      if (result.success) {
        const path = result.path || candidate.path;
        const remoteConnectionId =
          result.remote_connection_id ?? candidate.remote_connection_id;
        const remoteSshHost = result.remote_ssh_host ?? candidate.remote_ssh_host;
        const identity = { remoteConnectionId, remoteSshHost };
        setCurrentWorkspace({
          has_workspace: true,
          path,
          project_name: result.project_name || candidate.name,
          workspace_kind: remoteConnectionId || remoteSshHost
            ? 'remote'
            : candidate.workspace_kind,
          remote_connection_id: remoteConnectionId,
          remote_ssh_host: remoteSshHost,
        });
        await loadFirstPage(path, searchQuery, identity);
        return true;
      }
      setError(result.error || t('workspace.failedToSetWorkspace'));
      return false;
    } catch (e: any) {
      setError(e.message);
      return false;
    }
  }, [sessionMgr, setCurrentWorkspace, setError, loadFirstPage, searchQuery, t]);

  const loadNextPage = useCallback(async (
    workspacePath: string | undefined,
    query = '',
    identity?: { remoteConnectionId?: string; remoteSshHost?: string },
  ) => {
    if (loadingMore || !hasMore) return;
    const requestSeq = listRequestSeqRef.current;
    setLoadingMore(true);
    try {
      const resp = await sessionMgr.listSessions(
        workspacePath,
        PAGE_SIZE,
        offsetRef.current,
        query,
        identity,
      );
      if (requestSeq !== listRequestSeqRef.current) return;
      appendSessions(resp.sessions);
      setHasMore(resp.has_more);
      offsetRef.current += resp.sessions.length;
    } catch (e: any) {
      if (requestSeq !== listRequestSeqRef.current) return;
      setError(e.message);
    } finally {
      setLoadingMore(false);
    }
  }, [sessionMgr, appendSessions, setError, loadingMore, hasMore]);

  useEffect(() => {
    let cancelled = false;
    const init = async () => {
      try {
        const info = await sessionMgr.getWorkspaceInfo();
        if (cancelled) return;
        if (info.workspace_kind === 'assistant' && info.path) {
          setCurrentAssistant({
            path: info.path,
            name: info.project_name ?? 'Claw',
            assistant_id: info.assistant_id,
          });
          setCurrentWorkspace(null);
          setDisplayMode('assistant');
          initLoadedPathRef.current = info.path;
          await loadFirstPage(info.path);
        } else {
          const ws = info.has_workspace ? info : null;
          setCurrentWorkspace(ws);
          if (ws?.path) {
            initLoadedPathRef.current = ws.path;
            await loadFirstPage(ws.path, '', {
              remoteConnectionId: ws.remote_connection_id,
              remoteSshHost: ws.remote_ssh_host,
            });
          } else {
            await trySelectFirstProWorkspace();
          }
        }
      } catch (e: any) {
        if (!cancelled) setError(e.message);
      } finally {
        if (!cancelled) setPairedDisplayMode(null);
      }
    };
    init();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const refreshData = useCallback(async () => {
    const requestSeq = ++listRequestSeqRef.current;
    try {
      if (displayMode === 'pro') {
        const info = await sessionMgr.getWorkspaceInfo();
        if (info.workspace_kind === 'assistant') {
          setCurrentWorkspace(null);
          setSessions([]);
          setHasMore(false);
          offsetRef.current = 0;
          return;
        }
        const ws = info.has_workspace ? info : null;
        setCurrentWorkspace(ws);
        const resp = await sessionMgr.listSessions(ws?.path, PAGE_SIZE, 0, searchQuery, {
          remoteConnectionId: ws?.remote_connection_id,
          remoteSshHost: ws?.remote_ssh_host,
        });
        if (requestSeq !== listRequestSeqRef.current) return;
        setSessions(resp.sessions);
        setHasMore(resp.has_more);
        offsetRef.current = resp.sessions.length;
      } else {
        // Assistant mode: use currentAssistant path
        const resp = await sessionMgr.listSessions(currentAssistant?.path, PAGE_SIZE, 0, searchQuery);
        if (requestSeq !== listRequestSeqRef.current) return;
        setSessions(resp.sessions);
        setHasMore(resp.has_more);
        offsetRef.current = resp.sessions.length;
      }
    } catch { /* ignore */ }
  }, [sessionMgr, setSessions, setCurrentWorkspace, currentAssistant?.path, displayMode, searchQuery]);

  useEffect(() => {
    const poll = setInterval(refreshData, 10000);
    return () => clearInterval(poll);
  }, [refreshData]);

  useEffect(() => {
    const workspacePath = displayMode === 'assistant' ? currentAssistant?.path : currentWorkspace?.path;
    if (!workspacePath) return;
    // Skip the redundant first load when init() already loaded this path —
    // otherwise the state change from init() triggers a second loadFirstPage
    // 250 ms later, causing an extra network round-trip and a loading flicker.
    if (initLoadedPathRef.current === workspacePath) {
      initLoadedPathRef.current = undefined;
      return;
    }
    const identity = displayMode === 'assistant'
      ? undefined
      : {
          remoteConnectionId: currentWorkspace?.remote_connection_id,
          remoteSshHost: currentWorkspace?.remote_ssh_host,
        };
    const timer = setTimeout(() => {
      loadFirstPage(workspacePath, searchQuery, identity);
    }, 250);
    return () => clearTimeout(timer);
  }, [
    currentAssistant?.path,
    currentWorkspace?.path,
    currentWorkspace?.remote_connection_id,
    currentWorkspace?.remote_ssh_host,
    displayMode,
    loadFirstPage,
    searchQuery,
  ]);

  const PULL_THRESHOLD = 60;

  const handleTouchStart = useCallback((e: React.TouchEvent) => {
    const el = listRef.current;
    if (!el || el.scrollTop > 0 || refreshing) return;
    touchStartY.current = e.touches[0].clientY;
    isPulling.current = true;
  }, [refreshing]);

  const handleTouchMove = useCallback((e: React.TouchEvent) => {
    if (!isPulling.current) return;
    const delta = e.touches[0].clientY - touchStartY.current;
    if (delta > 0) {
      setPullDistance(Math.min(delta * 0.5, 80));
    } else {
      isPulling.current = false;
      setPullDistance(0);
    }
  }, []);

  const handleTouchEnd = useCallback(async () => {
    if (!isPulling.current) return;
    isPulling.current = false;
    if (pullDistance >= PULL_THRESHOLD) {
      setRefreshing(true);
      setPullDistance(PULL_THRESHOLD);
      await refreshData();
      setRefreshing(false);
    }
    setPullDistance(0);
  }, [pullDistance, refreshData]);

  const handleScroll = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 150) {
      const workspacePath = displayMode === 'assistant' ? currentAssistant?.path : currentWorkspace?.path;
      const identity = displayMode === 'assistant'
        ? undefined
        : {
            remoteConnectionId: currentWorkspace?.remote_connection_id,
            remoteSshHost: currentWorkspace?.remote_ssh_host,
          };
      loadNextPage(workspacePath, searchQuery, identity);
    }
  }, [
    displayMode,
    currentAssistant?.path,
    currentWorkspace?.path,
    currentWorkspace?.remote_connection_id,
    currentWorkspace?.remote_ssh_host,
    loadNextPage,
    searchQuery,
  ]);

  const handleCreate = useCallback(async (agentType: string) => {
    if (creating) return;
    setCreating(true);
    try {
      // For assistant mode (Claw), use currentAssistant.path
      // For pro mode (Code/Cowork), use currentWorkspace.path
      const workspacePath = displayMode === 'assistant' ? currentAssistant?.path : currentWorkspace?.path;
      const identity = displayMode === 'assistant'
        ? undefined
        : {
            remoteConnectionId: currentWorkspace?.remote_connection_id,
            remoteSshHost: currentWorkspace?.remote_ssh_host,
          };
      const id = await sessionMgr.createSession(agentType, undefined, workspacePath, identity);
      await loadFirstPage(workspacePath, searchQuery, identity);
      const label = isClawAgent(agentType)
        ? t('sessions.remoteClawSession')
        : isCoworkAgent(agentType)
          ? t('sessions.remoteCoworkSession')
          : t('sessions.remoteCodeSession');
      onSelectSession(id, label, true);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setCreating(false);
    }
  }, [
    creating,
    currentWorkspace?.path,
    currentWorkspace?.remote_connection_id,
    currentWorkspace?.remote_ssh_host,
    currentAssistant?.path,
    displayMode,
    loadFirstPage,
    onSelectSession,
    searchQuery,
    sessionMgr,
    setError,
    t,
  ]);

  const handleSelectMode = useCallback(async (mode: DisplayMode) => {
    setDisplayMode(mode);
    setShowAssistantPicker(false);
    if (mode === 'assistant') {
      const assistantPath = await loadAssistantList();
      loadFirstPage(assistantPath, searchQuery);
    } else {
      if (currentWorkspace?.path) {
        await loadFirstPage(currentWorkspace.path, searchQuery, {
          remoteConnectionId: currentWorkspace.remote_connection_id,
          remoteSshHost: currentWorkspace.remote_ssh_host,
        });
      } else {
        await trySelectFirstProWorkspace();
      }
    }
  }, [currentWorkspace?.path, loadFirstPage, loadAssistantList, searchQuery, trySelectFirstProWorkspace]);

  const handleSelectAssistant = useCallback(async (assistant: { path: string; name: string; assistant_id?: string }) => {
    try {
      await sessionMgr.setAssistant(assistant.path);
      setCurrentAssistant(assistant);
      setShowAssistantPicker(false);
      loadFirstPage(assistant.path, searchQuery);
    } catch (e: any) {
      setError(e.message);
    }
  }, [sessionMgr, setCurrentAssistant, setError, loadFirstPage, searchQuery]);

  const workspaceDisplayName = currentWorkspace?.project_name || t('sessions.noWorkspaceSelected');
  const assistantDisplayName = currentAssistant?.name || t('shared.agents.default');
  const isProMode = displayMode === 'pro';

  return (
    <div className="session-list">
      <div className="session-list__header">
        <div className="session-list__header-brand">
          <img src={logoIcon} alt="BitFun" className="session-list__logo" />
          <div className="session-list__header-copy">
            <h1>{t('shared.product.remote')}</h1>
            {authenticatedUserId && (
              <span className="session-list__header-user-id">
                <span className={`session-list__health-dot session-list__health-dot--${connectionHealth}`} title={(() => { switch (connectionHealth) { case 'connected': return t('sessions.connectionConnected'); case 'checking': return t('sessions.connectionChecking'); case 'unreachable': return t('sessions.connectionUnreachable'); default: return t('sessions.connectionUnpaired'); } })()} />
                {authenticatedUserId}
                {controlTarget && !controlTarget.isHome && controlTarget.deviceName && (
                  <span className="session-list__header-target" title={t('devices.controllingDevice', { name: controlTarget.deviceName })}>
                    {controlTarget.deviceName}
                  </span>
                )}
              </span>
            )}
          </div>
        </div>
        <div className="session-list__header-actions">
          {onOpenDevices && (
            <button
              className={`session-list__devices-btn ${controlTarget && !controlTarget.isHome ? 'is-remote' : ''}`}
              onClick={onOpenDevices}
              title={t('devices.title')} aria-label={t('devices.title')}>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
                <line x1="8" y1="21" x2="16" y2="21" />
                <line x1="12" y1="17" x2="12" y2="21" />
              </svg>
            </button>
          )}
          <LanguageToggleButton />
          <button className="session-list__theme-btn" onClick={toggleTheme} aria-label={t('common.toggleTheme')}>
            <ThemeToggleIcon isDark={isDark} />
          </button>
          <button className="session-list__disconnect-btn" onClick={() => setShowDisconnectConfirm(true)} aria-label={t('sessions.disconnect')} title={t('sessions.disconnect')}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
              <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
              <polyline points="16 17 21 12 16 7" />
              <line x1="21" y1="12" x2="9" y2="12" />
            </svg>
          </button>
        </div>
      </div>

      <div
        className="session-list__items"
        ref={listRef}
        onScroll={handleScroll}
        onTouchStart={handleTouchStart}
        onTouchMove={handleTouchMove}
        onTouchEnd={handleTouchEnd}
      >
        {(pullDistance > 0 || refreshing) && (
          <div
            className="session-list__pull-indicator"
            style={{ height: refreshing ? PULL_THRESHOLD : pullDistance }}
          >
            <div className={`session-list__pull-spinner${refreshing || pullDistance >= PULL_THRESHOLD ? ' is-active' : ''}`}>
              <svg width="18" height="18" viewBox="0 0 18 18" fill="none"
                style={{ transform: `rotate(${pullDistance * 4}deg)`, transition: refreshing ? 'transform 0s' : undefined }}>
                <path d="M9 2V5M9 13V16M2 9H5M13 9H16M4.22 4.22L6.34 6.34M11.66 11.66L13.78 13.78M13.78 4.22L11.66 6.34M6.34 11.66L4.22 13.78"
                  stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
              </svg>
            </div>
          </div>
        )}

        {/* Resume Card — quick continue for the most recent session */}
        {showResumeCard && (
          <button
            type="button"
            className="session-list__resume-card"
            onClick={(e) => handleSessionClick(sessions[0], e)}
            onTouchStart={(e) => handleSessionTouchStart(sessions[0], e)}
            onTouchMove={handleSessionTouchMove}
            onTouchEnd={handleSessionTouchEnd}
            onTouchCancel={handleSessionTouchEnd}
            onContextMenu={(e) => { e.preventDefault(); setMenuSession(sessions[0]); }}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                onSelectSession(sessions[0].session_id, sessions[0].name);
              }
            }}
          >
            <div className={`session-list__item-icon session-list__resume-icon session-list__item-icon--${sessions[0].agent_type}`}>
              <SessionTypeIcon agentType={sessions[0].agent_type} />
            </div>
            <div className="session-list__resume-body">
              <div className="session-list__resume-label">{t('sessions.continueSession')}</div>
              <div className="session-list__resume-name">{sessions[0].name || t('sessions.untitledSession')}</div>
              <div className="session-list__resume-meta">
                <span className={`session-list__agent-badge session-list__agent-badge--${sessions[0].agent_type}`}>
                  {agentLabel(sessions[0].agent_type, t)}
                </span>
                <span className="session-list__resume-time">{formatTime(sessions[0].updated_at, formatDate, t)}</span>
              </div>
            </div>
            <span className="session-list__resume-arrow">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="m9 18 6-6-6-6"/></svg>
            </span>
          </button>
        )}

        {/* Mode Toggle - Inline */}
        <div className="session-list__mode-toggle">
          <button
            className={`session-list__mode-toggle-btn ${isProMode ? 'is-active' : ''}`}
            onClick={() => handleSelectMode('pro')}
          >
            <ProModeIcon />
            <span>{t('shared.modes.expert')}</span>
          </button>
          <button
            className={`session-list__mode-toggle-btn ${!isProMode ? 'is-active' : ''}`}
            onClick={() => handleSelectMode('assistant')}
          >
            <AssistantModeIcon />
            <span>{t('shared.modes.assistant')}</span>
          </button>
        </div>

        {/* Pro Mode: Workspace Selection Required */}
        {isProMode && (
          <>
            <div
              className="session-list__workspace-bar"
              onClick={() => {
                loadWorkspaceList();
                setShowWorkspacePicker(true);
              }}
            >
              <span className="session-list__workspace-icon">
                <WorkspaceIcon />
              </span>
              <div className="session-list__workspace-copy">
                <span className="session-list__workspace-label">{t('shared.features.workspace')}</span>
                <span className="session-list__workspace-name" title={workspaceDisplayName}>{truncateMiddle(workspaceDisplayName, 24)}</span>
              </div>
              {currentWorkspace?.git_branch && (
                <span className="session-list__workspace-branch">
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="6" x2="6" y1="3" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/></svg>
                  {truncateMiddle(currentWorkspace.git_branch, 20)}
                </span>
              )}
              <span className="session-list__workspace-switch" aria-label={t('sessions.switchWorkspace')}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="m7 15 5 5 5-5"/><path d="m7 9 5-5 5 5"/></svg>
              </span>
            </div>

            {/* Workspace Picker Modal */}
            {showWorkspacePicker && (
              <div className="session-list__picker-overlay" onClick={() => setShowWorkspacePicker(false)}>
                <div className="session-list__picker-modal session-list__picker-modal--workspace" onClick={e => e.stopPropagation()}>
                  <div className="session-list__picker-header">
                    <h3>{t('sessions.selectWorkspace')}</h3>
                    <button className="session-list__picker-close" onClick={() => setShowWorkspacePicker(false)}>
                      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                    </button>
                  </div>
                  <div className="session-list__picker-list">
                    {workspaceList.length === 0 ? (
                      <div className="session-list__picker-empty">{t('sessions.noWorkspaces')}</div>
                    ) : (
                      workspaceList.map((workspace, index) => {
                        const isSelected =
                          currentWorkspace?.path === workspace.path
                          && (currentWorkspace?.remote_connection_id ?? undefined)
                            === (workspace.remote_connection_id ?? undefined)
                          && (currentWorkspace?.remote_ssh_host ?? undefined)
                            === (workspace.remote_ssh_host ?? undefined);
                        const itemKey = [
                          workspace.remote_connection_id ?? 'local',
                          workspace.remote_ssh_host ?? '',
                          workspace.path || String(index),
                        ].join(':');
                        return (
                        <button
                          key={itemKey}
                          className={`session-list__picker-item session-list__picker-item--workspace ${isSelected ? 'is-selected' : ''}`}
                          onClick={() => handleSelectWorkspace(workspace)}
                        >
                          <span className="session-list__picker-item-icon">
                            <WorkspaceIcon />
                          </span>
                          <span className="session-list__picker-item-name">{workspace.name}</span>
                          {isSelected && (
                            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
                          )}
                        </button>
                        );
                      })
                    )}
                  </div>
                </div>
              </div>
            )}
          </>
        )}

        {/* Assistant Mode: Assistant Selection */}
        {!isProMode && (
          <>
            <div
              className="session-list__assistant-bar"
              onClick={() => {
                loadAssistantList();
                setShowAssistantPicker(true);
              }}
            >
              <span className="session-list__assistant-icon">
                <AssistantModeIcon />
              </span>
              <div className="session-list__assistant-copy">
                <span className="session-list__assistant-label">{t('sessions.assistant')}</span>
                <span className="session-list__assistant-name">{assistantDisplayName}</span>
              </div>
              <span className="session-list__assistant-switch" aria-label={t('sessions.switchAssistant')}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="m7 15 5 5 5-5"/><path d="m7 9 5-5 5 5"/></svg>
              </span>
            </div>

            {/* Assistant Picker Modal */}
            {showAssistantPicker && (
              <div className="session-list__picker-overlay" onClick={() => setShowAssistantPicker(false)}>
                <div className="session-list__picker-modal" onClick={e => e.stopPropagation()}>
                  <div className="session-list__picker-header">
                    <h3>{t('sessions.selectAssistant')}</h3>
                    <button className="session-list__picker-close" onClick={() => setShowAssistantPicker(false)}>
                      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                    </button>
                  </div>
                  <div className="session-list__picker-list">
                    {assistantList.map((assistant, index) => (
                      <button
                        key={assistant.path || index}
                        className={`session-list__picker-item ${currentAssistant?.path === assistant.path ? 'is-selected' : ''}`}
                        onClick={() => handleSelectAssistant(assistant)}
                      >
                        <span className="session-list__picker-item-icon">
                          <AssistantModeIcon />
                        </span>
                        <span className="session-list__picker-item-name">{assistant.name}</span>
                        {currentAssistant?.path === assistant.path && (
                          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
                        )}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            )}
          </>
        )}

            {/* Session Creation Options */}
            <section className={`session-list__panel ${!isProMode ? 'session-list__panel--assistant' : ''}`}>
              <div className="session-list__section-head">
                <div>
                  <div className="session-list__section-kicker">{t('sessions.launch')}</div>
                  <div className="session-list__section-title">{t('sessions.startRemoteFlow')}</div>
                </div>
              </div>

              {isProMode ? (
                /* Pro Mode: Code / Cowork - only show if workspace selected */
                currentWorkspace ? (
                  <div className="session-list__create-row">
                    <button
                      className="session-list__create-btn session-list__create-btn--code"
                      onClick={() => handleCreate('code')}
                      disabled={creating}
                    >
                      <div className="session-list__create-icon">
                        <SessionTypeIcon agentType="code" />
                      </div>
                      <div className="session-list__create-copy">
                        <span className="session-list__create-title">{t('shared.agents.code')}</span>
                        <span className="session-list__create-desc">{t('sessions.codeSessionDesc')}</span>
                      </div>
                      <span className="session-list__create-arrow">
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="m9 18 6-6-6-6"/></svg>
                      </span>
                    </button>
                    <button
                      className="session-list__create-btn session-list__create-btn--cowork"
                      onClick={() => handleCreate('cowork')}
                      disabled={creating}
                    >
                      <div className="session-list__create-icon">
                        <SessionTypeIcon agentType="cowork" />
                      </div>
                      <div className="session-list__create-copy">
                        <span className="session-list__create-title">{t('shared.agents.cowork')}</span>
                        <span className="session-list__create-desc">{t('sessions.coworkSessionDesc')}</span>
                      </div>
                      <span className="session-list__create-arrow">
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="m9 18 6-6-6-6"/></svg>
                      </span>
                    </button>
                  </div>
                ) : null
              ) : (
                /* Assistant Mode: Claw */
                <div className="session-list__create-row">
                  <button
                    className="session-list__create-btn session-list__create-btn--claw"
                    onClick={() => handleCreate('claw')}
                    disabled={creating}
                  >
                    <div className="session-list__create-icon">
                      <SessionTypeIcon agentType="claw" />
                    </div>
                    <div className="session-list__create-copy">
                      <span className="session-list__create-title">{t('sessions.clawSession')}</span>
                      <span className="session-list__create-desc">{t('sessions.clawSessionDesc')}</span>
                    </div>
                    <span className="session-list__create-arrow">
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="m9 18 6-6-6-6"/></svg>
                    </span>
                  </button>
                </div>
              )}
            </section>

            {/* Session History */}
            <section className={`session-list__panel session-list__panel--sessions ${!isProMode ? 'session-list__panel--assistant' : ''}`}>
              <div className="session-list__section-head">
                <div>
                  <div className="session-list__section-kicker">{t('sessions.recent')}</div>
                  <div className="session-list__section-title">{t('sessions.sessionHistory')}</div>
                </div>
                <div className="session-list__section-meta">{t('common.itemCount', { count: sessions.length })}</div>
              </div>

              {/* Search */}
              <div className="session-list__search">
                <svg className="session-list__search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="11" cy="11" r="8" />
                  <line x1="21" y1="21" x2="16.65" y2="16.65" />
                </svg>
                <input
                  className="session-list__search-input"
                  type="search"
                  placeholder={t('sessions.searchSessions')}
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  enterKeyHint="search"
                />
                {searchQuery && (
                  <button className="session-list__search-clear" onClick={() => setSearchQuery('')} aria-label="Clear">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                  </button>
                )}
              </div>

              {loading && sessions.length === 0 && (
                <div className="session-list__empty">{t('sessions.loadingSessions')}</div>
              )}
              {!loading && sessions.length === 0 && !hasSearchQuery && (
                <div className="session-list__empty">{t('sessions.noSessions')}</div>
              )}
              {!loading && sessions.length === 0 && hasSearchQuery && (
                <div className="session-list__empty">{t('sessions.emptySearch')}</div>
              )}

              <div className="session-list__cards">
                {sessions.slice(showResumeCard ? 1 : 0).map((s) => (
                  <div
                    key={s.session_id}
                    className={`session-list__item${menuSession?.session_id === s.session_id ? ' session-list__item--active' : ''}`}
                    onClick={(e) => handleSessionClick(s, e)}
                    onTouchStart={(e) => handleSessionTouchStart(s, e)}
                    onTouchMove={handleSessionTouchMove}
                    onTouchEnd={handleSessionTouchEnd}
                    onTouchCancel={handleSessionTouchEnd}
                    onContextMenu={(e) => { e.preventDefault(); setMenuSession(s); }}
                  >
                    <div className={`session-list__item-icon session-list__item-icon--${s.agent_type}`}>
                      <SessionTypeIcon agentType={s.agent_type} />
                    </div>
                    <div className="session-list__item-body">
                      <div className="session-list__item-top">
                        <div className="session-list__item-name">{s.name || t('sessions.untitledSession')}</div>
                        <span className={`session-list__agent-badge session-list__agent-badge--${s.agent_type}`}>
                          {agentLabel(s.agent_type, t)}
                        </span>
                      </div>
                      <div className="session-list__item-time">{formatTime(s.updated_at, formatDate, t)}</div>
                    </div>
                  </div>
                ))}
              </div>

              {loadingMore && (
                <div className="session-list__load-more">{t('sessions.loadingMore')}</div>
              )}
            </section>
      </div>

      {/* Context Menu Bottom Sheet */}
      {menuSession && !renameTarget && !deleteConfirmTarget && (
        <div className="session-list__menu-overlay" onClick={() => setMenuSession(null)}>
          <div className="session-list__menu-sheet" onClick={(e) => e.stopPropagation()}>
            <div className="session-list__menu-handle" />
            <div className="session-list__menu-title">
              {menuSession.name || t('sessions.untitledSession')}
            </div>
            <div className="session-list__menu-actions">
              <button
                className="session-list__menu-btn"
                onClick={() => {
                  setRenameTarget(menuSession);
                  setRenameValue(menuSession.name || '');
                }}
              >
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
                  <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
                </svg>
                <span>{t('sessions.renameSession')}</span>
              </button>
              <button
                className="session-list__menu-btn session-list__menu-btn--danger"
                onClick={() => setDeleteConfirmTarget(menuSession)}
              >
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                  <polyline points="3 6 5 6 21 6" />
                  <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                </svg>
                <span>{t('sessions.deleteSession')}</span>
              </button>
            </div>
            <button className="session-list__menu-cancel" onClick={() => setMenuSession(null)}>
              {t('sessions.cancel')}
            </button>
          </div>
        </div>
      )}

      {/* Rename Modal */}
      {renameTarget && (
        <div className="session-list__picker-overlay" onClick={() => !renaming && setRenameTarget(null)}>
          <div className="session-list__rename-modal" onClick={(e) => e.stopPropagation()}>
            <h3 className="session-list__rename-title">{t('sessions.renameTitle')}</h3>
            <input
              className="session-list__rename-input"
              type="text"
              value={renameValue}
              onChange={(e) => setRenameValue(e.target.value)}
              placeholder={t('sessions.sessionNamePlaceholder')}
              autoFocus
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleRename();
                if (e.key === 'Escape') setRenameTarget(null);
              }}
            />
            <div className="session-list__rename-actions">
              <button
                className="session-list__rename-btn session-list__rename-btn--cancel"
                onClick={() => setRenameTarget(null)}
                disabled={renaming}
              >
                {t('sessions.cancel')}
              </button>
              <button
                className="session-list__rename-btn session-list__rename-btn--save"
                onClick={handleRename}
                disabled={renaming || !renameValue.trim()}
              >
                {renaming ? '...' : t('sessions.save')}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Delete Confirmation */}
      {deleteConfirmTarget && (
        <div className="session-list__picker-overlay" onClick={() => !deleting && setDeleteConfirmTarget(null)}
          onKeyDown={(e) => {
            if (e.key === 'Escape') setDeleteConfirmTarget(null);
            if (e.key === 'Enter' && !deleting) handleDelete();
          }}>
          <div className="session-list__confirm-modal" onClick={(e) => e.stopPropagation()}>
            <div className="session-list__confirm-icon">
              <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="10" />
                <line x1="12" y1="8" x2="12" y2="12" />
                <line x1="12" y1="16" x2="12.01" y2="16" />
              </svg>
            </div>
            <h3 className="session-list__confirm-title">{t('sessions.confirmDelete')}</h3>
            <p className="session-list__confirm-desc">
              "{deleteConfirmTarget.name || t('sessions.untitledSession')}"
              <br />
              {t('sessions.confirmDeleteDesc')}
            </p>
            <div className="session-list__confirm-actions">
              <button
                className="session-list__confirm-btn session-list__confirm-btn--cancel"
                onClick={() => setDeleteConfirmTarget(null)}
                disabled={deleting}
              >
                {t('sessions.cancel')}
              </button>
              <button
                className="session-list__confirm-btn session-list__confirm-btn--danger"
                onClick={handleDelete}
                disabled={deleting}
              >
                {deleting ? '...' : t('sessions.deleteSession')}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Disconnect Confirmation */}
      {showDisconnectConfirm && (
        <div
          className="session-list__picker-overlay"
          role="alertdialog"
          aria-modal="true"
          aria-labelledby="disconnect-confirm-title"
          aria-describedby="disconnect-confirm-desc"
          onClick={() => setShowDisconnectConfirm(false)}
          onKeyDown={(e) => {
            if (e.key === 'Escape') setShowDisconnectConfirm(false);
          }}
        >
          <div className="session-list__confirm-modal" onClick={(e) => e.stopPropagation()}>
            <div className="session-list__confirm-icon">
              <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
                <polyline points="16 17 21 12 16 7" />
                <line x1="21" y1="12" x2="9" y2="12" />
              </svg>
            </div>
            <h3 id="disconnect-confirm-title" className="session-list__confirm-title">{t('sessions.disconnect')}</h3>
            <p id="disconnect-confirm-desc" className="session-list__confirm-desc">{t('sessions.disconnectConfirm')}</p>
            <div className="session-list__confirm-actions">
              <button
                className="session-list__confirm-btn session-list__confirm-btn--cancel"
                onClick={() => setShowDisconnectConfirm(false)}
                autoFocus
              >
                {t('common.cancel')}
              </button>
              <button
                className="session-list__confirm-btn session-list__confirm-btn--danger"
                onClick={() => { setShowDisconnectConfirm(false); onDisconnect(); }}
              >
                {t('sessions.disconnect')}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Action Toast */}
      {actionToast && (
        <div className="session-list__toast" role="alert" aria-live="assertive">{actionToast}</div>
      )}
    </div>
  );
};

export default SessionListPage;
