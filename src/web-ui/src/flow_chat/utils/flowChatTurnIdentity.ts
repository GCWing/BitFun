import type { LocalCommandMetadata } from '@/shared/types/session-history';
import type {
  DialogTurn,
  DialogTurnIdentity,
  LocalTurnIndex,
  Session,
  StorageTurnIndex,
  TurnOrdinal,
} from '../types/flow-chat';

type TurnIdentitySession = Pick<
  Session,
  'sessionId' | 'dialogTurns' | 'isPartial' | 'totalTurnCount' | 'turnCatalog'
>;

export function asLocalTurnIndex(value: number): LocalTurnIndex {
  return value as LocalTurnIndex;
}

export function asTurnOrdinal(value: number): TurnOrdinal {
  return value as TurnOrdinal;
}

export function asStorageTurnIndex(value: number): StorageTurnIndex {
  return value as StorageTurnIndex;
}

export function isProvisionalUsageReportTurn(turn: DialogTurn): boolean {
  const metadata = turn.userMessage?.metadata as LocalCommandMetadata | undefined;
  return metadata?.localCommandKind === 'usage_report'
    && metadata.usageReportProvisional === true;
}

export function canonicalSessionTurns(
  session: Pick<TurnIdentitySession, 'dialogTurns'>,
): DialogTurn[] {
  return session.dialogTurns.filter(turn => !isProvisionalUsageReportTurn(turn));
}

export function validSessionTurnCatalog(session: TurnIdentitySession) {
  return session.turnCatalog?.sessionId === session.sessionId
    ? session.turnCatalog
    : undefined;
}

export function projectedSessionTurnCount(session: TurnIdentitySession): number {
  const persistedTurnCount = canonicalSessionTurns(session).length;
  return Math.max(
    session.totalTurnCount ?? 0,
    validSessionTurnCatalog(session)?.totalTurnCount ?? 0,
    persistedTurnCount,
  );
}

/** True only when the complete projected Session has no persisted Turn. */
export function isProjectedSessionEmpty(session: TurnIdentitySession): boolean {
  return projectedSessionTurnCount(session) === 0;
}

/** First runtime Turn, including the optimistic Turn already appended locally. */
export function isProjectedFirstRuntimeTurn(session: TurnIdentitySession): boolean {
  return projectedSessionTurnCount(session) <= 1;
}

export function resolveStorageTurnIndex(
  session: TurnIdentitySession,
  turnOrId: DialogTurn | string,
): StorageTurnIndex | undefined {
  const turn = typeof turnOrId === 'string'
    ? session.dialogTurns.find(candidate => candidate.id === turnOrId)
    : turnOrId;
  const directStorageIndex = turn?.storageTurnIndex ?? turn?.backendTurnIndex;
  if (directStorageIndex !== undefined) {
    return asStorageTurnIndex(directStorageIndex);
  }

  const turnId = typeof turnOrId === 'string' ? turnOrId : turnOrId.id;
  const catalogEntry = validSessionTurnCatalog(session)?.entries.find(
    entry => entry.turnId === turnId,
  );
  return catalogEntry
    ? asStorageTurnIndex(catalogEntry.storageTurnIndex)
    : undefined;
}

function buildCatalogOrdinalMaps(session: TurnIdentitySession): {
  ordinalByTurnId: Map<string, TurnOrdinal>;
  ordinalByStorageIndex: Map<number, TurnOrdinal>;
} {
  const ordinalByTurnId = new Map<string, TurnOrdinal>();
  const ordinalByStorageIndex = new Map<number, TurnOrdinal>();
  for (const entry of validSessionTurnCatalog(session)?.entries ?? []) {
    const ordinal = asTurnOrdinal(entry.ordinal);
    if (entry.turnId) {
      ordinalByTurnId.set(entry.turnId, ordinal);
    }
    ordinalByStorageIndex.set(entry.storageTurnIndex, ordinal);
  }
  return { ordinalByTurnId, ordinalByStorageIndex };
}

export function createTurnOrdinalResolver(
  session: TurnIdentitySession,
): (localIndex: LocalTurnIndex | number) => TurnOrdinal | undefined {
  const { ordinalByTurnId, ordinalByStorageIndex } = buildCatalogOrdinalMaps(session);
  const canonicalTurns = canonicalSessionTurns(session);
  const fallbackStartOrdinal = session.isPartial === true
    ? Math.max(0, projectedSessionTurnCount(session) - canonicalTurns.length)
    : 0;
  const fallbackOrdinalByTurnId = new Map<string, TurnOrdinal>();
  canonicalTurns.forEach((turn, index) => {
    fallbackOrdinalByTurnId.set(turn.id, asTurnOrdinal(fallbackStartOrdinal + index));
  });

  return (localIndex): TurnOrdinal | undefined => {
    const turn = session.dialogTurns[localIndex];
    if (!turn || isProvisionalUsageReportTurn(turn)) {
      return undefined;
    }

    const storageTurnIndex = resolveStorageTurnIndex(session, turn);
    return ordinalByTurnId.get(turn.id)
      ?? (storageTurnIndex !== undefined
        ? ordinalByStorageIndex.get(storageTurnIndex)
        : undefined)
      ?? fallbackOrdinalByTurnId.get(turn.id);
  };
}

export function resolveTurnOrdinal(
  session: TurnIdentitySession,
  turnOrId: DialogTurn | string,
): TurnOrdinal | undefined {
  const turnId = typeof turnOrId === 'string' ? turnOrId : turnOrId.id;
  const localIndex = session.dialogTurns.findIndex(turn => turn.id === turnId);
  if (localIndex < 0) {
    const catalogEntry = validSessionTurnCatalog(session)?.entries.find(
      entry => entry.turnId === turnId,
    );
    return catalogEntry ? asTurnOrdinal(catalogEntry.ordinal) : undefined;
  }
  return createTurnOrdinalResolver(session)(asLocalTurnIndex(localIndex));
}

export function resolveDialogTurnIdentity(
  session: TurnIdentitySession,
  turnOrId: DialogTurn | string,
): DialogTurnIdentity | undefined {
  const ordinal = resolveTurnOrdinal(session, turnOrId);
  if (ordinal === undefined) {
    return undefined;
  }
  const storageTurnIndex = resolveStorageTurnIndex(session, turnOrId);
  return {
    ordinal,
    storageTurnIndex,
    state: storageTurnIndex === undefined ? 'optimistic' : 'persisted',
  };
}
