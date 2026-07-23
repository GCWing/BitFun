/**
 * Workspace ↔ session binding. Never identify a remote workspace by path alone.
 *
 * - Prefer `workspaceId` (backend `WorkspaceInfo.id`) on the session when present.
 * - Otherwise use host + path + connection (see `sessionBelongsToWorkspaceNavRow`).
 */

import type { WorkspaceInfo } from '@/shared/types';
import type { Session } from '../types/flow-chat';
import { sessionBelongsToWorkspaceNavRow } from './sessionOrdering';

type SessionScope = Pick<
  Session,
  'workspaceId' | 'workspacePath' | 'remoteConnectionId' | 'remoteSshHost'
>;

type WorkspaceScope = Pick<WorkspaceInfo, 'id' | 'rootPath' | 'connectionId' | 'sshHost'>;

export function sessionMatchesWorkspace(session: SessionScope, workspace: WorkspaceScope): boolean {
  const sid = session.workspaceId?.trim();
  const wid = workspace.id?.trim();
  if (sid && wid && sid === wid) {
    return true;
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
  const sid = session.workspaceId?.trim();
  if (sid) {
    for (const w of workspaces) {
      if (w.id === sid) return w;
    }
  }
  for (const w of workspaces) {
    if (sessionMatchesWorkspace(session, w)) return w;
  }
  return undefined;
}
