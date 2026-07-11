import type { Session } from '@/flow_chat/types/flow-chat';
import type {
  ReviewPlatformPullRequest,
  ReviewPlatformPullRequestDetail,
} from '@/infrastructure/api';

export type PullRequestReviewFreshness = 'current' | 'stale' | 'unknown';

interface PullRequestReviewStatusInput {
  lifecycle: 'running' | 'completed' | 'error' | 'idle';
  resultState: 'loaded' | 'unloaded' | 'missing' | 'invalid';
  evidenceStatus: 'complete' | 'limited' | 'stale' | 'failed';
  issueCount: number;
  riskLevel?: string;
}

type PullRequestReviewIdentity = NonNullable<Session['reviewTargetEvidence']>['pullRequest'];

function normalizeProviderHost(value: string): string {
  return value.trim().toLowerCase().replace(/^https?:\/\//, '').replace(/\/+$/, '');
}

function normalizeProviderProjectPath(value: string): string {
  return value.trim().replace(/\\/g, '/').replace(/^\/+|\/+$/g, '').toLowerCase();
}

export function samePullRequestIdentity(
  identity: PullRequestReviewIdentity,
  current: {
    platform: string;
    host: string;
    projectPath: string;
    pullRequestId: string;
  },
): boolean {
  return Boolean(
    identity
    && identity.platform === current.platform
    && normalizeProviderHost(identity.host) === normalizeProviderHost(current.host)
    && normalizeProviderProjectPath(identity.projectPath) === normalizeProviderProjectPath(current.projectPath)
    && identity.pullRequestId === current.pullRequestId
  );
}

function isFullRevision(value?: string | null): value is string {
  return Boolean(value && /^[0-9a-f]{40,64}$/i.test(value.trim()));
}

export function pullRequestReviewFreshness(
  evidence: Session['reviewTargetEvidence'],
  current: Pick<ReviewPlatformPullRequest, 'baseRevision' | 'headRevision'>,
): PullRequestReviewFreshness {
  if (
    !isFullRevision(evidence?.baseRevision)
    || !isFullRevision(evidence?.headRevision)
    || !isFullRevision(current.baseRevision)
    || !isFullRevision(current.headRevision)
  ) {
    return 'unknown';
  }
  return evidence.baseRevision.toLowerCase() === current.baseRevision.toLowerCase()
    && evidence.headRevision.toLowerCase() === current.headRevision.toLowerCase()
    ? 'current'
    : 'stale';
}

export function effectivePullRequestReviewFreshness(
  evidence: Session['reviewTargetEvidence'],
  current: Pick<ReviewPlatformPullRequest, 'baseRevision' | 'headRevision'>,
  revisionsVerified: boolean,
  evidenceStatus?: 'complete' | 'limited' | 'stale' | 'failed',
): PullRequestReviewFreshness {
  if (evidenceStatus === 'stale') {
    return 'stale';
  }
  return revisionsVerified ? pullRequestReviewFreshness(evidence, current) : 'unknown';
}

export function pullRequestReviewLaunchKey(current: {
  platform: string;
  host: string;
  projectPath: string;
  pullRequestId: string;
  baseRevision?: string | null;
  headRevision?: string | null;
}): string {
  return [
    current.platform.trim().toLowerCase(),
    normalizeProviderHost(current.host),
    normalizeProviderProjectPath(current.projectPath),
    current.pullRequestId.trim(),
    current.baseRevision?.trim().toLowerCase() ?? '',
    current.headRevision?.trim().toLowerCase() ?? '',
  ].join('\0');
}

export function currentPullRequestReviewStatusText(session: PullRequestReviewStatusInput): string {
  if (session.lifecycle !== 'completed') {
    return session.lifecycle === 'idle'
      ? 'Review available · open to view'
      : `Review ${session.lifecycle}`;
  }
  if (session.evidenceStatus === 'failed') {
    return 'Review failed · open to inspect';
  }
  if (session.resultState === 'unloaded') {
    return 'Review complete · open to load result';
  }
  if (session.resultState === 'missing' || session.resultState === 'invalid') {
    return 'Review complete · result unavailable · open to inspect';
  }
  return `Review complete · ${session.issueCount} findings${session.riskLevel ? ` · ${session.riskLevel}` : ''}`
    + (session.evidenceStatus === 'limited' ? ' · limited coverage' : '');
}

export function mergeRevalidatedPullRequestOverview(
  current: ReviewPlatformPullRequestDetail | null,
  overview: ReviewPlatformPullRequestDetail,
): ReviewPlatformPullRequestDetail {
  if (!current || !samePullRequestRevisions(current, overview)) {
    return overview;
  }
  return {
    ...overview,
    ci: current.ci,
    files: current.files,
    commits: current.commits,
    threads: current.threads,
  };
}

export function samePullRequestRevisions(
  left: Pick<ReviewPlatformPullRequest, 'baseRevision' | 'headRevision'>,
  right: Pick<ReviewPlatformPullRequest, 'baseRevision' | 'headRevision'>,
): boolean {
  return Boolean(
    left.baseRevision
    && left.headRevision
    && right.baseRevision
    && right.headRevision
    && left.baseRevision.toLowerCase() === right.baseRevision.toLowerCase()
    && left.headRevision.toLowerCase() === right.headRevision.toLowerCase(),
  );
}
