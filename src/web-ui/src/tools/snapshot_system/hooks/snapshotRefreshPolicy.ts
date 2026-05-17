import type { Session } from '@/flow_chat/types/flow-chat';

type SnapshotRefreshSession = Pick<Session, 'isHistorical' | 'historyState'>;

export function shouldRefreshSnapshotForSession(
  session?: SnapshotRefreshSession | null
): boolean {
  if (!session || !session.isHistorical) {
    return true;
  }

  return session.historyState === undefined ||
    session.historyState === 'new' ||
    session.historyState === 'ready';
}
