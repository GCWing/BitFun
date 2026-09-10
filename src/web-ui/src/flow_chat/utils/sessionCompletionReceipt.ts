import type { Session } from '../types/flow-chat';
import { lastUserDialogTurn } from './flowChatTurnIdentity';
import { isTurnAwaitingRecovery } from './interruptedTurnRecovery';

/** Bind the visible result to its Turn and recovery generation. */
export function sessionCompletionReceipt(session: Session | undefined): string | null {
  if (!session?.hasUnreadCompletion) return null;
  if (session.historyState && session.historyState !== 'ready') return null;
  const turn = lastUserDialogTurn(session);
  if (session.unreadCompletionTurnId && (session.unreadCompletionTurnId !== turn?.id
    || session.unreadCompletionGeneration !== (turn?.recovery?.executionGeneration ?? turn?.recoveryEpoch))) return null;
  if (!turn || !['completed', 'error', 'cancelled'].includes(turn.status)) return null;
  return JSON.stringify([
    session.hasUnreadCompletion, turn.id, turn.endTime ?? null,
    session.lastFinishedAt ?? null, turn.recovery?.executionGeneration ?? turn.recoveryEpoch ?? null,
    isTurnAwaitingRecovery(turn),
  ]);
}

/** A Turn stopped before producing output can only expose its input boundary. */
export function completionResultItem<T extends { turnId?: string; type: string }>(
  items: readonly T[], turnId: string,
): T | undefined {
  let inputBoundary: T | undefined;
  for (let index = items.length - 1; index >= 0; index -= 1) {
    const item = items[index];
    if (item.turnId !== turnId) continue;
    if (item.type !== 'user-message' && item.type !== 'user-steering-message') return item;
    inputBoundary ??= item;
  }
  return inputBoundary;
}

export interface CompletionViewportRect {
  top: number;
  bottom: number;
  left: number;
  right: number;
}

/** The result's end must actually be in view; rendered overscan is insufficient. */
export function isCompletionResultVisible(
  result: CompletionViewportRect,
  viewport: CompletionViewportRect,
): boolean {
  return viewport.bottom > viewport.top && viewport.right > viewport.left
    && result.bottom > result.top && result.right > result.left
    && result.bottom > viewport.top && result.bottom <= viewport.bottom
    && result.right > viewport.left && result.left < viewport.right;
}
