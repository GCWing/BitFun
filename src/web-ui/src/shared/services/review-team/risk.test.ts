import { describe, expect, it } from 'vitest';
import { classifyReviewTargetFromFiles } from '../reviewTargetClassifier';
import { recommendBackendCompatibleStrategyForTarget } from './risk';

describe('review-team risk recommendation', () => {
  it('counts same-layer layered crate changes as cross-crate changes', () => {
    const target = classifyReviewTargetFromFiles(
      [
        'src/crates/execution/agent-runtime/src/lib.rs',
        'src/crates/execution/tool-contracts/src/lib.rs',
      ],
      'workspace_diff',
    );

    const recommendation = recommendBackendCompatibleStrategyForTarget(target, {
      fileCount: 2,
      totalLinesChanged: 40,
      lineCountSource: 'diff_stat',
    });

    expect(recommendation.factors.crossCrateChanges).toBe(1);
    expect(recommendation.score).toBe(4);
  });

  it('does not collapse service and adapter crates into one risk area', () => {
    const target = classifyReviewTargetFromFiles(
      [
        'src/crates/services/services-core/src/lib.rs',
        'src/crates/adapters/api-layer/src/lib.rs',
      ],
      'workspace_diff',
    );

    const recommendation = recommendBackendCompatibleStrategyForTarget(target, {
      fileCount: 2,
      totalLinesChanged: 40,
      lineCountSource: 'diff_stat',
    });

    expect(recommendation.factors.crossCrateChanges).toBe(1);
    expect(recommendation.score).toBe(4);
  });
});
