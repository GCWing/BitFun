import { describe, expect, it } from 'vitest';
import {
  WorkspaceKind,
  WorkspaceType,
  type WorkspaceInfo,
} from '@/shared/types';
import {
  isWorkspaceBackedSessionGroupActive,
  projectWorkspaceBackedSessionGroups,
} from './sessionNavigationProjection';

const createWorkspace = (
  id: string,
  workspaceKind: WorkspaceKind,
  overrides: Partial<WorkspaceInfo> = {},
): WorkspaceInfo => ({
  id,
  name: id,
  rootPath: `/owners/${id}`,
  workspaceType: WorkspaceType.SingleProject,
  workspaceKind,
  languages: [],
  openedAt: '2026-08-16T00:00:00.000Z',
  lastAccessed: '2026-08-16T00:00:00.000Z',
  tags: [],
  ...overrides,
});

describe('projectWorkspaceBackedSessionGroups', () => {
  const project = createWorkspace('local-project', WorkspaceKind.Normal);
  const assistant = createWorkspace('personal-assistant', WorkspaceKind.Assistant);
  const remoteProject = createWorkspace('remote-project', WorkspaceKind.Remote, {
    connectionId: 'ssh-1',
    sshHost: 'example-host',
  });

  it('projects projects and assistants as peer groups in canonical open order', () => {
    const groups = projectWorkspaceBackedSessionGroups(
      [project, assistant, remoteProject],
      'all',
    );

    expect(groups.map(group => ({
      groupId: group.groupId,
      kind: group.kind,
      ownerId: group.workspace.id,
    }))).toEqual([
      { groupId: 'workspace:local-project', kind: 'project', ownerId: 'local-project' },
      { groupId: 'workspace:personal-assistant', kind: 'assistant', ownerId: 'personal-assistant' },
      { groupId: 'workspace:remote-project', kind: 'project', ownerId: 'remote-project' },
    ]);
  });

  it('treats runtime location as project metadata rather than a separate group kind', () => {
    const groups = projectWorkspaceBackedSessionGroups(
      [project, assistant, remoteProject],
      'projects',
    );

    expect(groups.map(group => [group.workspace.id, group.kind])).toEqual([
      ['local-project', 'project'],
      ['remote-project', 'project'],
    ]);
    expect(projectWorkspaceBackedSessionGroups(
      [project, assistant, remoteProject],
      'assistants',
    ).map(group => group.workspace.id)).toEqual(['personal-assistant']);
  });

  it('hides a linked worktree owner only while its canonical project is also open', () => {
    const canonicalProject = createWorkspace('canonical-project', WorkspaceKind.Normal, {
      rootPath: '/repo',
    });
    const linkedWorktree = createWorkspace('linked-worktree', WorkspaceKind.Normal, {
      rootPath: '/repo/.worktrees/feature',
      worktree: {
        path: '/repo/.worktrees/feature',
        mainRepoPath: '/repo',
        branch: 'feature',
        isMain: false,
      },
    });

    expect(projectWorkspaceBackedSessionGroups(
      [canonicalProject, linkedWorktree],
      'all',
    ).map(group => group.workspace.id)).toEqual(['canonical-project']);
    expect(projectWorkspaceBackedSessionGroups(
      [linkedWorktree],
      'all',
    ).map(group => group.workspace.id)).toEqual(['linked-worktree']);
  });
});

describe('isWorkspaceBackedSessionGroupActive', () => {
  it('distinguishes identical paths opened through different remote connections', () => {
    const firstRemote = createWorkspace('remote-first', WorkspaceKind.Remote, {
      rootPath: '/workspace',
      connectionId: 'ssh-first',
      sshHost: 'host-a.example',
    });
    const secondRemote = createWorkspace('remote-second', WorkspaceKind.Remote, {
      rootPath: '/workspace',
      connectionId: 'ssh-second',
      sshHost: 'host-b.example',
    });

    expect(isWorkspaceBackedSessionGroupActive(firstRemote, firstRemote)).toBe(true);
    expect(isWorkspaceBackedSessionGroupActive(secondRemote, firstRemote)).toBe(false);
  });

  it('keeps the canonical local project active for its selected worktree', () => {
    const canonicalProject = createWorkspace('canonical-project', WorkspaceKind.Normal, {
      rootPath: '/repo',
    });
    const linkedWorktree = createWorkspace('linked-worktree', WorkspaceKind.Normal, {
      rootPath: '/repo/.worktrees/feature',
      worktree: {
        path: '/repo/.worktrees/feature',
        mainRepoPath: '/repo',
        branch: 'feature',
        isMain: false,
      },
    });

    expect(isWorkspaceBackedSessionGroupActive(canonicalProject, linkedWorktree)).toBe(true);
  });
});
