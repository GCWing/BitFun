import { describe, expect, it } from 'vitest';
import type { Session } from '@/flow_chat/types/flow-chat';
import {
  pullRequestReviewFreshness,
  samePullRequestIdentity,
} from './ReviewPlatformPanel';

const baseRevision = '1'.repeat(40);
const headRevision = '2'.repeat(40);

function evidence(
  overrides: Partial<NonNullable<Session['reviewTargetEvidence']>> = {},
): NonNullable<Session['reviewTargetEvidence']> {
  return {
    version: 1,
    source: 'pull_request',
    fingerprint: 'review-target-fingerprint',
    baseRevision,
    headRevision,
    completeness: 'complete',
    workspaceBinding: 'unavailable',
    files: [],
    limitations: [],
    omittedFileCount: 0,
    ...overrides,
  };
}

describe('pull request Review linking', () => {
  it('associates the provider PR independently of the local remote id', () => {
    expect(samePullRequestIdentity({
      remoteId: 'old-origin-name',
      platform: 'github',
      host: 'HTTPS://GitHub.com/',
      projectPath: '/GCWing/BitFun/',
      pullRequestId: '1502',
      number: 1502,
      webUrl: 'https://github.com/GCWing/BitFun/pull/1502',
    }, {
      platform: 'github',
      host: 'github.com',
      projectPath: 'gcwing/bitfun',
      pullRequestId: '1502',
    })).toBe(true);
  });

  it('requires exact full revisions before treating a result as current', () => {
    expect(pullRequestReviewFreshness(evidence(), {
      baseRevision,
      headRevision,
    })).toBe('current');
    expect(pullRequestReviewFreshness(evidence(), {
      baseRevision,
      headRevision: '3'.repeat(40),
    })).toBe('stale');
    expect(pullRequestReviewFreshness(evidence(), {
      baseRevision: null,
      headRevision,
    })).toBe('unknown');
  });
});
