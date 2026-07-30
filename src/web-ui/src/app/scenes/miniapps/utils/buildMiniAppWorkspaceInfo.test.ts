import { describe, expect, it } from 'vitest';

import { WorkspaceKind, WorkspaceType, type WorkspaceInfo } from '@/shared/types';
import { buildMiniAppWorkspaceInfo } from './buildMiniAppWorkspaceInfo';

function workspace(kind: WorkspaceKind): WorkspaceInfo {
  return {
    id: `workspace-${kind}`,
    name: `${kind} workspace`,
    rootPath: `C:\\projects\\${kind}`,
    workspaceType: WorkspaceType.SingleProject,
    workspaceKind: kind,
    languages: [],
    openedAt: '2026-07-27T00:00:00Z',
    lastAccessed: '2026-07-27T00:00:00Z',
    tags: [],
  };
}

describe('buildMiniAppWorkspaceInfo', () => {
  it('returns an unavailable contract when no workspace is open', () => {
    expect(buildMiniAppWorkspaceInfo(null, 'stale name', 'C:\\stale')).toEqual({
      available: false,
      name: '',
      path: '',
      kind: null,
      isRemote: false,
    });
  });

  it('returns the current local workspace context', () => {
    expect(buildMiniAppWorkspaceInfo(workspace(WorkspaceKind.Normal), 'BitFun', 'C:\\codeagent\\BitFun'))
      .toEqual({
        available: true,
        name: 'BitFun',
        path: 'C:\\codeagent\\BitFun',
        kind: WorkspaceKind.Normal,
        isRemote: false,
      });
  });

  it('marks a remote workspace without changing its displayed path', () => {
    expect(buildMiniAppWorkspaceInfo(workspace(WorkspaceKind.Remote), 'Remote app', '/srv/app'))
      .toEqual({
        available: true,
        name: 'Remote app',
        path: '/srv/app',
        kind: WorkspaceKind.Remote,
        isRemote: true,
      });
  });
});
