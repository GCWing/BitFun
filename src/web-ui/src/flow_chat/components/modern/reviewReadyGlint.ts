import type { DialogTurn } from '../../types/flow-chat';

export const REVIEW_READY_GLINT_DURATION_MS = 5200;

export interface ReviewReadyGlintInput {
  currentTurnId?: string | null;
  currentTurnStatus?: DialogTurn['status'] | null;
  observedProcessingTurnId?: string | null;
  promptedTurnId?: string | null;
  nextReviewableCount: number;
  loadingStats: boolean;
  reviewActionAvailable: boolean;
  sessionProcessing?: boolean;
}

export function shouldTriggerReviewReadyGlint({
  currentTurnId,
  currentTurnStatus,
  observedProcessingTurnId,
  promptedTurnId,
  nextReviewableCount,
  loadingStats,
  reviewActionAvailable,
  sessionProcessing = false,
}: ReviewReadyGlintInput): boolean {
  return (
    Boolean(currentTurnId) &&
    currentTurnStatus === 'completed' &&
    observedProcessingTurnId === currentTurnId &&
    promptedTurnId !== currentTurnId &&
    !loadingStats &&
    !sessionProcessing &&
    reviewActionAvailable &&
    nextReviewableCount > 0
  );
}
