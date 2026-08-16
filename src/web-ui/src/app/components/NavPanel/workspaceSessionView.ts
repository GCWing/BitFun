import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';
import type { Session } from '@/flow_chat/types/flow-chat';

export type WorkspaceSessionGrouping = 'grouped' | 'all';
export type WorkspaceSessionOrdering = 'updated' | 'status' | 'created' | 'name';
export type WorkspaceSessionShow = 'all' | 'unread' | 'attention';
export type WorkspaceSessionStatus = 'running' | 'attention' | 'error' | 'completed' | 'idle';
export type WorkspaceSessionEnvironment = 'local' | 'remote' | 'detached';
export type WorkspaceSessionSource = 'bitfun' | 'external';
export type WorkspaceSessionWorktree = 'main' | 'worktree';

export interface WorkspaceSessionFilters {
  hiddenStatuses: WorkspaceSessionStatus[];
  hiddenEnvironments: WorkspaceSessionEnvironment[];
  hiddenSources: WorkspaceSessionSource[];
  hiddenWorktrees: WorkspaceSessionWorktree[];
  hideArchived: boolean;
}

export interface WorkspaceSessionViewPreferences {
  grouping: WorkspaceSessionGrouping;
  ordering: WorkspaceSessionOrdering;
  show: WorkspaceSessionShow;
  filters: WorkspaceSessionFilters;
}

interface WorkspaceSessionViewState extends WorkspaceSessionViewPreferences {
  collapseAllRequestId: number;
  setGrouping: (grouping: WorkspaceSessionGrouping) => void;
  setOrdering: (ordering: WorkspaceSessionOrdering) => void;
  setShow: (show: WorkspaceSessionShow) => void;
  toggleHiddenStatus: (value: WorkspaceSessionStatus) => void;
  toggleHiddenEnvironment: (value: WorkspaceSessionEnvironment) => void;
  toggleHiddenSource: (value: WorkspaceSessionSource) => void;
  toggleHiddenWorktree: (value: WorkspaceSessionWorktree) => void;
  toggleArchived: () => void;
  resetFilters: () => void;
  requestCollapseAll: () => void;
}

export const DEFAULT_WORKSPACE_SESSION_FILTERS: WorkspaceSessionFilters = {
  hiddenStatuses: [],
  hiddenEnvironments: [],
  hiddenSources: [],
  hiddenWorktrees: [],
  hideArchived: true,
};

export const DEFAULT_WORKSPACE_SESSION_VIEW: WorkspaceSessionViewPreferences = {
  grouping: 'grouped',
  ordering: 'updated',
  show: 'all',
  filters: DEFAULT_WORKSPACE_SESSION_FILTERS,
};

export const getNextWorkspaceSessionGrouping = (
  grouping: WorkspaceSessionGrouping,
): WorkspaceSessionGrouping => grouping === 'grouped' ? 'all' : 'grouped';

export const normalizeWorkspaceSessionGrouping = (value: unknown): WorkspaceSessionGrouping =>
  value === 'all' ? 'all' : 'grouped';

const toggleArrayValue = <T extends string>(values: T[], value: T): T[] =>
  values.includes(value) ? values.filter(item => item !== value) : [...values, value];

export const useWorkspaceSessionViewStore = create<WorkspaceSessionViewState>()(
  persist(
    (set) => ({
      ...DEFAULT_WORKSPACE_SESSION_VIEW,
      collapseAllRequestId: 0,
      setGrouping: grouping => set({ grouping }),
      setOrdering: ordering => set({ ordering }),
      setShow: show => set({ show }),
      toggleHiddenStatus: value => set(state => ({
        filters: { ...state.filters, hiddenStatuses: toggleArrayValue(state.filters.hiddenStatuses, value) },
      })),
      toggleHiddenEnvironment: value => set(state => ({
        filters: { ...state.filters, hiddenEnvironments: toggleArrayValue(state.filters.hiddenEnvironments, value) },
      })),
      toggleHiddenSource: value => set(state => ({
        filters: { ...state.filters, hiddenSources: toggleArrayValue(state.filters.hiddenSources, value) },
      })),
      toggleHiddenWorktree: value => set(state => ({
        filters: { ...state.filters, hiddenWorktrees: toggleArrayValue(state.filters.hiddenWorktrees, value) },
      })),
      toggleArchived: () => set(state => ({
        filters: { ...state.filters, hideArchived: !state.filters.hideArchived },
      })),
      resetFilters: () => set({ filters: DEFAULT_WORKSPACE_SESSION_FILTERS }),
      requestCollapseAll: () => set(state => ({ collapseAllRequestId: state.collapseAllRequestId + 1 })),
    }),
    {
      name: 'bitfun.workspace-session-view.v2',
      version: 3,
      storage: createJSONStorage(() => localStorage),
      partialize: ({ grouping, ordering, show, filters }) => ({ grouping, ordering, show, filters }),
      migrate: (persistedState: unknown) => {
        const state = persistedState && typeof persistedState === 'object'
          ? persistedState as Partial<WorkspaceSessionViewState> & { grouping?: unknown }
          : {};
        return {
          ...state,
          grouping: normalizeWorkspaceSessionGrouping(state.grouping),
        } as WorkspaceSessionViewState;
      },
    },
  ),
);

const getUpdatedTimestamp = (session: Session): number =>
  session.updatedAt ?? session.lastFinishedAt ?? session.lastActiveAt ?? session.createdAt;

export function deriveWorkspaceSessionStatus(
  session: Session,
  isRunning: boolean,
): WorkspaceSessionStatus {
  if (isRunning) return 'running';
  if (session.needsUserAttention) return 'attention';
  if (session.status === 'error' || session.error) return 'error';
  if (session.persistedStatus === 'completed' || typeof session.lastFinishedAt === 'number') return 'completed';
  return 'idle';
}

export function deriveWorkspaceSessionEnvironment(session: Session): WorkspaceSessionEnvironment {
  if (session.config.dispatchTarget) return 'detached';
  if (session.remoteConnectionId || session.remoteSshHost) return 'remote';
  return 'local';
}

export function deriveWorkspaceSessionSource(session: Session): WorkspaceSessionSource {
  return session.config.agentType?.startsWith('acp:') ? 'external' : 'bitfun';
}

export function deriveWorkspaceSessionWorktree(session: Session): WorkspaceSessionWorktree {
  const target = session.config.executionTarget;
  return target && target.kind !== 'local' ? 'worktree' : 'main';
}

const STATUS_ORDER: Record<WorkspaceSessionStatus, number> = {
  running: 0,
  attention: 1,
  error: 2,
  idle: 3,
  completed: 4,
};

export function compareWorkspaceNavSessions(
  left: Session,
  right: Session,
  ordering: WorkspaceSessionOrdering,
  getTitle: (session: Session) => string,
  isRunning: (session: Session) => boolean = () => false,
): number {
  if (ordering === 'name') {
    const titleDiff = getTitle(left).localeCompare(getTitle(right), undefined, {
      numeric: true,
      sensitivity: 'base',
    });
    if (titleDiff !== 0) return titleDiff;
  } else if (ordering === 'status') {
    const statusDiff = STATUS_ORDER[deriveWorkspaceSessionStatus(left, isRunning(left))]
      - STATUS_ORDER[deriveWorkspaceSessionStatus(right, isRunning(right))];
    if (statusDiff !== 0) return statusDiff;
  } else {
    const leftTimestamp = ordering === 'updated' ? getUpdatedTimestamp(left) : left.createdAt;
    const rightTimestamp = ordering === 'updated' ? getUpdatedTimestamp(right) : right.createdAt;
    const timestampDiff = rightTimestamp - leftTimestamp;
    if (timestampDiff !== 0) return timestampDiff;
  }

  const updatedDiff = getUpdatedTimestamp(right) - getUpdatedTimestamp(left);
  return updatedDiff || right.createdAt - left.createdAt || left.sessionId.localeCompare(right.sessionId);
}

export function matchesWorkspaceSessionView(
  session: Session,
  show: WorkspaceSessionShow,
  filters: WorkspaceSessionFilters,
  isRunning: boolean,
): boolean {
  if (show === 'unread' && !session.hasUnreadCompletion) return false;
  if (show === 'attention' && !session.needsUserAttention) return false;
  if (filters.hideArchived && session.persistedStatus === 'archived') return false;
  if (filters.hiddenStatuses.includes(deriveWorkspaceSessionStatus(session, isRunning))) return false;
  if (filters.hiddenEnvironments.includes(deriveWorkspaceSessionEnvironment(session))) return false;
  if (filters.hiddenSources.includes(deriveWorkspaceSessionSource(session))) return false;
  if (filters.hiddenWorktrees.includes(deriveWorkspaceSessionWorktree(session))) return false;
  return true;
}

export function hasWorkspaceSessionFilters(filters: WorkspaceSessionFilters): boolean {
  return filters.hiddenStatuses.length > 0
    || filters.hiddenEnvironments.length > 0
    || filters.hiddenSources.length > 0
    || filters.hiddenWorktrees.length > 0
    || filters.hideArchived !== DEFAULT_WORKSPACE_SESSION_FILTERS.hideArchived;
}
