import { describe, expect, it } from 'vitest';
import type { Session } from '@/flow_chat/types/flow-chat';
import {
  compareWorkspaceNavSessions,
  DEFAULT_WORKSPACE_SESSION_FILTERS,
  deriveWorkspaceSessionEnvironment,
  deriveWorkspaceSessionSource,
  deriveWorkspaceSessionStatus,
  deriveWorkspaceSessionWorktree,
  getNextWorkspaceSessionGrouping,
  matchesWorkspaceSessionView,
  normalizeWorkspaceSessionGrouping,
} from './workspaceSessionView';

const session = (overrides: Partial<Session>): Session => ({
  sessionId: 'session',
  title: 'Session',
  config: {},
  createdAt: 1,
  lastActiveAt: 1,
  updatedAt: undefined,
  lastFinishedAt: undefined,
  persistedStatus: 'active',
  status: 'idle',
  error: null,
  dialogTurns: [],
  ...overrides,
} as Session);

describe('workspace session view model', () => {
  it('toggles between unified groups and the flat all-session view', () => {
    expect(getNextWorkspaceSessionGrouping('grouped')).toBe('all');
    expect(getNextWorkspaceSessionGrouping('all')).toBe('grouped');
  });

  it('migrates legacy workspace grouping to the unified grouped view', () => {
    expect(normalizeWorkspaceSessionGrouping('workspace')).toBe('grouped');
    expect(normalizeWorkspaceSessionGrouping('grouped')).toBe('grouped');
    expect(normalizeWorkspaceSessionGrouping('all')).toBe('all');
    expect(normalizeWorkspaceSessionGrouping(undefined)).toBe('grouped');
  });

  it('sorts by update time, status, creation time, and title', () => {
    const running = session({ sessionId: 'running', title: 'Zulu', createdAt: 10, updatedAt: 40 });
    const newer = session({ sessionId: 'newer', title: 'Alpha', createdAt: 20, updatedAt: 30 });
    const getTitle = (value: Session) => value.title || '';
    const isRunning = (value: Session) => value.sessionId === 'running';

    expect(compareWorkspaceNavSessions(running, newer, 'updated', getTitle, isRunning)).toBeLessThan(0);
    expect(compareWorkspaceNavSessions(running, newer, 'status', getTitle, isRunning)).toBeLessThan(0);
    expect(compareWorkspaceNavSessions(running, newer, 'created', getTitle, isRunning)).toBeGreaterThan(0);
    expect(compareWorkspaceNavSessions(running, newer, 'name', getTitle, isRunning)).toBeGreaterThan(0);
  });

  it('derives filter facets from canonical session data', () => {
    const value = session({
      status: 'error',
      remoteConnectionId: 'remote',
      config: {
        agentType: 'acp:codex',
        executionTarget: { kind: 'managedWorktree', rootPath: '/repo/.worktrees/task' },
      },
    });

    expect(deriveWorkspaceSessionStatus(value, false)).toBe('error');
    expect(deriveWorkspaceSessionStatus(value, true)).toBe('running');
    expect(deriveWorkspaceSessionEnvironment(value)).toBe('remote');
    expect(deriveWorkspaceSessionSource(value)).toBe('external');
    expect(deriveWorkspaceSessionWorktree(value)).toBe('worktree');
  });

  it('combines show presets with hidden facet values', () => {
    const unread = session({ hasUnreadCompletion: true });
    const completed = session({ persistedStatus: 'completed', lastFinishedAt: 50 });

    expect(matchesWorkspaceSessionView(unread, 'unread', DEFAULT_WORKSPACE_SESSION_FILTERS, false)).toBe(true);
    expect(matchesWorkspaceSessionView(completed, 'unread', DEFAULT_WORKSPACE_SESSION_FILTERS, false)).toBe(false);
    expect(matchesWorkspaceSessionView(completed, 'all', {
      ...DEFAULT_WORKSPACE_SESSION_FILTERS,
      hiddenStatuses: ['completed'],
    }, false)).toBe(false);
  });
});
