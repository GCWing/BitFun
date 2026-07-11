import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ReviewPlatformAPI');

export type ReviewPlatformKind = 'github' | 'gitlab' | 'gitcode' | 'unknown';
export type ReviewAuthState = 'not_connected' | 'not_required' | 'connected' | 'expired' | 'error' | 'unsupported';
export type ReviewAuthSource = 'env' | 'stored' | 'none' | 'unsupported';
export type ReviewAuthChallengeState = 'missing' | 'invalid' | 'insufficient_scope';
export type ReviewItemState = 'open' | 'merged' | 'closed' | 'draft';
export type ReviewDecision = 'approved' | 'changes_requested' | 'commented' | 'pending';
export type ReviewFileStatus = 'added' | 'modified' | 'deleted' | 'renamed';
export type ReviewPlatformDetailSection = 'overview' | 'ci' | 'files' | 'commits' | 'reviews';

export interface ReviewPlatformAccount {
  id: string;
  platform: ReviewPlatformKind;
  label: string;
  username?: string | null;
  host: string;
  authState: ReviewAuthState;
  authSource: ReviewAuthSource;
  scopes: string[];
  message?: string | null;
}

export interface ReviewPlatformRepositoryRef {
  providerId: string;
  platform: ReviewPlatformKind;
  host: string;
  owner: string;
  name: string;
  projectPath: string;
  defaultBranch: string;
  workspacePath?: string | null;
  webUrl: string;
}

export interface ReviewPlatformAuthChallenge {
  platform: ReviewPlatformKind;
  host: string;
  remoteId: string;
  projectPath: string;
  state: ReviewAuthChallengeState;
  message: string;
  requiredScopes: string[];
}

export interface ReviewPlatformRemote {
  id: string;
  name: string;
  url: string;
  platform: ReviewPlatformKind;
  host: string;
  owner: string;
  repositoryName: string;
  projectPath: string;
  webUrl: string;
  supported: boolean;
  authState: ReviewAuthState;
  authSource: ReviewAuthSource;
  message?: string | null;
}

export interface ReviewChecks {
  total: number;
  passed: number;
  failed: number;
  pending: number;
}

export interface ReviewPlatformCiItem {
  id: string;
  name: string;
  status: string;
  conclusion?: string | null;
  detail?: string | null;
  stage?: string | null;
  webUrl?: string | null;
  log?: string | null;
  logTruncated: boolean;
  startedAt?: string | null;
  finishedAt?: string | null;
}

export interface ReviewPlatformPullRequest {
  id: string;
  number: number;
  title: string;
  state: ReviewItemState;
  author: string;
  sourceBranch: string;
  targetBranch: string;
  baseRevision?: string | null;
  headRevision?: string | null;
  updatedAt: string;
  webUrl: string;
  additions: number;
  deletions: number;
  changedFiles: number;
  comments: number;
  reviewDecision: ReviewDecision;
  checks: ReviewChecks;
}

export interface ReviewPlatformFile {
  path: string;
  oldPath?: string | null;
  status: ReviewFileStatus;
  additions: number;
  deletions: number;
  patch?: string | null;
}

export interface ReviewPlatformReviewTargetFile {
  path: string;
  oldPath?: string | null;
  status: ReviewFileStatus;
  additions: number;
  deletions: number;
  diffAvailable: boolean;
}

export interface ReviewPlatformPullRequestReviewTarget {
  pullRequest: ReviewPlatformPullRequest;
  files: ReviewPlatformReviewTargetFile[];
  omittedFileCount: number;
  limitations: string[];
}

export interface ReviewPlatformCommit {
  hash: string;
  shortHash: string;
  title: string;
  author: string;
  committedAt: string;
}

export interface ReviewPlatformThread {
  id: string;
  providerThreadId?: string | null;
  providerCommentId?: string | null;
  kind: 'review' | 'comment';
  replyToProviderCommentId?: string | null;
  filePath?: string | null;
  line?: number | null;
  resolved: boolean;
  author: string;
  body: string;
  updatedAt: string;
}

export interface ReviewPlatformPullRequestDetail extends ReviewPlatformPullRequest {
  body: string;
  ci: ReviewPlatformCiItem[];
  files: ReviewPlatformFile[];
  commits: ReviewPlatformCommit[];
  threads: ReviewPlatformThread[];
}

export interface ReviewPlatformPullRequestDetailPage extends ReviewPlatformPullRequestDetail {
  section: ReviewPlatformDetailSection;
  pagination: ReviewPlatformPagination;
}

export interface ReviewPlatformCiLog {
  ciItemId: string;
  log?: string | null;
  truncated: boolean;
  message?: string | null;
}

export interface ReviewPlatformCapabilities {
  canCreateReview: boolean;
  canCreatePullRequest: boolean;
  canReplyToThread: boolean;
  canResolveThread: boolean;
  canApprove: boolean;
  canRevokeApproval: boolean;
  canRequestChanges: boolean;
  canMerge: boolean;
  supportsDraftReview: boolean;
}

export interface ReviewPlatformPagination {
  page: number;
  perPage: number;
  total?: number | null;
  hasNext: boolean;
}

export interface ReviewPlatformWorkspaceSnapshot {
  remotes: ReviewPlatformRemote[];
  selectedRemoteId?: string | null;
  accounts: ReviewPlatformAccount[];
  repository: ReviewPlatformRepositoryRef | null;
  pullRequests: ReviewPlatformPullRequest[];
  pagination: ReviewPlatformPagination;
  capabilities: ReviewPlatformCapabilities;
  message?: string | null;
  authChallenge?: ReviewPlatformAuthChallenge | null;
}

export interface ReviewPlatformWorkspaceSnapshotRequest {
  repositoryPath: string;
  remoteId?: string | null;
  page?: number;
  perPage?: number;
}

export interface ReviewPlatformPullRequestDetailRequest {
  repositoryPath: string;
  remoteId: string;
  pullRequestId: string;
}

export interface ReviewPlatformPullRequestDetailPageRequest extends ReviewPlatformPullRequestDetailRequest {
  section: ReviewPlatformDetailSection;
  page?: number;
  perPage?: number;
}

export interface ReviewPlatformPullRequestCiLogRequest extends ReviewPlatformPullRequestDetailRequest {
  ciItemId: string;
  ciItemName: string;
}

export interface ReviewPlatformUpdateAuthTokenRequest {
  platform: ReviewPlatformKind;
  host: string;
  token: string;
}

export interface ReviewPlatformClearAuthTokenRequest {
  platform: ReviewPlatformKind;
  host: string;
}

export class ReviewPlatformAPI {
  async getWorkspaceSnapshot(
    repositoryPath: string,
    remoteId?: string | null,
    page?: number,
    perPage?: number,
  ): Promise<ReviewPlatformWorkspaceSnapshot> {
    try {
      return await api.invoke('review_platform_get_workspace_snapshot', {
        request: { repositoryPath, remoteId, page, perPage },
      });
    } catch (error) {
      log.error('Failed to load review platform snapshot', { repositoryPath, remoteId, page, perPage, error });
      throw createTauriCommandError('review_platform_get_workspace_snapshot', error, {
        repositoryPath,
        remoteId,
        page,
        perPage,
      });
    }
  }

  async getPullRequestDetail(
    repositoryPath: string,
    remoteId: string,
    pullRequestId: string,
  ): Promise<ReviewPlatformPullRequestDetail> {
    try {
      return await api.invoke('review_platform_get_pull_request_detail', {
        request: { repositoryPath, remoteId, pullRequestId },
      });
    } catch (error) {
      log.error('Failed to load review platform pull request detail', {
        repositoryPath,
        remoteId,
        pullRequestId,
        error,
      });
      throw createTauriCommandError('review_platform_get_pull_request_detail', error, {
        repositoryPath,
        remoteId,
        pullRequestId,
      });
    }
  }

  async getPullRequestReviewTarget(
    repositoryPath: string,
    remoteId: string,
    pullRequestId: string,
  ): Promise<ReviewPlatformPullRequestReviewTarget> {
    try {
      return await api.invoke('review_platform_get_pull_request_review_target', {
        request: { repositoryPath, remoteId, pullRequestId },
      });
    } catch (error) {
      log.error('Failed to prepare review platform pull request target', {
        repositoryPath,
        remoteId,
        pullRequestId,
        error,
      });
      throw createTauriCommandError('review_platform_get_pull_request_review_target', error, {
        repositoryPath,
        remoteId,
        pullRequestId,
      });
    }
  }

  async getPullRequestDetailPage(
    request: ReviewPlatformPullRequestDetailPageRequest,
  ): Promise<ReviewPlatformPullRequestDetailPage> {
    try {
      return await api.invoke('review_platform_get_pull_request_detail_page', {
        request,
      });
    } catch (error) {
      log.error('Failed to load review platform pull request detail page', {
        repositoryPath: request.repositoryPath,
        remoteId: request.remoteId,
        pullRequestId: request.pullRequestId,
        section: request.section,
        page: request.page,
        perPage: request.perPage,
        error,
      });
      throw createTauriCommandError('review_platform_get_pull_request_detail_page', error, request);
    }
  }

  async getPullRequestCiLog(
    request: ReviewPlatformPullRequestCiLogRequest,
  ): Promise<ReviewPlatformCiLog> {
    try {
      return await api.invoke('review_platform_get_pull_request_ci_log', {
        request,
      });
    } catch (error) {
      log.error('Failed to load review platform CI log', {
        repositoryPath: request.repositoryPath,
        remoteId: request.remoteId,
        pullRequestId: request.pullRequestId,
        ciItemId: request.ciItemId,
        error,
      });
      throw createTauriCommandError('review_platform_get_pull_request_ci_log', error, request);
    }
  }

  async updateAuthToken(request: ReviewPlatformUpdateAuthTokenRequest): Promise<void> {
    try {
      await api.invoke('review_platform_update_auth_token', { request });
    } catch (error) {
      log.error('Failed to update review platform auth token', {
        platform: request.platform,
        host: request.host,
        error,
      });
      throw createTauriCommandError('review_platform_update_auth_token', error, {
        platform: request.platform,
        host: request.host,
      });
    }
  }

  async clearAuthToken(request: ReviewPlatformClearAuthTokenRequest): Promise<void> {
    try {
      await api.invoke('review_platform_clear_auth_token', { request });
    } catch (error) {
      log.error('Failed to clear review platform auth token', {
        platform: request.platform,
        host: request.host,
        error,
      });
      throw createTauriCommandError('review_platform_clear_auth_token', error, {
        platform: request.platform,
        host: request.host,
      });
    }
  }
}

export const reviewPlatformAPI = new ReviewPlatformAPI();
