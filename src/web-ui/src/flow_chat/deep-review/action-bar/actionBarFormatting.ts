const START_DIALOG_TURN_ERROR_PREFIX = 'Failed to start dialog turn:';

export type ReviewActionErrorPresentation =
  | { kind: 'start_dialog_turn_failed'; reason: string | null }
  | { kind: 'raw'; message: string };

export function classifyReviewActionErrorMessage(message: string): ReviewActionErrorPresentation {
  const normalizedMessage = message.trim();

  if (normalizedMessage.startsWith(START_DIALOG_TURN_ERROR_PREFIX)) {
    const reason = normalizedMessage.slice(START_DIALOG_TURN_ERROR_PREFIX.length).trim();
    return {
      kind: 'start_dialog_turn_failed',
      reason: reason || null,
    };
  }

  return {
    kind: 'raw',
    message: normalizedMessage,
  };
}

export function formatElapsedTime(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  if (minutes <= 0) {
    return `${remainingSeconds}s`;
  }
  return `${minutes}m ${remainingSeconds}s`;
}
