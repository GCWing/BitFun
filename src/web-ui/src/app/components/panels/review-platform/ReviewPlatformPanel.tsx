import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CheckCircle2,
  CircleDot,
  Clock3,
  Code2,
  Copy,
  ExternalLink,
  GitCommitHorizontal,
  GitPullRequest,
  GitPullRequestClosed,
  KeyRound,
  Link2,
  Loader2,
  MessageSquareText,
  RefreshCw,
  Search,
  ShieldCheck,
  Sparkles,
  Trash2,
  Terminal,
  UserRound,
  XCircle,
} from 'lucide-react';
import { Button, IconButton, Input, MarkdownRenderer, Modal, Select, Tabs, TabPane, Tooltip, type SelectOption } from '@/component-library';
import { reviewPlatformAPI, systemAPI, type ReviewPlatformAccount, type ReviewPlatformAuthChallenge, type ReviewPlatformCiItem, type ReviewPlatformCiLog, type ReviewPlatformCommit, type ReviewPlatformDetailSection, type ReviewPlatformFile, type ReviewPlatformPagination, type ReviewPlatformPullRequest, type ReviewPlatformPullRequestDetail, type ReviewPlatformPullRequestDetailPage, type ReviewPlatformRemote, type ReviewPlatformRepositoryRef, type ReviewPlatformThread, type ReviewPlatformWorkspaceSnapshot } from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';
import { notificationService } from '@/shared/notification-system';
import { i18nService } from '@/infrastructure/i18n';
import { openMainSession } from '@/flow_chat/services/sessionActivation';
import { openBtwSessionInAuxPane } from '@/flow_chat/services/btwSessionPane';
import {
  launchPreparedReviewSession,
  prepareReviewLaunchFromPullRequest,
} from '@/flow_chat/services/ReviewService';
import { useDeepReviewConsent } from '@/flow_chat/components/DeepReviewConsentDialog';
import { deriveDeepReviewSessionConcurrencyGuard } from '@/flow_chat/utils/deepReviewCapacityGuard';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import type { FlowToolItem, Session } from '@/flow_chat/types/flow-chat';
import { findLatestCodeReviewResultState, summarizeCodeReviewResult } from '@/flow_chat/utils/reviewSessionSummary';
import { parsePullRequestUrl, remoteMatchesPullRequestLink } from '@/shared/utils/pullRequestLinks';
import { useContextStore } from '@/shared/stores/contextStore';
import { quickActions } from '@/shared/services/ide-control';
import type { PullRequestContext } from '@/shared/types/context';
import {
  currentPullRequestReviewStatusText,
  effectivePullRequestReviewFreshness,
  mergeRevalidatedPullRequestOverview,
  pullRequestReviewFreshness,
  pullRequestReviewLaunchKey,
  samePullRequestRevisions,
  samePullRequestIdentity,
  type PullRequestReviewFreshness,
} from './reviewLinking';
import './ReviewPlatformPanel.scss';

const log = createLogger('ReviewPlatformPanel');

interface ReviewPlatformPanelProps {
  workspacePath?: string;
  initialRemoteId?: string;
  initialPullRequestId?: string;
  initialPullRequestUrl?: string;
  detailOnly?: boolean;
}

type DetailTab = 'overview' | 'changes' | 'commits';
type ListStateFilter = 'all' | 'open' | 'draft' | 'merged' | 'closed';
type SnapshotCacheState = 'none' | 'cached' | 'refreshing';

const PR_PAGE_SIZE = 10;
const CI_PAGE_SIZE = 20;
const CHANGE_PAGE_SIZE = 15;
const COMMIT_PAGE_SIZE = 30;
const REVIEW_PAGE_SIZE = 20;
const REMOTE_STORAGE_PREFIX = 'bitfun:review-platform:last-remote:';
const MAX_LINKED_REVIEW_SESSIONS = 6;

interface SnapshotCacheEntry {
  snapshot: ReviewPlatformWorkspaceSnapshot;
  fetchedAt: number;
}

interface DetailCacheEntry {
  detail: ReviewPlatformPullRequestDetail;
  fetchedAt: number;
}

interface DetailPageCacheEntry {
  detail: ReviewPlatformPullRequestDetailPage;
  fetchedAt: number;
}

interface PageInfo {
  pageIndex: number;
  totalPages: number;
  start: number;
  end: number;
  totalLabel: string;
  hasNext: boolean;
}

interface ReviewSessionMarkerInput {
  childSessionId?: string;
  parentSessionId?: string;
  kind?: 'review' | 'deep_review';
  title?: string;
  requestedFiles?: string[];
}

interface ReviewSessionMarker {
  childSessionId: string;
  parentSessionId?: string;
  kind: 'review' | 'deep_review';
  title?: string;
  requestedFiles: string[];
}

interface LinkedReviewSession {
  childSession: Session;
  parentSession?: Session;
  marker?: ReviewSessionMarker;
  kind: 'review' | 'deep_review';
  title: string;
  requestedFiles: string[];
  resultState: 'loaded' | 'unloaded' | 'missing' | 'invalid';
  issueCount: number;
  riskLevel?: string;
  lifecycle: 'running' | 'completed' | 'error' | 'idle';
  freshness: PullRequestReviewFreshness;
  evidenceStatus: 'complete' | 'limited' | 'stale' | 'failed';
  updatedAt: number;
}

const snapshotCache = new Map<string, SnapshotCacheEntry>();
const detailCache = new Map<string, DetailCacheEntry>();
const detailPageCache = new Map<string, DetailPageCacheEntry>();
const reviewLaunchesInFlight = new Set<string>();
const EMPTY_REVIEW_THREADS: ReviewPlatformThread[] = [];

function detailPageInfo(pagination: ReviewPlatformPagination, itemCount: number): PageInfo {
  const pageIndex = Math.max(0, (pagination.page || 1) - 1);
  const perPage = Math.max(1, pagination.perPage || itemCount || 1);
  const total = pagination.total ?? null;
  const totalPages = total !== null
    ? Math.max(1, Math.ceil(total / perPage))
    : pageIndex + (pagination.hasNext ? 2 : 1);
  const start = itemCount === 0 ? 0 : pageIndex * perPage + 1;
  const end = total !== null
    ? Math.min(total, pageIndex * perPage + itemCount)
    : pageIndex * perPage + itemCount;
  return {
    pageIndex,
    totalPages,
    start,
    end,
    totalLabel: total !== null ? String(total) : `${end}+`,
    hasNext: pagination.hasNext,
  };
}

function snapshotCacheKey(workspacePath: string, remoteId: string | null, page: number, perPage: number, mode: 'list' | 'context'): string {
  return `${workspacePath}::${remoteId ?? 'default'}::${page}::${perPage}::${mode}`;
}

function detailCacheKey(workspacePath: string, remoteId: string, pullRequestId: string): string {
  return `${workspacePath}::${remoteId}::${pullRequestId}`;
}

function detailPageCacheKey(workspacePath: string, remoteId: string, pullRequestId: string, section: ReviewPlatformDetailSection, page: number, perPage: number): string {
  return `${workspacePath}::${remoteId}::${pullRequestId}::${section}::${page}::${perPage}`;
}

function clearDetailPageCacheForPullRequest(workspacePath: string, remoteId: string, pullRequestId: string): void {
  const prefix = `${workspacePath}::${remoteId}::${pullRequestId}::`;
  for (const key of detailPageCache.keys()) {
    if (key.startsWith(prefix)) {
      detailPageCache.delete(key);
    }
  }
}

function emptyPagination(page: number, perPage: number): ReviewPlatformPagination {
  return { page, perPage, total: null, hasNext: false };
}

function mergeDetailPage(
  current: ReviewPlatformPullRequestDetail | null,
  page: ReviewPlatformPullRequestDetailPage,
): ReviewPlatformPullRequestDetail {
  const base = current ?? page;
  return {
    ...base,
    ...page,
    additions: page.additions || base.additions,
    deletions: page.deletions || base.deletions,
    changedFiles: page.changedFiles || base.changedFiles,
    ci: page.section === 'ci' ? page.ci : base.ci,
    files: page.section === 'files' ? page.files : base.files,
    commits: page.section === 'commits' ? page.commits : base.commits,
    threads: page.section === 'reviews' ? page.threads : base.threads,
    // Section caches may predate the freshly revalidated overview. Never let
    // them replace the revisions used to judge whether a Review is current.
    baseRevision: current ? base.baseRevision : page.baseRevision,
    headRevision: current ? base.headRevision : page.headRevision,
  };
}

function remotePreferenceKey(workspacePath: string): string {
  return `${REMOTE_STORAGE_PREFIX}${workspacePath}`;
}

function readRememberedRemote(workspacePath?: string): string | null {
  if (!workspacePath || typeof window === 'undefined') return null;
  try {
    return window.localStorage.getItem(remotePreferenceKey(workspacePath));
  } catch {
    return null;
  }
}

function rememberRemote(workspacePath: string | undefined, remoteId: string | null): void {
  if (!workspacePath || typeof window === 'undefined') return;
  try {
    const key = remotePreferenceKey(workspacePath);
    if (remoteId) {
      window.localStorage.setItem(key, remoteId);
    } else {
      window.localStorage.removeItem(key);
    }
  } catch {
    // Ignore storage failures; the selector still works for the current session.
  }
}

function formatRelativeTime(value: string): string {
  const time = new Date(value).getTime();
  if (!Number.isFinite(time)) return '';
  const diffMs = Date.now() - time;
  const minutes = Math.max(1, Math.floor(diffMs / 60000));
  if (minutes < 60) return i18nService.t('common:reviewPlatform.relativeTime.minutesAgo', { count: minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return i18nService.t('common:reviewPlatform.relativeTime.hoursAgo', { count: hours });
  return i18nService.t('common:reviewPlatform.relativeTime.daysAgo', { count: Math.floor(hours / 24) });
}

function formatAbsoluteTime(value: string): string {
  if (!value) return '';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  return i18nService.formatDate(date, {
    dateStyle: 'medium',
    timeStyle: 'short',
  });
}

function getPrIcon(pr: ReviewPlatformPullRequest) {
  if (pr.state === 'merged') return <GitPullRequest size={15} className="review-platform__state-icon review-platform__state-icon--merged" />;
  if (pr.state === 'closed') return <GitPullRequestClosed size={15} className="review-platform__state-icon review-platform__state-icon--closed" />;
  return <GitPullRequest size={15} className="review-platform__state-icon review-platform__state-icon--open" />;
}

function decisionLabel(decision: ReviewPlatformPullRequest['reviewDecision']): string {
  switch (decision) {
    case 'approved':
      return 'Approved';
    case 'changes_requested':
      return 'Changes requested';
    case 'commented':
      return 'Commented';
    default:
      return 'Pending review';
  }
}

function stateLabel(state: ReviewPlatformPullRequest['state']): string {
  switch (state) {
    case 'open':
      return 'Open';
    case 'draft':
      return 'Draft';
    case 'merged':
      return 'Merged';
    case 'closed':
      return 'Closed';
    default:
      return state;
  }
}

function providerLabel(remote: ReviewPlatformRemote | ReviewPlatformAccount | null): string {
  if (!remote) return 'No provider';
  switch (remote.platform) {
    case 'github':
      return 'GitHub';
    case 'gitlab':
      return 'GitLab';
    case 'gitcode':
      return 'GitCode';
    default:
      return 'Git';
  }
}

function remoteLabel(remote: ReviewPlatformRemote): string {
  return `${providerLabel(remote)} · ${remote.name} · ${remote.projectPath}`;
}

function authLabel(account: ReviewPlatformAccount | null): string {
  if (!account) return 'Disconnected';
  switch (account.authState) {
    case 'connected':
      return 'Connected';
    case 'not_required':
      return 'Public';
    case 'unsupported':
      return 'Unsupported';
    case 'expired':
      return 'Expired';
    case 'error':
      return 'Auth error';
    default:
      return 'Not connected';
  }
}

function authSourceLabel(source: ReviewPlatformAccount['authSource'] | undefined): string {
  switch (source) {
    case 'gh_cli':
      return 'GitHub CLI';
    case 'stored':
      return 'Saved token';
    case 'env':
      return 'Environment token';
    case 'unsupported':
      return 'Unsupported';
    default:
      return 'No token';
  }
}

function authChallengeTitle(challenge: ReviewPlatformAuthChallenge): string {
  if (challenge.platform === 'github') return 'GitHub CLI authentication required';
  switch (challenge.state) {
    case 'missing':
      return 'Token required';
    case 'insufficient_scope':
      return 'Token permissions required';
    default:
      return 'Token update required';
  }
}

function authChallengeScopes(challenge: ReviewPlatformAuthChallenge): string {
  return challenge.requiredScopes.length ? challenge.requiredScopes.join(', ') : 'Provider API access';
}

function emptySnapshot(): ReviewPlatformWorkspaceSnapshot {
  return {
    remotes: [],
    selectedRemoteId: null,
    accounts: [],
    repository: null,
    pullRequests: [],
    pagination: {
      page: 1,
      perPage: PR_PAGE_SIZE,
      total: 0,
      hasNext: false,
    },
    capabilities: {
      canCreateReview: false,
      canCreatePullRequest: false,
      canReplyToThread: false,
      canResolveThread: false,
      canApprove: false,
      canRevokeApproval: false,
      canRequestChanges: false,
      canMerge: false,
      supportsDraftReview: false,
    },
    message: null,
    authChallenge: null,
  };
}

function diffLineClass(line: string): string {
  if (line.startsWith('+++') || line.startsWith('---')) return 'review-platform__diff-line review-platform__diff-line--meta';
  if (line.startsWith('@@')) return 'review-platform__diff-line review-platform__diff-line--hunk';
  if (line.startsWith('+')) return 'review-platform__diff-line review-platform__diff-line--add';
  if (line.startsWith('-')) return 'review-platform__diff-line review-platform__diff-line--delete';
  return 'review-platform__diff-line';
}

function fileKey(file: { path: string; oldPath?: string | null }): string {
  return `${file.oldPath ?? ''}->${file.path}`;
}

function normalizePath(value: string): string {
  return value.replace(/\\/g, '/').trim();
}

function uniquePaths(paths: string[]): string[] {
  const seen = new Set<string>();
  const next: string[] = [];
  for (const path of paths) {
    const normalized = normalizePath(path);
    if (!normalized || seen.has(normalized)) continue;
    seen.add(normalized);
    next.push(normalized);
  }
  return next;
}

function isReviewSessionRunning(session: Session): boolean {
  const turn = session.dialogTurns[session.dialogTurns.length - 1];
  return turn?.status === 'pending' ||
    turn?.status === 'image_analyzing' ||
    turn?.status === 'processing' ||
    turn?.status === 'finishing';
}

function reviewSessionLifecycle(session: Session): LinkedReviewSession['lifecycle'] {
  const turn = session.dialogTurns[session.dialogTurns.length - 1];
  if (session.error || session.hasUnreadCompletion === 'error' || session.hasUnreadCompletion === 'interrupted' || turn?.status === 'error') return 'error';
  if (isReviewSessionRunning(session)) return 'running';
  if (
    turn?.status === 'completed' ||
    session.hasUnreadCompletion === 'completed' ||
    (session.historyState === 'metadata-only' && session.persistedStatus === 'completed')
  ) return 'completed';
  return 'idle';
}

function getSessionTitle(session?: Session, fallback = 'Review session'): string {
  return session?.title?.trim() || fallback;
}

function extractReviewSessionMarkers(session: Session): ReviewSessionMarker[] {
  const markers: ReviewSessionMarker[] = [];
  for (const turn of session.dialogTurns) {
    for (const round of turn.modelRounds) {
      for (const item of round.items) {
        if (item.type !== 'tool') continue;
        const toolItem = item as FlowToolItem;
        if (toolItem.toolName !== 'ReviewSessionSummary') continue;
        const input = (toolItem.toolCall?.input ?? {}) as ReviewSessionMarkerInput;
        if (!input.childSessionId) continue;
        markers.push({
          childSessionId: input.childSessionId,
          parentSessionId: input.parentSessionId ?? session.sessionId,
          kind: input.kind === 'deep_review' ? 'deep_review' : 'review',
          title: input.title,
          requestedFiles: uniquePaths(input.requestedFiles ?? []),
        });
      }
    }
  }
  return markers;
}

function buildPrChatPrompt(params: {
  pr: ReviewPlatformPullRequest;
  remote: ReviewPlatformRemote | null;
  repository: ReviewPlatformRepositoryRef | null;
  filePaths: string[];
  webUrl?: string;
}): string {
  const fileList = params.filePaths.length
    ? params.filePaths.map(path => `- ${path}`).join('\n')
    : '- No file list is loaded yet';
  const provider = params.remote ? providerLabel(params.remote) : 'review platform';
  const repository = params.repository?.projectPath ?? params.remote?.projectPath ?? 'current repository';

  return [
    `Review PR #${params.pr.number}: ${params.pr.title}`,
    '',
    `Provider: ${provider}`,
    `Repository: ${repository}`,
    `Branch: ${params.pr.sourceBranch} -> ${params.pr.targetBranch}`,
    params.webUrl ? `URL: ${params.webUrl}` : null,
    '',
    'Changed files:',
    fileList,
    '',
    'Please use this PR context with the current conversation. Focus on risks, review findings, and concrete fixes.',
  ].filter(Boolean).join('\n');
}

function createContextId(prefix: string): string {
  if (typeof globalThis.crypto?.randomUUID === 'function') {
    return globalThis.crypto.randomUUID();
  }
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
}

function formatChecksText(pr: ReviewPlatformPullRequest): string {
  return pr.checks.total > 0
    ? `${pr.checks.passed}/${pr.checks.total} passed, ${pr.checks.failed} failed, ${pr.checks.pending} pending`
    : 'No checks reported';
}

function buildPrOverviewContext(params: {
  pr: ReviewPlatformPullRequest;
  detail: ReviewPlatformPullRequestDetail | null;
  remote: ReviewPlatformRemote | null;
  repository: ReviewPlatformRepositoryRef | null;
  filePaths: string[];
  reviewItemCount: number;
  webUrl?: string;
}): string {
  const body = params.detail?.body?.trim() || 'No pull request description was returned by the provider.';
  return [
    buildPrChatPrompt(params),
    '',
    'Overview:',
    body,
    '',
    `State: ${stateLabel(params.pr.state)}`,
    `Review decision: ${decisionLabel(params.pr.reviewDecision)}`,
    `Checks: ${formatChecksText(params.pr)}`,
    `Comments: ${params.reviewItemCount}`,
  ].join('\n');
}

function buildPrFileDiffContext(pr: ReviewPlatformPullRequest, file: ReviewPlatformFile): string {
  return [
    `Pull request file diff: PR #${pr.number} ${pr.title}`,
    `File: ${file.path}`,
    file.oldPath && file.oldPath !== file.path ? `Old path: ${file.oldPath}` : null,
    `Status: ${file.status}`,
    `Delta: +${file.additions} -${file.deletions}`,
    '',
    'Diff:',
    file.patch?.trim() || 'No inline diff is available for this file.',
  ].filter(Boolean).join('\n');
}

function buildPrCommitsContext(pr: ReviewPlatformPullRequest, commits: ReviewPlatformCommit[]): string {
  if (!commits.length) {
    return `Pull request commits: PR #${pr.number} ${pr.title}\n\nNo commits were returned by the provider.`;
  }
  return [
    `Pull request commits: PR #${pr.number} ${pr.title}`,
    '',
    ...commits.map(commit => [
      `- ${commit.shortHash} ${commit.title}`,
      `  Author: ${commit.author}`,
      `  Committed: ${formatAbsoluteTime(commit.committedAt) || commit.committedAt}`,
      `  Hash: ${commit.hash}`,
    ].join('\n')),
  ].join('\n');
}

function buildPrReviewsContext(pr: ReviewPlatformPullRequest, threads: ReviewPlatformThread[]): string {
  if (!threads.length) {
    return `Pull request reviews: PR #${pr.number} ${pr.title}\n\nNo review threads were returned by the provider.`;
  }
  const threadByCommentId = new Map(
    threads
      .filter(thread => thread.providerCommentId)
      .map(thread => [thread.providerCommentId as string, thread]),
  );
  return [
    `Pull request reviews: PR #${pr.number} ${pr.title}`,
    '',
    ...threads.map(thread => [
      `- [${thread.kind === 'review' ? 'Review' : 'Comment'}] ${thread.resolved ? 'Resolved' : 'Open'} thread by ${thread.author}`,
      thread.replyToProviderCommentId
        ? `  Reply to: ${threadByCommentId.get(thread.replyToProviderCommentId)?.author ?? thread.replyToProviderCommentId}`
        : null,
      thread.filePath ? `  Location: ${thread.filePath}${thread.line ? `:${thread.line}` : ''}` : null,
      `  Updated: ${formatAbsoluteTime(thread.updatedAt) || thread.updatedAt}`,
      `  Body: ${thread.body}`,
    ].filter(Boolean).join('\n')),
  ].join('\n');
}

function ciItemTone(item: ReviewPlatformCiItem): 'passed' | 'failed' | 'pending' {
  const raw = `${item.conclusion ?? item.status}`.trim().toLowerCase();
  if (['success', 'neutral', 'skipped', 'passed', 'pass'].includes(raw)) return 'passed';
  if (['failure', 'failed', 'error', 'timed_out', 'timed-out', 'cancelled', 'canceled', 'action_required'].includes(raw)) return 'failed';
  return 'pending';
}

function ciItemStatusText(item: ReviewPlatformCiItem): string {
  const status = item.status.trim();
  const conclusion = item.conclusion?.trim();
  if (!conclusion || conclusion.toLowerCase() === status.toLowerCase()) {
    return status || 'unknown';
  }
  return `${status || 'unknown'} · ${conclusion}`;
}

function buildPrCiContext(pr: ReviewPlatformPullRequest, ciItems: ReviewPlatformCiItem[]): string {
  if (!ciItems.length) {
    return `Pull request CI: PR #${pr.number} ${pr.title}\n\nNo CI entries were returned by the provider.`;
  }
  return [
    `Pull request CI page: PR #${pr.number} ${pr.title}`,
    '',
    `Checks: ${formatChecksText(pr)}`,
    '',
    ...ciItems.map(item => [
      `- ${item.name}`,
      `  Status: ${ciItemStatusText(item)}`,
      item.stage ? `  Stage: ${item.stage}` : null,
      item.detail ? `  Detail: ${item.detail}` : null,
      item.webUrl ? `  URL: ${item.webUrl}` : null,
      item.startedAt ? `  Started: ${formatAbsoluteTime(item.startedAt) || item.startedAt}` : null,
      item.finishedAt ? `  Finished: ${formatAbsoluteTime(item.finishedAt) || item.finishedAt}` : null,
    ].filter(Boolean).join('\n')),
  ].join('\n');
}

function buildPrCiItemContext(pr: ReviewPlatformPullRequest, item: ReviewPlatformCiItem, ciLog?: ReviewPlatformCiLog | null): string {
  const hasLog = Boolean(ciLog?.log);
  return [
    `Pull request CI result: PR #${pr.number} ${pr.title}`,
    '',
    `Checks: ${formatChecksText(pr)}`,
    '',
    `Name: ${item.name}`,
    `Status: ${ciItemStatusText(item)}`,
    item.conclusion ? `Conclusion: ${item.conclusion}` : null,
    item.stage ? `Stage: ${item.stage}` : null,
    item.detail ? `Detail: ${item.detail}` : null,
    item.webUrl ? `URL: ${item.webUrl}` : null,
    item.startedAt ? `Started: ${formatAbsoluteTime(item.startedAt) || item.startedAt}` : null,
    item.finishedAt ? `Finished: ${formatAbsoluteTime(item.finishedAt) || item.finishedAt}` : null,
    '',
    hasLog ? 'Error log excerpt:' : 'Provider detail:',
    hasLog
      ? `${ciLog?.truncated ? '[Truncated error excerpt]\n' : ''}${ciLog?.log ?? ''}`
      : ciLog?.message || item.detail || 'No additional provider detail has been loaded for this CI result.',
  ].filter(Boolean).join('\n');
}

function canLoadCiLog(remote: ReviewPlatformRemote | null, _item: ReviewPlatformCiItem): boolean {
  return Boolean(remote);
}

function canExpandCiItem(remote: ReviewPlatformRemote | null, item: ReviewPlatformCiItem): boolean {
  return canLoadCiLog(remote, item) || Boolean(item.log || item.detail || item.stage || item.webUrl || item.startedAt || item.finishedAt);
}

export const ReviewPlatformPanel: React.FC<ReviewPlatformPanelProps> = ({
  workspacePath,
  initialRemoteId,
  initialPullRequestId,
  initialPullRequestUrl,
  detailOnly = false,
}) => {
  const snapshotRequestSeq = useRef(0);
  const detailRequestSeq = useRef(0);
  const detailSectionRequestSeq = useRef(0);
  const reviewLaunchInFlight = useRef(false);
  const [snapshot, setSnapshot] = useState<ReviewPlatformWorkspaceSnapshot>(emptySnapshot);
  const [selectedRemoteId, setSelectedRemoteId] = useState<string | null>(null);
  const [listRemoteId, setListRemoteId] = useState<string | null>(null);
  const [selectedPrId, setSelectedPrId] = useState<string | null>(null);
  const [detail, setDetail] = useState<ReviewPlatformPullRequestDetail | null>(null);
  const [verifiedDetailKey, setVerifiedDetailKey] = useState<string | null>(null);
  const [flowState, setFlowState] = useState(() => flowChatStore.getState());
  const [activeTab, setActiveTab] = useState<DetailTab>('overview');
  const [loading, setLoading] = useState(true);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [stateFilter, setStateFilter] = useState<ListStateFilter>('all');
  const [pageIndex, setPageIndex] = useState(0);
  const [ciPageIndex, setCiPageIndex] = useState(0);
  const [changePageIndex, setChangePageIndex] = useState(0);
  const [commitPageIndex, setCommitPageIndex] = useState(0);
  const [reviewPageIndex, setReviewPageIndex] = useState(0);
  const [ciPagination, setCiPagination] = useState<ReviewPlatformPagination>(() => emptyPagination(1, CI_PAGE_SIZE));
  const [changePagination, setChangePagination] = useState<ReviewPlatformPagination>(() => emptyPagination(1, CHANGE_PAGE_SIZE));
  const [commitPagination, setCommitPagination] = useState<ReviewPlatformPagination>(() => emptyPagination(1, COMMIT_PAGE_SIZE));
  const [reviewPagination, setReviewPagination] = useState<ReviewPlatformPagination>(() => emptyPagination(1, REVIEW_PAGE_SIZE));
  const [expandedFileKeys, setExpandedFileKeys] = useState<Set<string>>(() => new Set());
  const [expandedCiItemIds, setExpandedCiItemIds] = useState<Set<string>>(() => new Set());
  const [ciLogById, setCiLogById] = useState<Record<string, ReviewPlatformCiLog>>({});
  const [ciLogErrorById, setCiLogErrorById] = useState<Record<string, string>>({});
  const [ciLogLoadingIds, setCiLogLoadingIds] = useState<Set<string>>(() => new Set());
  const [snapshotCacheState, setSnapshotCacheState] = useState<SnapshotCacheState>('none');
  const [authModalOpen, setAuthModalOpen] = useState(false);
  const [authToken, setAuthToken] = useState('');
  const [authSaving, setAuthSaving] = useState(false);
  const [authError, setAuthError] = useState<string | null>(null);
  const [reviewLaunching, setReviewLaunching] = useState(false);
  const { confirmDeepReviewLaunch, deepReviewConsentDialog } = useDeepReviewConsent();

  const account = snapshot.accounts[0] ?? null;
  const selectedRemote = useMemo(
    () => snapshot.remotes.find(remote => remote.id === selectedRemoteId) ?? snapshot.remotes[0] ?? null,
    [selectedRemoteId, snapshot.remotes],
  );
  const repository = useMemo<ReviewPlatformRepositoryRef | null>(() => {
    if (!snapshot.repository || !selectedRemote) return snapshot.repository;
    return {
      ...snapshot.repository,
      providerId: selectedRemote.id,
      platform: selectedRemote.platform,
      host: selectedRemote.host,
      owner: selectedRemote.owner,
      name: selectedRemote.repositoryName,
      projectPath: selectedRemote.projectPath,
      webUrl: selectedRemote.webUrl,
    };
  }, [selectedRemote, snapshot.repository]);
  const authChallenge = snapshot.authChallenge ?? null;
  const selectedPrFromList = useMemo(
    () => snapshot.pullRequests.find(pr => (
      pr.id === selectedPrId && (!pr.providerId || pr.providerId === selectedRemoteId)
    )) ?? null,
    [selectedPrId, selectedRemoteId, snapshot.pullRequests],
  );
  const selectedPr = detail ?? selectedPrFromList;
  const hasDetail = detail !== null;
  const initialPullRequestTarget = useMemo(
    () => initialPullRequestUrl ? parsePullRequestUrl(initialPullRequestUrl) : null,
    [initialPullRequestUrl],
  );
  const prFilePaths = useMemo(
    () => uniquePaths((detail?.files ?? []).map(file => file.path)),
    [detail?.files],
  );
  const ciItems = useMemo(() => detail?.ci ?? [], [detail?.ci]);
  const changedFiles = useMemo(() => detail?.files ?? [], [detail?.files]);
  const commits = useMemo(() => detail?.commits ?? [], [detail?.commits]);
  const reviewThreads = useMemo(() => detail?.threads ?? EMPTY_REVIEW_THREADS, [detail?.threads]);
  const reviewThreadByCommentId = useMemo(
    () => new Map(
      reviewThreads
        .filter(thread => thread.providerCommentId)
        .map(thread => [thread.providerCommentId as string, thread]),
    ),
    [reviewThreads],
  );
  const reviewItemCount = reviewPagination.total
    ?? (reviewThreads.length > 0 ? reviewThreads.length : (selectedPr?.comments ?? 0));
  const ciTotal = ciPagination.total ?? ciItems.length;
  const ciPage = detailPageInfo(ciPagination, ciTotal);
  const changePage = detailPageInfo(changePagination, changedFiles.length);
  const commitPage = detailPageInfo(commitPagination, commits.length);
  const reviewPage = detailPageInfo(reviewPagination, reviewThreads.length);
  const pagedCiItems = ciItems;
  const pagedChangedFiles = changedFiles;
  const pagedCommits = commits;
  const pagedReviewThreads = reviewThreads;
  const remoteOptions = useMemo<SelectOption[]>(
    () => snapshot.remotes.map(remote => ({
      value: remote.id,
      label: remoteLabel(remote),
      description: `${remote.host} · ${authLabel(account && account.id === remote.id ? account : null)}`,
    })),
    [account, snapshot.remotes],
  );

  const loadSnapshot = useCallback(async (nextRemoteId?: string | null, options?: { force?: boolean; page?: number }) => {
    const requestSeq = ++snapshotRequestSeq.current;
    if (!workspacePath) {
      setSnapshot(emptySnapshot());
      setSelectedRemoteId(null);
      setListRemoteId(null);
      setSelectedPrId(null);
      setDetail(null);
      setVerifiedDetailKey(null);
      setDetailError(null);
      setError('No active workspace is available.');
      setLoading(false);
      return;
    }

    const requestedRemoteId = nextRemoteId !== undefined
      ? nextRemoteId
      : detailOnly
        ? readRememberedRemote(workspacePath)
        : null;
    const requestedPage = Math.max(1, options?.page ?? 1);
    const snapshotMode = detailOnly ? 'context' : 'list';
    setListRemoteId(requestedRemoteId ?? null);
    const requestedCacheKey = snapshotCacheKey(workspacePath, requestedRemoteId ?? null, requestedPage, PR_PAGE_SIZE, snapshotMode);
    const cached = snapshotCache.get(requestedCacheKey);
    const force = options?.force === true;

    if (cached && !force) {
      const remoteId = cached.snapshot.selectedRemoteId ?? cached.snapshot.remotes[0]?.id ?? null;
      setSnapshot(cached.snapshot);
      setSelectedRemoteId(remoteId);
      setPageIndex(Math.max(0, (cached.snapshot.pagination.page || requestedPage) - 1));
      setSelectedPrId(detailOnly ? null : cached.snapshot.pullRequests[0]?.id ?? null);
      setDetail(null);
      setVerifiedDetailKey(null);
      setDetailError(null);
      setError(null);
      setSnapshotCacheState('cached');
      setLoading(false);
      return;
    } else {
      setSnapshot(emptySnapshot());
      setSelectedPrId(null);
      setDetail(null);
      setVerifiedDetailKey(null);
      setDetailError(null);
      setSnapshotCacheState('none');
    }

    setLoading(true);
    setError(null);
    try {
      const next = detailOnly
        ? await reviewPlatformAPI.getWorkspaceContext(workspacePath, requestedRemoteId ?? null)
        : await reviewPlatformAPI.getWorkspaceSnapshot(workspacePath, requestedRemoteId ?? null, requestedPage, PR_PAGE_SIZE);
      if (snapshotRequestSeq.current !== requestSeq) return;
      setSnapshot(next);
      const remoteId = next.selectedRemoteId ?? next.remotes[0]?.id ?? null;
      setSelectedRemoteId(remoteId);
      setPageIndex(Math.max(0, (next.pagination.page || requestedPage) - 1));
      rememberRemote(workspacePath, remoteId);
      setSelectedPrId(detailOnly ? null : next.pullRequests[0]?.id ?? null);
      setDetail(null);
      setVerifiedDetailKey(null);
      setDetailError(null);
      const entry = { snapshot: next, fetchedAt: Date.now() };
      snapshotCache.set(requestedCacheKey, entry);
      if (remoteId) {
        snapshotCache.set(snapshotCacheKey(workspacePath, remoteId, requestedPage, PR_PAGE_SIZE, snapshotMode), entry);
      }
      setSnapshotCacheState('cached');
    } catch (err) {
      if (snapshotRequestSeq.current !== requestSeq) return;
      const message = err instanceof Error ? err.message : 'Failed to load pull requests';
      setError(message);
      if (!cached) {
        setSnapshot(emptySnapshot());
      }
      log.error('Failed to load review platform snapshot', { workspacePath, error: err });
    } finally {
      if (snapshotRequestSeq.current === requestSeq) {
        setLoading(false);
      }
    }
  }, [detailOnly, workspacePath]);

  const loadDetail = useCallback(async (repo: ReviewPlatformRepositoryRef | null, remoteId: string, pullRequestId: string, options?: { force?: boolean }) => {
    const requestSeq = ++detailRequestSeq.current;
    detailSectionRequestSeq.current += 1;
    const repositoryPath = workspacePath || repo?.workspacePath || '';
    const cacheKey = detailCacheKey(repositoryPath, remoteId, pullRequestId);
    const cached = detailCache.get(cacheKey);
    const force = options?.force === true;

    setDetailError(null);
    setVerifiedDetailKey(null);
    if (force) {
      detailCache.delete(cacheKey);
      clearDetailPageCacheForPullRequest(repositoryPath, remoteId, pullRequestId);
    }

    if (cached && !force) {
      setDetail(cached.detail);
    } else {
      setDetail(null);
    }

    setDetailLoading(true);
    try {
      const nextDetail = await reviewPlatformAPI.getPullRequestDetailPage({
        repositoryPath,
        remoteId,
        pullRequestId,
        section: 'overview',
        page: 1,
        perPage: 1,
      });
      if (detailRequestSeq.current !== requestSeq) return;
      if (cached && !samePullRequestRevisions(cached.detail, nextDetail)) {
        clearDetailPageCacheForPullRequest(repositoryPath, remoteId, pullRequestId);
      }
      setDetail((current) => mergeRevalidatedPullRequestOverview(current, nextDetail));
      detailCache.set(cacheKey, { detail: nextDetail, fetchedAt: Date.now() });
      setVerifiedDetailKey(cacheKey);
    } catch (err) {
      if (detailRequestSeq.current !== requestSeq) return;
      log.error('Failed to load pull request detail', { pullRequestId, error: err });
      setDetailError(err instanceof Error ? err.message : 'Failed to load pull request details.');
      if (!cached) {
        setDetail(null);
      }
    } finally {
      if (detailRequestSeq.current === requestSeq) {
        setDetailLoading(false);
      }
    }
  }, [workspacePath]);

  const applySectionPagination = useCallback((section: Exclude<ReviewPlatformDetailSection, 'overview'>, pagination: ReviewPlatformPagination) => {
    if (section === 'ci') {
      setCiPagination(pagination);
    } else if (section === 'files') {
      setChangePagination(pagination);
    } else if (section === 'commits') {
      setCommitPagination(pagination);
    } else {
      setReviewPagination(pagination);
    }
  }, []);

  const loadDetailSection = useCallback(async (
    repo: ReviewPlatformRepositoryRef | null,
    remoteId: string,
    pullRequestId: string,
    section: Exclude<ReviewPlatformDetailSection, 'overview'>,
    pageIndex: number,
    perPage: number,
    options?: { force?: boolean },
  ) => {
    const repositoryPath = workspacePath || repo?.workspacePath || '';
    const page = Math.max(1, pageIndex + 1);
    const cacheKey = detailPageCacheKey(repositoryPath, remoteId, pullRequestId, section, page, perPage);
    const overviewCacheKey = detailCacheKey(repositoryPath, remoteId, pullRequestId);
    const cached = detailPageCache.get(cacheKey);
    const force = options?.force === true;
    const matchesVerifiedOverview = (pageDetail: ReviewPlatformPullRequestDetail) => {
      const overview = detailCache.get(overviewCacheKey)?.detail;
      return Boolean(overview && samePullRequestRevisions(overview, pageDetail));
    };

    if (cached && !force) {
      if (!matchesVerifiedOverview(cached.detail)) {
        clearDetailPageCacheForPullRequest(repositoryPath, remoteId, pullRequestId);
        void loadDetail(repo, remoteId, pullRequestId, { force: true });
        return;
      }
      setDetail(prev => mergeDetailPage(prev, cached.detail));
      applySectionPagination(section, cached.detail.pagination);
      return;
    }

    const requestSeq = ++detailSectionRequestSeq.current;
    setDetailLoading(true);
    setDetailError(null);
    try {
      const nextPage = await reviewPlatformAPI.getPullRequestDetailPage({
        repositoryPath,
        remoteId,
        pullRequestId,
        section,
        page,
        perPage,
      });
      if (detailSectionRequestSeq.current !== requestSeq) return;
      if (!matchesVerifiedOverview(nextPage)) {
        clearDetailPageCacheForPullRequest(repositoryPath, remoteId, pullRequestId);
        void loadDetail(repo, remoteId, pullRequestId, { force: true });
        return;
      }
      detailPageCache.set(cacheKey, { detail: nextPage, fetchedAt: Date.now() });
      setDetail(prev => mergeDetailPage(prev, nextPage));
      applySectionPagination(section, nextPage.pagination);
    } catch (err) {
      if (detailSectionRequestSeq.current !== requestSeq) return;
      log.error('Failed to load pull request detail section', { pullRequestId, section, page, perPage, error: err });
      setDetailError(err instanceof Error ? err.message : 'Failed to load pull request details.');
    } finally {
      if (detailSectionRequestSeq.current === requestSeq) {
        setDetailLoading(false);
      }
    }
  }, [applySectionPagination, loadDetail, workspacePath]);

  useEffect(() => {
    void loadSnapshot(detailOnly && initialRemoteId ? initialRemoteId : undefined);
  }, [detailOnly, initialRemoteId, loadSnapshot]);

  useEffect(() => flowChatStore.subscribe(setFlowState), []);

  useEffect(() => {
    if (!selectedRemoteId) {
      setDetail(null);
      setVerifiedDetailKey(null);
      setDetailError(null);
      return;
    }
    if (!selectedPrId || (!repository && !workspacePath)) {
      setDetail(null);
      setVerifiedDetailKey(null);
      setDetailError(null);
      return;
    }
    void loadDetail(repository, selectedRemoteId, selectedPrId);
  }, [loadDetail, repository, selectedPrId, selectedRemoteId, workspacePath]);

  useEffect(() => {
    if (!snapshot.remotes.length) return;
    if (!selectedRemoteId && snapshot.selectedRemoteId) {
      setSelectedRemoteId(snapshot.selectedRemoteId);
    }
  }, [selectedRemoteId, snapshot.remotes.length, snapshot.selectedRemoteId]);

  useEffect(() => {
    if (!detailOnly) return;
    const targetPullRequestId = initialPullRequestId ?? initialPullRequestTarget?.pullRequestId ?? null;
    if (!targetPullRequestId) {
      if (initialPullRequestUrl) {
        setDetailError('This link is not a supported pull request URL.');
      }
      return;
    }

    const matchedRemote = initialRemoteId
      ? snapshot.remotes.find(remote => remote.id === initialRemoteId) ?? null
      : initialPullRequestTarget
        ? snapshot.remotes.find(remote => remoteMatchesPullRequestLink(remote, initialPullRequestTarget)) ?? null
        : null;
    const nextRemoteId = initialRemoteId
      ?? matchedRemote?.id
      ?? (snapshot.remotes.length === 1 ? snapshot.remotes[0].id : null)
      ?? snapshot.selectedRemoteId
      ?? selectedRemoteId;

    if (nextRemoteId && selectedRemoteId !== nextRemoteId) {
      setSelectedRemoteId(nextRemoteId);
      rememberRemote(workspacePath, nextRemoteId);
    }

    if (selectedPrId !== targetPullRequestId) {
      setSelectedPrId(targetPullRequestId);
    }
  }, [
    detailOnly,
    initialPullRequestId,
    initialPullRequestTarget,
    initialPullRequestUrl,
    initialRemoteId,
    selectedPrId,
    selectedRemoteId,
    snapshot.remotes,
    snapshot.selectedRemoteId,
    workspacePath,
  ]);

  useEffect(() => {
    setActiveTab('overview');
    setExpandedFileKeys(new Set());
    setExpandedCiItemIds(new Set());
    setCiLogById({});
    setCiLogErrorById({});
    setCiLogLoadingIds(new Set());
    setCiPageIndex(0);
    setChangePageIndex(0);
    setCommitPageIndex(0);
    setReviewPageIndex(0);
    setCiPagination(emptyPagination(1, CI_PAGE_SIZE));
    setChangePagination(emptyPagination(1, CHANGE_PAGE_SIZE));
    setCommitPagination(emptyPagination(1, COMMIT_PAGE_SIZE));
    setReviewPagination(emptyPagination(1, REVIEW_PAGE_SIZE));
  }, [selectedPrId]);

  useEffect(() => {
    if (activeTab !== 'changes' || changedFiles.length === 0 || expandedFileKeys.size > 0) return;
    setExpandedFileKeys(new Set(changedFiles.slice(0, 1).map(fileKey)));
  }, [activeTab, changedFiles, expandedFileKeys.size]);

  useEffect(() => {
    if (!hasDetail || !selectedRemoteId || !selectedPrId || (!repository && !workspacePath)) return;
    if (activeTab === 'overview') {
      void (async () => {
        await loadDetailSection(repository, selectedRemoteId, selectedPrId, 'ci', ciPageIndex, CI_PAGE_SIZE);
        await loadDetailSection(repository, selectedRemoteId, selectedPrId, 'reviews', reviewPageIndex, REVIEW_PAGE_SIZE);
      })();
    } else if (activeTab === 'changes') {
      void loadDetailSection(repository, selectedRemoteId, selectedPrId, 'files', changePageIndex, CHANGE_PAGE_SIZE);
    } else if (activeTab === 'commits') {
      void loadDetailSection(repository, selectedRemoteId, selectedPrId, 'commits', commitPageIndex, COMMIT_PAGE_SIZE);
    }
  }, [
    activeTab,
    ciPageIndex,
    changePageIndex,
    commitPageIndex,
    detail?.baseRevision,
    detail?.headRevision,
    hasDetail,
    loadDetailSection,
    repository,
    reviewPageIndex,
    selectedPrId,
    selectedRemoteId,
    workspacePath,
  ]);

  const visiblePullRequests = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return snapshot.pullRequests.filter(pr => {
      if (stateFilter !== 'all' && pr.state !== stateFilter) return false;
      if (!needle) return true;
      return [
        pr.title,
        pr.author,
        pr.sourceBranch,
        pr.targetBranch,
        `#${pr.number}`,
      ].some(value => value.toLowerCase().includes(needle));
    });
  }, [query, snapshot.pullRequests, stateFilter]);

  const parentSession = useMemo(() => {
    const sessions = Array.from(flowState.sessions.values());
    const activeSession = flowState.activeSessionId
      ? flowState.sessions.get(flowState.activeSessionId)
      : undefined;
    const sameWorkspace = (session?: Session) =>
      Boolean(session && (!workspacePath || normalizePath(session.workspacePath ?? '') === normalizePath(workspacePath)));

    if (activeSession?.sessionKind === 'normal' && sameWorkspace(activeSession)) {
      return activeSession;
    }

    if (
      activeSession &&
      (activeSession.sessionKind === 'review' || activeSession.sessionKind === 'deep_review') &&
      activeSession.parentSessionId
    ) {
      const parent = flowState.sessions.get(activeSession.parentSessionId);
      if (parent?.sessionKind === 'normal' && sameWorkspace(parent)) {
        return parent;
      }
    }

    return sessions
      .filter(session => session.sessionKind === 'normal' && sameWorkspace(session))
      .sort((left, right) => (right.lastActiveAt || right.updatedAt || right.createdAt) - (left.lastActiveAt || left.updatedAt || left.createdAt))[0];
  }, [flowState.activeSessionId, flowState.sessions, workspacePath]);

  const currentPullRequest = detail ?? selectedPr;
  const selectedDetailKey = selectedRemoteId && selectedPrId
    ? detailCacheKey(workspacePath || repository?.workspacePath || '', selectedRemoteId, selectedPrId)
    : null;
  const currentRevisionsVerified = Boolean(
    selectedDetailKey && verifiedDetailKey === selectedDetailKey,
  );

  const linkedReviewSessions = useMemo<LinkedReviewSession[]>(() => {
    if (!selectedRemote || !repository || !selectedPr || !currentPullRequest) {
      return [];
    }
    const sessions = Array.from(flowState.sessions.values());
    const markersByChildId = new Map<string, ReviewSessionMarker>();
    for (const session of sessions) {
      for (const marker of extractReviewSessionMarkers(session)) {
        markersByChildId.set(marker.childSessionId, marker);
      }
    }

    return sessions
      .filter(session =>
        session.sessionKind === 'review' || session.sessionKind === 'deep_review',
      )
      .map((session): LinkedReviewSession | null => {
        const marker = markersByChildId.get(session.sessionId);
        const evidence = session.reviewTargetEvidence
          ?? session.deepReviewRunManifest?.evidencePack?.reviewTarget;
        const identity = evidence?.pullRequest;
        if (
          !samePullRequestIdentity(identity, {
            platform: selectedRemote.platform,
            host: selectedRemote.host,
            projectPath: repository.projectPath,
            pullRequestId: selectedPr.id,
          })
        ) {
          return null;
        }
        const requestedFiles = marker?.requestedFiles ?? session.reviewTargetFilePaths ?? [];

        const reviewResultState = findLatestCodeReviewResultState(session);
        const reviewResult = reviewResultState.status === 'valid' ? reviewResultState.result : null;
        const summary = summarizeCodeReviewResult(reviewResult);
        const kind = session.sessionKind === 'deep_review' ? 'deep_review' : 'review';
        const runtimeEvidenceStatus = reviewResult?.evidence_status;
        const evidenceStatus = runtimeEvidenceStatus
          ?? (evidence?.completeness === 'complete' ? 'complete' : 'limited');
        const resultState = reviewResultState.status === 'valid'
          ? 'loaded'
          : session.historyState === 'metadata-only' || session.historyState === 'hydrating'
            ? 'unloaded'
            : reviewResultState.status;
        return {
          childSession: session,
          parentSession: marker?.parentSessionId ? flowState.sessions.get(marker.parentSessionId) : undefined,
          marker,
          kind,
          title: marker?.title || getSessionTitle(session, kind === 'deep_review' ? 'Review: Strict' : 'Review'),
          requestedFiles,
          resultState,
          issueCount: summary.issueCount,
          riskLevel: summary.riskLevel,
          lifecycle: reviewSessionLifecycle(session),
          freshness: effectivePullRequestReviewFreshness(
            evidence,
            currentPullRequest,
            currentRevisionsVerified,
            evidenceStatus,
          ),
          evidenceStatus,
          updatedAt: session.lastActiveAt || session.updatedAt || session.createdAt,
        };
      })
      .filter((session): session is LinkedReviewSession => Boolean(session))
      .sort((left, right) => right.updatedAt - left.updatedAt)
      .slice(0, MAX_LINKED_REVIEW_SESSIONS);
  }, [currentPullRequest, currentRevisionsVerified, flowState.sessions, repository, selectedPr, selectedRemote]);

  const latestCurrentReview = linkedReviewSessions.find((session) => session.freshness === 'current');
  const latestStaleReview = linkedReviewSessions.find((session) => session.freshness === 'stale');
  const latestUnknownReview = linkedReviewSessions.find((session) => session.freshness === 'unknown');

  const pagination = snapshot.pagination;
  const totalCount = pagination.total ?? null;
  const currentPageIndex = Math.max(0, (pagination.page || pageIndex + 1) - 1);
  const totalPages = totalCount !== null
    ? Math.max(1, Math.ceil(totalCount / pagination.perPage))
    : currentPageIndex + (pagination.hasNext ? 2 : 1);
  const pageStart = snapshot.pullRequests.length ? currentPageIndex * pagination.perPage + 1 : 0;
  const pageEnd = totalCount !== null
    ? Math.min(totalCount, currentPageIndex * pagination.perPage + snapshot.pullRequests.length)
    : currentPageIndex * pagination.perPage + snapshot.pullRequests.length;

  const summary = useMemo(() => {
    const prs = snapshot.pullRequests;
    return {
      open: prs.filter(pr => pr.state === 'open').length,
      draft: prs.filter(pr => pr.state === 'draft').length,
      merged: prs.filter(pr => pr.state === 'merged').length,
      reviewRequired: prs.filter(pr => pr.reviewDecision === 'changes_requested' || pr.reviewDecision === 'pending').length,
    };
  }, [snapshot.pullRequests]);

  const headerLabel = selectedRemote ? remoteLabel(selectedRemote) : repository ? repository.projectPath : 'No repository';
  const isGithubUserList = !detailOnly && selectedRemote?.platform === 'github';
  const panelTitle = detailOnly ? 'Pull Request' : isGithubUserList ? 'My Open Pull Requests' : 'Pull Requests';

  const handleRemoteChange = useCallback((value: string | number | (string | number)[]) => {
    const remoteId = Array.isArray(value) ? String(value[0] ?? '') : String(value);
    setSelectedRemoteId(remoteId || null);
    setSelectedPrId(null);
    setDetail(null);
    setDetailError(null);
    setStateFilter('all');
    setPageIndex(0);
    rememberRemote(workspacePath, remoteId || null);
    void loadSnapshot(remoteId || null, { page: 1 });
  }, [loadSnapshot, workspacePath]);

  const handlePageChange = useCallback((nextPageIndex: number) => {
    const nextPage = Math.max(1, nextPageIndex + 1);
    setSelectedPrId(null);
    setDetail(null);
    setDetailError(null);
    setPageIndex(nextPage - 1);
    void loadSnapshot(listRemoteId, { page: nextPage });
  }, [listRemoteId, loadSnapshot]);

  const toggleFileExpanded = useCallback((key: string) => {
    setExpandedFileKeys(prev => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }, []);

  const renderDetailPagination = useCallback((
    label: string,
    page: PageInfo,
    itemCount: number,
    onPageChange: (nextPageIndex: number) => void,
  ) => {
    if (itemCount <= 0 || (page.totalPages <= 1 && !page.hasNext && page.pageIndex === 0)) return null;
    return (
      <div className="review-platform__pagination review-platform__detail-pagination">
        <IconButton
          className="review-platform__icon-button"
          size="xs"
          variant="ghost"
          tooltip={`Previous ${label} page`}
          disabled={page.pageIndex === 0}
          onClick={() => onPageChange(page.pageIndex - 1)}
        >
          <ChevronLeft size={14} />
        </IconButton>
        <span>
          {label}: {page.start}-{page.end} of {page.totalLabel}
        </span>
        <IconButton
          className="review-platform__icon-button"
          size="xs"
          variant="ghost"
          tooltip={`Next ${label} page`}
          disabled={!page.hasNext && page.pageIndex >= page.totalPages - 1}
          onClick={() => onPageChange(page.pageIndex + 1)}
        >
          <ChevronRight size={14} />
        </IconButton>
      </div>
    );
  }, []);

  const renderDetailLoading = useCallback((message: string, refreshing = false) => (
    <div className={`review-platform__thread-loading${refreshing ? ' review-platform__thread-loading--refreshing' : ''}`} aria-live="polite">
      <Loader2 size={14} />
      <span>{message}</span>
    </div>
  ), []);

  const handleOpenExternal = useCallback(async () => {
    const webUrl = selectedPr?.webUrl || initialPullRequestUrl;
    if (!webUrl) return;
    try {
      await systemAPI.openExternal(webUrl);
    } catch (error) {
      log.error('Failed to open pull request URL', { error, webUrl });
    }
  }, [initialPullRequestUrl, selectedPr?.webUrl]);

  const handleOpenCiUrl = useCallback(async (webUrl?: string | null) => {
    if (!webUrl) return;
    try {
      await systemAPI.openExternal(webUrl);
    } catch (error) {
      log.error('Failed to open CI URL', { error, webUrl });
    }
  }, []);

  const loadCiLog = useCallback(async (item: ReviewPlatformCiItem): Promise<ReviewPlatformCiLog | null> => {
    const cached = ciLogById[item.id];
    if (cached) return cached;
    if (!canLoadCiLog(selectedRemote, item)) {
      return {
        ciItemId: item.id,
        log: item.log ?? null,
        truncated: item.logTruncated,
        message: item.detail || null,
      };
    }
    if ((!repository && !workspacePath) || !selectedRemoteId || !selectedPrId) return null;

    const repositoryPath = workspacePath || repository?.workspacePath || '';
    setCiLogLoadingIds(prev => {
      const next = new Set(prev);
      next.add(item.id);
      return next;
    });
    setCiLogErrorById(prev => {
      const next = { ...prev };
      delete next[item.id];
      return next;
    });

    try {
      const nextLog = await reviewPlatformAPI.getPullRequestCiLog({
        repositoryPath,
        remoteId: selectedRemoteId,
        pullRequestId: selectedPrId,
        ciItemId: item.id,
        ciItemName: item.name,
      });
      setCiLogById(prev => ({ ...prev, [item.id]: nextLog }));
      return nextLog;
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to load CI error log.';
      setCiLogErrorById(prev => ({ ...prev, [item.id]: message }));
      log.error('Failed to load CI log', { itemId: item.id, error: err });
      return null;
    } finally {
      setCiLogLoadingIds(prev => {
        const next = new Set(prev);
        next.delete(item.id);
        return next;
      });
    }
  }, [ciLogById, repository, selectedPrId, selectedRemote, selectedRemoteId, workspacePath]);

  const toggleCiExpanded = useCallback((item: ReviewPlatformCiItem) => {
    if (expandedCiItemIds.has(item.id)) {
      setExpandedCiItemIds(prev => {
        const next = new Set(prev);
        next.delete(item.id);
        return next;
      });
      return;
    }

    setExpandedCiItemIds(prev => {
      const next = new Set(prev);
      next.add(item.id);
      return next;
    });
    if (canLoadCiLog(selectedRemote, item) || item.log) {
      void loadCiLog(item);
    }
  }, [expandedCiItemIds, loadCiLog, selectedRemote]);

  const addPullRequestContextToChat = useCallback(async (input: {
    label: string;
    section: PullRequestContext['section'];
    content: string;
    metadata?: Record<string, unknown>;
  }) => {
    if (!parentSession) {
      notificationService.warning('Open or create a chat session before sending PR context.', { duration: 3500 });
      return;
    }

    await openMainSession(parentSession.sessionId);
    const context: PullRequestContext = {
      id: createContextId('pr'),
      type: 'pull-request',
      label: input.label,
      section: input.section,
      content: input.content,
      metadata: input.metadata,
      timestamp: Date.now(),
      sourceUrl: selectedPr?.webUrl || initialPullRequestUrl,
      remoteId: selectedRemote?.id,
      repository: repository?.projectPath ?? selectedRemote?.projectPath,
      pullRequestNumber: selectedPr?.number,
      pullRequestTitle: selectedPr?.title,
    };

    useContextStore.getState().addContext(context);
    window.dispatchEvent(new CustomEvent('insert-context-tag', { detail: { context } }));
  }, [initialPullRequestUrl, parentSession, repository?.projectPath, selectedPr, selectedRemote?.id, selectedRemote?.projectPath]);

  const handleFillPrContext = useCallback(async () => {
    if (!selectedPr) return;
    await addPullRequestContextToChat({
      label: `PR #${selectedPr.number} overview`,
      section: 'overview',
      content: buildPrOverviewContext({
        pr: selectedPr,
        detail,
        remote: selectedRemote,
        repository,
        filePaths: prFilePaths,
        reviewItemCount,
        webUrl: selectedPr.webUrl,
      }),
    });
  }, [addPullRequestContextToChat, detail, prFilePaths, repository, reviewItemCount, selectedPr, selectedRemote]);

  const handleStartReview = useCallback(async () => {
    if (!workspacePath || !selectedRemote || !repository || !selectedPr || !parentSession) {
      notificationService.warning('Open or create a chat session before reviewing this pull request.', {
        duration: 3500,
      });
      return;
    }
    if (reviewLaunchInFlight.current || latestCurrentReview?.lifecycle === 'running') {
      return;
    }
    reviewLaunchInFlight.current = true;
    setReviewLaunching(true);
    let sharedLaunchKey: string | null = null;
    let ownsSharedLaunch = false;
    try {
      const reviewTarget = await reviewPlatformAPI.getPullRequestReviewTarget(
        workspacePath,
        selectedRemote.id,
        selectedPr.id,
      );
      const freshPullRequest = reviewTarget.pullRequest;
      const freshIdentity = {
        platform: selectedRemote.platform,
        host: selectedRemote.host,
        projectPath: repository.projectPath,
        pullRequestId: freshPullRequest.id,
      };
      const runningReviewExists = () => Array.from(flowChatStore.getState().sessions.values()).some((session) => {
        if (
          (session.sessionKind !== 'review' && session.sessionKind !== 'deep_review')
          || reviewSessionLifecycle(session) !== 'running'
        ) {
          return false;
        }
        const evidence = session.reviewTargetEvidence
          ?? session.deepReviewRunManifest?.evidencePack?.reviewTarget;
        return samePullRequestIdentity(evidence?.pullRequest, freshIdentity)
          && pullRequestReviewFreshness(evidence, freshPullRequest) === 'current';
      });
      sharedLaunchKey = pullRequestReviewLaunchKey({
        ...freshIdentity,
        baseRevision: freshPullRequest.baseRevision,
        headRevision: freshPullRequest.headRevision,
      });
      const cacheKey = detailCacheKey(workspacePath, selectedRemote.id, selectedPr.id);
      setDetail((current) => current ? { ...current, ...reviewTarget.pullRequest } : current);
      setSnapshot((current) => ({
        ...current,
        pullRequests: current.pullRequests.map((pullRequest) =>
          pullRequest.id === freshPullRequest.id
            ? { ...pullRequest, ...freshPullRequest }
            : pullRequest,
        ),
      }));
      const cached = detailCache.get(cacheKey);
      if (cached) {
        detailCache.set(cacheKey, {
          detail: { ...cached.detail, ...freshPullRequest },
          fetchedAt: Date.now(),
        });
      }
      setVerifiedDetailKey(cacheKey);
      if (runningReviewExists()) {
        return;
      }
      const prepared = await prepareReviewLaunchFromPullRequest({
        workspacePath,
        remote: selectedRemote,
        repository,
        reviewTarget,
      });
      if (prepared.mode === 'strict' && prepared.requiresConsent) {
        const confirmed = await confirmDeepReviewLaunch(prepared.runManifest, {
          sessionConcurrencyGuard: deriveDeepReviewSessionConcurrencyGuard(
            flowChatStore.getState(),
            parentSession.sessionId,
          ),
        });
        if (!confirmed) return;
      }
      if (runningReviewExists() || reviewLaunchesInFlight.has(sharedLaunchKey)) {
        return;
      }
      reviewLaunchesInFlight.add(sharedLaunchKey);
      ownsSharedLaunch = true;
      const launched = await launchPreparedReviewSession({
        parentSessionId: parentSession.sessionId,
        workspacePath,
        displayMessage: `Review pull request #${reviewTarget.pullRequest.number}`,
        childSessionName: `Review: PR #${reviewTarget.pullRequest.number}`,
        prepared,
      });
      if (launched.launchStatus === 'uncertain') {
        notificationService.warning('Review started, but its start acknowledgement is uncertain.', {
          duration: 8000,
        });
      }
    } catch (reviewError) {
      log.error('Failed to start pull request Review', {
        pullRequestId: selectedPr.id,
        error: reviewError,
      });
      notificationService.error(
        reviewError instanceof Error ? reviewError.message : 'Failed to start pull request Review.',
        { duration: 6000 },
      );
    } finally {
      if (sharedLaunchKey && ownsSharedLaunch) {
        reviewLaunchesInFlight.delete(sharedLaunchKey);
      }
      reviewLaunchInFlight.current = false;
      setReviewLaunching(false);
    }
  }, [
    confirmDeepReviewLaunch,
    latestCurrentReview?.lifecycle,
    parentSession,
    repository,
    selectedPr,
    selectedRemote,
    workspacePath,
  ]);

  const handleAddFileDiffContext = useCallback(async (file: ReviewPlatformFile) => {
    if (!selectedPr) return;
    await addPullRequestContextToChat({
      label: `PR #${selectedPr.number} ${file.path}`,
      section: 'file-diff',
      content: buildPrFileDiffContext(selectedPr, file),
    });
  }, [addPullRequestContextToChat, selectedPr]);

  const handleAddCommitsContext = useCallback(async () => {
    if (!selectedPr) return;
    await addPullRequestContextToChat({
      label: `PR #${selectedPr.number} commits`,
      section: 'commits',
      content: buildPrCommitsContext(selectedPr, detail?.commits ?? []),
    });
  }, [addPullRequestContextToChat, detail?.commits, selectedPr]);

  const handleAddReviewsContext = useCallback(async () => {
    if (!selectedPr) return;
    await addPullRequestContextToChat({
      label: `PR #${selectedPr.number} reviews`,
      section: 'reviews',
      content: buildPrReviewsContext(selectedPr, detail?.threads ?? []),
    });
  }, [addPullRequestContextToChat, detail?.threads, selectedPr]);

  const handleAddCiPageContext = useCallback(async () => {
    if (!selectedPr) return;
    await addPullRequestContextToChat({
      label: `PR #${selectedPr.number} CI page`,
      section: 'ci',
      content: buildPrCiContext(selectedPr, detail?.ci ?? []),
    });
  }, [addPullRequestContextToChat, detail?.ci, selectedPr]);

  const handleAddCiItemContext = useCallback(async (item: ReviewPlatformCiItem) => {
    if (!selectedPr) return;
    const ciLog = ciLogById[item.id] ?? await loadCiLog(item);
    await addPullRequestContextToChat({
      label: `PR #${selectedPr.number} CI · ${item.name}`,
      section: 'ci',
      content: buildPrCiItemContext(selectedPr, item, ciLog),
      metadata: {
        ciItemId: item.id,
        ciItemName: item.name,
        ciItemStatus: item.status,
        ciItemConclusion: item.conclusion,
        ciItemStage: item.stage,
        ciLogTruncated: ciLog?.truncated ?? false,
      },
    });
  }, [addPullRequestContextToChat, ciLogById, loadCiLog, selectedPr]);

  const refreshAuthSnapshot = useCallback((remoteId: string | null) => {
    snapshotCache.clear();
    detailCache.clear();
    detailPageCache.clear();
    void loadSnapshot(detailOnly ? remoteId : listRemoteId, { force: true, page: currentPageIndex + 1 });
  }, [currentPageIndex, detailOnly, listRemoteId, loadSnapshot]);

  const handleOpenAuthModal = useCallback(() => {
    setAuthToken('');
    setAuthError(null);
    setAuthModalOpen(true);
  }, []);

  const handleSaveAuthToken = useCallback(async () => {
    if (!selectedRemote || selectedRemote.platform === 'unknown' || selectedRemote.platform === 'github') return;
    const token = authToken.trim();
    if (!token) {
      setAuthError('Token is required.');
      return;
    }

    setAuthSaving(true);
    setAuthError(null);
    try {
      await reviewPlatformAPI.updateAuthToken({
        platform: selectedRemote.platform,
        host: selectedRemote.host,
        token,
      });
      setAuthModalOpen(false);
      setAuthToken('');
      refreshAuthSnapshot(selectedRemote.id);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to save token.';
      setAuthError(message);
      log.error('Failed to save review platform token', { error: err, host: selectedRemote.host });
    } finally {
      setAuthSaving(false);
    }
  }, [authToken, refreshAuthSnapshot, selectedRemote]);

  const handleOpenGithubAuthTerminal = useCallback(async () => {
    if (!selectedRemote || selectedRemote.platform !== 'github') return;
    const command = `gh auth login --hostname ${selectedRemote.host}`;
    setAuthSaving(true);
    setAuthError(null);
    try {
      await systemAPI.setClipboard(command);
      setAuthModalOpen(false);
      quickActions.openTerminal(undefined, workspacePath);
      notificationService.success('GitHub CLI login command copied. Paste it in the terminal to continue.', {
        duration: 3500,
      });
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to open GitHub CLI authentication.';
      setAuthError(message);
      log.error('Failed to prepare GitHub CLI authentication', { error: err, host: selectedRemote.host });
    } finally {
      setAuthSaving(false);
    }
  }, [selectedRemote, workspacePath]);

  const handleCopyGithubAuthCommand = useCallback(async () => {
    if (!selectedRemote || selectedRemote.platform !== 'github') return;
    setAuthError(null);
    try {
      await systemAPI.setClipboard(`gh auth login --hostname ${selectedRemote.host}`);
      notificationService.success('GitHub CLI login command copied.', { duration: 2500 });
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to copy GitHub CLI login command.';
      setAuthError(message);
      log.error('Failed to copy GitHub CLI authentication command', { error: err, host: selectedRemote.host });
    }
  }, [selectedRemote]);

  const handleClearAuthToken = useCallback(async () => {
    if (!selectedRemote || selectedRemote.platform === 'unknown') return;
    setAuthSaving(true);
    setAuthError(null);
    try {
      await reviewPlatformAPI.clearAuthToken({
        platform: selectedRemote.platform,
        host: selectedRemote.host,
      });
      refreshAuthSnapshot(selectedRemote.id);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to clear token.';
      setAuthError(message);
      setAuthModalOpen(true);
      log.error('Failed to clear review platform token', { error: err, host: selectedRemote.host });
    } finally {
      setAuthSaving(false);
    }
  }, [refreshAuthSnapshot, selectedRemote]);

  const renderAuthGate = useCallback((mode: 'inline' | 'detail' = 'inline') => {
    if (!authChallenge || !selectedRemote || selectedRemote.platform === 'unknown') return null;
    return (
      <div className={`review-platform__auth-gate review-platform__auth-gate--${mode}`}>
        <div className="review-platform__auth-gate-icon">
          <KeyRound size={18} />
        </div>
        <div className="review-platform__auth-gate-copy">
          <strong>{authChallengeTitle(authChallenge)}</strong>
          <span>{authChallenge.message}</span>
          <span>{authChallenge.host} · {authChallenge.projectPath}</span>
          <span>{selectedRemote.platform === 'github' ? 'CLI authorization' : 'Required scopes'}: {authChallengeScopes(authChallenge)}</span>
        </div>
        <div className="review-platform__auth-gate-actions">
          <Button className="review-platform__panel-button" size="small" variant="primary" onClick={handleOpenAuthModal} disabled={authSaving}>
            <KeyRound size={13} />
            {selectedRemote.platform === 'github' ? 'Authenticate' : authChallenge.state === 'missing' ? 'Add token' : 'Update token'}
          </Button>
          <Button className="review-platform__panel-button" size="small" variant="secondary" onClick={() => refreshAuthSnapshot(selectedRemote.id)} disabled={authSaving || loading}>
            <RefreshCw size={13} />
            Retry
          </Button>
        </div>
      </div>
    );
  }, [authChallenge, authSaving, handleOpenAuthModal, loading, refreshAuthSnapshot, selectedRemote]);

  const handleRetryDetail = useCallback(() => {
    if ((!repository && !workspacePath) || !selectedRemoteId || !selectedPrId) return;
    if (activeTab === 'overview') {
      void (async () => {
        await loadDetail(repository, selectedRemoteId, selectedPrId, { force: true });
        await loadDetailSection(repository, selectedRemoteId, selectedPrId, 'ci', ciPageIndex, CI_PAGE_SIZE, { force: true });
        await loadDetailSection(repository, selectedRemoteId, selectedPrId, 'reviews', reviewPageIndex, REVIEW_PAGE_SIZE, { force: true });
      })();
      return;
    }
    if (activeTab === 'changes') {
      void loadDetailSection(repository, selectedRemoteId, selectedPrId, 'files', changePageIndex, CHANGE_PAGE_SIZE, { force: true });
      return;
    }
    if (activeTab === 'commits') {
      void loadDetailSection(repository, selectedRemoteId, selectedPrId, 'commits', commitPageIndex, COMMIT_PAGE_SIZE, { force: true });
      return;
    }
    void loadDetail(repository, selectedRemoteId, selectedPrId, { force: true });
  }, [
    activeTab,
    changePageIndex,
    commitPageIndex,
    loadDetail,
    loadDetailSection,
    ciPageIndex,
    repository,
    reviewPageIndex,
    selectedPrId,
    selectedRemoteId,
    workspacePath,
  ]);

  const handleRefreshDetail = useCallback(async () => {
    if ((!repository && !workspacePath) || !selectedRemoteId || !selectedPrId) return;
    await loadDetail(repository, selectedRemoteId, selectedPrId, { force: true });
    if (activeTab === 'overview') {
      await loadDetailSection(repository, selectedRemoteId, selectedPrId, 'ci', ciPageIndex, CI_PAGE_SIZE, { force: true });
      await loadDetailSection(repository, selectedRemoteId, selectedPrId, 'reviews', reviewPageIndex, REVIEW_PAGE_SIZE, { force: true });
    } else if (activeTab === 'changes') {
      await loadDetailSection(repository, selectedRemoteId, selectedPrId, 'files', changePageIndex, CHANGE_PAGE_SIZE, { force: true });
    } else if (activeTab === 'commits') {
      await loadDetailSection(repository, selectedRemoteId, selectedPrId, 'commits', commitPageIndex, COMMIT_PAGE_SIZE, { force: true });
    }
  }, [activeTab, changePageIndex, ciPageIndex, commitPageIndex, loadDetail, loadDetailSection, repository, reviewPageIndex, selectedPrId, selectedRemoteId, workspacePath]);

  const remoteStatus = selectedRemote
    ? `${providerLabel(selectedRemote)} · ${authLabel(account)}`
    : 'No remote detected';
  const displayPr = currentPullRequest;
  const checksText = displayPr && displayPr.checks.total > 0
    ? `${displayPr.checks.passed}/${displayPr.checks.total}`
    : 'N/A';
  const emptyStateMessage = snapshot.message
    || account?.message
    || selectedRemote?.message
    || (snapshot.remotes.length
      ? isGithubUserList && !query.trim()
        ? 'No open pull requests authored by the current GitHub CLI account.'
        : 'No pull requests match the current filter.'
      : 'No supported remotes were detected.');
  const loadingLabel = loading
    ? snapshotCacheState === 'refreshing'
      ? 'Refreshing cached pull requests...'
      : 'Loading pull requests...'
    : snapshotCacheState === 'cached'
      ? 'Cached pull requests'
      : null;
  const checksStatusText = !displayPr || displayPr.checks.total === 0
    ? 'No checks'
    : displayPr.checks.failed > 0
      ? `${displayPr.checks.failed} failed`
      : displayPr.checks.pending > 0
        ? `${displayPr.checks.pending} pending`
        : 'All checks passed';
  const commentsText = reviewItemCount > 0
    ? `${reviewItemCount} comment${reviewItemCount === 1 ? '' : 's'}`
    : 'No comments';
  const reviewStatusText = latestCurrentReview
    ? currentPullRequestReviewStatusText(latestCurrentReview)
    : latestStaleReview
      ? 'Previous Review is stale because the PR revisions or runtime evidence changed'
      : latestUnknownReview
        ? 'Review result cannot be matched to current PR revisions · refresh PR'
        : 'No Review has run for the current PR revisions';
  const handleOpenLatestReview = () => {
    const linked = latestCurrentReview ?? latestStaleReview ?? latestUnknownReview;
    const linkedParentSessionId = linked?.childSession.parentSessionId ?? linked?.parentSession?.sessionId;
    if (!linked || !linkedParentSessionId) return;
    openBtwSessionInAuxPane({
      childSessionId: linked.childSession.sessionId,
      parentSessionId: linkedParentSessionId,
      workspacePath: linked.childSession.workspacePath,
      expand: true,
      sessionKind: linked.kind,
      sessionTitle: linked.title,
      agentType: linked.childSession.config.agentType ?? (linked.kind === 'deep_review' ? 'DeepReview' : 'CodeReview'),
    });
  };

  return (
    <div className={`review-platform${detailOnly ? ' review-platform--detail-only' : ''}`}>
      {!detailOnly && (
        <div className="review-platform__topbar">
          <div className="review-platform__brand">
            <span className="review-platform__brand-icon"><GitPullRequest size={17} /></span>
            <div className="review-platform__brand-copy">
              <span className="review-platform__title">{panelTitle}</span>
              <span className="review-platform__subtitle">{headerLabel}</span>
            </div>
          </div>

          <div className="review-platform__topbar-actions">
            <div className="review-platform__remote-select">
              <Select
                size="small"
                value={selectedRemoteId ?? ''}
                options={remoteOptions}
                placeholder="Select remote"
                disabled={!remoteOptions.length || loading}
                searchable
                onChange={handleRemoteChange}
              />
            </div>
            {account && (
              <Tooltip content={`${account.label} · ${authSourceLabel(account.authSource)}`}>
                <span className={`review-platform__account review-platform__account--${account.authState}`}>
                  <ShieldCheck size={13} />
                  <span>{authLabel(account)}</span>
                </span>
              </Tooltip>
            )}
            <IconButton
              className="review-platform__icon-button"
              size="xs"
              variant="ghost"
              tooltip={selectedRemote?.platform === 'github' ? 'GitHub CLI authentication' : account?.authSource === 'stored' ? 'Update token' : 'Add token'}
              disabled={!selectedRemote || selectedRemote.platform === 'unknown' || loading || authSaving}
              onClick={handleOpenAuthModal}
            >
              <KeyRound size={14} />
            </IconButton>
            {account?.authSource === 'stored' && (
              <IconButton
                className="review-platform__icon-button"
                size="xs"
                variant="ghost"
                tooltip="Clear token"
                disabled={!selectedRemote || loading || authSaving}
                onClick={handleClearAuthToken}
              >
                <Trash2 size={14} />
              </IconButton>
            )}
            <IconButton
              className="review-platform__icon-button"
              size="xs"
              variant="ghost"
              tooltip="Refresh"
              onClick={() => void loadSnapshot(listRemoteId, { force: true, page: currentPageIndex + 1 })}
              isLoading={loading}
            >
              <RefreshCw size={14} />
            </IconButton>
          </div>
        </div>
      )}

      {!detailOnly && (
      <div className="review-platform__subbar">
        <div className="review-platform__status-line">
          <span><CircleDot size={12} /> {summary.open} open on page</span>
          {!isGithubUserList && <span><GitPullRequestClosed size={12} /> {summary.merged} merged on page</span>}
          <span><Sparkles size={12} /> {summary.reviewRequired} review on page</span>
          <span><Link2 size={12} /> {remoteStatus}</span>
          {loadingLabel && (
            <span className={loading ? 'review-platform__loading-status' : 'review-platform__cache-label'}>
              {loading && <Loader2 size={12} className="review-platform__loading-inline review-platform__loading-inline--icon" />}
              {loadingLabel}
            </span>
          )}
        </div>
      </div>
      )}

      {authChallenge && !detailOnly && renderAuthGate('inline')}

      <div className="review-platform__body">
        {!detailOnly && (
        <aside className="review-platform__list" aria-label="Pull request list">
          <div className="review-platform__list-toolbar">
            <Input
              inputSize="small"
              value={query}
              onChange={event => setQuery(event.target.value)}
              placeholder="Search pull requests"
              prefix={<Search size={14} />}
              suffix={query ? <IconButton className="review-platform__icon-button" size="xs" variant="ghost" onClick={() => setQuery('')}><XCircle size={14} /></IconButton> : undefined}
            />
            {!isGithubUserList && (
              <div className="review-platform__state-filters">
                {(['all', 'open', 'draft', 'merged', 'closed'] as ListStateFilter[]).map(state => (
                  <button
                    key={state}
                    type="button"
                    className={`review-platform__state-chip${stateFilter === state ? ' is-active' : ''}`}
                    onClick={() => setStateFilter(state)}
                  >
                    {state === 'all' ? 'All' : stateLabel(state)}
                  </button>
                ))}
              </div>
            )}
          </div>

          <div className="review-platform__list-scroll">
            {loading && (
              <div className="review-platform__empty-state">Loading pull requests...</div>
            )}
            {error && (
              <div className="review-platform__error-state">
                <XCircle size={16} />
                <span>{error}</span>
                <Button className="review-platform__panel-button" size="small" variant="secondary" onClick={() => void loadSnapshot(listRemoteId, { force: true, page: currentPageIndex + 1 })}>
                  Retry
                </Button>
              </div>
            )}
            {!loading && !error && !authChallenge && !visiblePullRequests.length && (
              <div className="review-platform__empty-state">
                <GitPullRequest size={18} />
                <span>{emptyStateMessage}</span>
              </div>
            )}
            {!loading && !error && visiblePullRequests.map(pr => (
              (() => {
                const pullRequestRemote = pr.providerId
                  ? snapshot.remotes.find(remote => remote.id === pr.providerId)
                  : selectedRemote;
                return (
                  <button
                    key={`${pr.providerId ?? selectedRemoteId ?? 'remote'}:${pr.id}`}
                    type="button"
                    className={`review-platform__pr-row${selectedPrId === pr.id && (!pr.providerId || pr.providerId === selectedRemoteId) ? ' is-selected' : ''}`}
                    onClick={() => {
                      if (pr.providerId && pr.providerId !== selectedRemoteId) {
                        setSelectedRemoteId(pr.providerId);
                        rememberRemote(workspacePath, pr.providerId);
                      }
                      setSelectedPrId(pr.id);
                    }}
                  >
                    <span className="review-platform__pr-icon">{getPrIcon(pr)}</span>
                    <span className="review-platform__pr-main">
                      <span className="review-platform__pr-title">{pr.title}</span>
                      <span className="review-platform__pr-meta">
                        {pullRequestRemote?.projectPath ? `${pullRequestRemote.projectPath} · ` : ''}#{pr.number} · {pr.sourceBranch} → {pr.targetBranch}
                      </span>
                      <span className="review-platform__pr-meta review-platform__pr-meta--secondary">
                        {pr.author} · {formatRelativeTime(pr.updatedAt)}
                      </span>
                    </span>
                    <span className="review-platform__pr-stats">
                      <span className={`review-platform__decision review-platform__decision--${pr.reviewDecision}`}>
                        {decisionLabel(pr.reviewDecision)}
                      </span>
                      <span className="review-platform__counts">
                        <span>{pr.changedFiles} files</span>
                        <span className="review-platform__additions">+{pr.additions}</span>
                        <span className="review-platform__deletions">-{pr.deletions}</span>
                      </span>
                    </span>
                  </button>
                );
              })()
            ))}
          </div>
          {!loading && !error && (totalPages > 1 || pagination.hasNext) && (
            <div className="review-platform__pagination">
              <IconButton
                className="review-platform__icon-button"
                size="xs"
                variant="ghost"
                tooltip="Previous page"
                disabled={currentPageIndex === 0}
                onClick={() => handlePageChange(currentPageIndex - 1)}
              >
                <ChevronLeft size={14} />
              </IconButton>
              <span>
                {pageStart}-{pageEnd} of {totalCount ?? `${pageEnd}+`}
              </span>
              <IconButton
                className="review-platform__icon-button"
                size="xs"
                variant="ghost"
                tooltip="Next page"
                disabled={!pagination.hasNext && currentPageIndex >= totalPages - 1}
                onClick={() => handlePageChange(currentPageIndex + 1)}
              >
                <ChevronRight size={14} />
              </IconButton>
            </div>
          )}
        </aside>
        )}

        <main className="review-platform__detail">
          {!selectedPr && detailOnly && (loading || detailLoading) && (
            <div className="review-platform__detail-empty">
              <Loader2 size={20} className="review-platform__loading-inline review-platform__loading-inline--icon" />
              <span>Loading pull request details...</span>
            </div>
          )}

          {!selectedPr && detailOnly && !loading && !detailLoading && authChallenge && (
            <div className="review-platform__detail-empty">
              {renderAuthGate('detail')}
            </div>
          )}

          {!selectedPr && detailOnly && !loading && !detailLoading && !authChallenge && (detailError || error) && (
            <div className="review-platform__detail-empty">
              <XCircle size={24} />
              <span>{detailError || error}</span>
              <div className="review-platform__detail-empty-actions">
                <Button className="review-platform__panel-button" size="small" variant="secondary" onClick={handleRetryDetail}>
                  Retry
                </Button>
                {selectedRemote && selectedRemote.platform !== 'unknown' && (
                  <Button className="review-platform__panel-button" size="small" variant="secondary" onClick={handleOpenAuthModal} disabled={authSaving}>
                    <KeyRound size={13} />
                    {selectedRemote.platform === 'github' ? 'Authenticate' : account?.authSource === 'stored' ? 'Update token' : 'Add token'}
                  </Button>
                )}
              </div>
            </div>
          )}

          {!selectedPr && detailOnly && !loading && !detailLoading && !authChallenge && !detailError && !error && (
            <div className="review-platform__detail-empty">
              <GitPullRequest size={24} />
              <span>
                {snapshot.message
                  || 'This pull request could not be resolved from the remotes of the current workspace.'}
              </span>
              <div className="review-platform__detail-empty-actions">
                <Button
                  className="review-platform__panel-button"
                  size="small"
                  variant="secondary"
                  onClick={() => void loadSnapshot(undefined, { force: true })}
                >
                  Retry
                </Button>
                {initialPullRequestUrl && (
                  <Button
                    className="review-platform__panel-button"
                    size="small"
                    variant="secondary"
                    onClick={handleOpenExternal}
                  >
                    <ExternalLink size={13} />
                    Open in browser
                  </Button>
                )}
              </div>
            </div>
          )}

          {!selectedPr && !detailOnly && !loading && (
            <div className="review-platform__detail-empty">
              <GitPullRequest size={24} />
              <span>Select a pull request to inspect it.</span>
            </div>
          )}

          {selectedPr && (
            <>
              <div className="review-platform__detail-header">
                <div className="review-platform__detail-title-block">
                  <div className="review-platform__detail-title-row">
                    {getPrIcon(selectedPr)}
                    <h3>{selectedPr.title}</h3>
                    <span className={`review-platform__detail-state review-platform__detail-state--${displayPr?.state ?? selectedPr.state}`}>
                      {stateLabel(displayPr?.state ?? selectedPr.state)}
                    </span>
                  </div>
                  <div className="review-platform__detail-meta">
                    <span>#{selectedPr.number}</span>
                    <span><Clock3 size={12} /> {formatAbsoluteTime(selectedPr.updatedAt) || formatRelativeTime(selectedPr.updatedAt)}</span>
                  </div>
                </div>
                <div className="review-platform__detail-actions">
                  <Tooltip content={!parentSession ? 'Open or create a chat first' : 'Start Review'}>
                    <span>
                      <Button
                        className="review-platform__panel-button"
                        size="small"
                        variant="primary"
                        onClick={handleStartReview}
                        disabled={
                          !parentSession ||
                          !repository ||
                          !selectedRemote ||
                          reviewLaunching ||
                          detailLoading ||
                          latestCurrentReview?.lifecycle === 'running'
                        }
                        isLoading={reviewLaunching}
                      >
                        <Sparkles size={13} />
                        {latestCurrentReview?.lifecycle === 'running' ? 'Review running' : 'Review'}
                      </Button>
                    </span>
                  </Tooltip>
                  <Button className="review-platform__panel-button" size="small" variant="secondary" onClick={handleOpenExternal} disabled={!selectedPr.webUrl && !initialPullRequestUrl}>
                    <Link2 size={13} />
                    Open
                  </Button>
                  {detailOnly && selectedRemote && selectedRemote.platform !== 'unknown' && (
                    <IconButton
                      className="review-platform__icon-button"
                      size="xs"
                      variant="ghost"
                      tooltip={selectedRemote.platform === 'github' ? 'GitHub CLI authentication' : account?.authSource === 'stored' ? 'Update token' : 'Add token'}
                      onClick={handleOpenAuthModal}
                      disabled={authSaving}
                    >
                      <KeyRound size={14} />
                    </IconButton>
                  )}
                  <IconButton
                    className="review-platform__icon-button"
                    size="xs"
                    variant="ghost"
                    tooltip="Refresh pull request"
                    disabled={detailLoading}
                    onClick={handleRefreshDetail}
                    isLoading={detailLoading}
                  >
                    <RefreshCw size={14} />
                  </IconButton>
                </div>
              </div>

              <div className="review-platform__fact-list">
                <div className="review-platform__fact-row">
                  <span className="review-platform__fact-label"><Code2 size={14} /> Branches</span>
                  <div className="review-platform__fact-value review-platform__fact-value--branch">
                    <strong>{displayPr?.sourceBranch ?? selectedPr.sourceBranch}</strong>
                    <ChevronRight size={13} />
                    <strong>{displayPr?.targetBranch ?? selectedPr.targetBranch}</strong>
                    <span>{displayPr?.changedFiles ?? selectedPr.changedFiles} files</span>
                    <span className="review-platform__additions">+{displayPr?.additions ?? selectedPr.additions}</span>
                    <span className="review-platform__deletions">-{displayPr?.deletions ?? selectedPr.deletions}</span>
                  </div>
                </div>
                <div className="review-platform__fact-row">
                  <span className="review-platform__fact-label"><UserRound size={14} /> Author</span>
                  <div className="review-platform__fact-value">
                    <strong>{displayPr?.author ?? selectedPr.author}</strong>
                  </div>
                </div>
                <div className="review-platform__fact-row">
                  <span className="review-platform__fact-label"><MessageSquareText size={14} /> Comments</span>
                  <div className="review-platform__fact-value">{commentsText}</div>
                </div>
                <div className="review-platform__fact-row">
                  <span className="review-platform__fact-label"><CheckCircle2 size={14} /> Checks</span>
                  <div className="review-platform__fact-value">
                    <strong>{checksStatusText}</strong>
                    {displayPr && displayPr.checks.total > 0 && <span>{checksText}</span>}
                  </div>
                </div>
                <div className="review-platform__fact-row">
                  <span className="review-platform__fact-label"><Sparkles size={14} /> BitFun Review</span>
                  <div className="review-platform__fact-value review-platform__fact-value--review">
                    <span>{reviewStatusText}</span>
                    {(latestCurrentReview || latestStaleReview || latestUnknownReview) && (
                      <Button className="review-platform__panel-button" size="small" variant="ghost" onClick={handleOpenLatestReview}>
                        Open Review
                      </Button>
                    )}
                  </div>
                </div>
              </div>

              <Tabs
                activeKey={activeTab}
                onChange={(key) => setActiveTab(key as DetailTab)}
                type="pill"
                size="small"
                className="review-platform__tabs"
              >
                <TabPane tabKey="overview" label="Overview">
                  <div className="review-platform__tab-content review-platform__overview-scroll">
                    <section className="review-platform__detail-section">
                      <div className="review-platform__detail-section-heading">
                        <span>Description</span>
                        <Button className="review-platform__panel-button" size="small" variant="ghost" onClick={handleFillPrContext} disabled={!selectedPr}>
                          <MessageSquareText size={13} />
                          Add to chat
                        </Button>
                      </div>
                      {detailError ? (
                        <div className="review-platform__detail-error">
                          <XCircle size={14} />
                          <span>{detailError}</span>
                          <Button className="review-platform__panel-button" size="small" variant="secondary" onClick={handleRetryDetail}>
                            Retry
                          </Button>
                        </div>
                      ) : detail ? (
                        <div className="review-platform__body-markdown review-platform__body-markdown--plain">
                          {detail.body
                            ? <MarkdownRenderer content={detail.body} basePath={workspacePath} />
                            : <span className="review-platform__section-empty">No description was provided.</span>}
                        </div>
                      ) : (
                        renderDetailLoading('Loading pull request summary...')
                      )}
                    </section>

                    <section className="review-platform__detail-section review-platform__ci-list">
                      <div className="review-platform__detail-section-heading">
                        <span>Checks</span>
                        <div className="review-platform__detail-section-actions">
                          <span className="review-platform__section-count">
                            {ciTotal ? `${ciTotal} items · ${checksText}` : checksStatusText}
                          </span>
                          <Button className="review-platform__panel-button" size="small" variant="ghost" onClick={handleAddCiPageContext} disabled={!selectedPr || !detail || detailLoading}>
                            <MessageSquareText size={13} />
                            Add page
                          </Button>
                        </div>
                      </div>
                      {detailLoading && renderDetailLoading(pagedCiItems.length ? 'Refreshing checks...' : 'Loading checks...', pagedCiItems.length > 0)}
                      {pagedCiItems.map(item => {
                        const tone = ciItemTone(item);
                        const isCiExpanded = expandedCiItemIds.has(item.id);
                        const ciLog = ciLogById[item.id];
                        const ciLogLoading = ciLogLoadingIds.has(item.id);
                        const ciLogError = ciLogErrorById[item.id];
                        const logAvailable = canLoadCiLog(selectedRemote, item);
                        const expandable = canExpandCiItem(selectedRemote, item);
                        return (
                          <article key={item.id} className={`review-platform__ci-item review-platform__ci-item--${tone}`}>
                            <div className="review-platform__ci-head">
                              <div className="review-platform__ci-main">
                                <strong>{item.name}</strong>
                                <span>{[item.detail, item.stage].filter(Boolean).join(' · ')}</span>
                              </div>
                              <div className="review-platform__ci-actions">
                                <span className={`review-platform__ci-status review-platform__ci-status--${tone}`}>
                                  {ciItemStatusText(item)}
                                </span>
                                {expandable && (
                                  <IconButton
                                    className="review-platform__icon-button review-platform__ci-action"
                                    size="xs"
                                    variant="ghost"
                                    tooltip={isCiExpanded ? 'Collapse details' : 'Expand details'}
                                    onClick={() => toggleCiExpanded(item)}
                                    disabled={ciLogLoading}
                                    aria-busy={ciLogLoading}
                                  >
                                    {isCiExpanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
                                  </IconButton>
                                )}
                                <IconButton
                                  className="review-platform__icon-button review-platform__ci-action"
                                  size="xs"
                                  variant="ghost"
                                  tooltip="Add this result to chat"
                                  onClick={() => void handleAddCiItemContext(item)}
                                  disabled={!selectedPr}
                                >
                                  <MessageSquareText size={13} />
                                </IconButton>
                                {item.webUrl && (
                                  <IconButton
                                    className="review-platform__icon-button review-platform__ci-action"
                                    size="xs"
                                    variant="ghost"
                                    tooltip="Open result in provider"
                                    onClick={() => void handleOpenCiUrl(item.webUrl)}
                                  >
                                    <Link2 size={12} />
                                  </IconButton>
                                )}
                              </div>
                            </div>
                            {isCiExpanded && (
                              <div className="review-platform__ci-log-panel">
                                <div className="review-platform__ci-detail-grid">
                                  {item.stage && <div><span>Stage</span><strong>{item.stage}</strong></div>}
                                  {item.detail && <div><span>Detail</span><strong>{item.detail}</strong></div>}
                                  {item.webUrl && <div><span>URL</span><strong>{item.webUrl}</strong></div>}
                                </div>
                                {ciLogLoading && renderDetailLoading('Loading check details...')}
                                {!ciLogLoading && ciLogError && logAvailable && (
                                  <div className="review-platform__detail-error">
                                    <XCircle size={14} />
                                    <span>{ciLogError}</span>
                                    <Button className="review-platform__panel-button" size="small" variant="secondary" onClick={() => void loadCiLog(item)}>Retry</Button>
                                  </div>
                                )}
                                {!ciLogLoading && !ciLogError && (ciLog?.log || item.log) && <pre className="review-platform__ci-log-block">{ciLog?.log || item.log}</pre>}
                                {!ciLogLoading && !ciLogError && ciLog && !ciLog.log && !item.log && ciLog.message && <div className="review-platform__ci-log-empty">{ciLog.message}</div>}
                              </div>
                            )}
                          </article>
                        );
                      })}
                      {!detailLoading && detail && ciItems.length === 0 && <div className="review-platform__empty-state">No checks were reported.</div>}
                      {renderDetailPagination('Checks', ciPage, ciTotal, setCiPageIndex)}
                    </section>

                    <section className="review-platform__detail-section review-platform__threads">
                      <div className="review-platform__detail-section-heading">
                        <span>Comments</span>
                        <div className="review-platform__detail-section-actions">
                          <span className="review-platform__section-count">{reviewItemCount}</span>
                          <Button className="review-platform__panel-button" size="small" variant="ghost" onClick={handleAddReviewsContext} disabled={!selectedPr || !detail}>
                            <MessageSquareText size={13} />
                            Add to chat
                          </Button>
                        </div>
                      </div>
                      {detailLoading && renderDetailLoading(reviewThreads.length ? 'Refreshing comments...' : 'Loading comments...', reviewThreads.length > 0)}
                      {pagedReviewThreads.map(thread => {
                        const parent = thread.replyToProviderCommentId
                          ? reviewThreadByCommentId.get(thread.replyToProviderCommentId)
                          : null;
                        return (
                          <article
                            key={thread.id}
                            className={[
                              'review-platform__thread',
                              thread.resolved ? 'is-resolved' : '',
                              `review-platform__thread--${thread.kind}`,
                              parent ? 'review-platform__thread--reply' : '',
                            ].filter(Boolean).join(' ')}
                          >
                            <div className="review-platform__thread-head">
                              <div className="review-platform__thread-tags">
                                <span className={`review-platform__thread-tag review-platform__thread-tag--${thread.kind}`}>
                                  {thread.kind === 'review' ? 'Review' : 'Comment'}
                                </span>
                                <span className={`review-platform__thread-tag review-platform__thread-tag--${thread.resolved ? 'resolved' : 'open'}`}>
                                  {thread.resolved ? 'Resolved' : 'Open'}
                                </span>
                              </div>
                              <span>{formatRelativeTime(thread.updatedAt) || formatAbsoluteTime(thread.updatedAt)}</span>
                            </div>
                            <div className="review-platform__thread-meta"><strong>{thread.author}</strong></div>
                            {parent && (
                              <div className="review-platform__thread-reply-block">
                                <div className="review-platform__thread-reply-header">
                                  <span className="review-platform__thread-reply-label">Reply to</span>
                                  <span className="review-platform__thread-reply-author">@{parent.author}</span>
                                </div>
                                <div className="review-platform__thread-reply-body"><MarkdownRenderer content={parent.body} basePath={workspacePath} /></div>
                              </div>
                            )}
                            <div className="review-platform__thread-body"><MarkdownRenderer content={thread.body} basePath={workspacePath} /></div>
                            {thread.filePath && <span className="review-platform__thread-anchor">{thread.filePath}{thread.line ? `:${thread.line}` : ''}</span>}
                          </article>
                        );
                      })}
                      {!detailLoading && detail && reviewThreads.length === 0 && <div className="review-platform__empty-state">No comments yet.</div>}
                      {renderDetailPagination('Comments', reviewPage, reviewThreads.length, setReviewPageIndex)}
                    </section>
                  </div>
                </TabPane>

                <TabPane tabKey="changes" label="Changes">
                  <section className="review-platform__tab-content review-platform__file-list">
                    {detailError && (
                      <div className="review-platform__detail-error">
                        <XCircle size={14} />
                        <span>{detailError}</span>
                        <Button className="review-platform__panel-button" size="small" variant="secondary" onClick={handleRetryDetail}>
                          Retry
                        </Button>
                      </div>
                    )}
                    {detailLoading && renderDetailLoading(pagedChangedFiles.length ? 'Refreshing files...' : 'Loading files...', pagedChangedFiles.length > 0)}
                    {pagedChangedFiles.map(file => {
                      const key = fileKey(file);
                      const isExpanded = expandedFileKeys.has(key);
                      return (
                        <article key={key} className="review-platform__file-card">
                          <div className="review-platform__file-row">
                            <button
                              type="button"
                              className="review-platform__file-main"
                              aria-expanded={isExpanded}
                              onClick={() => toggleFileExpanded(key)}
                            >
                              <span className="review-platform__file-toggle">
                                {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                              </span>
                              <span className={`review-platform__file-status review-platform__file-status--${file.status}`}>
                                {file.status}
                              </span>
                              <span className="review-platform__file-path">{file.path}</span>
                              <span className="review-platform__file-delta">
                                <span className="review-platform__additions">+{file.additions}</span>
                                <span className="review-platform__deletions">-{file.deletions}</span>
                              </span>
                            </button>
                            <Button className="review-platform__panel-button review-platform__file-add-button" size="small" variant="ghost" onClick={() => void handleAddFileDiffContext(file)} disabled={!selectedPr}>
                              <MessageSquareText size={13} />
                              Add
                            </Button>
                          </div>
                          {isExpanded && (
                            file.patch ? (
                              <pre className="review-platform__diff-block" aria-label={`Diff for ${file.path}`}>
                                {file.patch.split('\n').map((line, index) => (
                                  <span key={`${file.path}-${index}`} className={diffLineClass(line)}>
                                    {line || ' '}
                                  </span>
                                ))}
                              </pre>
                            ) : (
                              <div className="review-platform__diff-empty">No inline diff is available for this file.</div>
                            )
                          )}
                        </article>
                      );
                    })}
                    {!detailLoading && detail && detail.files.length === 0 && (
                      <div className="review-platform__empty-state">No changed files were returned by this provider.</div>
                    )}
                    {renderDetailPagination('Files', changePage, changedFiles.length, setChangePageIndex)}
                  </section>
                </TabPane>

                <TabPane tabKey="commits" label="Commits">
                  <section className="review-platform__tab-content review-platform__timeline">
                    <div className="review-platform__section-heading">
                      <span>Commits</span>
                      <Button className="review-platform__panel-button" size="small" variant="ghost" onClick={handleAddCommitsContext} disabled={!selectedPr || !detail}>
                        <MessageSquareText size={13} />
                        Add to chat
                      </Button>
                    </div>
                    {detailError && (
                      <div className="review-platform__detail-error">
                        <XCircle size={14} />
                        <span>{detailError}</span>
                        <Button className="review-platform__panel-button" size="small" variant="secondary" onClick={handleRetryDetail}>
                          Retry
                        </Button>
                      </div>
                    )}
                    {detailLoading && renderDetailLoading(pagedCommits.length ? 'Refreshing commits...' : 'Loading commits...', pagedCommits.length > 0)}
                    {pagedCommits.map(commit => (
                      <div key={commit.hash} className="review-platform__timeline-item">
                        <GitCommitHorizontal size={14} />
                        <span className="review-platform__timeline-main">
                          <strong>{commit.title}</strong>
                          <span>{commit.author} · {formatRelativeTime(commit.committedAt)}</span>
                        </span>
                        <code>{commit.shortHash}</code>
                      </div>
                    ))}
                    {!detailLoading && detail && commits.length === 0 && (
                      <div className="review-platform__empty-state">No commits were returned by this provider.</div>
                    )}
                    {renderDetailPagination('Commits', commitPage, commits.length, setCommitPageIndex)}
                  </section>
                </TabPane>

              </Tabs>
            </>
          )}
        </main>
      </div>
      <Modal
        isOpen={authModalOpen}
        onClose={() => {
          if (!authSaving) {
            setAuthModalOpen(false);
            setAuthError(null);
          }
        }}
        title={selectedRemote?.platform === 'github' ? 'GitHub CLI authentication' : `${selectedRemote ? providerLabel(selectedRemote) : 'Provider'} token`}
        size="small"
        contentInset
      >
        <form
          className="review-platform__auth-form"
          onSubmit={(event) => {
            event.preventDefault();
            void handleSaveAuthToken();
          }}
        >
          <div className="review-platform__auth-target">
            <span>{selectedRemote?.host ?? 'No remote'}</span>
            <strong>{selectedRemote?.projectPath ?? ''}</strong>
          </div>
          {selectedRemote?.platform === 'github' ? (
            <div className="review-platform__gh-auth">
              <span>Run this command in the integrated terminal, finish the GitHub CLI flow, then retry.</span>
              <code>{`gh auth login --hostname ${selectedRemote.host}`}</code>
              {authError && <span className="review-platform__gh-auth-error">{authError}</span>}
            </div>
          ) : (
            <Input
              type="password"
              autoComplete="off"
              autoFocus
              label="Token"
              value={authToken}
              disabled={authSaving}
              error={Boolean(authError)}
              errorMessage={authError ?? undefined}
              onChange={event => {
                setAuthToken(event.target.value);
                if (authError) setAuthError(null);
              }}
            />
          )}
          <div className="review-platform__auth-actions">
            <Button
              type="button"
              className="review-platform__panel-button"
              size="small"
              variant="ghost"
              disabled={authSaving}
              onClick={() => {
                setAuthModalOpen(false);
                setAuthError(null);
              }}
            >
              Cancel
            </Button>
            {selectedRemote?.platform === 'github' ? (
              <>
                <Button
                  type="button"
                  className="review-platform__panel-button"
                  size="small"
                  variant="secondary"
                  disabled={authSaving}
                  onClick={() => void handleCopyGithubAuthCommand()}
                >
                  <Copy size={13} />
                  Copy
                </Button>
                <Button
                  type="button"
                  className="review-platform__panel-button"
                  size="small"
                  variant="primary"
                  isLoading={authSaving}
                  onClick={() => void handleOpenGithubAuthTerminal()}
                >
                  <Terminal size={13} />
                  Open terminal
                </Button>
              </>
            ) : (
              <Button
                type="submit"
                className="review-platform__panel-button"
                size="small"
                variant="primary"
                isLoading={authSaving}
                disabled={!authToken.trim()}
              >
                Save
              </Button>
            )}
          </div>
        </form>
      </Modal>
      {deepReviewConsentDialog}
    </div>
  );
};

export default ReviewPlatformPanel;
