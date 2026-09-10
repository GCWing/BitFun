import type { Session } from '@/flow_chat/types/flow-chat';
import type {
  ReviewPlatformDetailSection,
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
type PullRequestChangedFileCount = Pick<
  ReviewPlatformPullRequest,
  'changedFiles' | 'changedFileCountKnown'
>;
type PullRequestLineStats = Pick<ReviewPlatformPullRequest, 'additions' | 'deletions' | 'lineStatsKnown'>;

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
    limitations: mergePullRequestDetailLimitations(current.limitations, overview.limitations, 'overview'),
    ...mergeOverviewLineStats(current, overview),
    ...mergeChangedFileCount(current, overview),
    ci: current.ci,
    files: current.files,
    commits: current.commits,
    threads: current.threads,
  };
}

export function mergePullRequestDetailLimitations(
  current: string[] | undefined,
  incoming: string[] | undefined,
  section: ReviewPlatformDetailSection,
): string[] {
  // A page refresh replaces only the coverage facts owned by that section.
  // Preserve warnings for other loaded sections until they are refreshed too.
  const refreshed = (limitation: string): boolean => {
    switch (limitation) {
      case 'gitee_file_list_limit': return section === 'files' || section === 'overview';
      case 'gitee_commit_list_limit': return section === 'commits';
      case 'provider_comment_list_incomplete': return section === 'reviews';
      case 'provider_ci_list_incomplete':
      case 'provider_ci_head_unavailable': return section === 'ci' || section === 'overview';
      default: return false;
    }
  };
  return [...new Set([...(current ?? []).filter(limitation => !refreshed(limitation)), ...(incoming ?? [])])];
}

export function mergeChangedFileCount(
  current: PullRequestChangedFileCount,
  incoming: PullRequestChangedFileCount,
): PullRequestChangedFileCount {
  if (incoming.changedFileCountKnown === true) {
    return {
      changedFiles: incoming.changedFiles,
      changedFileCountKnown: true,
    };
  }
  if (incoming.changedFileCountKnown === false) {
    const source = current.changedFileCountKnown !== false ? current : incoming;
    return {
      changedFiles: source.changedFiles,
      changedFileCountKnown: source.changedFileCountKnown,
    };
  }
  return {
    changedFiles: incoming.changedFiles || current.changedFiles,
    changedFileCountKnown: undefined,
  };
}

export function resolvedChangedFileCount(
  primary?: PullRequestChangedFileCount | null,
  fallback?: PullRequestChangedFileCount | null,
): number | null {
  if (primary && primary.changedFileCountKnown !== false) {
    return primary.changedFiles;
  }
  if (fallback && fallback.changedFileCountKnown !== false) {
    return fallback.changedFiles;
  }
  return null;
}

export function mergeLineStats(
  current: PullRequestLineStats,
  incoming: PullRequestLineStats,
): PullRequestLineStats {
  if (incoming.lineStatsKnown !== undefined) {
    const source = incoming.lineStatsKnown || current.lineStatsKnown === false ? incoming : current;
    return { additions: source.additions, deletions: source.deletions, lineStatsKnown: source.lineStatsKnown };
  }
  // Older providers use zero as the missing-section placeholder.
  return {
    additions: incoming.additions || current.additions,
    deletions: incoming.deletions || current.deletions,
    lineStatsKnown: current.lineStatsKnown,
  };
}

export function resolvedLineStats(value?: PullRequestLineStats | null): PullRequestLineStats | null {
  return value && value.lineStatsKnown !== false ? value : null;
}

function mergeOverviewLineStats(
  current: PullRequestChangedFileCount & PullRequestLineStats,
  overview: PullRequestChangedFileCount & PullRequestLineStats,
): PullRequestLineStats {
  if (overview.lineStatsKnown !== undefined) return mergeLineStats(current, overview);
  // Legacy overviews replace totals, including zero. Only section payloads use
  // zero as a placeholder; keep that fallback inside mergeLineStats.
  const source = overview.changedFileCountKnown === false && current.changedFileCountKnown !== false
    ? current : overview;
  return { additions: source.additions, deletions: source.deletions, lineStatsKnown: source.lineStatsKnown };
}

export function resolvedPullRequestStatistics(
  pullRequest: ReviewPlatformPullRequest,
  detail?: ReviewPlatformPullRequestDetail | null,
): PullRequestChangedFileCount & PullRequestLineStats {
  if (!detail || detail.id !== pullRequest.id || !samePullRequestRevisions(pullRequest, detail)) return pullRequest;
  return { ...mergeChangedFileCount(pullRequest, detail), ...mergeOverviewLineStats(pullRequest, detail) };
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
