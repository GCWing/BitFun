import type { AnyFlowItem, DialogTurn, Session } from '../types/flow-chat';
import type { VirtualItem } from '../store/modernFlowChatStore';
import type { ReviewTaskOutcome } from './reviewTaskOutcome';

export type ReviewDetailContentState =
  | 'loading'
  | 'load-failed'
  | 'unavailable';

export type ReviewDetailExecutionState =
  | 'preparing'
  | 'completed-empty'
  | 'partial-timeout'
  | 'stopped'
  | 'interrupted'
  | 'timed-out'
  | 'failed';

export interface ReviewDetailProjection {
  content: ReviewDetailContentState | null;
  execution: ReviewDetailExecutionState | null;
}

const ACTIVE_TURN_STATUSES = new Set<DialogTurn['status']>([
  'pending',
  'image_analyzing',
  'processing',
  'finishing',
  'cancelling',
]);

function hasApplicationRestartInterruption(turn: DialogTurn | undefined): boolean {
  if (!turn) {
    return false;
  }

  return turn.modelRounds.some((round) => {
    const attemptItems = round.attempts?.flatMap((attempt) => attempt.items) ?? [];
    return [...round.items, ...attemptItems].some((item: AnyFlowItem) => (
      item.type === 'tool' && item.interruptionReason === 'app_restart'
    ));
  });
}

function isTimeoutFailure(session: Session, turn: DialogTurn | undefined): boolean {
  if (turn?.errorDetail?.category === 'timeout') {
    return true;
  }

  const message = `${turn?.error ?? ''}\n${session.error ?? ''}`;
  return /\b(?:timeout|timed out)\b/i.test(message);
}

export function deriveReviewDetailProjection(
  session: Session | undefined,
  hasVisibleItems: boolean,
  taskOutcome: ReviewTaskOutcome | null = null,
): ReviewDetailProjection {
  if (!session) {
    return { content: null, execution: 'preparing' };
  }

  const content: ReviewDetailContentState | null =
    session.historyState === 'metadata-only' || session.historyState === 'hydrating'
      ? 'loading'
      : session.historyState === 'failed'
        ? 'load-failed'
        : session.historyState === 'ready' && session.dialogTurns.length === 0
          ? 'unavailable'
          : null;

  const lastTurn = session.dialogTurns[session.dialogTurns.length - 1];
  if (lastTurn && ACTIVE_TURN_STATUSES.has(lastTurn.status)) {
    return { content, execution: hasVisibleItems ? null : 'preparing' };
  }

  if (taskOutcome) {
    return { content, execution: taskOutcome };
  }

  if (!lastTurn) {
    return { content, execution: content ? null : 'preparing' };
  }

  if (
    session.hasUnreadCompletion === 'interrupted' ||
    lastTurn?.finishReason === 'interrupted' ||
    hasApplicationRestartInterruption(lastTurn)
  ) {
    return { content, execution: 'interrupted' };
  }

  if (isTimeoutFailure(session, lastTurn)) {
    return { content, execution: 'timed-out' };
  }
  if (lastTurn?.status === 'error' || session.error) {
    return { content, execution: 'failed' };
  }
  if (lastTurn?.status === 'cancelled') {
    return { content, execution: 'stopped' };
  }
  if (hasVisibleItems) {
    return { content, execution: null };
  }
  if (lastTurn?.status === 'completed') {
    return { content, execution: 'completed-empty' };
  }

  return { content, execution: 'preparing' };
}

const INTERNAL_REVIEW_DETAIL_ITEM_TYPES = new Set<VirtualItem['type']>([
  'user-message',
  'user-steering-message',
  'turn-failure-notice',
  'turn-completion-notice',
]);

export function filterReviewDetailItems(items: VirtualItem[]): VirtualItem[] {
  return items.filter((item) => !INTERNAL_REVIEW_DETAIL_ITEM_TYPES.has(item.type));
}
