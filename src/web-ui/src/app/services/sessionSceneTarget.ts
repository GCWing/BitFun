import type { Session } from '@/flow_chat/types/flow-chat';
import { sessionProjectWorkspacePath } from '@/flow_chat/utils/sessionWorkspace';
import { findWorkspaceForSession } from '@/flow_chat/utils/workspaceScope';
import type { WorkspaceInfo } from '@/shared/types';
import { normalizePath, normalizeRemoteWorkspacePath } from '@/shared/utils/pathUtils';
import { normalizeRemoteSessionScope } from '@/shared/utils/remoteSessionScope';
import type { SessionSceneTarget } from '../components/SceneBar/types';

/** Resolve the owning project, including worktree sessions and legacy metadata. */
export function resolveSessionSceneWorkspace(session: Session, workspaces: Iterable<WorkspaceInfo>) {
  return findWorkspaceForSession({
    ...session,
    workspacePath: sessionProjectWorkspacePath(session),
    remoteConnectionId: session.remoteConnectionId || session.config?.remoteConnectionId,
    remoteSshHost: session.remoteSshHost || session.config?.remoteSshHost,
  }, workspaces);
}

export function resolveSessionSceneTarget(
  session: Session,
  workspaces: Iterable<WorkspaceInfo>,
  surfaceId: string,
): SessionSceneTarget {
  const workspace = resolveSessionSceneWorkspace(session, workspaces);
  const workspaceId = workspace?.id || session.workspaceId;
  const path = sessionProjectWorkspacePath(session) ?? '';
  const remote = normalizeRemoteSessionScope(
    session.remoteConnectionId || session.config?.remoteConnectionId,
    session.remoteSshHost || session.config?.remoteSshHost,
  );
  // Older hosts need no migration: retain a scope derived from their existing
  // metadata until a workspace id is available. Remote roots stay case-sensitive.
  const workspaceKey = workspaceId
    ? JSON.stringify(['workspace', workspaceId])
    : remote.remoteConnectionId || remote.remoteSshHost
      ? JSON.stringify(['remote', remote.remoteSshHost?.toLowerCase(), remote.remoteConnectionId, normalizeRemoteWorkspacePath(path)])
      : JSON.stringify(['local', normalizePath(path).replace(/\/$/, '')]);
  return { surfaceId, workspaceKey, sessionId: session.sessionId };
}
