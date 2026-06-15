import type { VirtualItem } from '../../store/modernFlowChatStore';

export interface HistoryProjectionHandoffSnapshot {
  sessionId: string;
  reason: string;
  createdAtMs: number;
  items: VirtualItem[];
  mode: 'scroll-position' | 'bottom-tail';
  targetTurnId: string | null;
  anchorKey: string | null;
  anchorOffsetTopPx: number;
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
  footerHeightPx: number;
}

export function activeSessionHistoryProjectionHandoff(
  snapshot: HistoryProjectionHandoffSnapshot | null,
  activeSessionId: string | null
): HistoryProjectionHandoffSnapshot | null {
  return snapshot?.sessionId === activeSessionId ? snapshot : null;
}
