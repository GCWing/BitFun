import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FlowChatState, Session } from '../types/flow-chat';
import { insertReviewSessionSummaryMarker } from './ReviewSessionMarkerService';

const storeMock = vi.hoisted(() => ({
  state: {
    sessions: new Map(),
    activeSessionId: null,
  } as FlowChatState,
  addModelRoundItem: vi.fn((sessionId: string, dialogTurnId: string, item: any, modelRoundId?: string) => {
    const session = storeMock.state.sessions.get(sessionId);
    const turn = session?.dialogTurns.find((candidate) => candidate.id === dialogTurnId);
    const round = modelRoundId
      ? turn?.modelRounds.find((candidate) => candidate.id === modelRoundId)
      : turn?.modelRounds[turn.modelRounds.length - 1];
    round?.items.push(item);
  }),
}));

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => storeMock.state,
    addModelRoundItem: storeMock.addModelRoundItem,
  },
}));

function parentSession(): Session {
  return {
    sessionId: 'parent-session',
    title: 'Parent',
    dialogTurns: [{
      id: 'turn-1',
      sessionId: 'parent-session',
      userMessage: {
        id: 'user-1',
        content: 'please change files',
        timestamp: 1,
      },
      modelRounds: [{
        id: 'round-1',
        index: 0,
        items: [],
        isStreaming: false,
        isComplete: true,
        status: 'completed',
        startTime: 1,
      }],
      status: 'completed',
      startTime: 1,
    }],
    status: 'idle',
    config: {},
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
  };
}

function setStoreState(state: FlowChatState): void {
  storeMock.state = state;
}

describe('ReviewSessionMarkerService', () => {
  afterEach(() => {
    setStoreState({
      sessions: new Map(),
      activeSessionId: null,
    });
    storeMock.addModelRoundItem.mockClear();
  });

  it('inserts a review summary marker into the source dialog turn', () => {
    setStoreState({
      sessions: new Map([['parent-session', parentSession()]]),
      activeSessionId: 'parent-session',
    });

    const inserted = insertReviewSessionSummaryMarker({
      parentSessionId: 'parent-session',
      childSessionId: 'review-child',
      kind: 'deep_review',
      title: 'Deep review',
      requestedFiles: ['src/app.ts'],
      parentDialogTurnId: 'turn-1',
    });

    const parent = storeMock.state.sessions.get('parent-session');
    const marker = parent?.dialogTurns[0]?.modelRounds[0]?.items[0];

    expect(inserted).toBe(true);
    expect(marker).toMatchObject({
      type: 'tool',
      toolName: 'ReviewSessionSummary',
      status: 'completed',
      toolCall: {
        input: {
          childSessionId: 'review-child',
          parentSessionId: 'parent-session',
          kind: 'deep_review',
          requestedFiles: ['src/app.ts'],
        },
      },
    });
  });
});
