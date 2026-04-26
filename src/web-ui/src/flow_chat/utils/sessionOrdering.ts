import type { Session } from '../types/flow-chat';
import { isSamePath, normalizeRemoteWorkspacePath } from '@/shared/utils/pathUtils';

/** Extract `host` from our saved form `ssh-{user}@{host}:{port}` (used when metadata omits `remoteSshHost`). */
function hostFromSshConnectionId(connectionId: string): string | null {
  const t = connectionId.trim();
  const m = t.match(/^ssh-[^@]+@(.+):(\d+)$/);
  return m ? m[1].trim().toLowerCase() : null;
}

/** Row-level SSH host: prefer workspace metadata, else parse from `connectionId` (sidebar may lack `sshHost`). */
function effectiveWorkspaceSshHost(
  remoteSshHost?: string | null,
  remoteConnectionId?: string | null
): string {
  const h = remoteSshHost?.trim().toLowerCase() ?? '';
  if (h) return h;
  return hostFromSshConnectionId(remoteConnectionId?.trim() ?? '') ?? '';
}

/**
 * Whether a persisted session belongs to a nav row for this workspace.
 * Remote workspaces are scoped by **SSH host + normalized remote root** (and connection id when present).
 * We must never treat "same host" as sufficient: two tabs to the same server at `/a` vs `/b` are distinct.
 */
export function sessionBelongsToWorkspaceNavRow(
  session: Pick<Session, 'workspacePath' | 'remoteConnectionId' | 'remoteSshHost'>,
  workspacePath: string,
  remoteConnectionId?: string | null,
  remoteSshHost?: string | null
): boolean {
  const sessionRoot = session.workspacePath || workspacePath;
  const pathsMatch =
    isSamePath(sessionRoot, workspacePath) ||
    normalizeRemoteWorkspacePath(sessionRoot) === normalizeRemoteWorkspacePath(workspacePath);

  const wsConn = remoteConnectionId?.trim() ?? '';
  const sessConn = session.remoteConnectionId?.trim() ?? '';
  const wsHostEff = effectiveWorkspaceSshHost(remoteSshHost, remoteConnectionId);
  const sessHost = session.remoteSshHost?.trim().toLowerCase() ?? '';
  const sessConnHost = hostFromSshConnectionId(sessConn);
  const wsConnHost = hostFromSshConnectionId(wsConn);

  if (wsHostEff.length > 0) {
    // Host match alone is insufficient (same server, different remote folders).
    if (sessHost === wsHostEff && pathsMatch) {
      return true;
    }
    if (sessConnHost === wsHostEff && pathsMatch) {
      return true;
    }
    if (sessConnHost && wsConnHost && sessConnHost === wsConnHost) {
      return pathsMatch;
    }
  }

  if (!pathsMatch) return false;

  if (wsConn.length > 0 || sessConn.length > 0) {
    return sessConn === wsConn;
  }
  return true;
}

export function getSessionSortTimestamp(session: Pick<Session, 'createdAt' | 'lastActiveAt' | 'lastFinishedAt'>): number {
  return session.lastActiveAt ?? session.lastFinishedAt ?? session.createdAt;
}

export function compareSessionsForDisplay(
  a: Pick<Session, 'sessionId' | 'createdAt' | 'lastActiveAt' | 'lastFinishedAt'>,
  b: Pick<Session, 'sessionId' | 'createdAt' | 'lastActiveAt' | 'lastFinishedAt'>
): number {
  const timestampDiff = getSessionSortTimestamp(b) - getSessionSortTimestamp(a);
  if (timestampDiff !== 0) {
    return timestampDiff;
  }

  const createdAtDiff = b.createdAt - a.createdAt;
  if (createdAtDiff !== 0) {
    return createdAtDiff;
  }

  return a.sessionId.localeCompare(b.sessionId);
}

/**
 * Left-nav session list order: newest-created first, stable while switching sessions
 * (does not use `lastActiveAt`, so rows do not jump to the top on click).
 */
export function compareSessionsForNavStable(
  a: Pick<Session, 'sessionId' | 'createdAt'>,
  b: Pick<Session, 'sessionId' | 'createdAt'>
): number {
  const createdAtDiff = b.createdAt - a.createdAt;
  if (createdAtDiff !== 0) {
    return createdAtDiff;
  }
  return a.sessionId.localeCompare(b.sessionId);
}
