import { classifyLaunchError } from '../../utils/deepReviewExperience';

export type DeepReviewLaunchStep =
  | 'create_child_session'
  | 'open_aux_pane'
  | 'send_start_message';

export interface FailedDeepReviewCleanupResult {
  cleanupCompleted: boolean;
  cleanupIssues: string[];
}

export interface DeepReviewLaunchError extends Error {
  launchErrorCategory?: string;
  launchErrorActions?: string[];
  launchErrorMessageKey?: string;
  launchErrorStep?: string;
  originalMessage?: string;
  childSessionId?: string;
  cleanupCompleted?: boolean;
  cleanupIssues?: string[];
}

const LAUNCH_ERROR_DEFAULT_MESSAGES: Record<string, string> = {
  'deepReviewActionBar.launchError.modelConfig': 'Deep review could not create a review session. Check the model configuration.',
  'deepReviewActionBar.launchError.network': 'Network connection was interrupted before Deep Review could start.',
  'deepReviewActionBar.launchError.unknown': 'Deep review failed to start. Please try again.',
};

export function normalizeErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message.trim();
  }

  if (typeof error === 'string' && error.trim()) {
    return error.trim();
  }

  return 'Deep review failed to start';
}

export function isSessionMissingError(error: unknown): boolean {
  const message = normalizeErrorMessage(error).toLowerCase();
  return message.includes('session does not exist') || message.includes('not found');
}

function describeLaunchStep(step: DeepReviewLaunchStep): string {
  switch (step) {
    case 'create_child_session':
      return 'creating the deep review session';
    case 'open_aux_pane':
      return 'opening the deep review pane';
    case 'send_start_message':
      return 'starting the deep review run';
    default:
      return 'launching deep review';
  }
}

export function createDeepReviewLaunchError(
  launchStep: DeepReviewLaunchStep,
  originalError: unknown,
  childSessionId?: string,
  cleanupResult?: FailedDeepReviewCleanupResult,
): DeepReviewLaunchError {
  const classified = classifyLaunchError(launchStep, originalError);
  const friendlyError = new Error(
    LAUNCH_ERROR_DEFAULT_MESSAGES[classified.messageKey] ??
      LAUNCH_ERROR_DEFAULT_MESSAGES['deepReviewActionBar.launchError.unknown'],
  ) as DeepReviewLaunchError;

  friendlyError.launchErrorCategory = classified.category;
  friendlyError.launchErrorActions = classified.actions;
  friendlyError.launchErrorMessageKey = classified.messageKey;
  friendlyError.launchErrorStep = classified.step;
  friendlyError.originalMessage = normalizeErrorMessage(originalError);
  if (childSessionId) {
    friendlyError.childSessionId = childSessionId;
  }
  if (cleanupResult) {
    friendlyError.cleanupCompleted = cleanupResult.cleanupCompleted;
    friendlyError.cleanupIssues = cleanupResult.cleanupIssues;
  }

  return friendlyError;
}

export function getDeepReviewLaunchErrorMessage(
  error: unknown,
  translate: (key: string, options?: { defaultValue?: string }) => string,
  fallback = LAUNCH_ERROR_DEFAULT_MESSAGES['deepReviewActionBar.launchError.unknown'],
): string {
  const launchError = error as DeepReviewLaunchError | null | undefined;
  if (launchError?.launchErrorMessageKey) {
    return translate(launchError.launchErrorMessageKey, {
      defaultValue: launchError.message || fallback,
    });
  }

  if (error instanceof Error && error.message.trim()) {
    return error.message.trim();
  }

  return fallback;
}

export function buildLaunchCleanupError(
  launchStep: DeepReviewLaunchStep,
  childSessionId: string,
  originalError: unknown,
  cleanupResult: FailedDeepReviewCleanupResult,
): Error {
  const originalMessage = normalizeErrorMessage(originalError);
  if (cleanupResult.cleanupCompleted) {
    return originalError instanceof Error ? originalError : new Error(originalMessage);
  }

  const cleanupSummary = cleanupResult.cleanupIssues.join(' ');
  return new Error(
    `${originalMessage} Cleanup was incomplete after failure while ${describeLaunchStep(launchStep)}. ` +
      `The partially created deep review session (${childSessionId}) may need manual cleanup. ${cleanupSummary}`.trim(),
  );
}
