import { isRemoteWorkspace, type WorkspaceInfo } from '@/shared/types';

export interface MiniAppWorkspaceInfo {
  available: boolean;
  name: string;
  path: string;
  kind: string | null;
  isRemote: boolean;
}

export function buildMiniAppWorkspaceInfo(
  workspace: WorkspaceInfo | null | undefined,
  workspaceName: string,
  workspacePath: string,
): MiniAppWorkspaceInfo {
  return {
    available: Boolean(workspace),
    name: workspace ? workspaceName : '',
    path: workspace ? workspacePath : '',
    kind: workspace?.workspaceKind ?? null,
    isRemote: isRemoteWorkspace(workspace),
  };
}
