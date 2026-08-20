import type { ActiveTurnRenderRange } from '../../types/flow-chat';

export interface FlowChatViewportSnapshot {
  sessionId: string;
  presentationMode: 'tail' | 'history-window';
  viewportMode: 'live-tail' | 'history-reading';
  historyWindow: ActiveTurnRenderRange | null;
  anchorTurnId: string | null;
  anchorOffsetPx: number | null;
  scrollTopPx: number;
  isAtTail: boolean;
  capturedAtMs: number;
}
