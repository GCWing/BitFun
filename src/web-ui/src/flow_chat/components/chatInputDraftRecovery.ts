interface ComposerSubmissionClearActions {
  clearValue: () => void;
  clearContexts: () => void;
  clearPendingLargePastes: () => void;
  clearQueuedInput: () => void;
}

/**
 * Clear every user-visible part of a submitted composer before transport work
 * begins. Keeping these mutations in one synchronous call prevents React from
 * painting a text-only or attachment-only intermediate draft.
 */
export function clearComposerForSubmission(
  actions: ComposerSubmissionClearActions,
): void {
  actions.clearValue();
  actions.clearContexts();
  actions.clearPendingLargePastes();
  actions.clearQueuedInput();
}

export function failedSubmissionRecoveryTarget(
  submissionSessionId: string | null,
  currentSessionId: string | null,
  submissionRevision: number,
  currentRevision: number,
): 'current' | 'stored' | 'none' {
  if (submissionSessionId === currentSessionId) {
    return submissionRevision === currentRevision ? 'current' : 'none';
  }
  return submissionSessionId && submissionRevision === currentRevision ? 'stored' : 'none';
}

export function shouldRecordContextMutation(
  contextsChanged: boolean,
  isRestoringSessionDraft: boolean,
): boolean {
  return contextsChanged && !isRestoringSessionDraft;
}

export function successfulRetryCleanupTarget(
  submissionSessionId: string,
  currentSessionId: string | null,
  baselineRevision: number,
  currentRevision: number,
  currentMessage: string,
  currentContextIds: string[],
  submittedMessage: string,
  submittedContextIds: string[],
): 'current' | 'stored' | 'none' {
  const composerIsUnchanged =
    baselineRevision === currentRevision
    && currentMessage.trim() === submittedMessage.trim()
    && currentContextIds.length === submittedContextIds.length
    && currentContextIds.every((id, index) => id === submittedContextIds[index]);

  if (!composerIsUnchanged) return 'none';
  return currentSessionId === submissionSessionId ? 'current' : 'stored';
}
