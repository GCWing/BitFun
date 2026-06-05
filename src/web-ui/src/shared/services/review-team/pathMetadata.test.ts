import { describe, expect, it } from 'vitest';
import { workspaceAreaForReviewPath } from './pathMetadata';

describe('review-team path metadata', () => {
  it('uses the real crate segment for layered src/crates paths', () => {
    expect(workspaceAreaForReviewPath(
      'src/crates/execution/agent-runtime/src/lib.rs',
    )).toBe('crate:agent-runtime');
    expect(workspaceAreaForReviewPath(
      'src/crates/execution/agent-tools/src/lib.rs',
    )).toBe('crate:agent-tools');
    expect(workspaceAreaForReviewPath(
      'src/crates/facade/core/src/lib.rs',
    )).toBe('crate:core');
  });
});
