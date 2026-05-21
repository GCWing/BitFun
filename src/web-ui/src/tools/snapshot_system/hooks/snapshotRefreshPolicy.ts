import type { Session } from '@/flow_chat/types/flow-chat';

type SnapshotRefreshSession = Pick<Session, 'isHistorical' | 'historyState' | 'contextRestoreState'>;

export function shouldRefreshSnapshotForSession(
  session?: SnapshotRefreshSession | null
): boolean {
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
