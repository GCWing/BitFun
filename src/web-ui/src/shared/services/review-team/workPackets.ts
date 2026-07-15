import type { ReviewTargetClassification } from '../reviewTargetClassifier';
import type {
  ReviewTeamChangeStats,
  ReviewTokenBudgetMode,
} from './types';

// Legacy manifests may still contain work packets, but new strict reviews do
// not pre-schedule reviewer calls. This module now retains only the small
// normalization helpers used while building a launch manifest.
export function resolveMaxExtraReviewers(
  mode: ReviewTokenBudgetMode,
  eligibleExtraReviewerCount: number,
  strategyMaxExtraReviewers = Number.MAX_SAFE_INTEGER,
): number {
  if (mode === 'economy') {
    return 0;
  }
  return Math.min(eligibleExtraReviewerCount, strategyMaxExtraReviewers);
}

export function resolveChangeStats(
  target: ReviewTargetClassification,
  stats?: Partial<ReviewTeamChangeStats>,
): ReviewTeamChangeStats {
  const fileCount = Math.max(
    0,
    Math.floor(
      stats?.fileCount ??
        target.files.filter((file) => !file.excluded).length,
    ),
  );
  const totalLinesChanged =
    typeof stats?.totalLinesChanged === 'number' &&
    Number.isFinite(stats.totalLinesChanged)
      ? Math.max(0, Math.floor(stats.totalLinesChanged))
      : undefined;

  return {
    fileCount,
    ...(totalLinesChanged !== undefined ? { totalLinesChanged } : {}),
    lineCountSource:
      totalLinesChanged !== undefined
        ? stats?.lineCountSource ?? 'diff_stat'
        : 'unknown',
  };
}
