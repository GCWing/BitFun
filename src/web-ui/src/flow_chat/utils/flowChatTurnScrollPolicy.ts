import type { DialogTurn } from '../types/flow-chat';

const TERMINAL_DIALOG_TURN_STATUSES = new Set<DialogTurn['status']>([
  'completed',
  'cancelled',
  'error',
]);

export function isDialogTurnInFlight(turn: DialogTurn | undefined): boolean {
  if (!turn) {
    return false;
  }
  if (!TERMINAL_DIALOG_TURN_STATUSES.has(turn.status)) {
    return true;
  }
  return turn.modelRounds.some(round => round.isStreaming);
}

export function isThreadGoalContinuationTurn(turn: DialogTurn | undefined): boolean {
  return Boolean(turn?.userMessage?.metadata?.threadGoalContinuation);
}

/** Follow-output owns active user turns; auto goal checks stay at the natural tail. */
export function shouldUseLatestTurnFollowOutput(turn: DialogTurn | undefined): boolean {
  if (!turn || isThreadGoalContinuationTurn(turn)) {
    return false;
  }
  return isDialogTurnInFlight(turn);
}

/** Sticky-latest pin starts only after model output exists. */
export function shouldUseStickyLatestPin(turn: DialogTurn | undefined): boolean {
  if (!turn || !shouldUseLatestTurnFollowOutput(turn)) {
    return false;
  }
  return turn.modelRounds.length > 0;
}

export function findDialogTurn(
  dialogTurns: DialogTurn[] | undefined,
  turnId: string | null | undefined,
): DialogTurn | undefined {
  if (!turnId || !dialogTurns) {
    return undefined;
  }
  return dialogTurns.find(turn => turn.id === turnId);
}
