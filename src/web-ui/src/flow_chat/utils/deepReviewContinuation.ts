import type { AiErrorAction, AiErrorDetail } from '@/shared/ai-errors/aiErrorPresenter';
import {
  getAiErrorPresentation,
  normalizeAiErrorDetail,
} from '@/shared/ai-errors/aiErrorPresenter';
import type { FlowToolItem, Session } from '../types/flow-chat';

export type DeepReviewContinuationPhase = 'review_interrupted' | 'resume_blocked';
export type DeepReviewReviewerStatus =
  | 'completed'
  | 'partial_timeout'
  | 'timed_out'
  | 'failed'
  | 'cancelled'
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
  const hasFailure = lastTurn?.status === 'error' || Boolean(session.error);
  if (!hasFailure) {
    return null;
  }

  const normalizedError = normalizeAiErrorDetail(errorDetail, session.error ?? lastTurn?.error ?? '');
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
  };
}

export function buildDeepReviewContinuationPrompt(interruption: DeepReviewInterruption): string {
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
  const retryBudgetRules = formatRetryBudgetRules(interruption.runManifest);
  const incrementalCacheBlock = formatIncrementalReviewCacheGuidance(
    interruption.runManifest,
  );

  return [
    'Continue the interrupted Deep Review in this same session.',
    '',
    'Recovery rules:',
    '- Do not restart completed reviewer work unless the existing result is clearly incomplete or unusable.',
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
    '',
    'Last error:',
    `- category: ${interruption.errorDetail.category ?? 'unknown'}`,
    interruption.errorDetail.providerCode ? `- provider code: ${interruption.errorDetail.providerCode}` : '- provider code: unknown',
    interruption.errorDetail.requestId ? `- request id: ${interruption.errorDetail.requestId}` : '- request id: unknown',
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

function getPartialOutput(item: FlowToolItem): string | undefined {
  const result = item.toolResult?.result;
  const value = result?.partial_output ?? result?.partialOutput;
  return typeof value === 'string' && value.trim() ? value.trim() : undefined;
}
