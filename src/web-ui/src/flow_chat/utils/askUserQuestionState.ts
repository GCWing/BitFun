import { stateMachineManager } from '../state-machine/SessionStateMachineManager';
import type { DialogTurn, FlowToolItem, Session } from '../types/flow-chat';
import { effectiveToolInvocation } from './toolInvocationIdentity';

export const TRANSIENT_TURN_STATUSES = new Set<DialogTurn['status']>([
  'pending',
  'image_analyzing',
  'processing',
  'finishing',
  'cancelling',
]);

const TERMINAL_TOOL_STATUSES = new Set<FlowToolItem['status']>([
  'completed',
  'error',
  'cancelled',
  'rejected',
]);

/**
 * Scan a dialog turn's model rounds (newest-first) for a non-terminal
 * AskUserQuestion tool item whose parameters have finished streaming and
 * whose questions array is non-empty.
 *
 * Works for both active and non-active sessions because it inspects the
 * turn items directly rather than relying on the needsUserAttention flag
 * (which is only set for non-active sessions).
 */
export function findPendingAskUserQuestion(
  turn: DialogTurn | undefined,
): FlowToolItem | undefined {
  if (!turn || !TRANSIENT_TURN_STATUSES.has(turn.status)) {
    return undefined;
  }

  for (let roundIndex = turn.modelRounds.length - 1; roundIndex >= 0; roundIndex -= 1) {
    const round = turn.modelRounds[roundIndex];
    for (let itemIndex = round.items.length - 1; itemIndex >= 0; itemIndex -= 1) {
      const item = round.items[itemIndex];
      if (
        item.type !== 'tool'
        || TERMINAL_TOOL_STATUSES.has(item.status)
        || item.isParamsStreaming
      ) {
        continue;
      }

      const effective = effectiveToolInvocation(item.toolName, item.toolCall?.input);
      if (effective.toolName !== 'AskUserQuestion') {
        continue;
      }

      const questions = effective.input && typeof effective.input === 'object'
        ? (effective.input as Record<string, unknown>).questions
        : undefined;
      if (Array.isArray(questions) && questions.length > 0) {
        return item;
      }
    }
  }

  return undefined;
}

/**
 * Boolean wrapper around findPendingAskUserQuestion for use in selectors
 * and render-time checks where only the presence (not the item itself) is
 * needed.
 */
export function hasPendingAskUserQuestion(
  turn: DialogTurn | undefined,
): boolean {
  return !!findPendingAskUserQuestion(turn);
}

/**
 * Resolve the dialog turn that the state machine is currently tracking for a
 * session, falling back to the last turn when the machine has no
 * currentDialogTurnId (e.g. session never started or was reset).
 *
 * This is necessary because the composer may append a newer turn (e.g. a
 * queued user message) while the state machine is still executing an older
 * turn that has a pending AskUserQuestion. Checking the last turn would miss
 * the pending question; the tracked turn is the correct one to inspect.
 */
export function resolveTrackedTurn(
  session: Session,
): DialogTurn | undefined {
  const trackedTurnId = stateMachineManager
    .get(session.sessionId)
    ?.getContext()?.currentDialogTurnId;
  if (trackedTurnId) {
    const tracked = session.dialogTurns.find(turn => turn.id === trackedTurnId);
    if (tracked) {
      return tracked;
    }
  }
  return session.dialogTurns[session.dialogTurns.length - 1];
}
