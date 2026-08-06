import type { Session } from '../types/flow-chat';
import {
  createTurnOrdinalResolver,
  resolveTurnOrdinal,
  validSessionTurnCatalog,
} from './flowChatTurnIdentity';

export function absoluteSessionTurnIndexForLocalIndex(
  session: Session,
  localIndex: number,
): number {
  const ordinal = createTurnOrdinalResolver(session)(localIndex);
  return ordinal === undefined ? localIndex + 1 : ordinal + 1;
}

export function createAbsoluteSessionTurnIndexResolver(
  session: Session,
): (localIndex: number) => number {
  const resolveOrdinal = createTurnOrdinalResolver(session);
  return localIndex => {
    const ordinal = resolveOrdinal(localIndex);
    return ordinal === undefined ? localIndex + 1 : ordinal + 1;
  };
}

export function absoluteSessionTurnIndexForId(
  session: Session,
  turnId: string,
): number | undefined {
  const ordinal = resolveTurnOrdinal(session, turnId);
  if (ordinal !== undefined) return ordinal + 1;
  const localIndex = session.dialogTurns.findIndex(turn => turn.id === turnId);
  return localIndex < 0 ? undefined : absoluteSessionTurnIndexForLocalIndex(session, localIndex);
}

export function loadedSessionTurnIdForAbsoluteIndex(
  session: Session,
  turnIndex: number,
): string | undefined {
  const ordinal = turnIndex - 1;
  const catalogTurnId = validSessionTurnCatalog(session)?.entries.find(
    entry => entry.ordinal === ordinal,
  )?.turnId;
  if (catalogTurnId) {
    return catalogTurnId;
  }

  const resolveOrdinal = createTurnOrdinalResolver(session);
  return session.dialogTurns.find((_, localIndex) => resolveOrdinal(localIndex) === ordinal)?.id;
}
