import type {
  DialogTurn,
  Session,
  SessionHistoryPresentation,
} from '../../types/flow-chat';

export const CONTINUOUS_HISTORY_PROJECTION_MAX_TURN_COUNT = 24;
export const CONTINUOUS_HISTORY_PROJECTION_MAX_VIRTUAL_ITEM_COUNT = 200;

type ContinuousHistorySession = Pick<
  Session,
  'dialogTurns' | 'totalTurnCount' | 'turnCatalog'
>;

function sessionTotalTurnCount(session: ContinuousHistorySession): number {
  return Math.max(
    session.totalTurnCount ?? 0,
    session.turnCatalog?.totalTurnCount ?? 0,
    session.dialogTurns.length,
  );
}

export function buildContinuousHistoryProjection(
  session: ContinuousHistorySession,
  presentation: SessionHistoryPresentation | null,
): SessionHistoryPresentation | null {
  if (!presentation || presentation.range.startOrdinal !== 0) {
    return null;
  }

  const totalTurnCount = sessionTotalTurnCount(session);
  const presentedTurnCount = presentation.range.endOrdinalExclusive;
  if (
    totalTurnCount <= 0
    || presentedTurnCount > totalTurnCount
    || presentation.turns.length !== presentedTurnCount
  ) {
    return null;
  }

  const canonicalTailStartOrdinal = totalTurnCount - session.dialogTurns.length;
  if (canonicalTailStartOrdinal > presentedTurnCount) {
    return null;
  }

  const turns: Array<DialogTurn | undefined> = Array.from(
    { length: totalTurnCount },
    (_, ordinal) => presentation.turns[ordinal],
  );
  for (let index = 0; index < session.dialogTurns.length; index += 1) {
    const ordinal = canonicalTailStartOrdinal + index;
    const canonicalTurn = session.dialogTurns[index];
    const cachedTurn = turns[ordinal];
    if (cachedTurn && cachedTurn.id !== canonicalTurn.id) {
      return null;
    }
    turns[ordinal] = canonicalTurn;
  }

  if (turns.some(turn => !turn)) {
    return null;
  }
  const continuousTurns = turns as DialogTurn[];
  if (new Set(continuousTurns.map(turn => turn.id)).size !== continuousTurns.length) {
    return null;
  }

  const range = {
    ...presentation.range,
    endOrdinalExclusive: totalTurnCount,
    targetTurnId: presentation.range.targetTurnId
      && continuousTurns.some(turn => turn.id === presentation.range.targetTurnId)
      ? presentation.range.targetTurnId
      : null,
    mode: 'history-window' as const,
  };
  const unchanged = (
    range.endOrdinalExclusive === presentation.range.endOrdinalExclusive
    && continuousTurns.every((turn, index) => turn === presentation.turns[index])
  );
  return unchanged
    ? presentation
    : { range, turns: continuousTurns };
}

export function canRetainContinuousHistoryProjection(
  presentation: SessionHistoryPresentation | null,
  virtualItemCount: number,
): boolean {
  return Boolean(
    presentation
    && presentation.range.startOrdinal === 0
    && presentation.turns.length === presentation.range.endOrdinalExclusive
    && presentation.turns.length <= CONTINUOUS_HISTORY_PROJECTION_MAX_TURN_COUNT
    && virtualItemCount <= CONTINUOUS_HISTORY_PROJECTION_MAX_VIRTUAL_ITEM_COUNT
  );
}
