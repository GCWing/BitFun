import type { FlowToolItem, Session } from '../types/flow-chat';

export type ReviewTaskOutcome =
  | 'partial-timeout'
  | 'timed-out'
  | 'stopped'
  | 'failed';

function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

export function deriveReviewTaskOutcome(toolItem: FlowToolItem): ReviewTaskOutcome | null {
  const result = toolItem.toolResult?.result;
  const resultStatus = result && typeof result === 'object'
    ? readString((result as Record<string, unknown>).status).toLowerCase()
    : '';

  if (resultStatus === 'partial_timeout') {
    return 'partial-timeout';
  }
  if (resultStatus === 'timed_out' || resultStatus === 'timeout') {
    return 'timed-out';
  }
  if (
    toolItem.status === 'cancelled' ||
    toolItem.status === 'rejected' ||
    resultStatus === 'cancelled' ||
    resultStatus === 'canceled' ||
    resultStatus === 'cancelled_by_user'
  ) {
    return 'stopped';
  }

  const error = readString(toolItem.toolResult?.error);
  if (/\b(?:timeout|timed out)\b/i.test(error)) {
    return 'timed-out';
  }
  if (toolItem.status === 'error' || toolItem.toolResult?.success === false) {
    return 'failed';
  }
  return null;
}

export function findReviewTaskOutcome(
  parentSession: Session | undefined,
  parentToolCallId: string | undefined,
): ReviewTaskOutcome | null {
  if (!parentSession || !parentToolCallId) {
    return null;
  }

  for (let turnIndex = parentSession.dialogTurns.length - 1; turnIndex >= 0; turnIndex -= 1) {
    const turn = parentSession.dialogTurns[turnIndex];
    for (let roundIndex = turn.modelRounds.length - 1; roundIndex >= 0; roundIndex -= 1) {
      const round = turn.modelRounds[roundIndex];
      const attemptItems = round.attempts?.flatMap((attempt) => attempt.items) ?? [];
      const items = [...round.items, ...attemptItems];
      for (let itemIndex = items.length - 1; itemIndex >= 0; itemIndex -= 1) {
        const item = items[itemIndex];
        if (
          item.type === 'tool' &&
          (item.toolCall?.id === parentToolCallId || item.id === parentToolCallId)
        ) {
          return deriveReviewTaskOutcome(item);
        }
      }
    }
  }

  return null;
}
