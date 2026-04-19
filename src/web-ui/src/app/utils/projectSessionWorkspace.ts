import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import type { Session } from '@/flow_chat/types/flow-chat';
import { WorkspaceKind, isRemoteWorkspace, type WorkspaceInfo } from '@/shared/types';

type SessionDisplayBucket = 'code' | 'cowork' | 'design' | 'claw';

function normalizeAgentModeForWorkspace(mode: string | undefined, workspace: WorkspaceInfo): string {
  if (workspace.workspaceKind === WorkspaceKind.Assistant) {
    return 'Claw';
  }
  return mode || 'agentic';
}

function sessionDisplayBucket(sessionMode: string | undefined, workspace: WorkspaceInfo): SessionDisplayBucket {
  if (workspace.workspaceKind === WorkspaceKind.Assistant) {
    return 'claw';
  }
  if (!sessionMode) {
    return 'code';
  }
  const normalized = sessionMode.toLowerCase();
  if (normalized === 'cowork') {
    return 'cowork';
  }
  if (normalized === 'design') {
    return 'design';
  }
  if (normalized === 'claw') {
    return 'claw';
  }
  return 'code';
}

function targetDisplayBucket(requestedMode: string | undefined, workspace: WorkspaceInfo): SessionDisplayBucket {
  const agentMode = normalizeAgentModeForWorkspace(requestedMode, workspace);
  return sessionDisplayBucket(agentMode, workspace);
}

function sessionBelongsToWorkspace(session: Session, workspace: WorkspaceInfo): boolean {
  const path = session.workspacePath?.trim();
  const root = workspace.rootPath?.trim();
  if (!path || !root || path !== root) {
    return false;
  }
  if (isRemoteWorkspace(workspace)) {
    const wc = workspace.connectionId?.trim() ?? '';
    const sc = session.remoteConnectionId?.trim() ?? '';
    if (wc.length > 0 || sc.length > 0) {
      return wc === sc;
    }
  }
  return true;
}

function isEmptyReusableSession(session: Session, workspace: WorkspaceInfo, bucket: SessionDisplayBucket): boolean {
  if (session.sessionKind !== 'normal') {
    return false;
  }
  if (session.isHistorical) {
    return false;
  }
  if (session.dialogTurns.length > 0) {
    return false;
  }
  if (!sessionBelongsToWorkspace(session, workspace)) {
    return false;
  }
  return sessionDisplayBucket(session.mode, workspace) === bucket;
}

/**
 * If the workspace already has a main session with no dialog turns for the same UI mode
 * (Code / Cowork / Design / Claw), return its id so callers can switch instead of creating another.
 */
export function findReusableEmptySessionId(
  workspace: WorkspaceInfo,
  requestedMode?: string
): string | null {
  const bucket = targetDisplayBucket(requestedMode, workspace);
  const sessions = flowChatStore.getState().sessions;
  let best: { id: string; lastActiveAt: number } | null = null;
  for (const session of sessions.values()) {
    if (!isEmptyReusableSession(session, workspace, bucket)) {
      continue;
    }
    if (!best || session.lastActiveAt > best.lastActiveAt) {
      best = { id: session.sessionId, lastActiveAt: session.lastActiveAt };
    }
  }
  return best?.id ?? null;
}

/**
 * Code / Cowork / Design sessions belong to project (non-assistant) workspaces only.
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
