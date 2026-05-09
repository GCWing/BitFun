/**
 * Deep Review experience utilities.
 *
 * Aggregates raw session/tool state into user-friendly experience data
 * such as reviewer progress, error attribution, partial results, and
 * degradation options. All functions are pure and side-effect free.
 */

import { getAiErrorPresentation } from '@/shared/ai-errors/aiErrorPresenter';
import type { Session } from '../types/flow-chat';
import type { CodeReviewRemediationData } from './codeReviewRemediation';
import type { DeepReviewInterruption, DeepReviewReviewerProgress } from './deepReviewContinuation';
import { collectReviewerProgress } from './deepReviewContinuation';

// ---------------------------------------------------------------------------
// Reviewer progress
// ---------------------------------------------------------------------------

export interface ReviewerProgressItem extends DeepReviewReviewerProgress {
  /** Human-readable display name */
  displayName: string;
}

export interface ReviewerProgressSummary {
  completed: number;
  failed: number;
  timedOut: number;
  running: number;
  skipped: number;
  unknown: number;
  total: number;
  /** Localised short text, e.g. "3/5 completed" */
  text: string;
}

/**
 * Aggregate reviewer progress from a live session.
 * Reuses the existing `collectReviewerProgress` logic.
 */
export function aggregateReviewerProgress(
  session: Session,
): ReviewerProgressItem[] {
  const progress = collectReviewerProgress(session);
  return progress.map((p) => ({
    ...p,
    displayName: p.reviewer,
  }));
}

export function buildReviewerProgressSummary(
  progress: ReviewerProgressItem[],
): ReviewerProgressSummary {
  const completed = progress.filter((p) => p.status === 'completed').length;
  const failed = progress.filter((p) => p.status === 'failed').length;
  const timedOut = progress.filter((p) => p.status === 'timed_out').length;
  const running = progress.filter((p) => p.status === 'unknown').length;
  const skipped = progress.filter((p) => p.status === 'cancelled').length;
  const unknown = progress.filter((p) => p.status === 'unknown').length;
  const total = progress.length;

  return {
    completed,
    failed,
    timedOut,
    running,
    skipped,
    unknown,
    total,
    text: `${completed}/${total} completed`,
  };
}

// ---------------------------------------------------------------------------
// Partial results extraction
// ---------------------------------------------------------------------------

export interface PartialReviewData {
  /** Whether any reviewer completed successfully */
  hasPartialResults: boolean;
  /** Number of completed reviewers */
  completedReviewerCount: number;
  /** Total reviewer count */
  totalReviewerCount: number;
  /** Issues found by completed reviewers */
  completedIssues: NonNullable<CodeReviewRemediationData['issues']>;
  /** Remediation items from completed reviewers */
  completedRemediationItems: string[];
  /** Summaries from completed reviewers */
  completedReviewerSummaries: string[];
}

/**
 * Extract partial review data from a session that may have been
 * interrupted before all reviewers finished.
 */
export function extractPartialReviewData(
  session: Session,
): PartialReviewData | null {
  const progress = collectReviewerProgress(session);
  const completedReviewers = progress.filter((p) => p.status === 'completed');

  if (completedReviewers.length === 0) {
    return null;
  }

  const completedIssues: NonNullable<CodeReviewRemediationData['issues']> = [];
  const completedRemediationItems: string[] = [];
  const completedReviewerSummaries: string[] = [];

  for (const turn of session.dialogTurns) {
    for (const round of turn.modelRounds) {
      for (const item of round.items) {
        if (item.type !== 'tool' || item.toolName !== 'Task') {
          continue;
        }
        const reviewer = String(
          (item.toolCall.input as Record<string, unknown>)?.subagent_type ??
            (item.toolCall.input as Record<string, unknown>)?.subagentType ??
            '',
        ).trim();

        const isCompleted = completedReviewers.some(
          (p) => p.reviewer === reviewer,
        );
        if (!isCompleted || !item.toolResult?.success) {
          continue;
        }

        // Try to parse the tool result for review data.
        const result = item.toolResult.result;
        if (typeof result === 'string') {
          try {
            const parsed = JSON.parse(result) as Record<string, unknown>;
            if (parsed.issues && Array.isArray(parsed.issues)) {
              const issues = parsed.issues as NonNullable<CodeReviewRemediationData['issues']>;
              completedIssues.push(...issues);
            }
            if (parsed.remediation_plan && Array.isArray(parsed.remediation_plan)) {
              completedRemediationItems.push(...(parsed.remediation_plan as string[]));
            }
            if (parsed.summary) {
              completedReviewerSummaries.push(String(parsed.summary));
            }
          } catch {
            // Not JSON, treat as plain text summary
            completedReviewerSummaries.push(result);
          }
        }
      }
    }
  }

  return {
    hasPartialResults: true,
    completedReviewerCount: completedReviewers.length,
    totalReviewerCount: progress.length,
    completedIssues,
    completedRemediationItems,
    completedReviewerSummaries,
  };
}

// ---------------------------------------------------------------------------
// Error attribution
// ---------------------------------------------------------------------------

export interface ErrorAttribution {
  category: string;
  title: string;
  description: string;
  severity: 'warning' | 'error';
  actions: Array<{ code: string; labelKey: string }>;
}

/**
 * Build a user-friendly error attribution from an interruption.
 * Leverages the existing `getAiErrorPresentation` system.
 */
export function buildErrorAttribution(
  interruption: DeepReviewInterruption,
): ErrorAttribution {
  const presentation = getAiErrorPresentation(interruption.errorDetail);

  return {
    category: presentation.category,
    title: presentation.titleKey,
    description: presentation.messageKey,
    severity: presentation.severity,
    actions: presentation.actions.map((a) => ({
      code: a.code,
      labelKey: a.labelKey,
    })),
  };
}

// ---------------------------------------------------------------------------
// Recovery plan
// ---------------------------------------------------------------------------

export interface RecoveryPlan {
  willRerun: string[];
  willPreserve: string[];
  willSkip: string[];
  summaryText: string;
}

/**
 * Build a recovery plan that describes what will happen when the user
 * chooses to continue an interrupted deep review.
 */
export function buildRecoveryPlan(
  interruption: DeepReviewInterruption,
): RecoveryPlan {
  const reviewers = interruption.reviewers;

  const willPreserve = reviewers
    .filter((r) => r.status === 'completed')
    .map((r) => r.reviewer);

  const willRerun = reviewers
    .filter(
      (r) =>
        r.status === 'failed' ||
        r.status === 'timed_out' ||
        r.status === 'cancelled' ||
        r.status === 'unknown',
    )
    .map((r) => r.reviewer);

  const willSkip = reviewers
    .filter((r) => r.status === 'skipped')
    .map((r) => r.reviewer);

  const parts: string[] = [];
  if (willPreserve.length > 0) {
    parts.push(`${willPreserve.length} completed reviewers will be preserved`);
  }
  if (willRerun.length > 0) {
    parts.push(`${willRerun.length} reviewers will be rerun`);
  }
  if (willSkip.length > 0) {
    parts.push(`${willSkip.length} reviewers will be skipped`);
  }

  return {
    willRerun,
    willPreserve,
    willSkip,
    summaryText: parts.join('; ') || 'No recovery plan available.',
  };
}

// ---------------------------------------------------------------------------
// Degradation options
// ---------------------------------------------------------------------------

export interface DegradationOption {
  type: 'reduce_reviewers' | 'compress_context' | 'view_partial';
  labelKey: string;
  descriptionKey: string;
  enabled: boolean;
}

/**
 * Evaluate available degradation options when a deep review fails
 * (especially for context_overflow).
 */
export function evaluateDegradationOptions(
  interruption: DeepReviewInterruption,
): DegradationOption[] {
  const hasPartialResults = interruption.reviewers.some(
    (r) => r.status === 'completed',
  );

  return [
    {
      type: 'reduce_reviewers',
      labelKey: 'deepReviewActionBar.degradation.reduceReviewers',
      descriptionKey: 'deepReviewActionBar.degradation.reduceReviewersDesc',
      enabled: false, // Requires backend support
    },
    {
      type: 'compress_context',
      labelKey: 'deepReviewActionBar.degradation.compressContext',
      descriptionKey: 'deepReviewActionBar.degradation.compressContextDesc',
      enabled: false, // Requires backend support
    },
    {
      type: 'view_partial',
      labelKey: 'deepReviewActionBar.degradation.viewPartial',
      descriptionKey: 'deepReviewActionBar.degradation.viewPartialDesc',
      enabled: hasPartialResults,
    },
  ];
}

// ---------------------------------------------------------------------------
// Token estimation
// ---------------------------------------------------------------------------

export interface TokenEstimate {
  min: number;
  max: number;
}

const BASE_TOKENS = 5000;
const PER_REVIEWER_MIN = 8000;
const PER_REVIEWER_MAX = 25000;

/**
 * Rough token consumption estimate for a deep review run.
 */
export function estimateTokenConsumption(
  reviewerCount: number,
): TokenEstimate {
  return {
    min: BASE_TOKENS + reviewerCount * PER_REVIEWER_MIN,
    max: BASE_TOKENS + reviewerCount * PER_REVIEWER_MAX,
  };
}

export function formatTokenCount(count: number): string {
  if (count >= 1000000) {
    return `${(count / 1000000).toFixed(1)}M`;
  }
  if (count >= 1000) {
    return `${(count / 1000).toFixed(0)}k`;
  }
  return String(count);
}

// ---------------------------------------------------------------------------
// Launch error classification
// ---------------------------------------------------------------------------

export interface LaunchErrorInfo {
  step: string;
  category: 'model_config' | 'network' | 'unknown';
  messageKey: string;
  actions: Array<'retry' | 'open_model_settings'>;
}

/**
 * Classify a launch failure into a user-friendly error description.
 */
export function classifyLaunchError(
  step: string,
  error: unknown,
): LaunchErrorInfo {
  const message = error instanceof Error ? error.message : String(error);
  const lower = message.toLowerCase();

  if (step === 'create_child_session') {
    if (/model|provider|api key|authentication|unauthorized/i.test(lower)) {
      return {
        step,
        category: 'model_config',
        messageKey: 'deepReviewActionBar.launchError.modelConfig',
        actions: ['open_model_settings', 'retry'],
      };
    }
    return {
      step,
      category: 'unknown',
      messageKey: 'deepReviewActionBar.launchError.unknown',
      actions: ['retry'],
    };
  }

  if (step === 'send_start_message') {
    if (/network|timeout|connection|sse|stream/i.test(lower)) {
      return {
        step,
        category: 'network',
        messageKey: 'deepReviewActionBar.launchError.network',
        actions: ['retry'],
      };
    }
    return {
      step,
      category: 'unknown',
      messageKey: 'deepReviewActionBar.launchError.unknown',
      actions: ['retry'],
    };
  }

  return {
    step,
    category: 'unknown',
    messageKey: 'deepReviewActionBar.launchError.unknown',
    actions: ['retry'],
  };
}
