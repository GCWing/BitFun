import type { ReviewTargetClassification } from '../reviewTargetClassifier';
import type {
  ReviewStrategyLevel,
  ReviewTeamBackendRiskFactors,
  ReviewTeamBackendStrategyRecommendation,
  ReviewTeamChangeStats,
  ReviewTeamRiskFactors,
  ReviewTeamStrategyDecision,
  ReviewTeamStrategyMismatchSeverity,
  ReviewTeamStrategyRecommendation,
} from './types';
import {
  isSecuritySensitiveReviewPath,
  pluralize,
  workspaceAreaForReviewPath,
} from './pathMetadata';

export function recommendReviewStrategyForTarget(
  target: ReviewTargetClassification,
  changeStats: ReviewTeamChangeStats,
): ReviewTeamStrategyRecommendation {
  const includedFiles = target.files.filter((file) => !file.excluded);
  const securityFileCount = includedFiles.filter((file) =>
    isSecuritySensitiveReviewPath(file.normalizedPath),
  ).length;
  const workspaceAreaCount = new Set(
    includedFiles.map((file) => workspaceAreaForReviewPath(file.normalizedPath)),
  ).size;
  const contractSurfaceChanged = target.tags.includes('frontend_contract') ||
    target.tags.includes('desktop_contract') ||
    target.tags.includes('web_server_contract') ||
    target.tags.includes('api_layer') ||
    target.tags.includes('transport');
  const totalLinesChanged = changeStats.totalLinesChanged;
  const factors: ReviewTeamRiskFactors = {
    fileCount: changeStats.fileCount,
    ...(totalLinesChanged !== undefined ? { totalLinesChanged } : {}),
    lineCountSource: changeStats.lineCountSource,
    securityFileCount,
    workspaceAreaCount,
    contractSurfaceChanged,
  };

  if (target.resolution === 'unknown' || changeStats.fileCount === 0) {
    return {
      strategyLevel: 'normal',
      score: 0,
      rationale: 'unresolved target; keep a conservative normal review recommendation.',
      factors,
    };
  }

  const lineScore =
    totalLinesChanged === undefined
      ? 0
      : Math.floor(totalLinesChanged / 100);
  const crossAreaScore = Math.max(0, workspaceAreaCount - 1) * 2;
  const score =
    changeStats.fileCount +
    lineScore +
    securityFileCount * 3 +
    crossAreaScore +
    (contractSurfaceChanged ? 2 : 0);
  const strategyLevel: ReviewStrategyLevel =
    score <= 5
      ? 'quick'
      : score <= 20
        ? 'normal'
        : 'deep';
  const sizeLabel = totalLinesChanged === undefined
    ? `${changeStats.fileCount} files, unknown lines`
    : `${changeStats.fileCount} files, ${totalLinesChanged} lines`;
  const riskDetails = [
    pluralize(securityFileCount, 'security-sensitive file'),
    pluralize(workspaceAreaCount, 'workspace area'),
    contractSurfaceChanged ? 'contract surface changed' : undefined,
  ].filter(Boolean).join(', ');
  const rationale =
    strategyLevel === 'quick'
      ? `Small change (${sizeLabel}). Quick scan sufficient.`
      : strategyLevel === 'normal'
        ? `Medium change (${sizeLabel}; ${riskDetails}). Standard review recommended.`
        : `Large/high-risk change (${sizeLabel}; ${riskDetails}). Deep review recommended.`;

  return {
    strategyLevel,
    score,
    rationale,
    factors,
  };
}

const REVIEW_STRATEGY_RANK: Record<ReviewStrategyLevel, number> = {
  quick: 0,
  normal: 1,
  deep: 2,
};

function crossCrateChangeCountForReviewTarget(
  target: ReviewTargetClassification,
): number {
  const crateNames = new Set(
    target.files
      .filter((file) => !file.excluded)
      .map((file) => /^src\/crates\/([^/]+)/.exec(file.normalizedPath)?.[1])
      .filter((crateName): crateName is string => Boolean(crateName)),
  );

  return Math.max(0, crateNames.size - 1);
}

function buildBackendCompatibleRiskFactors(
  target: ReviewTargetClassification,
  changeStats: ReviewTeamChangeStats,
): ReviewTeamBackendRiskFactors {
  const includedFiles = target.files.filter((file) => !file.excluded);

  return {
    fileCount: changeStats.fileCount,
    totalLinesChanged: changeStats.totalLinesChanged ?? 0,
    lineCountSource: changeStats.lineCountSource,
    filesInSecurityPaths: includedFiles.filter((file) =>
      isSecuritySensitiveReviewPath(file.normalizedPath),
    ).length,
    crossCrateChanges: crossCrateChangeCountForReviewTarget(target),
    maxCyclomaticComplexityDelta: 0,
    maxCyclomaticComplexityDeltaSource: 'not_measured',
  };
}

export function recommendBackendCompatibleStrategyForTarget(
  target: ReviewTargetClassification,
  changeStats: ReviewTeamChangeStats,
): ReviewTeamBackendStrategyRecommendation {
  const factors = buildBackendCompatibleRiskFactors(target, changeStats);
  const score =
    factors.fileCount +
    Math.floor(factors.totalLinesChanged / 100) +
    factors.filesInSecurityPaths * 3 +
    factors.crossCrateChanges * 2;
  const strategyLevel: ReviewStrategyLevel =
    score <= 5
      ? 'quick'
      : score <= 20
        ? 'normal'
        : 'deep';
  const rationale =
    strategyLevel === 'quick'
      ? `Backend-compatible policy sees a small change (${factors.fileCount} files, ${factors.totalLinesChanged} lines).`
      : strategyLevel === 'normal'
        ? `Backend-compatible policy sees a medium change (${factors.fileCount} files, ${factors.totalLinesChanged} lines).`
        : `Backend-compatible policy sees a large/high-risk change (${factors.fileCount} files, ${factors.totalLinesChanged} lines, ${factors.filesInSecurityPaths} security files).`;

  return {
    strategyLevel,
    score,
    rationale,
    factors,
  };
}

function resolveStrategyMismatchSeverity(params: {
  finalStrategy: ReviewStrategyLevel;
  frontendRecommendation: ReviewStrategyLevel;
  backendRecommendation: ReviewStrategyLevel;
}): ReviewTeamStrategyMismatchSeverity {
  const finalRank = REVIEW_STRATEGY_RANK[params.finalStrategy];
  const recommendedRank = Math.max(
    REVIEW_STRATEGY_RANK[params.frontendRecommendation],
    REVIEW_STRATEGY_RANK[params.backendRecommendation],
  );
  const distance = Math.abs(finalRank - recommendedRank);

  if (distance === 0) {
    return 'none';
  }
  if (distance >= 2) {
    return 'high';
  }
  return finalRank < recommendedRank ? 'medium' : 'low';
}

export function buildReviewStrategyDecision(params: {
  teamDefaultStrategy: ReviewStrategyLevel;
  finalStrategy: ReviewStrategyLevel;
  userOverride?: ReviewStrategyLevel;
  frontendRecommendation: ReviewTeamStrategyRecommendation;
  backendRecommendation: ReviewTeamBackendStrategyRecommendation;
}): ReviewTeamStrategyDecision {
  const mismatch =
    params.finalStrategy !== params.frontendRecommendation.strategyLevel ||
    params.finalStrategy !== params.backendRecommendation.strategyLevel;
  const mismatchSeverity = resolveStrategyMismatchSeverity({
    finalStrategy: params.finalStrategy,
    frontendRecommendation: params.frontendRecommendation.strategyLevel,
    backendRecommendation: params.backendRecommendation.strategyLevel,
  });
  const recommendationSummary = [
    `frontend=${params.frontendRecommendation.strategyLevel}`,
    `backend=${params.backendRecommendation.strategyLevel}`,
  ].join(', ');

  return {
    authority: 'mismatch_warning',
    teamDefaultStrategy: params.teamDefaultStrategy,
    ...(params.userOverride ? { userOverride: params.userOverride } : {}),
    finalStrategy: params.finalStrategy,
    frontendRecommendation: params.frontendRecommendation,
    backendRecommendation: params.backendRecommendation,
    mismatch,
    mismatchSeverity,
    rationale: mismatch
      ? `Final strategy ${params.finalStrategy} differs from advisory recommendations (${recommendationSummary}); keep this as non-blocking launch/report metadata.`
      : `Final strategy ${params.finalStrategy} matches advisory recommendations (${recommendationSummary}).`,
  };
}
