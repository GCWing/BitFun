import { describe, expect, it } from 'vitest';
import { workspaceAreaForReviewPath } from './pathMetadata';

describe('review-team path metadata', () => {
  it('uses the real crate segment for layered src/crates paths', () => {
    expect(workspaceAreaForReviewPath(
      'src/crates/execution/agent-runtime/src/lib.rs',
    )).toBe('crate:agent-runtime');
    expect(workspaceAreaForReviewPath(
      'src/crates/execution/tool-contracts/src/lib.rs',
    )).toBe('crate:tool-contracts');
    expect(workspaceAreaForReviewPath(
      'src/crates/assembly/core/src/lib.rs',
    )).toBe('crate:core');
    expect(workspaceAreaForReviewPath(
      'src/crates/services/services-core/src/lib.rs',
    )).toBe('crate:services-core');
    expect(workspaceAreaForReviewPath(
      'src/crates/adapters/api-layer/src/lib.rs',
    )).toBe('crate:api-layer');
    expect(workspaceAreaForReviewPath(
      'src/crates/interfaces/acp/src/lib.rs',
    )).toBe('crate:acp');
    expect(workspaceAreaForReviewPath(
      'src/crates/contracts/product-domains/src/lib.rs',
    )).toBe('crate:product-domains');
  });
});
