import type { Session } from '../types/flow-chat';
import { normalizeRemoteWorkspacePath } from '@/shared/utils/pathUtils';

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
 * Remote mirror lists sessions by host+path on disk; metadata `workspacePath` / `remoteSshHost` can be stale,
 * so we must match by SSH host (from metadata or embedded in connection id) before rejecting on path alone.
 */
export function sessionBelongsToWorkspaceNavRow(
  session: Pick<Session, 'workspacePath' | 'remoteConnectionId' | 'remoteSshHost'>,
  workspacePath: string,
  remoteConnectionId?: string | null,
  remoteSshHost?: string | null
): boolean {
  const wp = normalizeRemoteWorkspacePath(workspacePath);
  const sp = normalizeRemoteWorkspacePath(session.workspacePath || workspacePath);

  const wsConn = remoteConnectionId?.trim() ?? '';
  const sessConn = session.remoteConnectionId?.trim() ?? '';
  const wsHostEff = effectiveWorkspaceSshHost(remoteSshHost, remoteConnectionId);
  const sessHost = session.remoteSshHost?.trim().toLowerCase() ?? '';
  const sessConnHost = hostFromSshConnectionId(sessConn);
  const wsConnHost = hostFromSshConnectionId(wsConn);

  if (wsHostEff.length > 0) {
    if (sessHost === wsHostEff) {
      return true;
    }
    if (sessConnHost === wsHostEff) {
      return true;
    }
    if (sessConnHost && wsConnHost && sessConnHost === wsConnHost) {
      return sp === wp;
    }
  }

  if (sp !== wp) return false;

  if (wsConn.length > 0 || sessConn.length > 0) {
    return sessConn === wsConn;
  }
  return true;
}

export function getSessionSortTimestamp(session: Pick<Session, 'createdAt' | 'lastFinishedAt'>): number {
  return session.lastFinishedAt ?? session.createdAt;
}

export function compareSessionsForDisplay(
  a: Pick<Session, 'sessionId' | 'createdAt' | 'lastFinishedAt'>,
  b: Pick<Session, 'sessionId' | 'createdAt' | 'lastFinishedAt'>
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
