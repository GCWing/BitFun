import type { AiErrorAction, AiErrorDetail } from '@/shared/ai-errors/aiErrorPresenter';
import {
  getAiErrorPresentation,
  normalizeAiErrorDetail,
} from '@/shared/ai-errors/aiErrorPresenter';
import type { FlowToolItem, Session } from '../types/flow-chat';

export type DeepReviewContinuationPhase = 'review_interrupted' | 'resume_blocked';
export type DeepReviewReviewerStatus = 'completed' | 'timed_out' | 'failed' | 'cancelled' | 'skipped' | 'unknown';

export interface DeepReviewReviewerProgress {
  reviewer: string;
  status: DeepReviewReviewerStatus;
  toolCallId?: string;
  error?: string;
}

export interface DeepReviewInterruption {
  phase: DeepReviewContinuationPhase;
  childSessionId: string;
  parentSessionId?: string;
  originalTarget: string;
  errorDetail: AiErrorDetail;
  canResume: boolean;
  recommendedActions: AiErrorAction[];
  reviewers: DeepReviewReviewerProgress[];
}

const RESUME_BLOCKING_CATEGORIES = new Set([
  'provider_quota',
  'provider_billing',
  'auth',
  'permission',
]);

export function deriveDeepReviewInterruption(
  session: Session,
  errorDetail?: AiErrorDetail | null,
): DeepReviewInterruption | null {
  if (session.sessionKind !== 'deep_review') {
    return null;
  }

  const lastTurn = session.dialogTurns[session.dialogTurns.length - 1];
  const wasManuallyCancelled = lastTurn?.status === 'cancelled';
  const hasFailure = lastTurn?.status === 'error' || wasManuallyCancelled || Boolean(session.error);
  if (!hasFailure) {
    return null;
  }

  const fallbackMessage =
    session.error ??
    lastTurn?.error ??
    (wasManuallyCancelled ? 'Deep Review was stopped by the user.' : '');
  const effectiveErrorDetail =
    errorDetail ??
    (wasManuallyCancelled
      ? {
        category: 'unknown' as const,
        retryable: true,
        actionHints: ['continue', 'copy_diagnostics'] as const,
        rawMessage: fallbackMessage,
      }
      : null);
  const normalizedError = normalizeAiErrorDetail(effectiveErrorDetail, fallbackMessage);
  const presentation = getAiErrorPresentation(normalizedError);
  const canResume = !RESUME_BLOCKING_CATEGORIES.has(presentation.category);

  return {
    phase: canResume ? 'review_interrupted' : 'resume_blocked',
    childSessionId: session.sessionId,
    parentSessionId: session.btwOrigin?.parentSessionId ?? session.parentSessionId,
    originalTarget: findOriginalTarget(session),
    errorDetail: normalizedError,
    canResume,
    recommendedActions: presentation.actions,
    reviewers: collectReviewerProgress(session),
  };
}

export function buildDeepReviewContinuationPrompt(interruption: DeepReviewInterruption): string {
  const reviewerLines = interruption.reviewers.length
    ? interruption.reviewers
        .map((reviewer) => {
          const suffix = reviewer.error ? ` (${reviewer.error})` : '';
          return `- ${reviewer.reviewer}: ${reviewer.status}${suffix}`;
        })
        .join('\n')
    : '- No reliable reviewer progress was detected. Reconstruct progress from this session before deciding what to rerun.';

  return [
    'Continue the interrupted Deep Review in this same session.',
    '',
    'Recovery rules:',
    '- Do not restart completed reviewer work unless the existing result is clearly incomplete or unusable.',
    '- Do not re-run skipped, non-applicable, or policy-ineligible reviewers; keep them recorded as skipped coverage.',
    '- Re-run only missing, failed, timed-out, or cancelled reviewers when enough context exists.',
    '- If reviewer coverage remains incomplete, say that explicitly and mark the final report as lower confidence.',
    '- Run ReviewJudge before the final submit_code_review result when reviewer findings exist.',
    '',
    'Original review target:',
    interruption.originalTarget,
    '',
    'Known reviewer progress:',
    reviewerLines,
    '',
    'Last error:',
    `- category: ${interruption.errorDetail.category ?? 'unknown'}`,
    interruption.errorDetail.providerCode ? `- provider code: ${interruption.errorDetail.providerCode}` : '- provider code: unknown',
    interruption.errorDetail.requestId ? `- request id: ${interruption.errorDetail.requestId}` : '- request id: unknown',
  ].join('\n');
}

function findOriginalTarget(session: Session): string {
  const firstTurn = session.dialogTurns[0];
  return firstTurn?.userMessage?.content?.trim() || 'Unknown Deep Review target.';
}

export function collectReviewerProgress(session: Session): DeepReviewReviewerProgress[] {
  const byReviewer = new Map<string, DeepReviewReviewerProgress>();

  for (const turn of session.dialogTurns) {
    for (const round of turn.modelRounds) {
      for (const item of round.items) {
        if (item.type !== 'tool' || item.toolName !== 'Task') {
          continue;
        }
        const progress = getReviewerProgressFromTask(item);
        if (!progress) {
          continue;
        }
        byReviewer.set(progress.reviewer, progress);
      }
    }
  }

  return [...byReviewer.values()];
}

function getReviewerProgressFromTask(item: FlowToolItem): DeepReviewReviewerProgress | null {
  const reviewer = String(
    item.toolCall.input?.subagent_type ??
      item.toolCall.input?.subagentType ??
      item.toolCall.input?.agent_type ??
      item.toolCall.input?.agentType ??
      '',
  ).trim();

  if (!reviewer.startsWith('Review')) {
    return null;
  }

  const error = item.toolResult?.error;
  let status: DeepReviewReviewerStatus = 'unknown';
  if (item.toolResult?.success === true || item.status === 'completed') {
    status = 'completed';
  } else if (/timeout|timed out/i.test(error ?? '')) {
    status = 'timed_out';
  } else if (isPolicyIneligibleReviewerError(error)) {
    // A Task policy violation means the orchestrator scheduled a reviewer that
    // is not eligible for this pass. Retrying the same Task only repeats that
    // product error, so continuation should preserve it as skipped coverage.
    status = 'skipped';
  } else if (item.status === 'cancelled') {
    status = 'cancelled';
  } else if (item.toolResult?.success === false || item.status === 'error') {
    status = 'failed';
  }

  return {
    reviewer,
    status,
    toolCallId: item.toolCall.id,
    error,
  };
}

function isPolicyIneligibleReviewerError(error?: string): boolean {
  if (!error) {
    return false;
  }
  return /DeepReview Task policy violation|deep_review_subagent_(?:not_review|not_allowed|not_readonly)/i.test(error);
}
