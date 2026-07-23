import { WorkspaceKind, isRemoteWorkspace, type WorkspaceInfo } from '@/shared/types';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';

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

/**
 * Build create_session config from the live workspace. After Peer Device Mode
 * switch, callers must pass this (not `{}`) so the peer host never sees a
 * stale controller path. See `infrastructure/peer-device/README.md`.
 */
export function flowChatSessionConfigForWorkspace(workspace: WorkspaceInfo) {
  return {
    workspacePath: workspace.rootPath,
    ...(isRemoteWorkspace(workspace) && workspace.connectionId
      ? { remoteConnectionId: workspace.connectionId }
      : {}),
    ...(isRemoteWorkspace(workspace) && workspace.sshHost
      ? { remoteSshHost: workspace.sshHost }
      : {}),
  };
}

/**
 * Prefer the live workspaceManager workspace for create_session. Returns `{}`
 * only when no workspace is open yet (caller / SessionModule must still resolve).
 */
export function flowChatSessionConfigForCurrentWorkspace(
  workspace?: WorkspaceInfo | null,
) {
  const live = workspace ?? workspaceManager.getState().currentWorkspace;
  if (!live) {
    return {};
  }
  return flowChatSessionConfigForWorkspace(live);
}
