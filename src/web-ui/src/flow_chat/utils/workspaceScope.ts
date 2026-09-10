/**
 * Workspace ↔ session binding. Never identify a remote workspace by path alone.
 *
 * - Prefer `workspaceId` (backend `WorkspaceInfo.id`) on the session when present.
 * - Otherwise use host + path + connection (see `sessionBelongsToWorkspaceNavRow`).
 */

import type { WorkspaceInfo } from '@/shared/types';
import type { Session } from '../types/flow-chat';
import { sessionBelongsToWorkspaceNavRow } from './sessionOrdering';
import { isRemoteSessionScope } from '@/shared/utils/remoteSessionScope';
import { normalizeRemoteWorkspacePath } from '@/shared/utils/pathUtils';

type SessionScope = Pick<
  Session,
  'workspaceId' | 'workspacePath' | 'projectWorkspacePath' | 'remoteConnectionId' | 'remoteSshHost'
>;

type WorkspaceScope = Pick<WorkspaceInfo, 'id' | 'rootPath' | 'connectionId' | 'sshHost'>;

export function sessionMatchesWorkspace(session: SessionScope, workspace: WorkspaceScope): boolean {
  const sid = session.workspaceId?.trim();
  const wid = workspace.id?.trim();
  if (sid && wid && sid === wid) {
    return true;
  }
  const remote = isRemoteSessionScope(session.remoteConnectionId, session.remoteSshHost);
  if (remote !== isRemoteSessionScope(workspace.connectionId, workspace.sshHost)) return false;
  if (remote && ![session.workspacePath, session.projectWorkspacePath].some(path => path
    && normalizeRemoteWorkspacePath(path) === normalizeRemoteWorkspacePath(workspace.rootPath))) {
    return false;
  }
  // Stale or missing id on the session: still match by path + remote scope.
  return sessionBelongsToWorkspaceNavRow(
    session,
    workspace.rootPath,
    workspace.connectionId ?? null,
    workspace.sshHost ?? null
  );
}

export function findWorkspaceForSession(
  session: SessionScope,
  workspaces: Iterable<WorkspaceInfo>
): WorkspaceInfo | undefined {
  // Callers also pass Map iterators; preserve candidates for the legacy-id fallback.
  const candidates = [...workspaces];
  const sid = session.workspaceId?.trim();
  if (sid) {
    for (const w of candidates) {
      if (w.id === sid) return w;
    }
  }
  for (const w of candidates) {
    if (sessionMatchesWorkspace(session, w)) return w;
  }
  return undefined;
}
