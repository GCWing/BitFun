import { describe, expect, it } from 'vitest';
import {
  WorkspaceKind,
  WorkspaceType,
  type WorkspaceInfo,
} from '@/shared/types/global-state';
import { getWorkspaceGitBasicInfoOptions } from './workspaceGitRefreshOptions';

const createWorkspace = (workspaceKind: WorkspaceKind): WorkspaceInfo => ({
  id: `${workspaceKind}-workspace`,
  name: 'BitFun',
  rootPath: '/workspace/BitFun',
  workspaceType: WorkspaceType.SingleProject,
  workspaceKind,
  languages: [],
  openedAt: '2026-06-02T00:00:00Z',
  lastAccessed: '2026-06-02T00:00:00Z',
  tags: [],
  ...(workspaceKind === WorkspaceKind.Remote
    ? {
        connectionId: 'remote-connection',
        sshHost: 'remote.example.com',
      }
    : {}),
});

describe('getWorkspaceGitBasicInfoOptions', () => {
  it('refreshes active local workspace rows on mount', () => {
    expect(getWorkspaceGitBasicInfoOptions(createWorkspace(WorkspaceKind.Normal), true))
      .toEqual({
        isActive: true,
        refreshOnMount: true,
        refreshOnActive: true,
        participateInWindowFocusRefresh: false,
      });
  });

  it('defers inactive local workspace row refresh until activation', () => {
    expect(getWorkspaceGitBasicInfoOptions(createWorkspace(WorkspaceKind.Normal), false))
      .toEqual({
        isActive: false,
        refreshOnMount: false,
        refreshOnActive: true,
        participateInWindowFocusRefresh: false,
      });
  });

  it('keeps remote workspace rows on the existing default git refresh behavior', () => {
    expect(getWorkspaceGitBasicInfoOptions(createWorkspace(WorkspaceKind.Remote), false))
      .toBeUndefined();
  });
});
