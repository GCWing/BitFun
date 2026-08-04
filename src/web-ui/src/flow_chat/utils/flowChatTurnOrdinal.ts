import type { Session } from '../types/flow-chat';

function sessionTurnCatalog(session: Session) {
  return session.turnCatalog?.sessionId === session.sessionId
    ? session.turnCatalog
    : undefined;
}

export function absoluteSessionTurnIndexForLocalIndex(
  session: Session,
  localIndex: number,
): number {
  const turn = session.dialogTurns[localIndex];
  if (!turn) {
    return localIndex + 1;
  }

  const catalog = sessionTurnCatalog(session);
  const catalogEntry = catalog?.entries.find(entry =>
    entry.turnId === turn.id
    || (
      typeof turn.backendTurnIndex === 'number'
      && entry.storageTurnIndex === turn.backendTurnIndex
    )
  );
  if (catalogEntry) {
    return catalogEntry.ordinal + 1;
  }
  if (typeof turn.backendTurnIndex === 'number') {
    return turn.backendTurnIndex + 1;
  }
  if (session.isPartial === true) {
    const totalTurnCount = Math.max(
      session.totalTurnCount ?? 0,
      catalog?.totalTurnCount ?? 0,
      session.dialogTurns.length,
    );
    return Math.max(0, totalTurnCount - session.dialogTurns.length) + localIndex + 1;
  }
  return localIndex + 1;
}

export function createAbsoluteSessionTurnIndexResolver(
  session: Session,
): (localIndex: number) => number {
  const catalog = sessionTurnCatalog(session);
  const catalogOrdinalByTurnId = new Map<string, number>();
  const catalogOrdinalByStorageIndex = new Map<number, number>();
  for (const entry of catalog?.entries ?? []) {
    if (entry.turnId) {
      catalogOrdinalByTurnId.set(entry.turnId, entry.ordinal);
    }
    catalogOrdinalByStorageIndex.set(entry.storageTurnIndex, entry.ordinal);
  }
  const partialTurnOffset = session.isPartial === true
    ? Math.max(
        0,
        Math.max(
          session.totalTurnCount ?? 0,
          catalog?.totalTurnCount ?? 0,
          session.dialogTurns.length,
        ) - session.dialogTurns.length,
      )
    : 0;

  return (localIndex: number): number => {
    const turn = session.dialogTurns[localIndex];
    if (!turn) {
      return localIndex + 1;
    }

    const catalogOrdinal = catalogOrdinalByTurnId.get(turn.id)
      ?? (
        typeof turn.backendTurnIndex === 'number'
          ? catalogOrdinalByStorageIndex.get(turn.backendTurnIndex)
          : undefined
      );
    if (catalogOrdinal !== undefined) {
      return catalogOrdinal + 1;
    }
    if (typeof turn.backendTurnIndex === 'number') {
      return turn.backendTurnIndex + 1;
    }
    return partialTurnOffset + localIndex + 1;
  };
}

export function absoluteSessionTurnIndexForId(
  session: Session,
  turnId: string,
): number | undefined {
  const catalogEntry = sessionTurnCatalog(session)?.entries.find(entry => entry.turnId === turnId);
  if (catalogEntry) {
    return catalogEntry.ordinal + 1;
  }

  const localIndex = session.dialogTurns.findIndex(turn => turn.id === turnId);
  return localIndex >= 0
    ? absoluteSessionTurnIndexForLocalIndex(session, localIndex)
    : undefined;
}

export function loadedSessionTurnIdForAbsoluteIndex(
  session: Session,
  turnIndex: number,
): string | undefined {
  const ordinal = turnIndex - 1;
  const catalogTurnId = sessionTurnCatalog(session)?.entries.find(
    entry => entry.ordinal === ordinal,
  )?.turnId;
  if (catalogTurnId) {
    return catalogTurnId;
  }

  return session.dialogTurns.find((_, localIndex) =>
    absoluteSessionTurnIndexForLocalIndex(session, localIndex) === turnIndex
  )?.id;
}
