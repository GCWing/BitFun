import { WorkspaceKind, isRemoteWorkspace, type WorkspaceInfo } from '@/shared/types';

/**
 * Always create a new session instead of reusing an existing empty one.
 */
export function findReusableEmptySessionId(
  _workspace: WorkspaceInfo,
  _requestedMode?: string
): string | null {
  return null;
}

/**
 * Code / Cowork sessions belong to project (non-assistant) workspaces only.
 * Assistant “instances” use Claw sessions under their own storage.
 */
export function pickWorkspaceForProjectChatSession(
  currentWorkspace: WorkspaceInfo | null | undefined,
  normalWorkspacesList: WorkspaceInfo[]
): WorkspaceInfo | null {
  if (currentWorkspace && currentWorkspace.workspaceKind !== WorkspaceKind.Assistant) {
    return currentWorkspace;
  }
  return normalWorkspacesList[0] ?? null;
}

export function flowChatSessionConfigForWorkspace(workspace: WorkspaceInfo) {
  return {
    workspacePath: workspace.rootPath,
    ...(isRemoteWorkspace(workspace) && workspace.connectionId
      ? { remoteConnectionId: workspace.connectionId }
      : {}),
  };
}
