import type { AiErrorAction, AiErrorDetail } from '@/shared/ai-errors/aiErrorPresenter';
import {
  getAiErrorPresentation,
  normalizeAiErrorDetail,
} from '@/shared/ai-errors/aiErrorPresenter';
import type { FlowToolItem, Session } from '../types/flow-chat';

export type DeepReviewContinuationPhase = 'review_interrupted' | 'resume_blocked';
export type DeepReviewResultRecoveryReason =
  | 'missing_submit_code_review'
  | 'invalid_submit_code_review'
  | 'wrong_review_mode';
export type DeepReviewInterruptionReason = 'manual_cancelled';
export type DeepReviewReviewerStatus =
  | 'completed'
  | 'partial_timeout'
  | 'timed_out'
  | 'failed'
  | 'cancelled'
  | 'skipped'
  | 'unknown';

export interface DeepReviewReviewerProgress {
  reviewer: string;
  status: DeepReviewReviewerStatus;
  toolCallId?: string;
  error?: string;
  partialOutput?: string;
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
  runManifest?: Session['deepReviewRunManifest'];
  resultRecoveryReason?: DeepReviewResultRecoveryReason;
  interruptionReason?: DeepReviewInterruptionReason;
}

const RESUME_BLOCKING_CATEGORIES = new Set([
  'provider_quota',
  'provider_billing',
  'auth',
  'permission',
]);

const RESULT_RECOVERY_MESSAGES: Record<DeepReviewResultRecoveryReason, string> = {
  missing_submit_code_review:
    'Deep Review completed, but BitFun did not receive a structured submit_code_review result.',
  invalid_submit_code_review:
    'Deep Review submitted a structured result that BitFun could not read.',
  wrong_review_mode:
    'Deep Review submitted a standard Code Review result instead of a Deep Review result.',
};

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
    runManifest: session.deepReviewRunManifest,
    interruptionReason: wasManuallyCancelled ? 'manual_cancelled' : undefined,
  };
}

export function deriveDeepReviewResultRecoveryInterruption(
  session: Session,
  reason: DeepReviewResultRecoveryReason,
): DeepReviewInterruption | null {
  if (session.sessionKind !== 'deep_review') {
    return null;
  }

  const errorDetail = normalizeAiErrorDetail({
    category: 'model_error',
    retryable: true,
    actionHints: ['continue', 'copy_diagnostics'],
    rawMessage: RESULT_RECOVERY_MESSAGES[reason],
  });
  const presentation = getAiErrorPresentation(errorDetail);

  return {
    phase: 'review_interrupted',
    childSessionId: session.sessionId,
    parentSessionId: session.btwOrigin?.parentSessionId ?? session.parentSessionId,
    originalTarget: findOriginalTarget(session),
    errorDetail,
    canResume: true,
    recommendedActions: presentation.actions,
    reviewers: collectReviewerProgress(session),
    runManifest: session.deepReviewRunManifest,
    resultRecoveryReason: reason,
  };
}

export function buildDeepReviewContinuationPrompt(interruption: DeepReviewInterruption): string {
  const wasManuallyCancelled = interruption.interruptionReason === 'manual_cancelled';
  const reviewerLines = interruption.reviewers.length
    ? interruption.reviewers
        .map((reviewer) => {
          const suffix = reviewer.error ? ` (${reviewer.error})` : '';
          const partialOutput = reviewer.partialOutput
            ? `; partial output: ${reviewer.partialOutput}`
            : '';
          return `- ${reviewer.reviewer}: ${reviewer.status}${suffix}${partialOutput}`;
        })
        .join('\n')
    : '- No reliable reviewer progress was detected. Reconstruct progress from this session before deciding what to rerun.';
  const skippedReviewers = interruption.runManifest?.skippedReviewers ?? [];
  const manifestSkippedReviewers = formatManifestSkippedReviewers(skippedReviewers);
  const manifestRules = skippedReviewers.some((reviewer) => reviewer.reason === 'not_applicable')
    ? [
        '- Do not run reviewers skipped as not_applicable.',
      ]
    : [];
  const manifestBlock = manifestSkippedReviewers.length
    ? [
        '',
        'Run manifest reviewer skips:',
        manifestSkippedReviewers.join('\n'),
      ]
    : [];
  const retryBudgetRules = wasManuallyCancelled
    ? formatManualCancelRetryBudgetRules()
    : formatRetryBudgetRules(interruption.runManifest);
  const incrementalCacheBlock = formatIncrementalReviewCacheGuidance(
    interruption.runManifest,
  );
  const manualCancelRules = wasManuallyCancelled
    ? [
        '- The previous interruption was requested by the user. Treat it as a user stop/pause, not as a model failure or reviewer timeout.',
        '- Preserve completed reviewer output and continue only unfinished reviewer work where enough context exists.',
      ]
    : [];
  const resultRecoveryRules = interruption.resultRecoveryReason
    ? [
        '- The previous Deep Review ended without a usable structured submit_code_review result.',
        '- First reconstruct and submit the missing final report from preserved reviewer outputs.',
        '- Do not rerun completed reviewer work just to regenerate the report.',
        '- If preserved reviewer output is insufficient, rerun only missing, failed, timed-out, or cancelled reviewers before submitting the report.',
      ]
    : [];

  return [
    'Continue the interrupted Deep Review in this same session.',
    '',
    'Recovery rules:',
    ...manualCancelRules,
    ...resultRecoveryRules,
    '- Do not restart completed reviewer work unless the existing result is clearly incomplete or unusable.',
    '- Do not re-run skipped, non-applicable, or policy-ineligible reviewers; keep them recorded as skipped coverage.',
    ...retryBudgetRules,
    ...manifestRules,
    '- Re-run only missing, failed, timed-out, or cancelled reviewers when enough context exists.',
    '- If reviewer coverage remains incomplete, say that explicitly and mark the final report as lower confidence.',
    '- Run ReviewJudge before the final submit_code_review result when reviewer findings exist.',
    '',
    'Original review target:',
    interruption.originalTarget,
    '',
    'Known reviewer progress:',
    reviewerLines,
    ...manifestBlock,
    ...incrementalCacheBlock,
    ...formatLastInterruptionBlock(interruption),
  ].join('\n');
}

function formatIncrementalReviewCacheGuidance(
  runManifest: Session['deepReviewRunManifest'] | undefined,
): string[] {
  const cachePlan = runManifest?.incrementalReviewCache;
  if (!cachePlan) {
    return [];
  }

  return [
    '',
    'Incremental review cache guidance:',
    `- cache_key: ${cachePlan.cacheKey}`,
    `- fingerprint: ${cachePlan.fingerprint}`,
    `- strategy: ${cachePlan.strategy}`,
    `- reviewer_packet_ids: ${cachePlan.reviewerPacketIds.join(', ') || 'none'}`,
    `- invalidates_on: ${cachePlan.invalidatesOn.join(', ') || 'none'}`,
    '- Only reuse completed reviewer outputs when the current review target fingerprint still matches.',
    '- If any invalidates_on condition changed, rerun affected reviewer packets and explain the fresh review boundary.',
  ];
}

function formatRetryBudgetRules(
  runManifest: Session['deepReviewRunManifest'] | undefined,
): string[] {
  const maxRetriesPerRole = runManifest?.executionPolicy?.maxRetriesPerRole;
  const baseRules = [
    '- Treat partial_timeout reviewers as preserved partial evidence. Re-run them only when useful evidence is missing or unusable.',
  ];

  if (typeof maxRetriesPerRole !== 'number') {
    return [
      ...baseRules,
      '- Respect the original retry budget if it is recoverable from context; do not retry the same reviewer repeatedly.',
    ];
  }

  if (maxRetriesPerRole <= 0) {
    return [
      ...baseRules,
      '- Retry budget from manifest: max_retries_per_role = 0. Do not re-run failed, timed-out, or partial reviewers automatically; report remaining gaps instead.',
    ];
  }

  return [
    ...baseRules,
    `- Retry budget from manifest: max_retries_per_role = ${maxRetriesPerRole}.`,
    '- For each retry, use the same subagent_type with retry = true, reduce the scope to missing evidence, downgrade strategy when possible, and use a shorter timeout.',
  ];
}

function formatManualCancelRetryBudgetRules(): string[] {
  return [
    '- Treat partial_timeout reviewers as preserved partial evidence. Re-run them only when useful evidence is missing or unusable.',
    '- User cancellation does not consume reviewer retry budget.',
    '- Do not expose internal retry-budget settings or names such as max retry counts in the user-facing reply.',
    '- An initial timed-out or cancelled reviewer attempt does not by itself mean reviewer retry budget is exhausted.',
    '- Use retry=true only when the Task tool permits structured retry_coverage for partial_timeout or transient capacity failure. Otherwise continue missing or user-cancelled reviewer work as normal continuation work when possible, or report reduced coverage without internal budget jargon.',
  ];
}

function formatLastInterruptionBlock(interruption: DeepReviewInterruption): string[] {
  if (interruption.interruptionReason === 'manual_cancelled') {
    return [
      '',
      'Last interruption:',
      '- reason: user_cancelled',
    ];
  }

  return [
    '',
    'Last error:',
    `- category: ${interruption.errorDetail.category ?? 'unknown'}`,
    interruption.errorDetail.providerCode
      ? `- provider code: ${interruption.errorDetail.providerCode}`
      : '- provider code: unknown',
    interruption.errorDetail.requestId
      ? `- request id: ${interruption.errorDetail.requestId}`
      : '- request id: unknown',
  ];
}

function formatManifestSkippedReviewers(
  skippedReviewers: NonNullable<Session['deepReviewRunManifest']>['skippedReviewers'],
): string[] {
  return skippedReviewers.map((reviewer) => {
    const reviewerName = reviewer.subagentId || reviewer.displayName;
    const reason = reviewer.reason ?? 'unknown';
    return `- ${reviewerName}: skipped (${reason})`;
  });
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
  const resultStatus = String(item.toolResult?.result?.status ?? '').trim();
  const partialOutput = getPartialOutput(item);
  let status: DeepReviewReviewerStatus = 'unknown';
  if (resultStatus === 'partial_timeout' || /partial[_ -]?timeout/i.test(error ?? '')) {
    status = 'partial_timeout';
  } else if (item.toolResult?.success === true || item.status === 'completed') {
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
    partialOutput,
  };
}

function isPolicyIneligibleReviewerError(error?: string): boolean {
  if (!error) {
    return false;
  }
  return /DeepReview Task policy violation|deep_review_subagent_(?:not_review|not_allowed|not_readonly)/i.test(error);
}

function getPartialOutput(item: FlowToolItem): string | undefined {
  const result = item.toolResult?.result;
  const value = result?.partial_output ?? result?.partialOutput;
  return typeof value === 'string' && value.trim() ? value.trim() : undefined;
}
