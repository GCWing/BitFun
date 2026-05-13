import type { ReviewTargetClassification } from '../reviewTargetClassifier';
import {
  PROMPT_BYTE_ESTIMATE_BASE_BYTES,
  PROMPT_BYTE_ESTIMATE_PER_CHANGED_LINE_BYTES,
  PROMPT_BYTE_ESTIMATE_PER_FILE_BYTES,
  PROMPT_BYTE_ESTIMATE_UNKNOWN_LINES_PER_FILE,
  REVIEW_STRATEGY_RUNTIME_BUDGETS,
  TOKEN_BUDGET_PROMPT_BYTE_LIMIT_BY_MODE,
} from './defaults';
import type {
  ReviewStrategyLevel,
  ReviewTeamChangeStats,
  ReviewTeamExecutionPolicy,
  ReviewTeamTokenBudgetDecision,
  ReviewTeamTokenBudgetPlan,
  ReviewTeamWorkPacket,
  ReviewTeamWorkPacketScope,
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

function estimateChangedLinesForScope(params: {
  scope: ReviewTeamWorkPacketScope;
  changeStats: ReviewTeamChangeStats;
  totalIncludedFileCount: number;
}): number {
  if (params.changeStats.totalLinesChanged === undefined) {
    return params.scope.fileCount * PROMPT_BYTE_ESTIMATE_UNKNOWN_LINES_PER_FILE;
  }

  if (params.totalIncludedFileCount <= 0) {
    return params.changeStats.totalLinesChanged;
  }

  return Math.ceil(
    params.changeStats.totalLinesChanged *
      (params.scope.fileCount / params.totalIncludedFileCount),
  );
}

function estimateReviewerPromptBytes(params: {
  packet: ReviewTeamWorkPacket;
  changeStats: ReviewTeamChangeStats;
  totalIncludedFileCount: number;
}): number {
  const pathBytes = params.packet.assignedScope.files.reduce(
    (total, filePath) => total + filePath.length + 1,
    0,
  );
  const estimatedChangedLines = estimateChangedLinesForScope({
    scope: params.packet.assignedScope,
    changeStats: params.changeStats,
    totalIncludedFileCount: params.totalIncludedFileCount,
  });

  return Math.ceil(
    PROMPT_BYTE_ESTIMATE_BASE_BYTES +
      pathBytes +
      params.packet.assignedScope.fileCount * PROMPT_BYTE_ESTIMATE_PER_FILE_BYTES +
      estimatedChangedLines * PROMPT_BYTE_ESTIMATE_PER_CHANGED_LINE_BYTES,
  );
}

function estimateMaxReviewerPromptBytes(params: {
  workPackets: ReviewTeamWorkPacket[];
  target: ReviewTargetClassification;
  changeStats: ReviewTeamChangeStats;
}): number {
  const reviewerPackets = params.workPackets.filter(
    (packet) => packet.phase === 'reviewer',
  );
  const totalIncludedFileCount = params.target.files.filter(
    (file) => !file.excluded,
  ).length;

  if (reviewerPackets.length === 0) {
    return PROMPT_BYTE_ESTIMATE_BASE_BYTES;
  }

  return Math.max(
    ...reviewerPackets.map((packet) =>
      estimateReviewerPromptBytes({
        packet,
        changeStats: params.changeStats,
        totalIncludedFileCount,
      }),
    ),
  );
}

export function buildTokenBudgetPlan(params: {
  mode: ReviewTokenBudgetMode;
  activeReviewerCalls: number;
  eligibleExtraReviewerCount: number;
  maxExtraReviewers: number;
  skippedReviewerIds: string[];
  target: ReviewTargetClassification;
  changeStats: ReviewTeamChangeStats;
  executionPolicy: ReviewTeamExecutionPolicy;
  workPackets: ReviewTeamWorkPacket[];
}): ReviewTeamTokenBudgetPlan {
  const includedFileCount = params.target.files.filter(
    (file) => !file.excluded,
  ).length;
  const fileSplitGuardrailActive =
    params.executionPolicy.reviewerFileSplitThreshold > 0 &&
    includedFileCount > params.executionPolicy.reviewerFileSplitThreshold;
  const maxPromptBytesPerReviewer =
    TOKEN_BUDGET_PROMPT_BYTE_LIMIT_BY_MODE[params.mode];
  const estimatedPromptBytesPerReviewer = estimateMaxReviewerPromptBytes({
    workPackets: params.workPackets,
    target: params.target,
    changeStats: params.changeStats,
  });
  const promptByteLimitExceeded =
    estimatedPromptBytesPerReviewer > maxPromptBytesPerReviewer;
  const largeDiffSummaryFirst = promptByteLimitExceeded;
  const decisions: ReviewTeamTokenBudgetDecision[] = [];
  const warnings: string[] = [];

  if (promptByteLimitExceeded) {
    decisions.push({
      kind: 'summary_first_full_scope',
      reason: 'prompt_bytes_exceeded',
      detail:
        `Estimated reviewer prompt ${estimatedPromptBytesPerReviewer} bytes exceeds ${maxPromptBytesPerReviewer} bytes for ${params.mode} budget; use summary-first while keeping every assigned_scope file visible.`,
    });
    warnings.push(
      'Estimated reviewer prompt exceeds the selected token budget; use summary-first without hiding assigned files.',
    );
  }

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
    maxReviewerCalls:
      params.activeReviewerCalls +
      Math.max(0, params.eligibleExtraReviewerCount - params.maxExtraReviewers),
    maxExtraReviewers: params.maxExtraReviewers,
    ...(fileSplitGuardrailActive
      ? { maxFilesPerReviewer: params.executionPolicy.reviewerFileSplitThreshold }
      : {}),
    maxPromptBytesPerReviewer,
    estimatedPromptBytesPerReviewer,
    promptByteEstimateSource: 'manifest_heuristic',
    promptByteLimitExceeded,
    largeDiffSummaryFirst,
    decisions,
    skippedReviewerIds: params.skippedReviewerIds,
    warnings,
  };
}
