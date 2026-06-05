import { describe, expect, it } from 'vitest';
import { classifyReviewTargetFromFiles } from '../reviewTargetClassifier';
import { recommendBackendCompatibleStrategyForTarget } from './risk';

describe('review-team risk recommendation', () => {
  it('counts same-layer layered crate changes as cross-crate changes', () => {
    const target = classifyReviewTargetFromFiles(
      [
        'src/crates/execution/agent-runtime/src/lib.rs',
        'src/crates/execution/agent-tools/src/lib.rs',
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
