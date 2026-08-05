import { describe, expect, it } from 'vitest';
import type { DialogTurn, Session } from '../types/flow-chat';
import type { VirtualItem } from '../store/modernFlowChatStore';
import { deriveReviewDetailProjection, filterReviewDetailItems } from './reviewDetailState';

function createTurn(
  status: DialogTurn['status'],
  overrides: Partial<DialogTurn> = {},
): DialogTurn {
  return {
    id: 'turn-1',
    sessionId: 'review-check',
    userMessage: { id: 'user-1', content: 'review', timestamp: 1 },
    modelRounds: [],
    status,
    startTime: 1,
    ...overrides,
  };
}

function createSession(overrides: Partial<Session> = {}): Session {
  return {
    sessionId: 'review-check',
    dialogTurns: [],
    status: 'idle',
    config: {},
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    sessionKind: 'subagent',
    ...overrides,
  } as Session;
}

describe('deriveReviewDetailProjection', () => {
  it('keeps a load failure visible alongside previously loaded content', () => {
    const session = createSession({ historyState: 'failed' });

    expect(deriveReviewDetailProjection(session, true)).toEqual({
      content: 'load-failed',
      execution: null,
    });
  });

  it.each(['metadata-only', 'hydrating'] as const)(
    'shows loading for %s history',
    (historyState) => {
      expect(deriveReviewDetailProjection(createSession({ historyState }), false)).toEqual({
        content: 'loading',
        execution: null,
      });
    },
  );

  it('shows load failure without treating it as execution failure', () => {
    expect(deriveReviewDetailProjection(
      createSession({ historyState: 'failed', error: 'turn load failed' }),
      false,
    )).toEqual({ content: 'load-failed', execution: null });
  });

  it('shows preparing while the live turn has not produced visible items', () => {
    const session = createSession({
      historyState: 'ready',
      dialogTurns: [createTurn('processing')],
    });

    expect(deriveReviewDetailProjection(session, false)).toEqual({
      content: null,
      execution: 'preparing',
    });
  });

  it('shows a completed empty result honestly', () => {
    const session = createSession({
      historyState: 'ready',
      dialogTurns: [createTurn('completed')],
    });

    expect(deriveReviewDetailProjection(session, false)).toEqual({
      content: null,
      execution: 'completed-empty',
    });
  });

  it('does not claim an empty restored legacy session is still preparing', () => {
    const session = createSession({
      historyState: 'ready',
      dialogTurns: [],
    });

    expect(deriveReviewDetailProjection(session, false)).toEqual({
      content: 'unavailable',
      execution: null,
    });
  });

  it('does not mark an unhydrated non-empty history as unavailable', () => {
    const session = createSession({
      historyState: 'ready',
      dialogTurns: [],
      totalTurnCount: 3,
    });

    expect(deriveReviewDetailProjection(session, false)).toEqual({
      content: null,
      execution: 'preparing',
    });
  });

  it('keeps an explicit user cancellation distinct from failure', () => {
    const session = createSession({
      historyState: 'ready',
      dialogTurns: [createTurn('cancelled')],
    });

    expect(deriveReviewDetailProjection(session, false).execution).toBe('stopped');
  });

  it('recognizes an unfinished tool recovered after application restart', () => {
    const session = createSession({
      historyState: 'ready',
      dialogTurns: [createTurn('cancelled', {
        modelRounds: [{
          id: 'round-1',
          index: 0,
          isStreaming: false,
          isComplete: true,
          status: 'cancelled',
          startTime: 1,
          items: [{
            id: 'tool-1',
            type: 'tool',
            toolName: 'read_file',
            timestamp: 1,
            status: 'cancelled',
            interruptionReason: 'app_restart',
          }],
        }],
      })],
    });

    expect(deriveReviewDetailProjection(session, false).execution).toBe('interrupted');
  });

  it('recognizes a recovered in-progress turn that was interrupted before starting a tool', () => {
    const session = createSession({
      historyState: 'ready',
      dialogTurns: [createTurn('cancelled', {
        finishReason: 'interrupted',
        modelRounds: [],
      })],
    });

    expect(deriveReviewDetailProjection(session, false).execution).toBe('interrupted');
  });

  it('uses structured timeout category without exposing provider text', () => {
    const session = createSession({
      historyState: 'ready',
      dialogTurns: [createTurn('error', {
        error: 'Provider gateway request 91 timed out',
        errorDetail: { category: 'timeout', requestId: 'private-request-id' },
      })],
    });

    expect(deriveReviewDetailProjection(session, false).execution).toBe('timed-out');
  });

  it('treats model access errors as a generic failure state', () => {
    const session = createSession({
      historyState: 'ready',
      dialogTurns: [createTurn('error', {
        error: 'Private provider connection failed',
        errorDetail: { category: 'provider_unavailable' },
      })],
    });

    expect(deriveReviewDetailProjection(session, false).execution).toBe('failed');
  });

  it('lets a structured partial timeout override an empty completed child turn', () => {
    const session = createSession({
      historyState: 'ready',
      dialogTurns: [createTurn('completed')],
    });

    expect(deriveReviewDetailProjection(session, false, 'partial-timeout').execution)
      .toBe('partial-timeout');
  });

  it('keeps transcript failure and structured execution outcome independent', () => {
    const session = createSession({ historyState: 'failed' });

    expect(deriveReviewDetailProjection(session, false, 'partial-timeout')).toEqual({
      content: 'load-failed',
      execution: 'partial-timeout',
    });
  });

  it('removes internal launch and terminal wrapper items from Review-check details', () => {
    const items = [
      { type: 'user-message' },
      { type: 'user-steering-message' },
      { type: 'turn-failure-notice' },
      { type: 'turn-completion-notice' },
      { type: 'model-round' },
    ] as VirtualItem[];

    expect(filterReviewDetailItems(items).map((item) => item.type)).toEqual(['model-round']);
  });
});
