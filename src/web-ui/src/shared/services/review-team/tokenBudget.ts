import type { ReviewTargetClassification } from '../reviewTargetClassifier';
import {
  REVIEW_STRATEGY_RUNTIME_BUDGETS,
} from './defaults';
import type {
  ReviewStrategyLevel,
  ReviewTeamChangeStats,
  ReviewTeamExecutionPolicy,
  ReviewTeamTokenBudgetDecision,
  ReviewTeamTokenBudgetPlan,
  ReviewTeamWorkPacket,
  ReviewTokenBudgetMode,
} from './types';

function strategyBoundedTimeoutSeconds(
  configuredTimeoutSeconds: number,
  strategyTimeoutSeconds: number,
): number {
  if (configuredTimeoutSeconds === 0) {
    return 0;
  }
  return Math.min(configuredTimeoutSeconds, strategyTimeoutSeconds);
}

function strategyBoundedSplitThreshold(
  configuredThreshold: number,
  strategyThreshold: number,
): number {
  if (configuredThreshold === 0 || strategyThreshold === 0) {
    return 0;
  }
  return Math.min(configuredThreshold, strategyThreshold);
}

export function buildEffectiveExecutionPolicy(params: {
  basePolicy: ReviewTeamExecutionPolicy;
  strategyLevel: ReviewStrategyLevel;
  target: ReviewTargetClassification;
  changeStats: ReviewTeamChangeStats;
}): ReviewTeamExecutionPolicy {
  if (
    params.target.resolution === 'unknown' &&
    params.changeStats.fileCount === 0 &&
    params.changeStats.totalLinesChanged === undefined
  ) {
    return params.basePolicy;
  }

  const strategyBudget = REVIEW_STRATEGY_RUNTIME_BUDGETS[params.strategyLevel];

  return {
    ...params.basePolicy,
    reviewerTimeoutSeconds: strategyBoundedTimeoutSeconds(
      params.basePolicy.reviewerTimeoutSeconds,
      strategyBudget.executionPolicy.reviewerTimeoutSeconds,
    ),
    judgeTimeoutSeconds: strategyBoundedTimeoutSeconds(
      params.basePolicy.judgeTimeoutSeconds,
      strategyBudget.executionPolicy.judgeTimeoutSeconds,
    ),
    reviewerFileSplitThreshold: strategyBoundedSplitThreshold(
      params.basePolicy.reviewerFileSplitThreshold,
      strategyBudget.executionPolicy.reviewerFileSplitThreshold,
    ),
    maxSameRoleInstances: Math.min(
      params.basePolicy.maxSameRoleInstances,
      strategyBudget.executionPolicy.maxSameRoleInstances,
    ),
  };
}

export function buildTokenBudgetPlan(params: {
  mode: ReviewTokenBudgetMode;
  activeReviewerCalls: number;
  maxReviewerCalls?: number;
  eligibleExtraReviewerCount: number;
  maxExtraReviewers: number;
  skippedReviewerIds: string[];
  target: ReviewTargetClassification;
  changeStats: ReviewTeamChangeStats;
  executionPolicy: ReviewTeamExecutionPolicy;
  /** Retained at the call boundary for compatibility; no prompt-size estimate is derived. */
  workPackets?: ReviewTeamWorkPacket[];
}): ReviewTeamTokenBudgetPlan {
  const includedFileCount = params.target.files.filter(
    (file) => !file.excluded,
  ).length;
  const fileSplitGuardrailActive =
    params.executionPolicy.reviewerFileSplitThreshold > 0 &&
    includedFileCount > params.executionPolicy.reviewerFileSplitThreshold;
  const decisions: ReviewTeamTokenBudgetDecision[] = [];
  const warnings: string[] = [];

  if (params.skippedReviewerIds.length > 0) {
    decisions.push({
      kind: 'skip_extra_reviewers',
      reason: 'extra_reviewers_skipped',
      detail:
        'Some extra reviewers were skipped by the selected token budget mode.',
      affectedReviewerIds: [...params.skippedReviewerIds],
    });
    warnings.push(
      'Some extra reviewers were skipped by the selected token budget mode.',
    );
  }

  return {
    mode: params.mode,
    estimatedReviewerCalls: params.activeReviewerCalls,
    maxReviewerCalls: params.maxReviewerCalls ?? params.activeReviewerCalls,
    maxExtraReviewers: params.maxExtraReviewers,
    ...(fileSplitGuardrailActive
      ? { maxFilesPerReviewer: params.executionPolicy.reviewerFileSplitThreshold }
      : {}),
    largeDiffSummaryFirst: false,
    decisions,
    skippedReviewerIds: params.skippedReviewerIds,
    warnings,
  };
}
