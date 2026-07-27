import { describe, expect, it } from 'vitest';
import type { Session } from '../types/flow-chat';
import {
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
