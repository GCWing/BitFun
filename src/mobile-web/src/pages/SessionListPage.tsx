import React, { useEffect, useLayoutEffect, useRef, useCallback, useMemo, useState } from 'react';
import {
  MobileButton,
  MobileBanner,
  MobileChoiceSheet,
  MobileFloatingActions,
  MobileIconButton,
  MobileSection,
  MobileSegmentedControl,
  MobileStatus,
  MobileTextField,
} from '@openbitfun/ui/mobile';
import LanguageToggleButton from '../components/LanguageToggleButton';
import SessionOverlays from '../components/SessionOverlays';
import CompactSettingsSheet from '../components/CompactSettingsSheet';
import { SessionHistoryPanel, SessionLaunchPanel } from '../components/SessionDashboardSections';
import { useControlTargetEpoch } from '../hooks/useControlTargetEpoch';
import { useI18n } from '../i18n';
import {
  isRemoteControlTargetChangedError,
  REMOTE_CAPABILITY_HARNESS_PROFILES_V1,
  RemoteSessionManager,
  type RecentWorkspaceEntry,
  type SessionInfo,
} from '../services/RemoteSessionManager';
import { useMobileStore } from '../services/store';
import { createRemoteCacheScope, remoteCache } from '../services/RemoteCache';
import { useTheme } from '../theme';
import logoMarkDark from '../assets/openbitfun-mark-dark.png';
import logoMarkLight from '../assets/openbitfun-mark-light.png';
import {
  isDelegatedIdentityChangedError,
  type RelayHttpClient,
} from '../services/RelayHttpClient';

const PAGE_SIZE = 30;

type DisplayMode = 'pro' | 'assistant';

interface SessionListPageProps {
  sessionMgr: RemoteSessionManager;
  client?: RelayHttpClient;
  compact?: boolean;
  activeSessionId?: string | null;
  onSelectSession: (
    sessionId: string,
    sessionName?: string,
    isNew?: boolean,
    agentType?: string,
  ) => void;
  onOpenWorkspace: () => void;
  onDisconnect: () => void;
  onOpenDevices?: () => void;
  onControlTargetChanged?: () => void;
}

type CompactDevice = {
  device_id: string;
  device_name: string;
  online: boolean;
  /** The QR room is a valid control target even when no account device id was delegated. */
  room_route?: boolean;
};

const COMPACT_PAIRED_ROOM_DEVICE_ID = '__openbitfun_paired_room__';

function compactSelectedDeviceIdForClient(client?: RelayHttpClient): string | null {
  if (!client) return null;
  return client.pairedDeviceId
    ?? (client.isPaired ? COMPACT_PAIRED_ROOM_DEVICE_ID : null);
}

type CompactWorkspaceLoadStatus = 'idle' | 'loading' | 'ready' | 'failed';

function compactWorkspaceKey(workspace: RecentWorkspaceEntry): string {
  return `${workspace.remote_connection_id ?? 'local'}:${workspace.remote_ssh_host ?? ''}:${workspace.path}`;
}

function mergeCompactWorkspaces(
  recent: RecentWorkspaceEntry[],
  currentWorkspace: {
    path?: string;
    project_name?: string;
    workspace_kind?: 'normal' | 'assistant' | 'remote';
    remote_connection_id?: string;
    remote_ssh_host?: string;
  } | null,
  sessions: SessionInfo[],
): RecentWorkspaceEntry[] {
  const merged: RecentWorkspaceEntry[] = [];
  const seen = new Set<string>();
  const append = (workspace: RecentWorkspaceEntry) => {
    if (!workspace.path) return;
    const key = compactWorkspaceKey(workspace);
    if (seen.has(key)) return;
    seen.add(key);
    merged.push(workspace);
  };

  if (currentWorkspace?.path) {
    append({
      path: currentWorkspace.path,
      name: currentWorkspace.project_name || currentWorkspace.path.split('/').filter(Boolean).pop() || currentWorkspace.path,
      last_opened: '',
      workspace_kind: currentWorkspace.workspace_kind,
      remote_connection_id: currentWorkspace.remote_connection_id,
      remote_ssh_host: currentWorkspace.remote_ssh_host,
    });
  }
  recent.forEach(append);
  sessions.forEach((session) => {
    if (!session.workspace_path) return;
    append({
      path: session.workspace_path,
      name: session.workspace_name || session.workspace_path.split('/').filter(Boolean).pop() || session.workspace_path,
      last_opened: session.updated_at,
    });
  });
  return merged;
}

type SessionListTargetOwner = {
  sessionMgr: RemoteSessionManager;
  epoch: number;
  active: boolean;
};

/**
 * Resolve the epoch owned by one render. The explicit renderedEpoch check is
 * what prevents an old timer/poll closure from borrowing a newer mutable ref
 * owner during the render-to-passive-cleanup window.
 */
export function captureSessionListOwnerEpoch(
  owner: SessionListTargetOwner,
  sessionMgr: RemoteSessionManager,
  renderedEpoch: number,
): number | null {
  if (
    !owner.active
    || owner.sessionMgr !== sessionMgr
    || owner.epoch !== renderedEpoch
    || sessionMgr.controlTargetEpoch !== renderedEpoch
  ) return null;
  return renderedEpoch;
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
    case 'minimal':
      return t('sessions.harnessMinimal');
    case 'Ultra':
    case 'ultra':
    case 'ultimate':
      return t('sessions.harnessUltimate');
    case 'code':
      return t('sessions.agentCode');
    case 'agentic':
      return t('sessions.harnessStandard');
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

function CompactDeviceIcon({ name }: { name: string }) {
  const normalized = name.toLocaleLowerCase();
  if (/(macbook|laptop|notebook)/.test(normalized)) {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.65" strokeLinecap="round" strokeLinejoin="round">
        <rect x="4" y="4" width="16" height="11" rx="2"/><path d="M2.5 19h19M7 19l1-4h8l1 4"/>
      </svg>
    );
  }
  if (/(server|ecs|cloud|host)/.test(normalized)) {
    return (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.65" strokeLinecap="round" strokeLinejoin="round">
        <rect x="4" y="3" width="16" height="6" rx="2"/><rect x="4" y="15" width="16" height="6" rx="2"/><path d="M8 6h.01M8 18h.01M12 9v6"/>
      </svg>
    );
  }
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.65" strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="4" width="18" height="14" rx="2.4"/><path d="M8 21h8M12 18v3"/>
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

const SessionListPage: React.FC<SessionListPageProps> = ({
  sessionMgr,
  client,
  compact = false,
  activeSessionId,
  onSelectSession,
  onOpenWorkspace,
  onDisconnect,
  onOpenDevices,
  onControlTargetChanged,
}) => {
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
    authenticatedUserLabel,
    connectionHealth,
    controlTarget,
    setControlTarget,
    resetForDeviceSwitch,
  } = useMobileStore();
  const { isDark, toggleTheme } = useTheme();
  const logoMark = isDark ? logoMarkLight : logoMarkDark;
  const [creating, setCreating] = useState(false);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [targetInitializing, setTargetInitializing] = useState(true);
  const targetInitializingRef = useRef(true);
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
  const [compactSearchOpen, setCompactSearchOpen] = useState(false);
  const [compactDevices, setCompactDevices] = useState<CompactDevice[]>([]);
  const [compactDirectoryLoading, setCompactDirectoryLoading] = useState(false);
  const [compactSelectedDeviceId, setCompactSelectedDeviceId] = useState<string | null>(
    () => compactSelectedDeviceIdForClient(client),
  );
  const [compactSwitchingDeviceId, setCompactSwitchingDeviceId] = useState<string | null>(null);
  const [compactExpandedWorkspaces, setCompactExpandedWorkspaces] = useState<Set<string>>(() => new Set());
  const [compactWorkspaceSessions, setCompactWorkspaceSessions] = useState<Record<string, SessionInfo[]>>({});
  const [compactWorkspaceStatuses, setCompactWorkspaceStatuses] = useState<Record<string, CompactWorkspaceLoadStatus>>({});
  const [compactWorkspaceHasMore, setCompactWorkspaceHasMore] = useState<Record<string, boolean>>({});
  const [compactWorkspaceLoadingMore, setCompactWorkspaceLoadingMore] = useState<Set<string>>(() => new Set());
  const [compactVisibleSessionCounts, setCompactVisibleSessionCounts] = useState<Record<string, number>>({});
  const [compactRecentVisibleCount, setCompactRecentVisibleCount] = useState(6);
  const [compactVisibleDeviceCount, setCompactVisibleDeviceCount] = useState(3);
  const [compactVisibleWorkspaceCount, setCompactVisibleWorkspaceCount] = useState(3);
  const [compactSettingsOpen, setCompactSettingsOpen] = useState(false);
  const [harnessCreateRequest, setHarnessCreateRequest] = useState<{
    workspace?: RecentWorkspaceEntry;
  } | null>(null);

  const longPressTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const longPressPosRef = useRef({ x: 0, y: 0 });
  const longPressTriggeredRef = useRef(false);
  const toastTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const controlTargetEpoch = useControlTargetEpoch(sessionMgr);
  const cacheScope = useMemo(() => createRemoteCacheScope(
    authenticatedUserId,
    controlTarget?.deviceId ?? client?.pairedDeviceId,
  ), [authenticatedUserId, client?.pairedDeviceId, controlTarget?.deviceId]);
  const liveDataSeqRef = useRef(0);
  const sessionListOwnerRef = useRef({
    sessionMgr,
    epoch: controlTargetEpoch,
    active: true,
  });
  if (
    sessionListOwnerRef.current.sessionMgr !== sessionMgr
    || sessionListOwnerRef.current.epoch !== controlTargetEpoch
  ) {
    sessionListOwnerRef.current = {
      sessionMgr,
      epoch: controlTargetEpoch,
      active: true,
    };
  }

  const captureSessionListEpoch = useCallback((): number | null => {
    return captureSessionListOwnerEpoch(
      sessionListOwnerRef.current,
      sessionMgr,
      controlTargetEpoch,
    );
  }, [controlTargetEpoch, sessionMgr]);

  const isSessionListCurrent = useCallback((epoch: number | null): boolean => {
    const owner = sessionListOwnerRef.current;
    return epoch !== null
      && owner.active
      && owner.sessionMgr === sessionMgr
      && owner.epoch === epoch
      && sessionMgr.controlTargetEpoch === epoch;
  }, [controlTargetEpoch, sessionMgr]);

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
    onSelectSession(s.session_id, s.name, false, s.agent_type);
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
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
    setRenaming(true);
    try {
      await sessionMgr.renameSession(renameTarget.session_id, renameValue.trim());
      if (!isSessionListCurrent(targetEpoch)) return;
      const nextName = renameValue.trim();
      useMobileStore.getState().updateSessionName(renameTarget.session_id, nextName);
      setCompactWorkspaceSessions((current) => Object.fromEntries(
        Object.entries(current).map(([key, workspaceSessions]) => [
          key,
          workspaceSessions.map((session) => (
            session.session_id === renameTarget.session_id
              ? { ...session, name: nextName }
              : session
          )),
        ]),
      ));
      remoteCache.renameSession(cacheScope, renameTarget.session_id, nextName);
      setRenameTarget(null);
      setMenuSession(null);
    } catch (e: any) {
      if (isSessionListCurrent(targetEpoch) && !isRemoteControlTargetChangedError(e)) {
        showToast(e.message || t('sessions.renameFailed'));
      }
    } finally {
      if (isSessionListCurrent(targetEpoch)) setRenaming(false);
    }
  }, [cacheScope, captureSessionListEpoch, isSessionListCurrent, renameTarget, renameValue, sessionMgr, showToast, t]);

  const handleDelete = useCallback(async () => {
    if (!deleteConfirmTarget) return;
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
    setDeleting(true);
    try {
      await sessionMgr.deleteSession(deleteConfirmTarget.session_id);
      if (!isSessionListCurrent(targetEpoch)) return;
      useMobileStore.getState().removeSession(deleteConfirmTarget.session_id);
      setCompactWorkspaceSessions((current) => Object.fromEntries(
        Object.entries(current).map(([key, workspaceSessions]) => [
          key,
          workspaceSessions.filter((session) => (
            session.session_id !== deleteConfirmTarget.session_id
          )),
        ]),
      ));
      remoteCache.deleteSession(cacheScope, deleteConfirmTarget.session_id);
      setDeleteConfirmTarget(null);
      setMenuSession(null);
      showToast(t('sessions.deleted'));
    } catch (e: any) {
      if (isSessionListCurrent(targetEpoch) && !isRemoteControlTargetChangedError(e)) {
        showToast(e.message || t('sessions.deleteFailed'));
      }
    } finally {
      if (isSessionListCurrent(targetEpoch)) setDeleting(false);
    }
  }, [cacheScope, captureSessionListEpoch, deleteConfirmTarget, isSessionListCurrent, sessionMgr, showToast, t]);

  const [pullDistance, setPullDistance] = useState(0);
  const [refreshing, setRefreshing] = useState(false);
  const offsetRef = useRef(0);
  const listRef = useRef<HTMLDivElement>(null);
  const listRequestSeqRef = useRef(0);
  const initLoadedPathRef = useRef<string | undefined>(undefined);
  const touchStartY = useRef(0);
  const isPulling = useRef(false);

  const committedSessionListTargetRef = useRef({ sessionMgr, epoch: controlTargetEpoch });
  useLayoutEffect(() => {
    const previous = committedSessionListTargetRef.current;
    const targetChanged = previous.sessionMgr !== sessionMgr
      || previous.epoch !== controlTargetEpoch;
    const owner = sessionListOwnerRef.current;
    owner.active = owner.sessionMgr === sessionMgr
      && owner.epoch === controlTargetEpoch
      && sessionMgr.controlTargetEpoch === controlTargetEpoch;
    if (targetChanged) {
      targetInitializingRef.current = true;
      setTargetInitializing(true);
      listRequestSeqRef.current += 1;
      setLoading(false);
      setLoadingMore(false);
      setRefreshing(false);
      clearLongPressTimer();
      longPressTriggeredRef.current = false;
      isPulling.current = false;
      setPullDistance(0);
      if (toastTimerRef.current) {
        clearTimeout(toastTimerRef.current);
        toastTimerRef.current = undefined;
      }
      setCreating(false);
      setRenaming(false);
      setDeleting(false);
      setAssistantList([]);
      setWorkspaceList([]);
      setShowAssistantPicker(false);
      setShowWorkspacePicker(false);
      setMenuSession(null);
      setRenameTarget(null);
      setRenameValue('');
      setDeleteConfirmTarget(null);
      setActionToast(null);
      setShowDisconnectConfirm(false);
      setSearchQuery('');
      setCompactSelectedDeviceId(compactSelectedDeviceIdForClient(client));
      setCompactSwitchingDeviceId(null);
      setCompactExpandedWorkspaces(new Set());
      setCompactWorkspaceSessions({});
      setCompactWorkspaceStatuses({});
      setCompactWorkspaceHasMore({});
      setCompactWorkspaceLoadingMore(new Set());
      setCompactVisibleSessionCounts({});
      setCompactRecentVisibleCount(6);
      setCompactVisibleDeviceCount(3);
      setCompactVisibleWorkspaceCount(3);
      setCompactSettingsOpen(false);
      setDisplayMode('pro');
      setHasMore(false);
      setSessions([]);
      setCurrentWorkspace(null);
      setCurrentAssistant(null);
      setPairedDisplayMode(null);
      setError(null);
      offsetRef.current = 0;
      initLoadedPathRef.current = undefined;
    }
    committedSessionListTargetRef.current = { sessionMgr, epoch: controlTargetEpoch };
    return () => {
      owner.active = false;
      listRequestSeqRef.current += 1;
    };
  }, [
    controlTargetEpoch,
    client,
    sessionMgr,
    setCurrentAssistant,
    setCurrentWorkspace,
    setError,
    setPairedDisplayMode,
    setSessions,
  ]);

  useEffect(() => {
    if (!compact || !cacheScope) return;
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
    const liveDataSeq = liveDataSeqRef.current;
    let cancelled = false;
    void remoteCache.loadSessionState(cacheScope).then((cached) => {
      if (
        cancelled
        || !cached
        || liveDataSeqRef.current !== liveDataSeq
        || !isSessionListCurrent(targetEpoch)
      ) return;
      if (useMobileStore.getState().sessions.length === 0) {
        setSessions(cached.sessions);
        offsetRef.current = cached.sessions.length;
      }
      setWorkspaceList(cached.workspaces);

      const cachedByWorkspace: Record<string, SessionInfo[]> = {};
      const cachedStatuses: Record<string, CompactWorkspaceLoadStatus> = {};
      cached.workspaces.forEach((workspace) => {
        const key = compactWorkspaceKey(workspace);
        const workspaceSessions = cached.sessions.filter((session) => (
          session.workspace_path === workspace.path
        ));
        if (workspaceSessions.length > 0) {
          cachedByWorkspace[key] = workspaceSessions;
          cachedStatuses[key] = 'idle';
        }
      });
      setCompactWorkspaceSessions(cachedByWorkspace);
      setCompactWorkspaceStatuses(cachedStatuses);
    });
    return () => { cancelled = true; };
  }, [cacheScope, captureSessionListEpoch, compact, controlTargetEpoch, isSessionListCurrent, setSessions]);

  // Load assistant list when entering assistant mode
  const loadAssistantList = useCallback(async () => {
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return undefined;
    try {
      const assistants = await sessionMgr.listAssistants();
      if (!isSessionListCurrent(targetEpoch)) return undefined;
      setAssistantList(assistants);
      // Set default assistant if none selected
      if (!currentAssistant && assistants.length > 0) {
        const defaultAssistant = assistants.find(a => !a.assistant_id) || assistants[0];
        setCurrentAssistant(defaultAssistant);
        return defaultAssistant.path;
      }
      return currentAssistant?.path;
    } catch (e: any) {
      if (isSessionListCurrent(targetEpoch) && !isRemoteControlTargetChangedError(e)) {
        setError(e.message);
      }
      return undefined;
    }
  }, [captureSessionListEpoch, currentAssistant, isSessionListCurrent, sessionMgr, setCurrentAssistant, setError]);

  const loadFirstPage = useCallback(async (
    workspacePath: string | undefined,
    query = '',
    identity?: { remoteConnectionId?: string; remoteSshHost?: string },
  ) => {
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
    const requestSeq = ++listRequestSeqRef.current;
    // A new first page owns the complete list and supersedes pagination.
    setLoadingMore(false);
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
      if (
        requestSeq !== listRequestSeqRef.current
        || !isSessionListCurrent(targetEpoch)
      ) return;
      liveDataSeqRef.current += 1;
      setSessions(resp.sessions);
      setHasMore(resp.has_more);
      offsetRef.current = resp.sessions.length;
      remoteCache.saveSessionPage(cacheScope, resp.sessions, {
        workspacePath,
        replaceWorkspace: query.trim().length === 0,
      });
    } catch (e: any) {
      if (
        requestSeq !== listRequestSeqRef.current
        || !isSessionListCurrent(targetEpoch)
      ) return;
      if (!isRemoteControlTargetChangedError(e)) setError(e.message);
    } finally {
      if (
        requestSeq === listRequestSeqRef.current
        && isSessionListCurrent(targetEpoch)
      ) {
        setLoading(false);
      }
    }
  }, [cacheScope, captureSessionListEpoch, isSessionListCurrent, sessionMgr, setError, setSessions]);

  // Load workspace list for Pro mode picker
  const loadWorkspaceList = useCallback(async () => {
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
    try {
      const workspaces = await sessionMgr.listRecentWorkspaces();
      if (!isSessionListCurrent(targetEpoch)) return;
      liveDataSeqRef.current += 1;
      setWorkspaceList(workspaces);
      remoteCache.saveWorkspaceCatalog(cacheScope, workspaces);
    } catch (e: any) {
      if (isSessionListCurrent(targetEpoch) && !isRemoteControlTargetChangedError(e)) {
        setError(e.message);
      }
    }
  }, [cacheScope, captureSessionListEpoch, isSessionListCurrent, sessionMgr, setError]);

  const loadCompactDirectory = useCallback(async () => {
    if (!compact) return;
    setCompactDirectoryLoading(true);
    try {
      const tasks: Promise<unknown>[] = [loadWorkspaceList()];
      if (client?.hasDelegatedIdentity) {
        tasks.push(client.listDevices().then((list) => {
          setCompactDevices(list.filter((device) => (
            device.device_id !== client.controllerDeviceId
          )));
        }));
      }
      await Promise.all(tasks);
    } catch (e: any) {
      setError(e?.message || t('devices.loadFailed'));
    } finally {
      setCompactDirectoryLoading(false);
    }
  }, [client, compact, loadWorkspaceList, setError, t]);

  const loadCompactWorkspaceCatalog = useCallback(async (expectedTargetEpoch: number) => {
    if (!compact || !client) return;
    setCompactDirectoryLoading(true);
    try {
      const workspaces = await sessionMgr.listRecentWorkspaces();
      if (client.controlTargetEpoch !== expectedTargetEpoch) return;
      liveDataSeqRef.current += 1;
      setWorkspaceList(workspaces);
      remoteCache.saveWorkspaceCatalog(cacheScope, workspaces);
    } catch (error: unknown) {
      if (
        client.controlTargetEpoch === expectedTargetEpoch
        && !isRemoteControlTargetChangedError(error)
      ) {
        setError(String((error as { message?: string })?.message || error));
      }
    } finally {
      if (client.controlTargetEpoch === expectedTargetEpoch) {
        setCompactDirectoryLoading(false);
      }
    }
  }, [cacheScope, client, compact, sessionMgr, setError]);

  useEffect(() => {
    if (!compact) return;
    void loadCompactDirectory();
  }, [compact, loadCompactDirectory]);

  const handleSelectCompactDevice = useCallback(async (device: CompactDevice) => {
    if (!client || !device.online || compactSwitchingDeviceId) return;
    setCompactSelectedDeviceId(device.device_id);
    if (device.room_route) {
      await loadCompactWorkspaceCatalog(client.controlTargetEpoch);
      return;
    }
    if (client.pairedDeviceId === device.device_id) {
      await loadCompactWorkspaceCatalog(client.controlTargetEpoch);
      return;
    }
    const accountEpoch = client.delegatedAccountEpoch;
    const targetEpoch = client.controlTargetEpoch;
    setCompactSwitchingDeviceId(device.device_id);
    setError(null);
    try {
      const ping = await client.sendDeviceRpc<{ resp?: string; ok?: boolean; error?: string }>(
        device.device_id,
        {
          cmd: 'host_invoke',
          command: 'peer_mode_ping',
          args: {},
        },
        { retryable: true },
      );
      if (
        client.delegatedAccountEpoch !== accountEpoch
        || client.controlTargetEpoch !== targetEpoch
      ) return;
      if (ping.resp === 'host_invoke_result' && ping.ok === false) {
        throw new Error(ping.error || t('devices.switchFailed'));
      }
      client.setPairedDeviceId(device.device_id);
      const switchedTargetEpoch = client.controlTargetEpoch;
      resetForDeviceSwitch();
      setControlTarget({
        deviceId: device.device_id,
        deviceName: device.device_name || null,
        isHome: device.device_id === client.homeDeviceId,
      });
      onControlTargetChanged?.();
      await loadCompactWorkspaceCatalog(switchedTargetEpoch);
    } catch (error: unknown) {
      if (isDelegatedIdentityChangedError(error)) return;
      const message = String((error as { message?: string })?.message || error);
      setError(message || t('devices.switchFailed'));
    } finally {
      setCompactSwitchingDeviceId((current) => (
        current === device.device_id ? null : current
      ));
    }
  }, [
    client,
    compactSwitchingDeviceId,
    loadCompactWorkspaceCatalog,
    onControlTargetChanged,
    resetForDeviceSwitch,
    setControlTarget,
    setError,
    t,
  ]);

  const handleToggleCompactWorkspace = useCallback(async (workspace: RecentWorkspaceEntry) => {
    const key = compactWorkspaceKey(workspace);
    if (compactExpandedWorkspaces.has(key)) {
      setCompactExpandedWorkspaces((current) => {
        const next = new Set(current);
        next.delete(key);
        return next;
      });
      return;
    }

    setCompactExpandedWorkspaces((current) => new Set(current).add(key));
    if (compactWorkspaceStatuses[key] === 'ready' || compactWorkspaceStatuses[key] === 'loading') {
      return;
    }

    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
    setCompactWorkspaceStatuses((current) => ({ ...current, [key]: 'loading' }));
    try {
      const response = await sessionMgr.listSessions(workspace.path, PAGE_SIZE, 0, '', {
        remoteConnectionId: workspace.remote_connection_id,
        remoteSshHost: workspace.remote_ssh_host,
      });
      if (!isSessionListCurrent(targetEpoch)) return;
      liveDataSeqRef.current += 1;
      setCompactWorkspaceSessions((current) => ({ ...current, [key]: response.sessions }));
      setCompactWorkspaceStatuses((current) => ({ ...current, [key]: 'ready' }));
      setCompactWorkspaceHasMore((current) => ({ ...current, [key]: response.has_more }));
      setCompactVisibleSessionCounts((current) => ({ ...current, [key]: 3 }));
      remoteCache.saveSessionPage(cacheScope, response.sessions, {
        workspacePath: workspace.path,
        replaceWorkspace: true,
      });
    } catch (error: unknown) {
      if (!isSessionListCurrent(targetEpoch) || isRemoteControlTargetChangedError(error)) return;
      setCompactWorkspaceStatuses((current) => ({ ...current, [key]: 'failed' }));
    }
  }, [
    captureSessionListEpoch,
    compactExpandedWorkspaces,
    compactWorkspaceStatuses,
    cacheScope,
    isSessionListCurrent,
    sessionMgr,
  ]);

  const handleRetryCompactWorkspace = useCallback(async (workspace: RecentWorkspaceEntry) => {
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
    const key = compactWorkspaceKey(workspace);
    setCompactWorkspaceStatuses((current) => ({ ...current, [key]: 'loading' }));
    try {
      const response = await sessionMgr.listSessions(workspace.path, PAGE_SIZE, 0, '', {
        remoteConnectionId: workspace.remote_connection_id,
        remoteSshHost: workspace.remote_ssh_host,
      });
      if (!isSessionListCurrent(targetEpoch)) return;
      liveDataSeqRef.current += 1;
      setCompactWorkspaceSessions((current) => ({ ...current, [key]: response.sessions }));
      setCompactWorkspaceStatuses((current) => ({ ...current, [key]: 'ready' }));
      setCompactWorkspaceHasMore((current) => ({ ...current, [key]: response.has_more }));
      setCompactVisibleSessionCounts((current) => ({ ...current, [key]: 3 }));
      remoteCache.saveSessionPage(cacheScope, response.sessions, {
        workspacePath: workspace.path,
        replaceWorkspace: true,
      });
    } catch (error: unknown) {
      if (!isSessionListCurrent(targetEpoch) || isRemoteControlTargetChangedError(error)) return;
      setCompactWorkspaceStatuses((current) => ({ ...current, [key]: 'failed' }));
    }
  }, [cacheScope, captureSessionListEpoch, isSessionListCurrent, sessionMgr]);

  const handleLoadMoreCompactWorkspace = useCallback(async (workspace: RecentWorkspaceEntry) => {
    const key = compactWorkspaceKey(workspace);
    const visibleCount = compactVisibleSessionCounts[key] ?? 3;
    const loadedSessions = compactWorkspaceSessions[key] ?? [];
    if (visibleCount < loadedSessions.length) {
      setCompactVisibleSessionCounts((current) => ({
        ...current,
        [key]: Math.min(visibleCount + 3, loadedSessions.length),
      }));
      return;
    }
    if (!compactWorkspaceHasMore[key] || compactWorkspaceLoadingMore.has(key)) return;

    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
    setCompactWorkspaceLoadingMore((current) => new Set(current).add(key));
    try {
      const response = await sessionMgr.listSessions(
        workspace.path,
        PAGE_SIZE,
        loadedSessions.length,
        '',
        {
          remoteConnectionId: workspace.remote_connection_id,
          remoteSshHost: workspace.remote_ssh_host,
        },
      );
      if (!isSessionListCurrent(targetEpoch)) return;
      liveDataSeqRef.current += 1;
      const merged = [...loadedSessions];
      const existingIds = new Set(merged.map((session) => session.session_id));
      response.sessions.forEach((session) => {
        if (!existingIds.has(session.session_id)) merged.push(session);
      });
      setCompactWorkspaceSessions((current) => ({ ...current, [key]: merged }));
      setCompactWorkspaceHasMore((current) => ({ ...current, [key]: response.has_more }));
      setCompactVisibleSessionCounts((current) => ({
        ...current,
        [key]: Math.min(visibleCount + 3, merged.length),
      }));
      remoteCache.saveSessionPage(cacheScope, response.sessions, { workspacePath: workspace.path });
    } catch (error: unknown) {
      if (!isSessionListCurrent(targetEpoch) || isRemoteControlTargetChangedError(error)) return;
      setError(String((error as { message?: string })?.message || error));
    } finally {
      if (isSessionListCurrent(targetEpoch)) {
        setCompactWorkspaceLoadingMore((current) => {
          const next = new Set(current);
          next.delete(key);
          return next;
        });
      }
    }
  }, [
    cacheScope,
    captureSessionListEpoch,
    compactVisibleSessionCounts,
    compactWorkspaceHasMore,
    compactWorkspaceLoadingMore,
    compactWorkspaceSessions,
    isSessionListCurrent,
    sessionMgr,
    setError,
  ]);

  const handleCreateInCompactWorkspace = useCallback(async (
    workspace: RecentWorkspaceEntry,
    agentType = 'code',
  ) => {
    if (creating || targetInitializingRef.current) return;
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
    setCreating(true);
    try {
      const identity = {
        remoteConnectionId: workspace.remote_connection_id,
        remoteSshHost: workspace.remote_ssh_host,
      };
      const sessionId = await sessionMgr.createSession(agentType, undefined, workspace.path, identity);
      if (!isSessionListCurrent(targetEpoch)) return;
      const response = await sessionMgr.listSessions(workspace.path, PAGE_SIZE, 0, '', identity);
      if (!isSessionListCurrent(targetEpoch)) return;
      const key = compactWorkspaceKey(workspace);
      liveDataSeqRef.current += 1;
      setCompactExpandedWorkspaces((current) => new Set(current).add(key));
      setCompactWorkspaceSessions((current) => ({ ...current, [key]: response.sessions }));
      setCompactWorkspaceStatuses((current) => ({ ...current, [key]: 'ready' }));
      setCompactWorkspaceHasMore((current) => ({ ...current, [key]: response.has_more }));
      setCompactVisibleSessionCounts((current) => ({ ...current, [key]: 3 }));
      remoteCache.saveSessionPage(cacheScope, response.sessions, {
        workspacePath: workspace.path,
        replaceWorkspace: true,
      });
      onSelectSession(sessionId, t('sessions.remoteCodeSession'), true, agentType);
    } catch (error: unknown) {
      if (isSessionListCurrent(targetEpoch) && !isRemoteControlTargetChangedError(error)) {
        setError(String((error as { message?: string })?.message || error));
      }
    } finally {
      if (isSessionListCurrent(targetEpoch)) setCreating(false);
    }
  }, [
    captureSessionListEpoch,
    cacheScope,
    creating,
    isSessionListCurrent,
    onSelectSession,
    sessionMgr,
    setError,
    t,
  ]);

  const handleSelectWorkspace = useCallback(async (workspace: {
    path: string;
    name: string;
    remote_connection_id?: string;
    remote_ssh_host?: string;
  }) => {
    if (targetInitializingRef.current) return;
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
    try {
      const result = await sessionMgr.setWorkspace(workspace.path, {
        remoteConnectionId: workspace.remote_connection_id,
        remoteSshHost: workspace.remote_ssh_host,
      });
      if (!isSessionListCurrent(targetEpoch)) return;
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
      if (isSessionListCurrent(targetEpoch) && !isRemoteControlTargetChangedError(e)) {
        setError(e.message);
      }
    }
  }, [captureSessionListEpoch, isSessionListCurrent, loadFirstPage, searchQuery, sessionMgr, setCurrentWorkspace, setError]);

  const trySelectFirstProWorkspace = useCallback(async (): Promise<boolean> => {
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return false;
    try {
      const list = await sessionMgr.listRecentWorkspaces();
      if (!isSessionListCurrent(targetEpoch)) return false;
      const candidate = pickFirstProWorkspace(list);
      if (!candidate) return false;
      const result = await sessionMgr.setWorkspace(candidate.path, {
        remoteConnectionId: candidate.remote_connection_id,
        remoteSshHost: candidate.remote_ssh_host,
      });
      if (!isSessionListCurrent(targetEpoch)) return false;
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
        return isSessionListCurrent(targetEpoch);
      }
      setError(result.error || t('workspace.failedToSetWorkspace'));
      return false;
    } catch (e: any) {
      if (isSessionListCurrent(targetEpoch) && !isRemoteControlTargetChangedError(e)) {
        setError(e.message);
      }
      return false;
    }
  }, [captureSessionListEpoch, isSessionListCurrent, loadFirstPage, searchQuery, sessionMgr, setCurrentWorkspace, setError, t]);

  const loadNextPage = useCallback(async (
    workspacePath: string | undefined,
    query = '',
    identity?: { remoteConnectionId?: string; remoteSshHost?: string },
  ) => {
    if (loading || loadingMore || !hasMore) return;
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
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
      if (
        requestSeq !== listRequestSeqRef.current
        || !isSessionListCurrent(targetEpoch)
      ) return;
      appendSessions(resp.sessions);
      setHasMore(resp.has_more);
      offsetRef.current += resp.sessions.length;
      liveDataSeqRef.current += 1;
      remoteCache.saveSessionPage(cacheScope, resp.sessions, { workspacePath });
    } catch (e: any) {
      if (
        requestSeq !== listRequestSeqRef.current
        || !isSessionListCurrent(targetEpoch)
      ) return;
      if (!isRemoteControlTargetChangedError(e)) setError(e.message);
    } finally {
      if (
        requestSeq === listRequestSeqRef.current
        && isSessionListCurrent(targetEpoch)
      ) setLoadingMore(false);
    }
  }, [appendSessions, cacheScope, captureSessionListEpoch, hasMore, isSessionListCurrent, loading, loadingMore, sessionMgr, setError]);

  useEffect(() => {
    let cancelled = false;
    setHarnessCreateRequest(null);
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
    const isInitCurrent = () => (
      !cancelled && isSessionListCurrent(targetEpoch)
    );
    const init = async () => {
      try {
        const info = await sessionMgr.getWorkspaceInfo();
        if (!isInitCurrent()) return;
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
          setDisplayMode('pro');
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
        if (isInitCurrent() && !isRemoteControlTargetChangedError(e)) setError(e.message);
      } finally {
        if (isInitCurrent()) {
          setPairedDisplayMode(null);
          setLoading(false);
          targetInitializingRef.current = false;
          setTargetInitializing(false);
        }
      }
    };
    init();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [controlTargetEpoch]);

  const refreshData = useCallback(async () => {
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
    const requestSeq = ++listRequestSeqRef.current;
    // Refresh replaces both a first-page request and pagination. Their stale
    // finally blocks intentionally cannot publish, so this owner must also
    // settle the flags it superseded.
    setLoadingMore(false);
    try {
      if (displayMode === 'pro') {
        const info = await sessionMgr.getWorkspaceInfo();
        if (
          requestSeq !== listRequestSeqRef.current
          || !isSessionListCurrent(targetEpoch)
        ) return;
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
        if (
          requestSeq !== listRequestSeqRef.current
          || !isSessionListCurrent(targetEpoch)
        ) return;
        liveDataSeqRef.current += 1;
        setSessions(resp.sessions);
        setHasMore(resp.has_more);
        offsetRef.current = resp.sessions.length;
        remoteCache.saveSessionPage(cacheScope, resp.sessions, {
          workspacePath: ws?.path,
          replaceWorkspace: searchQuery.trim().length === 0,
        });
      } else {
        // Assistant mode: use currentAssistant path
        const resp = await sessionMgr.listSessions(currentAssistant?.path, PAGE_SIZE, 0, searchQuery);
        if (
          requestSeq !== listRequestSeqRef.current
          || !isSessionListCurrent(targetEpoch)
        ) return;
        liveDataSeqRef.current += 1;
        setSessions(resp.sessions);
        setHasMore(resp.has_more);
        offsetRef.current = resp.sessions.length;
        remoteCache.saveSessionPage(cacheScope, resp.sessions, {
          workspacePath: currentAssistant?.path,
          replaceWorkspace: searchQuery.trim().length === 0,
        });
      }
    } catch { /* ignore */ }
    finally {
      if (
        requestSeq === listRequestSeqRef.current
        && isSessionListCurrent(targetEpoch)
      ) {
        setLoading(false);
        setLoadingMore(false);
      }
    }
  }, [cacheScope, captureSessionListEpoch, currentAssistant?.path, displayMode, isSessionListCurrent, searchQuery, sessionMgr, setCurrentWorkspace, setSessions]);

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
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
    if (pullDistance >= PULL_THRESHOLD) {
      setRefreshing(true);
      setPullDistance(PULL_THRESHOLD);
      await refreshData();
      if (isSessionListCurrent(targetEpoch)) setRefreshing(false);
    }
    if (isSessionListCurrent(targetEpoch)) setPullDistance(0);
  }, [captureSessionListEpoch, isSessionListCurrent, pullDistance, refreshData]);

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

  const handleLoadMoreRecent = useCallback(async () => {
    if (compactRecentVisibleCount < sessions.length) {
      setCompactRecentVisibleCount((count) => count + 6);
      return;
    }
    const workspacePath = displayMode === 'assistant' ? currentAssistant?.path : currentWorkspace?.path;
    const identity = displayMode === 'assistant'
      ? undefined
      : {
          remoteConnectionId: currentWorkspace?.remote_connection_id,
          remoteSshHost: currentWorkspace?.remote_ssh_host,
        };
    await loadNextPage(workspacePath, searchQuery, identity);
    setCompactRecentVisibleCount((count) => count + 6);
  }, [
    compactRecentVisibleCount,
    currentAssistant?.path,
    currentWorkspace?.path,
    currentWorkspace?.remote_connection_id,
    currentWorkspace?.remote_ssh_host,
    displayMode,
    loadNextPage,
    searchQuery,
    sessions.length,
  ]);

  useEffect(() => {
    setCompactRecentVisibleCount(6);
  }, [searchQuery, controlTargetEpoch]);

  const handleCreate = useCallback(async (agentType: string) => {
    if (creating || targetInitializingRef.current) return;
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
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
      if (!isSessionListCurrent(targetEpoch)) return;
      await loadFirstPage(workspacePath, searchQuery, identity);
      if (!isSessionListCurrent(targetEpoch)) return;
      const label = isClawAgent(agentType)
        ? t('sessions.remoteClawSession')
        : isCoworkAgent(agentType)
          ? t('sessions.remoteCoworkSession')
          : t('sessions.remoteCodeSession');
      onSelectSession(id, label, true, agentType);
    } catch (e: any) {
      if (isSessionListCurrent(targetEpoch) && !isRemoteControlTargetChangedError(e)) {
        setError(e.message);
      }
    } finally {
      if (isSessionListCurrent(targetEpoch)) setCreating(false);
    }
  }, [
    creating,
    captureSessionListEpoch,
    currentWorkspace?.path,
    currentWorkspace?.remote_connection_id,
    currentWorkspace?.remote_ssh_host,
    currentAssistant?.path,
    displayMode,
    isSessionListCurrent,
    loadFirstPage,
    onSelectSession,
    searchQuery,
    sessionMgr,
    setError,
    t,
  ]);

  const requestHarnessCreate = useCallback((workspace?: RecentWorkspaceEntry) => {
    if (creating || targetInitializingRef.current) return;
    if (sessionMgr.supportsHostCapability(REMOTE_CAPABILITY_HARNESS_PROFILES_V1)) {
      setHarnessCreateRequest({ workspace });
      return;
    }
    if (workspace) {
      void handleCreateInCompactWorkspace(workspace, 'code');
    } else {
      void handleCreate('code');
    }
  }, [creating, handleCreate, handleCreateInCompactWorkspace, sessionMgr]);

  const handleHarnessSelect = useCallback((agentType: string) => {
    const request = harnessCreateRequest;
    setHarnessCreateRequest(null);
    if (!request) return;
    if (request.workspace) {
      void handleCreateInCompactWorkspace(request.workspace, agentType);
    } else {
      void handleCreate(agentType);
    }
  }, [handleCreate, handleCreateInCompactWorkspace, harnessCreateRequest]);

  const handleSelectMode = useCallback(async (mode: DisplayMode) => {
    if (targetInitializingRef.current) return;
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
    setDisplayMode(mode);
    setShowAssistantPicker(false);
    if (mode === 'assistant') {
      const assistantPath = await loadAssistantList();
      if (!isSessionListCurrent(targetEpoch)) return;
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
  }, [captureSessionListEpoch, currentWorkspace?.path, isSessionListCurrent, loadAssistantList, loadFirstPage, searchQuery, trySelectFirstProWorkspace]);

  const handleSelectAssistant = useCallback(async (assistant: { path: string; name: string; assistant_id?: string }) => {
    if (targetInitializingRef.current) return;
    const targetEpoch = captureSessionListEpoch();
    if (targetEpoch === null) return;
    try {
      await sessionMgr.setAssistant(assistant.path);
      if (!isSessionListCurrent(targetEpoch)) return;
      setCurrentAssistant(assistant);
      setShowAssistantPicker(false);
      loadFirstPage(assistant.path, searchQuery);
    } catch (e: any) {
      if (isSessionListCurrent(targetEpoch) && !isRemoteControlTargetChangedError(e)) {
        setError(e.message);
      }
    }
  }, [captureSessionListEpoch, isSessionListCurrent, loadFirstPage, searchQuery, sessionMgr, setCurrentAssistant, setError]);

  const workspaceDisplayName = currentWorkspace?.project_name || t('sessions.noWorkspaceSelected');
  const assistantDisplayName = currentAssistant?.name || t('shared.agents.default');
  const isProMode = displayMode === 'pro';

  if (compact) {
    const query = searchQuery.trim().toLocaleLowerCase();
    const visibleSessions = sessions.filter((session) => (
      query.length === 0 || (session.name || '').toLocaleLowerCase().includes(query)
    ));
    const compactWorkspaces = mergeCompactWorkspaces(workspaceList, currentWorkspace, sessions);
    const activeDeviceId = client?.pairedDeviceId
      ?? (client?.isPaired ? COMPACT_PAIRED_ROOM_DEVICE_ID : null);
    const projectedCompactDevices = !activeDeviceId || compactDevices.some((device) => (
      device.device_id === activeDeviceId
    ))
      ? compactDevices
      : [{
          device_id: activeDeviceId,
          device_name: client?.pairedDeviceId
            ? controlTarget?.deviceName || client.pairedDeviceId
            : t('devices.pairedDesktopName'),
          online: connectionHealth !== 'unreachable',
          room_route: !client?.pairedDeviceId,
        }, ...compactDevices];

    return (
      <div className="harmony-sidebar">
        <header className="harmony-sidebar__header">
          <h1>OpenBitFun</h1>
          <MobileIconButton
            appearance="floating"
            className="harmony-sidebar__round-action"
            aria-label={t('shared.tools.search')}
            icon={(
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
                <circle cx="11" cy="11" r="7" />
                <path d="m16.5 16.5 4 4" />
              </svg>
            )}
            onClick={() => {
              setCompactSearchOpen((open) => !open);
              if (compactSearchOpen) setSearchQuery('');
            }}
            selected={compactSearchOpen}
          />
        </header>

        {compactSearchOpen && (
          <MobileTextField
            className="harmony-sidebar__search"
            type="search"
            value={searchQuery}
            onChange={(event) => setSearchQuery(event.target.value)}
            placeholder={t('sessions.searchSessions')}
            autoFocus
          />
        )}

        <div className="harmony-sidebar__scroll">
          <MobileSection className="harmony-sidebar__section">
            <div className="harmony-sidebar__section-heading">
              <h2>{t('devices.title')}</h2>
              <span className="harmony-sidebar__heading-actions">
                <MobileIconButton appearance="plain" size="sm" aria-label={t('devices.refresh')} loading={compactDirectoryLoading} onClick={() => void loadCompactDirectory()} icon={<svg className={compactDirectoryLoading ? 'is-spinning' : ''} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                    <path d="M20 11a8 8 0 1 0-2.34 5.66"/><path d="M20 4v7h-7"/>
                  </svg>} />
                <MobileIconButton appearance="plain" size="sm" aria-label={t('devices.title')} onClick={onOpenDevices} icon={<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" aria-hidden="true"><path d="M12 4v16M4 12h16"/></svg>} />
              </span>
            </div>
            <div className="harmony-sidebar__rows">
              {projectedCompactDevices.slice(0, compactVisibleDeviceCount).map((device) => {
                const isCurrent = device.device_id === compactSelectedDeviceId;
                const isSwitching = device.device_id === compactSwitchingDeviceId;
                return (
                  <MobileButton
                    appearance="plain"
                    block
                    className={`harmony-sidebar__device-row${isCurrent ? ' is-current' : ''}`}
                    key={device.device_id}
                    disabled={!device.online || (!!compactSwitchingDeviceId && !isSwitching)}
                    onClick={() => void handleSelectCompactDevice(device)}
                  >
                    <span className="harmony-sidebar__device-icon" aria-hidden="true">
                      <CompactDeviceIcon name={device.device_name || device.device_id}/>
                    </span>
                    <span className="harmony-sidebar__row-label">{device.device_name || device.device_id}</span>
                    {isSwitching
                      ? <span className="spinner harmony-sidebar__row-spinner"/>
                      : <span className={`harmony-sidebar__status${device.online ? ' is-online' : ''}`}/>}
                    <span className={`harmony-sidebar__chevron${isCurrent ? ' is-expanded' : ''}`} aria-hidden="true">
                      <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="m4.5 2.5 4.5 4.5-4.5 4.5"/></svg>
                    </span>
                  </MobileButton>
                );
              })}
              {projectedCompactDevices.length > compactVisibleDeviceCount && (
                <MobileButton appearance="plain" block className="harmony-sidebar__more-row" onClick={() => setCompactVisibleDeviceCount((count) => count + 3)}>
                  <span>···</span>{t('shell.moreDevices', { count: projectedCompactDevices.length - compactVisibleDeviceCount })}
                </MobileButton>
              )}
            </div>
          </MobileSection>

          {compactSelectedDeviceId && (
            <MobileSection className="harmony-sidebar__section harmony-sidebar__section--workspaces">
              <div className="harmony-sidebar__section-heading">
                <h2>{t('shared.features.workspace')}</h2>
                <MobileIconButton appearance="plain" size="sm" aria-label={t('workspace.selectWorkspace')} onClick={onOpenWorkspace} icon={<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" aria-hidden="true"><path d="M12 4v16M4 12h16"/></svg>} />
              </div>
              <div className="harmony-sidebar__rows">
                {compactDirectoryLoading && compactWorkspaces.length === 0 && (
                  <MobileStatus className="harmony-sidebar__empty" loading title={t('common.loading')} />
                )}
                {!compactDirectoryLoading && compactWorkspaces.length === 0 && (
                  <MobileStatus className="harmony-sidebar__empty" description={t('sessions.noWorkspaces')} />
                )}
                {compactWorkspaces.slice(0, compactVisibleWorkspaceCount).map((workspace) => {
                  const key = compactWorkspaceKey(workspace);
                  const expanded = compactExpandedWorkspaces.has(key);
                  const projectedSessions = sessions.filter((session) => (
                    (session.workspace_path || currentWorkspace?.path) === workspace.path
                  ));
                  const workspaceSessions = compactWorkspaceSessions[key] ?? projectedSessions;
                  const status = compactWorkspaceStatuses[key]
                    ?? (projectedSessions.length > 0 ? 'ready' : 'idle');
                  const visibleCount = compactVisibleSessionCounts[key] ?? 3;
                  const current = currentWorkspace?.path === workspace.path
                    && currentWorkspace?.remote_connection_id === workspace.remote_connection_id;
                  return (
                    <div className="harmony-sidebar__workspace-group" key={key}>
                      <div className={`harmony-sidebar__workspace-row${current ? ' is-current' : ''}`}>
                        <MobileIconButton appearance="plain" size="sm" className={`harmony-sidebar__workspace-disclosure${expanded ? ' is-expanded' : ''}`} onClick={() => void handleToggleCompactWorkspace(workspace)} aria-label={expanded ? t('common.close') : t('sessions.sessionHistory')} icon={<svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="m4.5 2.5 4.5 4.5-4.5 4.5"/></svg>} />
                        <MobileButton appearance="plain" block className="harmony-sidebar__workspace-main" onClick={() => void handleToggleCompactWorkspace(workspace)}>
                          <span className="harmony-sidebar__folder-icon" aria-hidden="true">
                            <svg width="21" height="21" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.65" strokeLinecap="round" strokeLinejoin="round"><path d="M3 7.5A2.5 2.5 0 0 1 5.5 5h4l2 2h7A2.5 2.5 0 0 1 21 9.5v8A2.5 2.5 0 0 1 18.5 20h-13A2.5 2.5 0 0 1 3 17.5z"/></svg>
                          </span>
                          <span className="harmony-sidebar__row-label">{workspace.name || workspace.path}</span>
                        </MobileButton>
                        <MobileIconButton appearance="plain" size="sm" className="harmony-sidebar__row-plus" onClick={() => requestHarnessCreate(workspace)} aria-label={t('shell.newChat')} disabled={creating} icon={<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round"><path d="M12 4v16M4 12h16"/></svg>} />
                      </div>
                      {expanded && (
                        <div className="harmony-sidebar__workspace-sessions">
                          {status === 'loading' && <MobileStatus className="harmony-sidebar__workspace-message" loading title={t('sessions.loadingSessions')} />}
                          {status === 'failed' && <MobileButton appearance="plain" block className="harmony-sidebar__workspace-message" onClick={() => void handleRetryCompactWorkspace(workspace)}>{t('devices.retry')}</MobileButton>}
                          {status === 'ready' && workspaceSessions.length === 0 && <MobileStatus className="harmony-sidebar__workspace-message" description={t('sessions.noSessions')} />}
                          {workspaceSessions.slice(0, visibleCount).map((session) => (
                            <div
                              className={`harmony-sidebar__workspace-session${activeSessionId === session.session_id ? ' is-current' : ''}`}
                              key={session.session_id}
                              onContextMenu={(event) => { event.preventDefault(); setMenuSession(session); }}
                            >
                              <MobileButton appearance="plain" block className="harmony-sidebar__session-main" onClick={(event) => handleSessionClick(session, event)}>
                                <span className="harmony-sidebar__session-icon" aria-hidden="true"><SessionTypeIcon agentType={session.agent_type}/></span>
                                <span className="harmony-sidebar__row-label">{session.name || t('sessions.untitledSession')}</span>
                              </MobileButton>
                              <MobileIconButton
                                appearance="plain"
                                size="sm"
                                className="harmony-sidebar__session-more"
                                aria-label={t('sessions.sessionActions')}
                                onClick={(event) => { event.stopPropagation(); setMenuSession(session); }}
                                icon={<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><circle cx="6" cy="12" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="18" cy="12" r="1.2"/></svg>}
                              />
                            </div>
                          ))}
                          {(workspaceSessions.length > visibleCount || compactWorkspaceHasMore[key]) && (
                            <MobileButton
                              appearance="plain"
                              block
                              className="harmony-sidebar__more-row harmony-sidebar__more-row--nested"
                              onClick={() => void handleLoadMoreCompactWorkspace(workspace)}
                              disabled={compactWorkspaceLoadingMore.has(key)}
                            >
                              {compactWorkspaceLoadingMore.has(key)
                                ? <><span className="spinner"/>{t('sessions.loadingMore')}</>
                                : <><span>···</span>{t('shell.moreConversations', { count: Math.max(workspaceSessions.length - visibleCount, 1) })}</>}
                            </MobileButton>
                          )}
                        </div>
                      )}
                    </div>
                  );
                })}
                {compactWorkspaces.length > compactVisibleWorkspaceCount && (
                  <MobileButton appearance="plain" block className="harmony-sidebar__more-row" onClick={() => setCompactVisibleWorkspaceCount((count) => count + 3)}>
                    <span>···</span>{t('shell.moreWorkspaces', { count: compactWorkspaces.length - compactVisibleWorkspaceCount })}
                  </MobileButton>
                )}
              </div>
            </MobileSection>
          )}

          <MobileSection className="harmony-sidebar__section harmony-sidebar__section--conversations">
            <div className="harmony-sidebar__section-heading harmony-sidebar__section-heading--plain">
              <h2>{t('shell.recentConversations')}</h2>
            </div>
            {visibleSessions.length === 0 ? (
              <MobileStatus className="harmony-sidebar__empty" description={hasSearchQuery ? t('sessions.emptySearch') : t('sessions.noSessions')} />
            ) : (
              <div className="harmony-sidebar__rows">
                {visibleSessions.slice(0, compactRecentVisibleCount).map((session) => (
                  <div
                    className={`harmony-sidebar__session-row${activeSessionId === session.session_id ? ' is-current' : ''}`}
                    key={session.session_id}
                    onContextMenu={(event) => { event.preventDefault(); setMenuSession(session); }}
                  >
                    <MobileButton appearance="plain" block className="harmony-sidebar__session-main" onClick={(event) => handleSessionClick(session, event)}>
                      <span className="harmony-sidebar__session-icon" aria-hidden="true"><SessionTypeIcon agentType={session.agent_type}/></span>
                      <span className="harmony-sidebar__row-label">{session.name || t('sessions.untitledSession')}</span>
                    </MobileButton>
                    <MobileIconButton
                      appearance="plain"
                      size="sm"
                      className="harmony-sidebar__session-more"
                      aria-label={t('sessions.sessionActions')}
                      onClick={(event) => { event.stopPropagation(); setMenuSession(session); }}
                      icon={<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><circle cx="6" cy="12" r="1.25"/><circle cx="12" cy="12" r="1.25"/><circle cx="18" cy="12" r="1.25"/></svg>}
                    />
                  </div>
                ))}
                {(visibleSessions.length > compactRecentVisibleCount || hasMore) && (
                  <MobileButton
                    appearance="plain"
                    block
                    className="harmony-sidebar__more-row"
                    onClick={() => void handleLoadMoreRecent()}
                    disabled={loadingMore}
                  >
                    {loadingMore
                      ? <><span className="spinner"/>{t('sessions.loadingMore')}</>
                      : <><span>···</span>{t('shell.moreConversations', { count: Math.max(visibleSessions.length - compactRecentVisibleCount, 1) })}</>}
                  </MobileButton>
                )}
              </div>
            )}
          </MobileSection>
        </div>

        <MobileFloatingActions
          className="harmony-sidebar__footer"
          leading={(
            <MobileButton appearance="secondary" className="harmony-sidebar__new-chat" onClick={() => isProMode ? requestHarnessCreate() : void handleCreate('claw')} disabled={creating || targetInitializing} leading={<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L8 18l-4 1 1-4z"/></svg>}>
              <span>{t('shell.newChat')}</span>
            </MobileButton>
          )}
          trailing={(
            <MobileIconButton
              appearance="floating"
              className="harmony-sidebar__settings"
              onClick={() => setCompactSettingsOpen(true)}
              aria-label={t('shared.features.settings')}
              icon={<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06A1.7 1.7 0 0 0 15 19.4a1.7 1.7 0 0 0-1 .6 1.7 1.7 0 0 0-.4 1.1V21h-4v-.09A1.7 1.7 0 0 0 8.6 19.4a1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.2 15a1.7 1.7 0 0 0-.6-1 1.7 1.7 0 0 0-1.1-.4H2.4v-4h.09A1.7 1.7 0 0 0 4.2 8.6a1.7 1.7 0 0 0-.34-1.88l-.06-.06 2.83-2.83.06.06A1.7 1.7 0 0 0 8.6 4.2a1.7 1.7 0 0 0 1-.6 1.7 1.7 0 0 0 .4-1.1V2.4h4v.09A1.7 1.7 0 0 0 15 4.2a1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06A1.7 1.7 0 0 0 19.4 8.6a1.7 1.7 0 0 0 .6 1 1.7 1.7 0 0 0 1.1.4h.09v4h-.09a1.7 1.7 0 0 0-1.7 1z"/></svg>}
            />
          )}
        />

        <CompactSettingsSheet
          accountLabel={authenticatedUserLabel}
          devices={projectedCompactDevices}
          isDark={isDark}
          onClose={() => setCompactSettingsOpen(false)}
          onDisconnectRequest={() => { setCompactSettingsOpen(false); setShowDisconnectConfirm(true); }}
          onSelectDevice={(device) => void handleSelectCompactDevice(device)}
          onToggleTheme={toggleTheme}
          open={compactSettingsOpen}
          renderDeviceIcon={(name) => <CompactDeviceIcon name={name} />}
          selectedDeviceId={compactSelectedDeviceId}
        />

        <SessionOverlays
          compact
          deleteTarget={deleteConfirmTarget}
          deleting={deleting}
          harnessOpen={harnessCreateRequest !== null}
          menuSession={menuSession}
          onCloseDelete={() => !deleting && setDeleteConfirmTarget(null)}
          onCloseDisconnect={() => setShowDisconnectConfirm(false)}
          onCloseHarness={() => setHarnessCreateRequest(null)}
          onCloseMenu={() => setMenuSession(null)}
          onCloseRename={() => !renaming && setRenameTarget(null)}
          onConfirmDelete={() => void handleDelete()}
          onConfirmDisconnect={() => { setShowDisconnectConfirm(false); onDisconnect(); }}
          onConfirmRename={() => void handleRename()}
          onDeleteRequest={setDeleteConfirmTarget}
          onHarnessSelect={handleHarnessSelect}
          onRenameRequest={(session) => { setRenameTarget(session); setRenameValue(session.name || ''); }}
          onRenameValueChange={setRenameValue}
          renameTarget={renameTarget}
          renameValue={renameValue}
          renaming={renaming}
          showDisconnectConfirm={showDisconnectConfirm}
        />
      </div>
    );
  }

  return (
    <div className="session-list">
      <div className="session-list__header">
        <div className="session-list__header-brand">
          <img src={logoMark} alt="OpenBitFun" className="session-list__logo" />
          <div className="session-list__header-copy">
            <h1>OpenBitFun</h1>
            {authenticatedUserLabel && (
              <span className="session-list__header-account-name">
                <span className={`session-list__health-dot session-list__health-dot--${connectionHealth}`} title={(() => { switch (connectionHealth) { case 'connected': return t('sessions.connectionConnected'); case 'checking': return t('sessions.connectionChecking'); case 'unreachable': return t('sessions.connectionUnreachable'); default: return t('sessions.connectionUnpaired'); } })()} />
                {authenticatedUserLabel}
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
            <MobileIconButton
              appearance="plain"
              className={`session-list__devices-btn ${controlTarget && !controlTarget.isHome ? 'is-remote' : ''}`}
              onClick={onOpenDevices}
              title={t('devices.title')} aria-label={t('devices.title')} icon={<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
                <line x1="8" y1="21" x2="16" y2="21" />
                <line x1="12" y1="17" x2="12" y2="21" />
              </svg>} />
          )}
          <LanguageToggleButton className="session-list__language-btn" />
          <MobileIconButton appearance="plain" className="session-list__theme-btn" onClick={toggleTheme} aria-label={t('common.toggleTheme')} icon={<ThemeToggleIcon isDark={isDark} />} />
          <MobileIconButton appearance="plain" className="session-list__disconnect-btn" onClick={() => setShowDisconnectConfirm(true)} aria-label={t('sessions.disconnect')} title={t('sessions.disconnect')} icon={<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
              <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
              <polyline points="16 17 21 12 16 7" />
              <line x1="21" y1="12" x2="9" y2="12" />
            </svg>} />
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
          <MobileButton
            appearance="secondary"
            block
            className={`session-list__resume-card${activeSessionId === sessions[0].session_id ? ' is-selected' : ''}`}
            onClick={(e) => handleSessionClick(sessions[0], e)}
            onTouchStart={(e) => handleSessionTouchStart(sessions[0], e)}
            onTouchMove={handleSessionTouchMove}
            onTouchEnd={handleSessionTouchEnd}
            onTouchCancel={handleSessionTouchEnd}
            onContextMenu={(e) => { e.preventDefault(); setMenuSession(sessions[0]); }}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                onSelectSession(sessions[0].session_id, sessions[0].name, false, sessions[0].agent_type);
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
          </MobileButton>
        )}

        {/* Mode Toggle - Inline */}
        <MobileSegmentedControl
          aria-label={t('shared.modes.expert')}
          className="session-list__mode-toggle"
          onChange={handleSelectMode}
          options={[
            { disabled: targetInitializing, label: <><ProModeIcon /><span>{t('shared.modes.expert')}</span></>, value: 'pro' },
            { disabled: targetInitializing, label: <><AssistantModeIcon /><span>{t('shared.modes.assistant')}</span></>, value: 'assistant' },
          ]}
          value={displayMode}
        />

        {/* Pro Mode: Workspace Selection Required */}
        {isProMode && (
          <>
            <MobileButton
              appearance="plain"
              block
              className="session-list__workspace-bar"
              onClick={() => {
                if (targetInitializingRef.current) return;
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
            </MobileButton>

            <MobileChoiceSheet
              className="session-list__picker-modal session-list__picker-modal--workspace"
              emptyContent={<MobileStatus title={t('sessions.noWorkspaces')} />}
              headerAction={<MobileIconButton appearance="plain" className="session-list__picker-close" onClick={() => setShowWorkspacePicker(false)} aria-label={t('common.close')} icon={<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>} />}
              onOpenChange={() => setShowWorkspacePicker(false)}
              onSelect={(value) => {
                const workspace = workspaceList.find((candidate, index) => [
                  candidate.remote_connection_id ?? 'local',
                  candidate.remote_ssh_host ?? '',
                  candidate.path || String(index),
                ].join(':') === value);
                if (workspace) void handleSelectWorkspace(workspace);
              }}
              open={showWorkspacePicker}
              optionAppearance="plain"
              options={workspaceList.map((workspace, index) => {
                const selected = currentWorkspace?.path === workspace.path
                  && (currentWorkspace?.remote_connection_id ?? undefined) === (workspace.remote_connection_id ?? undefined)
                  && (currentWorkspace?.remote_ssh_host ?? undefined) === (workspace.remote_ssh_host ?? undefined);
                return {
                  className: `session-list__picker-item session-list__picker-item--workspace ${selected ? 'is-selected' : ''}`,
                  label: workspace.name,
                  leading: <span className="session-list__picker-item-icon"><WorkspaceIcon /></span>,
                  trailing: selected ? <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><polyline points="20 6 9 17 4 12"/></svg> : undefined,
                  value: [workspace.remote_connection_id ?? 'local', workspace.remote_ssh_host ?? '', workspace.path || String(index)].join(':'),
                };
              })}
              selectedValue={currentWorkspace?.path ? [currentWorkspace.remote_connection_id ?? 'local', currentWorkspace.remote_ssh_host ?? '', currentWorkspace.path].join(':') : undefined}
              showHandle={false}
              title={t('sessions.selectWorkspace')}
            />
          </>
        )}

        {/* Assistant Mode: Assistant Selection */}
        {!isProMode && (
          <>
            <MobileButton
              appearance="plain"
              block
              className="session-list__assistant-bar"
              onClick={() => {
                if (targetInitializingRef.current) return;
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
            </MobileButton>

            <MobileChoiceSheet
              className="session-list__picker-modal"
              headerAction={<MobileIconButton appearance="plain" className="session-list__picker-close" onClick={() => setShowAssistantPicker(false)} aria-label={t('common.close')} icon={<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>} />}
              onOpenChange={() => setShowAssistantPicker(false)}
              onSelect={(path) => {
                const assistant = assistantList.find((candidate, index) => (candidate.path || String(index)) === path);
                if (assistant) void handleSelectAssistant(assistant);
              }}
              open={showAssistantPicker}
              optionAppearance="plain"
              options={assistantList.map((assistant, index) => ({
                className: `session-list__picker-item ${currentAssistant?.path === assistant.path ? 'is-selected' : ''}`,
                label: assistant.name,
                leading: <span className="session-list__picker-item-icon"><AssistantModeIcon /></span>,
                trailing: currentAssistant?.path === assistant.path ? <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><polyline points="20 6 9 17 4 12"/></svg> : undefined,
                value: assistant.path || String(index),
              }))}
              selectedValue={currentAssistant?.path}
              showHandle={false}
              title={t('sessions.selectAssistant')}
            />
          </>
        )}

        <SessionLaunchPanel
          creating={creating}
          hasWorkspace={!!currentWorkspace}
          isProMode={isProMode}
          targetInitializing={targetInitializing}
          onCreateClaw={() => void handleCreate('claw')}
          onCreateCowork={() => void handleCreate('cowork')}
          onRequestCodeHarness={() => requestHarnessCreate()}
          renderSessionIcon={(agentType) => <SessionTypeIcon agentType={agentType} />}
        />

        <SessionHistoryPanel
          activeSessionId={activeSessionId}
          hasSearchQuery={hasSearchQuery}
          isProMode={isProMode}
          loading={loading}
          loadingMore={loadingMore}
          menuSessionId={menuSession?.session_id}
          searchQuery={searchQuery}
          sessions={sessions.slice(showResumeCard ? 1 : 0)}
          totalSessionCount={sessions.length}
          targetInitializing={targetInitializing}
          onOpenMenu={setMenuSession}
          onSearchQueryChange={setSearchQuery}
          onSessionClick={handleSessionClick}
          onSessionTouchEnd={handleSessionTouchEnd}
          onSessionTouchMove={handleSessionTouchMove}
          onSessionTouchStart={handleSessionTouchStart}
          renderAgentLabel={(agentType) => agentLabel(agentType, t)}
          renderSessionIcon={(agentType) => <SessionTypeIcon agentType={agentType} />}
          renderSessionTime={(updatedAt) => formatTime(updatedAt, formatDate, t)}
        />
      </div>

      <SessionOverlays
        compact={false}
        deleteTarget={deleteConfirmTarget}
        deleting={deleting}
        harnessOpen={harnessCreateRequest !== null}
        menuSession={menuSession}
        onCloseDelete={() => !deleting && setDeleteConfirmTarget(null)}
        onCloseDisconnect={() => setShowDisconnectConfirm(false)}
        onCloseHarness={() => setHarnessCreateRequest(null)}
        onCloseMenu={() => setMenuSession(null)}
        onCloseRename={() => !renaming && setRenameTarget(null)}
        onConfirmDelete={() => void handleDelete()}
        onConfirmDisconnect={() => { setShowDisconnectConfirm(false); onDisconnect(); }}
        onConfirmRename={() => void handleRename()}
        onDeleteRequest={setDeleteConfirmTarget}
        onHarnessSelect={handleHarnessSelect}
        onRenameRequest={(session) => { setRenameTarget(session); setRenameValue(session.name || ''); }}
        onRenameValueChange={setRenameValue}
        renameTarget={renameTarget}
        renameValue={renameValue}
        renaming={renaming}
        showDisconnectConfirm={showDisconnectConfirm}
      />

      {/* Action Toast */}
      {actionToast && <MobileBanner className="session-list__toast" role="alert" aria-live="assertive">{actionToast}</MobileBanner>}
    </div>
  );
};

export default SessionListPage;
