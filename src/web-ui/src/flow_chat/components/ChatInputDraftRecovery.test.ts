import { describe, expect, it } from 'vitest';
import {
  clearComposerForSubmission,
  failedSubmissionRecoveryTarget,
  shouldRecordContextMutation,
  successfulRetryCleanupTarget,
} from './chatInputDraftRecovery';

describe('ChatInput failed submission recovery', () => {
  it('clears text and attachments in the same synchronous submission step', () => {
    const draft = {
      value: 'describe this image',
      contextIds: ['image-1'],
      pendingLargePastes: ['paste-1'],
      queuedInput: 'describe this image' as string | null,
    };

    clearComposerForSubmission({
      clearValue: () => { draft.value = ''; },
      clearContexts: () => { draft.contextIds = []; },
      clearPendingLargePastes: () => { draft.pendingLargePastes = []; },
      clearQueuedInput: () => { draft.queuedInput = null; },
    });

    expect(draft).toEqual({
      value: '',
      contextIds: [],
      pendingLargePastes: [],
      queuedInput: null,
    });
  });

  it('restores the visible composer only when the same session is still unchanged', () => {
    expect(failedSubmissionRecoveryTarget('session-a', 'session-a', 4, 4)).toBe('current');
  });

  it('does not overwrite a newer draft in the same session', () => {
    expect(failedSubmissionRecoveryTarget('session-a', 'session-a', 4, 5)).toBe('none');
  });

  it('restores the failed draft to its session store after the user switches sessions', () => {
    expect(failedSubmissionRecoveryTarget('session-a', 'session-b', 4, 4)).toBe('stored');
  });

  it('does not overwrite a newer draft in the original session after switching away', () => {
    expect(failedSubmissionRecoveryTarget('session-a', 'session-b', 4, 5)).toBe('none');
  });

  it('does not count A to B to A draft restoration as a user context mutation', () => {
    expect(shouldRecordContextMutation(true, true)).toBe(false);
    expect(failedSubmissionRecoveryTarget('session-a', 'session-a', 4, 4)).toBe('current');
  });

  it('clears the stored draft after retry succeeds while another session is visible', () => {
    expect(successfulRetryCleanupTarget(
      'session-a',
      'session-b',
      4,
      4,
      'retry this',
      ['context-a'],
      'retry this',
      ['context-a'],
    )).toBe('stored');
  });

  it('does not clear a stored draft changed before or during retry', () => {
    expect(successfulRetryCleanupTarget(
      'session-a',
      'session-b',
      4,
      5,
      'newer draft',
      ['context-a'],
      'retry this',
      ['context-a'],
    )).toBe('none');
  });
});
