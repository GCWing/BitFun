import { getDeepReviewLaunchErrorMessage, type DeepReviewLaunchError } from '../launch/launchErrors';

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

export function getReviewActionErrorMessage(
  error: unknown,
  translate: (key: string, options?: { defaultValue?: string; reason?: string }) => string,
  fallback: string,
): string {
  const launchError = error as DeepReviewLaunchError | null | undefined;
  if (launchError?.launchErrorMessageKey) {
    const message = getDeepReviewLaunchErrorMessage(error, translate, fallback);
    const original = launchError.originalMessage?.trim();
    const presentation = original ? classifyReviewActionErrorMessage(original) : null;
    const reason = presentation?.kind === 'start_dialog_turn_failed'
      ? presentation.reason
      : original;
    return reason && reason !== message ? `${message}\n${reason}` : message;
  }

  const message = error instanceof Error ? error.message : typeof error === 'string' ? error : '';
  if (!message.trim()) return fallback;

  const presentation = classifyReviewActionErrorMessage(message);
  if (presentation.kind === 'raw') return presentation.message;
  return presentation.reason
    ? translate('deepReviewActionBar.actionStartFailedWithReason', { reason: presentation.reason })
    : translate('deepReviewActionBar.actionStartFailed');
}
