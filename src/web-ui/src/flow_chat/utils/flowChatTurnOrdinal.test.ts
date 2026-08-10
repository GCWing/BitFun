import { describe, expect, it } from 'vitest';
import type { Session } from '../types/flow-chat';
import {
  absoluteSessionTurnIndexForId,
  createAbsoluteSessionTurnIndexResolver,
  loadedSessionTurnIdForAbsoluteIndex,
} from './flowChatTurnOrdinal';
import {
  isProjectedSessionEmpty,
  projectedSessionTurnCount,
  resolveDialogTurnIdentity,
  resolveStorageTurnIndex,
} from './flowChatTurnIdentity';

function createPartialSession(): Session {
  return {
    sessionId: 'session-1',
    dialogTurns: [98, 99, 100].map(turnIndex => ({
      id: `turn-${turnIndex}`,
      sessionId: 'session-1',
      userMessage: {
        id: `user-${turnIndex}`,
        content: `Prompt ${turnIndex}`,
        timestamp: 1000,
      },
      modelRounds: [],
      status: 'completed',
      startTime: 1000,
    })),
    status: 'idle',
    config: {},
    createdAt: 1000,
    lastActiveAt: 1000,
    error: null,
    sessionKind: 'flow_chat',
    isPartial: true,
    loadedTurnCount: 3,
    totalTurnCount: 100,
  };
}

describe('FlowChat absolute Turn ordinal helpers', () => {
  it('derives absolute indexes for a partial restored tail', () => {
    const session = createPartialSession();

    expect(absoluteSessionTurnIndexForId(session, 'turn-98')).toBe(98);
    expect(absoluteSessionTurnIndexForId(session, 'turn-100')).toBe(100);
    expect(loadedSessionTurnIdForAbsoluteIndex(session, 99)).toBe('turn-99');
    expect(loadedSessionTurnIdForAbsoluteIndex(session, 2)).toBeUndefined();
  });

  it('prefers catalog ordinals over sparse storage indexes', () => {
    const session = createPartialSession();
    session.dialogTurns[0].backendTurnIndex = 140;
    session.turnCatalog = {
      schemaVersion: 1,
      sessionId: session.sessionId,
      revision: 'catalog-1',
      totalTurnCount: 100,
      complete: false,
      entries: Array.from({ length: 100 }, (_, ordinal) => ({
        ordinal,
        storageTurnIndex: ordinal === 97 ? 140 : ordinal,
        turnId: ordinal === 97 ? 'turn-98' : undefined,
        preview: ordinal === 97 ? 'Prompt 98' : undefined,
        previewTruncated: false,
      })),
    };

    expect(absoluteSessionTurnIndexForId(session, 'turn-98')).toBe(98);
    expect(loadedSessionTurnIdForAbsoluteIndex(session, 98)).toBe('turn-98');
    expect(createAbsoluteSessionTurnIndexResolver(session)(0)).toBe(98);
    expect(resolveDialogTurnIdentity(session, 'turn-98')).toEqual({
      ordinal: 97,
      storageTurnIndex: 140,
      state: 'persisted',
    });
  });

  it('keeps optimistic ordinal separate from unavailable storage identity', () => {
    const session = createPartialSession();
    session.dialogTurns.push({
      id: 'turn-101',
      sessionId: session.sessionId,
      userMessage: { id: 'user-101', content: 'new prompt', timestamp: 1001 },
      modelRounds: [],
      status: 'pending',
      startTime: 1001,
    });
    session.totalTurnCount = 101;

    expect(resolveDialogTurnIdentity(session, 'turn-101')).toEqual({
      ordinal: 100,
      storageTurnIndex: undefined,
      state: 'optimistic',
    });
    expect(resolveStorageTurnIndex(session, 'turn-101')).toBeUndefined();
  });

  it('excludes provisional usage reports from projected counts and tail offsets', () => {
    const session = createPartialSession();
    session.dialogTurns.push({
      id: 'usage-1',
      sessionId: session.sessionId,
      kind: 'local_command',
      userMessage: {
        id: 'usage-user-1',
        content: 'report',
        timestamp: 1002,
        metadata: {
          localCommandKind: 'usage_report',
          usageReportProvisional: true,
          modelVisible: false,
        },
      },
      modelRounds: [],
      status: 'completed',
      startTime: 1002,
    });

    expect(projectedSessionTurnCount(session)).toBe(100);
    expect(absoluteSessionTurnIndexForId(session, 'turn-98')).toBe(98);
    expect(absoluteSessionTurnIndexForId(session, 'usage-1')).toBeUndefined();
  });

  it('does not treat metadata-only history as an empty Session', () => {
    const session = createPartialSession();
    session.dialogTurns = [];
    session.isPartial = undefined;
    session.totalTurnCount = 23;

    expect(projectedSessionTurnCount(session)).toBe(23);
    expect(isProjectedSessionEmpty(session)).toBe(false);
  });
});
