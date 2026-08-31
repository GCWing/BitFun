import type { Session } from '@/flow_chat/types/flow-chat';
import { isRemoteSessionScope } from '@/shared/utils/remoteSessionScope';

type SnapshotRefreshSession = Pick<Session, 'isHistorical' | 'historyState' | 'contextRestoreState'> &
  Partial<Pick<Session, 'remoteConnectionId' | 'remoteSshHost'>> & {
    config?: Pick<Session['config'], 'remoteConnectionId' | 'remoteSshHost'>;
  };

// SSH file operations do not record file snapshots. Use the session's durable
// binding, including legacy config fields, so disconnecting or switching the
// active workspace cannot accidentally enable controller-local snapshot IO.
export function hasSessionFileSnapshots(session?: SnapshotRefreshSession | null): boolean {
  return !isRemoteSessionScope(
    session?.remoteConnectionId || session?.config?.remoteConnectionId,
    session?.remoteSshHost || session?.config?.remoteSshHost,
  );
}

export function shouldRefreshSnapshotForSession(
  session?: SnapshotRefreshSession | null
): boolean {
  if (!hasSessionFileSnapshots(session)) return false;

  if (!session || !session.isHistorical) {
    return session?.contextRestoreState !== 'pending';
  }

  if (session.contextRestoreState === 'pending') {
    return false;
  }

  return session.historyState === undefined ||
    session.historyState === 'new' ||
    session.historyState === 'ready';
}
