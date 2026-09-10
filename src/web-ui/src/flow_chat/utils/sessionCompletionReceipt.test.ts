import { describe, expect, it } from 'vitest';
import type { Session } from '../types/flow-chat';
import { completionResultItem, isCompletionResultVisible, sessionCompletionReceipt } from './sessionCompletionReceipt';

function completedSession(): Session {
  return { hasUnreadCompletion: 'completed', historyState: 'ready', lastFinishedAt: 10,
    dialogTurns: [{ id: 'turn', status: 'completed', endTime: 10 }] } as Session;
}

describe('completion receipt', () => {
  it('uses existing completion identity and ignores title/selection changes', () => {
    const session = completedSession();
    expect(sessionCompletionReceipt({ ...session, title: 'Renamed', status: 'active' }))
      .toBe(sessionCompletionReceipt(session));
    expect(sessionCompletionReceipt({ ...session, lastFinishedAt: 11 }))
      .not.toBe(sessionCompletionReceipt(session));
  });

  it('does not acknowledge a metadata-only record, a loading result, or a running turn', () => {
    const session = completedSession();
    expect(sessionCompletionReceipt({ ...session, historyState: 'metadata-only', dialogTurns: [] })).toBeNull();
    expect(sessionCompletionReceipt({ ...session, historyState: 'hydrating' })).toBeNull();
    expect(sessionCompletionReceipt({ ...session, dialogTurns: [{ ...session.dialogTurns[0], status: 'processing' }] })).toBeNull();
    expect(sessionCompletionReceipt({ ...session, hasUnreadCompletion: undefined })).toBeNull();
  });

  it('still accepts legacy hydrated records without a history-state field', () => {
    expect(sessionCompletionReceipt({ ...completedSession(), historyState: undefined })).not.toBeNull();
  });

  it('cannot acknowledge a new summary result while an older result is still displayed', () => {
    expect(sessionCompletionReceipt({ ...completedSession(), unreadCompletionTurnId: 'new-turn' })).toBeNull();
    expect(sessionCompletionReceipt({ ...completedSession(), unreadCompletionTurnId: 'turn' })).not.toBeNull();
  });

  it('requires the displayed recovery generation even after the turn is terminal', () => {
    const session = { ...completedSession(), unreadCompletionTurnId: 'turn', unreadCompletionGeneration: 2 };
    expect(sessionCompletionReceipt(session)).toBeNull();
    expect(sessionCompletionReceipt({ ...session,
      dialogTurns: [{ ...session.dialogTurns[0], recoveryEpoch: 1 }],
    })).toBeNull();
    expect(sessionCompletionReceipt({ ...session,
      dialogTurns: [{ ...session.dialogTurns[0], recoveryEpoch: 2 }],
    })).not.toBeNull();
  });

  it.each(['error', 'cancelled'] as const)('acknowledges %s only after the result has hydrated', (status) => {
    const session = completedSession();
    session.hasUnreadCompletion = status === 'error' ? 'error' : 'interrupted';
    session.dialogTurns[0].status = status;
    expect(sessionCompletionReceipt(session)).not.toBeNull();
    expect(sessionCompletionReceipt({ ...session, historyState: 'metadata-only' })).toBeNull();
  });

  it('keeps the result receipt bound to a user Turn when a local command follows it', () => {
    const session = completedSession();
    const receipt = sessionCompletionReceipt(session);
    session.dialogTurns.push({ ...session.dialogTurns[0], id: 'usage', kind: 'local_command' });
    expect(sessionCompletionReceipt(session)).toBe(receipt);
  });

  it('can acknowledge a cancellation before the first output without borrowing another Turn', () => {
    const input = { type: 'user-message', turnId: 'stopped' };
    expect(completionResultItem([input, { type: 'text', turnId: 'other' }], 'stopped')).toBe(input);
    expect(completionResultItem([input], 'other')).toBeUndefined();
  });

  it('requires the failure notice to be visible when an error has a rendered result', () => {
    const failure = { type: 'turn-failure-notice', turnId: 'failed' };
    expect(completionResultItem([{ type: 'user-message', turnId: 'failed' }, failure], 'failed')).toBe(failure);
  });
});

describe('completion result visibility rule', () => {
  const viewport = { top: 100, bottom: 500, left: 0, right: 500 };
  it('requires the result end, not just an overscanned or partially visible result', () => {
    expect(isCompletionResultVisible({ top: 400, bottom: 600, left: 0, right: 400 }, viewport)).toBe(false);
    expect(isCompletionResultVisible({ top: 50, bottom: 100, left: 0, right: 400 }, viewport)).toBe(false);
    expect(isCompletionResultVisible({ top: 50, bottom: 450, left: 0, right: 400 }, viewport)).toBe(true);
  });

  it('rejects collapsed, zero-size and horizontally hidden views', () => {
    const result = { top: 100, bottom: 200, left: 0, right: 400 };
    expect(isCompletionResultVisible(result, { ...viewport, bottom: 100 })).toBe(false);
    expect(isCompletionResultVisible({ ...result, bottom: 100 }, viewport)).toBe(false);
    expect(isCompletionResultVisible({ ...result, left: 500, right: 600 }, viewport)).toBe(false);
  });
});
