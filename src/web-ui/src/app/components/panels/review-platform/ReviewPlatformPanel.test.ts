import { describe, expect, it } from 'vitest';
import type { Session } from '@/flow_chat/types/flow-chat';
import type { ReviewPlatformPullRequestDetail } from '@/infrastructure/api';
import {
  currentPullRequestReviewStatusText,
  effectivePullRequestReviewFreshness,
  mergeRevalidatedPullRequestOverview,
  pullRequestReviewFreshness,
  pullRequestReviewLaunchKey,
  samePullRequestIdentity,
} from './reviewLinking';

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

  it('keeps cached revisions unknown and honors runtime stale evidence', () => {
    const current = { baseRevision, headRevision };
    expect(effectivePullRequestReviewFreshness(evidence(), current, false, 'complete')).toBe('unknown');
    expect(effectivePullRequestReviewFreshness(evidence(), current, true, 'stale')).toBe('stale');
  });

  it('normalizes provider identity when coordinating launches', () => {
    const first = pullRequestReviewLaunchKey({
      platform: 'GitHub',
      host: 'HTTPS://GitHub.com/',
      projectPath: '/GCWing/BitFun/',
      pullRequestId: '1503',
      baseRevision,
      headRevision,
    });
    const second = pullRequestReviewLaunchKey({
      platform: 'github',
      host: 'github.com',
      projectPath: 'gcwing/bitfun',
      pullRequestId: '1503',
      baseRevision: baseRevision.toUpperCase(),
      headRevision: headRevision.toUpperCase(),
    });
    expect(first).toBe(second);
  });

  it('does not present stale, failed, or unavailable evidence as a clean completion', () => {
    expect(currentPullRequestReviewStatusText({
      lifecycle: 'completed',
      resultState: 'loaded',
      evidenceStatus: 'failed',
      issueCount: 0,
    })).toBe('Review failed · open to inspect');
    expect(currentPullRequestReviewStatusText({
      lifecycle: 'completed',
      resultState: 'unloaded',
      evidenceStatus: 'complete',
      issueCount: 0,
    })).toBe('Review complete · open to load result');
    expect(currentPullRequestReviewStatusText({
      lifecycle: 'completed',
      resultState: 'invalid',
      evidenceStatus: 'complete',
      issueCount: 0,
    })).toBe('Review complete · result unavailable · open to inspect');
    expect(currentPullRequestReviewStatusText({
      lifecycle: 'completed',
      resultState: 'loaded',
      evidenceStatus: 'limited',
      issueCount: 2,
    })).toContain('limited coverage');
  });

  it('preserves loaded sections when a cached overview is revalidated', () => {
    const current = {
      baseRevision,
      headRevision: headRevision,
      ci: [{ id: 'ci-1' }],
      files: [{ path: 'src/lib.rs' }],
      commits: [{ id: 'commit-1' }],
      threads: [{ id: 'thread-1' }],
    } as ReviewPlatformPullRequestDetail;
    const overview = {
      baseRevision,
      headRevision,
      ci: [],
      files: [],
      commits: [],
      threads: [],
    } as unknown as ReviewPlatformPullRequestDetail;

    const merged = mergeRevalidatedPullRequestOverview(current, overview);

    expect(merged.headRevision).toBe(headRevision);
    expect(merged.ci).toBe(current.ci);
    expect(merged.files).toBe(current.files);
    expect(merged.commits).toBe(current.commits);
    expect(merged.threads).toBe(current.threads);
  });

  it('drops cached sections when provider revisions change', () => {
    const current = {
      baseRevision,
      headRevision,
      ci: [{ id: 'ci-1' }],
      files: [{ path: 'src/lib.rs' }],
      commits: [{ id: 'commit-1' }],
      threads: [{ id: 'thread-1' }],
    } as ReviewPlatformPullRequestDetail;
    const overview = {
      baseRevision,
      headRevision: '3'.repeat(40),
      ci: [],
      files: [],
      commits: [],
      threads: [],
    } as unknown as ReviewPlatformPullRequestDetail;

    const merged = mergeRevalidatedPullRequestOverview(current, overview);

    expect(merged).toBe(overview);
  });
});
