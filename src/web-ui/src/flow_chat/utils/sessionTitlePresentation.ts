import type { Session } from '../types/flow-chat';
import { isDefaultSessionTitle, normalizeWorkspaceSessionNumber } from './sessionTitle';
import { sessionProjectWorkspacePath } from './sessionWorkspace';
import { normalizePath, normalizeRemoteWorkspacePath } from '@/shared/utils/pathUtils';
import { normalizeRemoteSessionScope } from '@/shared/utils/remoteSessionScope';

/** The owning project is shared by local and managed-worktree sessions. */
function workspaceKey(session: Session): string {
  const path = sessionProjectWorkspacePath(session);
  if (!path) return JSON.stringify(['workspace', session.workspaceId ?? session.sessionId]);
  const remote = normalizeRemoteSessionScope(
    session.remoteConnectionId || session.config?.remoteConnectionId,
    session.remoteSshHost || session.config?.remoteSshHost,
  );
  if (remote.remoteConnectionId || remote.remoteSshHost) {
    const host = remote.remoteSshHost?.toLowerCase()
      || remote.remoteConnectionId?.match(/^ssh-[^@]+@(.+?)(?::\d+)?$/)?.[1]?.toLowerCase()
      || remote.remoteConnectionId;
    return JSON.stringify(['remote', host, normalizeRemoteWorkspacePath(path)]);
  }
  let normalized = normalizePath(path).replace(/\/$/, '');
  if (/^[a-z]:/i.test(normalized) || path.startsWith('\\\\')) normalized = normalized.toLowerCase();
  return JSON.stringify(['local', normalized]);
}

/** Compute against the full surface catalog, before filtering or limiting rows. */
export function sessionTitleNumbers(sessions: Iterable<Session>): Map<string, number> {
  const groups = new Map<string, Session[]>();
  for (const session of sessions) {
    if (!isDefaultSessionTitle(session)
      || session.persistedStatus === 'archived'
      || (session.sessionKind && session.sessionKind !== 'normal')
      || session.parentSessionId
      || normalizeWorkspaceSessionNumber(session.workspaceSessionNumber) === undefined) continue;
    const key = workspaceKey(session);
    const group = groups.get(key) ?? [];
    group.push(session);
    groups.set(key, group);
  }
  const result = new Map<string, number>();
  for (const group of groups.values()) {
    if (group.length < 2) continue;
    for (const session of group) result.set(session.sessionId, session.workspaceSessionNumber!);
  }
  return result;
}
