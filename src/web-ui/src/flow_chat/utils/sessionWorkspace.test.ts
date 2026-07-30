import { describe, expect, it } from 'vitest';
import { WorkspaceKind, type WorkspaceInfo } from '@/shared/types';
import type { Session } from '../types/flow-chat';
import {
  isRemoteWorkspaceSession,
  requireSessionProjectWorkspacePath,
  sessionExecutionWorkspacePath,
  sessionProjectWorkspacePath,
} from './sessionWorkspace';

function session(
  values: Partial<Pick<Session, 'workspacePath' | 'projectWorkspacePath' | 'config'>>,
): Pick<Session, 'workspacePath' | 'projectWorkspacePath' | 'config'> {
  return {
    workspacePath: undefined,
    projectWorkspacePath: undefined,
    config: {},
    ...values,
  };
}

describe('sessionWorkspace', () => {
  it('identifies remote workspace sessions from either session or workspace metadata', () => {
    const localWorkspace = {
      workspaceKind: WorkspaceKind.Normal,
    } as WorkspaceInfo;
    const remoteWorkspace = {
      workspaceKind: WorkspaceKind.Remote,
    } as WorkspaceInfo;

    expect(isRemoteWorkspaceSession(undefined, localWorkspace)).toBe(false);
    expect(
      isRemoteWorkspaceSession({ remoteConnectionId: 'remote-1' }, localWorkspace),
    ).toBe(true);
    expect(
      isRemoteWorkspaceSession(
        { config: { remoteConnectionId: 'remote-2' } },
        localWorkspace,
      ),
    ).toBe(true);
    expect(isRemoteWorkspaceSession(undefined, remoteWorkspace)).toBe(true);
  });

  it('keeps execution and project roots distinct for a worktree session', () => {
    const worktreeSession = session({
      workspacePath: '/worktrees/wt-1',
      projectWorkspacePath: '/repo',
      config: {
        workspacePath: '/worktrees/wt-1',
        projectWorkspacePath: '/repo',
      },
    });

    expect(sessionExecutionWorkspacePath(worktreeSession)).toBe('/worktrees/wt-1');
    expect(sessionProjectWorkspacePath(worktreeSession)).toBe('/repo');
    expect(requireSessionProjectWorkspacePath(worktreeSession, 'session-1')).toBe('/repo');
  });

  it('treats legacy sessions as local to their execution root', () => {
    const legacySession = session({
      config: { workspacePath: '/repo' },
    });

    expect(sessionExecutionWorkspacePath(legacySession)).toBe('/repo');
    expect(sessionProjectWorkspacePath(legacySession)).toBe('/repo');
  });
});
