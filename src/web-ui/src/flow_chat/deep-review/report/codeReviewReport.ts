import {
  getActiveReviewTeamManifestMembers,
  type ReviewTeamManifestMember,
  type ReviewTeamRunManifest,
} from '@/shared/services/reviewTeamService';

export type ReviewRiskLevel = 'low' | 'medium' | 'high' | 'critical';
export type ReviewAction = 'approve' | 'approve_with_suggestions' | 'request_changes' | 'block';
export type ReviewMode = 'standard' | 'deep';
export type ReviewIssueSeverity = 'critical' | 'high' | 'medium' | 'low' | 'info';
export type ReviewIssueCertainty = 'confirmed' | 'likely' | 'possible';
export type ReviewPacketStatusSource = 'reported' | 'inferred' | 'missing';
export type ReviewSectionId =
  | 'summary'
  | 'issues'
  | 'remediation'
  | 'strengths'
  | 'runManifest'
  | 'team'
  | 'coverage';
export type RemediationGroupId = 'must_fix' | 'should_improve' | 'needs_decision' | 'verification';
export type StrengthGroupId =
  | 'architecture'
  | 'maintainability'
  | 'tests'
  | 'security'
  | 'performance'
  | 'user_experience'
  | 'other';

export interface CodeReviewSummary {
  overall_assessment?: string;
  risk_level?: ReviewRiskLevel;
  recommended_action?: ReviewAction;
  confidence_note?: string;
}

export interface CodeReviewIssue {
  severity?: ReviewIssueSeverity;
  certainty?: ReviewIssueCertainty;
  category?: string;
  file?: string;
  line?: number | null;
  title?: string;
  description?: string;
  suggestion?: string | null;
  source_reviewer?: string;
  validation_note?: string;
}

export interface CodeReviewReviewer {
  name: string;
  specialty: string;
  status: string;
  summary: string;
  partial_output?: string;
  packet_id?: string;
  packet_status_source?: ReviewPacketStatusSource;
  issue_count?: number;
  covered_files?: string[];
  retry_scope_files?: string[];
  unresolved_files?: string[];
  capacity_reason?: string;
  provider_capacity_reason?: string;
  queue_skip_reason?: string;
}

export interface CodeReviewReportSectionsData {
  executive_summary?: string[];
  remediation_groups?: Partial<Record<RemediationGroupId, (string | DecisionContext)[]>>;
  strength_groups?: Partial<Record<StrengthGroupId, string[]>>;
  coverage_notes?: string[];
}

/**
 * Structured decision context for `needs_decision` remediation items.
 * Falls back to a plain string when the AI returns a legacy format.
 */
export interface DecisionContext {
  question: string;
  plan: string;
  options?: string[];
  tradeoffs?: string;
  recommendation?: number;
}

/** Normalize a raw `needs_decision` entry to a DecisionContext object. */
export function normalizeDecisionEntry(entry: string | DecisionContext): DecisionContext {
  if (typeof entry === 'string') {
    return { question: entry, plan: entry };
  }
  return entry;
}

export interface CodeReviewReportData {
  schema_version?: number;
  schemaVersion?: number;
  summary?: CodeReviewSummary;
  issues?: CodeReviewIssue[];
  positive_points?: string[];
  review_mode?: ReviewMode;
  review_scope?: string;
  reviewers?: CodeReviewReviewer[];
  remediation_plan?: string[];
  report_sections?: CodeReviewReportSectionsData;
  reliability_signals?: CodeReviewReliabilitySignal[];
}

export interface ReviewReportGroup<TId extends string = string> {
  id: TId;
  items: string[];
}

export interface ReviewIssueStats {
  total: number;
  critical: number;
  high: number;
  medium: number;
  low: number;
  info: number;
}

export interface ReviewReviewerStats {
  total: number;
  completed: number;
  degraded: number;
}

export interface ReviewReportSections {
  executiveSummary: string[];
  remediationGroups: Array<ReviewReportGroup<RemediationGroupId>>;
  strengthGroups: Array<ReviewReportGroup<StrengthGroupId>>;
  coverageNotes: string[];
  issueStats: ReviewIssueStats;
  reviewerStats: ReviewReviewerStats;
}

export type ReviewReliabilityNoticeKind =
  | 'context_pressure'
  | 'compression_preserved'
  | 'cache_hit'
  | 'cache_miss'
  | 'concurrency_limited'
  | 'partial_reviewer'
  | 'retry_guidance'
  | 'skipped_reviewers'
  | 'token_budget_limited'
  | 'user_decision';

export type ReviewReliabilityNoticeSeverity = 'info' | 'warning' | 'action';
export type ReviewReliabilitySignalSource = 'runtime' | 'manifest' | 'report' | 'inferred';

export interface ReviewReliabilityNotice {
  kind: ReviewReliabilityNoticeKind;
  severity: ReviewReliabilityNoticeSeverity;
  count?: number;
  source?: ReviewReliabilitySignalSource;
  detail?: string;
}

export interface CodeReviewReliabilitySignal {
  kind: ReviewReliabilityNoticeKind;
  severity?: ReviewReliabilityNoticeSeverity;
  count?: number;
  source?: ReviewReliabilitySignalSource;
  detail?: string;
}

export interface CodeReviewReportMarkdownLabels {
  titleStandard: string;
  titleDeep: string;
  executiveSummary: string;
  reviewDecision: string;
  runManifest: string;
  riskLevel: string;
  recommendedAction: string;
  scope: string;
  target: string;
  budget: string;
  estimatedCalls: string;
  activeReviewers: string;
  skippedReviewers: string;
  issues: string;
  noIssues: string;
  remediationPlan: string;
  strengths: string;
  reviewTeam: string;
  reliabilitySignals: string;
  coverageNotes: string;
  status: string;
  packet: string;
  partialOutput: string;
  findings: string;
  validation: string;
  suggestion: string;
  source: string;
  noItems: string;
  groupTitles: Record<RemediationGroupId | StrengthGroupId, string>;
  reliabilityNoticeLabels: Record<ReviewReliabilityNoticeKind, string>;
}

export interface CodeReviewReportMarkdownOptions {
  runManifest?: ReviewTeamRunManifest;
}

const REMEDIATION_GROUP_ORDER: RemediationGroupId[] = [
  'must_fix',
  'should_improve',
  'needs_decision',
  'verification',
];

const STRENGTH_GROUP_ORDER: StrengthGroupId[] = [
  'architecture',
  'maintainability',
  'tests',
  'security',
  'performance',
  'user_experience',
  'other',
];

const DEGRADED_REVIEWER_STATUSES = new Set(['timed_out', 'cancelled_by_user', 'failed', 'skipped']);
const PARTIAL_TIMEOUT_REVIEWER_STATUSES = new Set(['partial_timeout', 'timed_out', 'cancelled_by_user']);
const RETRYABLE_CAPACITY_REASONS = new Set([
  'provider_rate_limit',
  'provider_concurrency_limit',
  'retry_after',
  'temporary_overload',
]);
const RELIABILITY_NOTICE_ORDER: ReviewReliabilityNoticeKind[] = [
  'context_pressure',
  'skipped_reviewers',
  'token_budget_limited',
  'compression_preserved',
  'cache_hit',
  'cache_miss',
  'concurrency_limited',
  'partial_reviewer',
  'retry_guidance',
  'user_decision',
];
const RELIABILITY_NOTICE_FALLBACK_LABELS: Record<ReviewReliabilityNoticeKind, string> = {
  context_pressure: 'Context pressure rising',
  compression_preserved: 'Compression preserved key facts',
  cache_hit: 'Incremental cache reused reviewer output',
  cache_miss: 'Incremental cache missed or refreshed',
  concurrency_limited: 'Reviewer launch was concurrency-limited',
  partial_reviewer: 'Reviewer timed out with partial result',
  retry_guidance: 'Retry guidance emitted',
  skipped_reviewers: 'Skipped reviewers',
  token_budget_limited: 'Token budget limited reviewer coverage',
  user_decision: 'User decision needed',
};
const RELIABILITY_NOTICE_SEVERITY_BY_KIND: Record<ReviewReliabilityNoticeKind, ReviewReliabilityNoticeSeverity> = {
  context_pressure: 'info',
  compression_preserved: 'info',
  cache_hit: 'info',
  cache_miss: 'info',
  concurrency_limited: 'warning',
  partial_reviewer: 'warning',
  retry_guidance: 'warning',
  skipped_reviewers: 'info',
  token_budget_limited: 'warning',
  user_decision: 'action',
};

export const DEFAULT_CODE_REVIEW_MARKDOWN_LABELS: CodeReviewReportMarkdownLabels = {
  titleStandard: 'Code Review Report',
  titleDeep: 'Deep Review Report',
  executiveSummary: 'Executive Summary',
  reviewDecision: 'Review Decision',
  runManifest: 'Run manifest',
  riskLevel: 'Risk Level',
  recommendedAction: 'Recommended Action',
  scope: 'Scope',
  target: 'Target',
  budget: 'Budget',
  estimatedCalls: 'Estimated calls',
  activeReviewers: 'Active reviewers',
  skippedReviewers: 'Skipped reviewers',
  issues: 'Issues',
  noIssues: 'No validated issues.',
  remediationPlan: 'Remediation Plan',
  strengths: 'Strengths',
  reviewTeam: 'Code Review Team',
  reliabilitySignals: 'Review Reliability',
  coverageNotes: 'Coverage Notes',
  status: 'Status',
  packet: 'Packet',
  partialOutput: 'Partial output',
  findings: 'Findings',
  validation: 'Validation',
  suggestion: 'Suggestion',
  source: 'Source',
  noItems: 'None.',
  reliabilityNoticeLabels: RELIABILITY_NOTICE_FALLBACK_LABELS,
  groupTitles: {
    must_fix: 'Must Fix',
    should_improve: 'Should Improve',
    needs_decision: 'Needs Decision',
    verification: 'Verification',
    architecture: 'Architecture',
    maintainability: 'Maintainability',
    tests: 'Tests',
    security: 'Security',
    performance: 'Performance',
    user_experience: 'User Experience',
    other: 'Other',
  },
};

export type DeepReviewRetryableSliceSourceStatus = 'partial_timeout' | 'capacity_skipped';

export interface DeepReviewRetryableSlice {
  sourcePacketId: string;
  reviewerId: string;
  reviewerName: string;
  sourceStatus: DeepReviewRetryableSliceSourceStatus;
  coveredFiles: string[];
  retryScopeFiles: string[];
  retryTimeoutSeconds: number;
  capacityReason?: string;
}

function nonEmpty(values?: Array<string | undefined | null>): string[] {
  const seen = new Set<string>();
  const result: string[] = [];

  for (const value of values ?? []) {
    const trimmed = value?.trim();
    if (!trimmed || seen.has(trimmed)) {
      continue;
    }
    seen.add(trimmed);
    result.push(trimmed);
  }

  return result;
}

function reviewerStringArray(
  reviewer: CodeReviewReviewer,
  keys: Array<keyof CodeReviewReviewer | string>,
): string[] {
  const raw = reviewer as unknown as Record<string, unknown>;
  for (const key of keys) {
    const value = raw[String(key)];
    if (!Array.isArray(value)) {
      continue;
    }
    return nonEmpty(value.map((item) => (typeof item === 'string' ? item : undefined)));
  }
  return [];
}

function reviewerString(
  reviewer: CodeReviewReviewer,
  keys: Array<keyof CodeReviewReviewer | string>,
): string | undefined {
  const raw = reviewer as unknown as Record<string, unknown>;
  for (const key of keys) {
    const value = raw[String(key)];
    if (typeof value === 'string' && value.trim()) {
      return value.trim();
    }
  }
  return undefined;
}

function findWorkPacket(
  runManifest: ReviewTeamRunManifest,
  packetId: string,
  reviewerId: string,
) {
  return runManifest.workPackets?.find((packet) => (
    packet.packetId === packetId && packet.subagentId === reviewerId
  ));
}

function retryTimeoutSecondsForPacket(sourceTimeoutSeconds: number): number | null {
  if (!Number.isFinite(sourceTimeoutSeconds) || sourceTimeoutSeconds <= 1) {
    return null;
  }
  return Math.max(1, Math.min(sourceTimeoutSeconds - 1, Math.floor(sourceTimeoutSeconds / 2)));
}

function isRetryableCapacityReviewer(reviewer: CodeReviewReviewer): string | null {
  if (reviewer.status !== 'capacity_skipped') {
    return null;
  }

  const terminalReason = reviewerString(reviewer, ['queue_skip_reason', 'queueSkipReason']);
  if (terminalReason && !RETRYABLE_CAPACITY_REASONS.has(terminalReason)) {
    return null;
  }

  const capacityReason = terminalReason ??
    reviewerString(reviewer, [
      'capacity_reason',
      'capacityReason',
      'provider_capacity_reason',
      'providerCapacityReason',
    ]);
  return capacityReason && RETRYABLE_CAPACITY_REASONS.has(capacityReason)
    ? capacityReason
    : null;
}

export function extractDeepReviewRetryableSlices(
  report: CodeReviewReportData,
  runManifest?: ReviewTeamRunManifest,
): DeepReviewRetryableSlice[] {
  if (report.review_mode !== 'deep' || !runManifest?.workPackets?.length) {
    return [];
  }

  const slices: DeepReviewRetryableSlice[] = [];
  for (const reviewer of report.reviewers ?? []) {
    const sourceStatus = reviewer.status === 'partial_timeout'
      ? 'partial_timeout'
      : reviewer.status === 'capacity_skipped'
        ? 'capacity_skipped'
        : null;
    if (!sourceStatus) {
      continue;
    }

    const capacityReason = isRetryableCapacityReviewer(reviewer);
    if (sourceStatus === 'capacity_skipped' && !capacityReason) {
      continue;
    }

    const sourcePacketId = reviewer.packet_id?.trim();
    if (!sourcePacketId) {
      continue;
    }
    const reviewerId = reviewerString(reviewer, ['subagent_id', 'subagentId']) ??
      sourcePacketId.split(':')[1]?.split(':')[0]?.trim() ??
      '';
    if (!reviewerId) {
      continue;
    }

    const packet = findWorkPacket(runManifest, sourcePacketId, reviewerId);
    const packetFiles = nonEmpty(packet?.assignedScope.files ?? []);
    if (!packet || packetFiles.length <= 1) {
      continue;
    }

    const retryScopeFiles = reviewerStringArray(reviewer, [
      'retry_scope_files',
      'retryScopeFiles',
      'unresolved_files',
      'unresolvedFiles',
    ]);
    if (retryScopeFiles.length === 0 || retryScopeFiles.length >= packetFiles.length) {
      continue;
    }

    const packetFileSet = new Set(packetFiles);
    if (retryScopeFiles.some((file) => !packetFileSet.has(file))) {
      continue;
    }

    const retryScopeSet = new Set(retryScopeFiles);
    const coveredFiles = reviewerStringArray(reviewer, ['covered_files', 'coveredFiles'])
      .filter((file) => packetFileSet.has(file) && !retryScopeSet.has(file));
    const normalizedCoveredFiles = coveredFiles.length > 0
      ? coveredFiles
      : packetFiles.filter((file) => !retryScopeSet.has(file));

    const retryTimeoutSeconds = retryTimeoutSecondsForPacket(packet.timeoutSeconds);
    if (!retryTimeoutSeconds) {
      continue;
    }

    slices.push({
      sourcePacketId,
      reviewerId,
      reviewerName: reviewer.name || packet.displayName || reviewerId,
      sourceStatus,
      coveredFiles: normalizedCoveredFiles,
      retryScopeFiles,
      retryTimeoutSeconds,
      ...(capacityReason ? { capacityReason } : {}),
    });
  }

  return slices;
}

export function buildDeepReviewRetryPrompt(slices: DeepReviewRetryableSlice[]): string {
  const retryTasks = slices.map((slice) => ({
    subagent_type: slice.reviewerId,
    retry: true,
    timeout_seconds: slice.retryTimeoutSeconds,
    retry_coverage: {
      source_packet_id: slice.sourcePacketId,
      source_status: slice.sourceStatus,
      ...(slice.capacityReason ? { capacity_reason: slice.capacityReason } : {}),
      covered_files: slice.coveredFiles,
      retry_scope_files: slice.retryScopeFiles,
    },
  }));

  return [
    'Retry only the listed incomplete Deep Review slices in this same session.',
    'Use the Task tool once for each retry task below. Do not retry files outside retry_scope_files.',
    'After these retry tasks finish, run ReviewJudge and submit an updated code review report with honest coverage notes.',
    '',
    '<deep_review_retry_tasks>',
    JSON.stringify(retryTasks, null, 2),
    '</deep_review_retry_tasks>',
  ].join('\n');
}

function buildGroups<TId extends string>(
  order: TId[],
  data?: Partial<Record<TId, string[]>>,
): Array<ReviewReportGroup<TId>> {
  return order
    .map((id) => ({ id, items: nonEmpty(data?.[id]) }))
    .filter((group) => group.items.length > 0);
}

function buildLegacyRemediationGroups(report: CodeReviewReportData): Array<ReviewReportGroup<RemediationGroupId>> {
  const items = nonEmpty(report.remediation_plan);
  if (items.length === 0) {
    return [];
  }

  const recommendedAction = report.summary?.recommended_action;
  const id: RemediationGroupId =
    recommendedAction === 'request_changes' || recommendedAction === 'block'
      ? 'must_fix'
      : 'should_improve';

  return [{ id, items }];
}

function buildLegacyStrengthGroups(report: CodeReviewReportData): Array<ReviewReportGroup<StrengthGroupId>> {
  const items = nonEmpty(report.positive_points).filter((item) => item.toLowerCase() !== 'none');
  return items.length > 0 ? [{ id: 'other', items }] : [];
}

function buildIssueStats(issues: CodeReviewIssue[] = []): ReviewIssueStats {
  const stats: ReviewIssueStats = {
    total: 0,
    critical: 0,
    high: 0,
    medium: 0,
    low: 0,
    info: 0,
  };

  for (const issue of issues) {
    const severity = issue.severity ?? 'info';
    stats[severity] += 1;
    stats.total += 1;
  }

  return stats;
}

function buildReviewerStats(reviewers: CodeReviewReviewer[] = []): ReviewReviewerStats {
  let completed = 0;
  let degraded = 0;

  for (const reviewer of reviewers) {
    if (reviewer.status === 'completed') {
      completed += 1;
    } else if (
      DEGRADED_REVIEWER_STATUSES.has(reviewer.status) ||
      reviewer.status === 'partial_timeout'
    ) {
      degraded += 1;
    }
  }

  return {
    total: reviewers.length,
    completed,
    degraded,
  };
}

function buildPartialReviewerCoverageNotes(reviewers: CodeReviewReviewer[] = []): string[] {
  return reviewers
    .map((reviewer) => {
      const partialOutput = reviewer.partial_output?.trim();
      if (!partialOutput || !PARTIAL_TIMEOUT_REVIEWER_STATUSES.has(reviewer.status)) {
        return null;
      }
      return `${reviewer.name} timed out after producing partial output: ${partialOutput}`;
    })
    .filter((note): note is string => Boolean(note));
}

function hasCompressionPreservationNote(report: CodeReviewReportData): boolean {
  const notes = [
    ...(report.report_sections?.coverage_notes ?? []),
    report.summary?.confidence_note,
  ];

  return notes.some((note) => {
    const normalized = note?.toLowerCase() ?? '';
    return normalized.includes('compress') && normalized.includes('preserv');
  });
}

function countPartialReviewers(reviewers: CodeReviewReviewer[] = []): number {
  return reviewers.filter((reviewer) =>
    reviewer.status === 'partial_timeout' ||
    (
      PARTIAL_TIMEOUT_REVIEWER_STATUSES.has(reviewer.status) &&
      Boolean(reviewer.partial_output?.trim())
    )
  ).length;
}

function countSkippedReviewers(runManifest?: ReviewTeamRunManifest): number {
  return runManifest?.skippedReviewers.length ?? 0;
}

function countTokenBudgetLimitedReviewers(runManifest?: ReviewTeamRunManifest): number {
  if (!runManifest) {
    return 0;
  }
  const skippedByBudget = new Set(runManifest.tokenBudget.skippedReviewerIds);
  for (const reviewer of runManifest.skippedReviewers) {
    if (reviewer.reason === 'budget_limited') {
      skippedByBudget.add(reviewer.subagentId);
    }
  }
  return skippedByBudget.size;
}

function countDecisionItems(report: CodeReviewReportData): number {
  const structuredDecisionItems = report.report_sections?.remediation_groups?.needs_decision ?? [];
  if (structuredDecisionItems.length > 0) {
    const stringItems = structuredDecisionItems.filter((item): item is string => typeof item === 'string');
    return nonEmpty(stringItems).length;
  }

  return report.summary?.recommended_action === 'block' ? 1 : 0;
}

function isReliabilityNoticeKind(value: string): value is ReviewReliabilityNoticeKind {
  return RELIABILITY_NOTICE_ORDER.includes(value as ReviewReliabilityNoticeKind);
}

function isReliabilitySeverity(value: string): value is ReviewReliabilityNoticeSeverity {
  return value === 'info' || value === 'warning' || value === 'action';
}

function isReliabilitySignalSource(value: string): value is ReviewReliabilitySignalSource {
  return value === 'runtime' || value === 'manifest' || value === 'report' || value === 'inferred';
}

function normalizeStructuredReliabilityNotice(
  signal: CodeReviewReliabilitySignal,
): ReviewReliabilityNotice | null {
  if (!isReliabilityNoticeKind(signal.kind)) {
    return null;
  }

  const detail = signal.detail?.trim();
  return {
    kind: signal.kind,
    severity: signal.severity && isReliabilitySeverity(signal.severity)
      ? signal.severity
      : RELIABILITY_NOTICE_SEVERITY_BY_KIND[signal.kind],
    ...(typeof signal.count === 'number' ? { count: signal.count } : {}),
    ...(signal.source && isReliabilitySignalSource(signal.source)
      ? { source: signal.source }
      : {}),
    ...(detail ? { detail } : {}),
  };
}

function structuredReliabilityNoticeMap(
  report: CodeReviewReportData,
): Map<ReviewReliabilityNoticeKind, ReviewReliabilityNotice> {
  const notices = new Map<ReviewReliabilityNoticeKind, ReviewReliabilityNotice>();
  for (const signal of report.reliability_signals ?? []) {
    const notice = normalizeStructuredReliabilityNotice(signal);
    if (notice && !notices.has(notice.kind)) {
      notices.set(notice.kind, notice);
    }
  }
  return notices;
}

function reliabilityNoticeLabel(
  kind: ReviewReliabilityNoticeKind,
  labels: CodeReviewReportMarkdownLabels,
): string {
  return labels.reliabilityNoticeLabels[kind] ?? RELIABILITY_NOTICE_FALLBACK_LABELS[kind];
}

function reliabilityNoticeMarkdownDetail(notice: ReviewReliabilityNotice): string {
  if (notice.detail?.trim()) {
    return notice.detail.trim();
  }
  if (typeof notice.count === 'number') {
    return `Count: ${notice.count}`;
  }
  return '';
}

function reliabilityNoticeMarkdownLine(
  notice: ReviewReliabilityNotice,
  labels: CodeReviewReportMarkdownLabels,
): string {
  const tags = [notice.severity, notice.source].filter(Boolean).join('/');
  const detail = reliabilityNoticeMarkdownDetail(notice);
  const tagText = tags ? ` [${tags}]` : '';
  return detail
    ? `- ${reliabilityNoticeLabel(notice.kind, labels)}${tagText}: ${detail}`
    : `- ${reliabilityNoticeLabel(notice.kind, labels)}${tagText}`;
}

export function buildCodeReviewReliabilityNotices(
  report: CodeReviewReportData,
  runManifest?: ReviewTeamRunManifest,
): ReviewReliabilityNotice[] {
  const notices: ReviewReliabilityNotice[] = [];
  const structuredNotices = structuredReliabilityNoticeMap(report);
  const hasContextPressure = runManifest
    ? runManifest.tokenBudget.largeDiffSummaryFirst || runManifest.tokenBudget.warnings.length > 0
    : false;

  const structuredContextPressure = structuredNotices.get('context_pressure');
  if (structuredContextPressure) {
    notices.push(structuredContextPressure);
  } else if (hasContextPressure && runManifest) {
    notices.push({
      kind: 'context_pressure',
      severity: 'info',
      count: runManifest.tokenBudget.estimatedReviewerCalls,
      source: 'manifest',
    });
  }

  const structuredCompressionPreserved = structuredNotices.get('compression_preserved');
  if (structuredCompressionPreserved) {
    notices.push(structuredCompressionPreserved);
  } else if (hasCompressionPreservationNote(report)) {
    notices.push({
      kind: 'compression_preserved',
      severity: 'info',
      source: 'inferred',
    });
  }

  for (const kind of ['cache_hit', 'cache_miss', 'concurrency_limited'] as const) {
    const structuredNotice = structuredNotices.get(kind);
    if (structuredNotice) {
      notices.push(structuredNotice);
    }
  }

  const partialReviewerCount = countPartialReviewers(report.reviewers);
  const structuredPartialReviewer = structuredNotices.get('partial_reviewer');
  if (structuredPartialReviewer) {
    notices.push(structuredPartialReviewer);
  } else if (partialReviewerCount > 0) {
    notices.push({
      kind: 'partial_reviewer',
      severity: 'warning',
      count: partialReviewerCount,
      source: 'runtime',
    });
  }

  const structuredRetryGuidance = structuredNotices.get('retry_guidance');
  if (structuredRetryGuidance) {
    notices.push(structuredRetryGuidance);
  } else if (partialReviewerCount > 0) {
    notices.push({
      kind: 'retry_guidance',
      severity: 'warning',
      count: partialReviewerCount,
      source: 'runtime',
    });
  }

  const skippedReviewerCount = countSkippedReviewers(runManifest);
  const structuredSkippedReviewers = structuredNotices.get('skipped_reviewers');
  if (structuredSkippedReviewers) {
    notices.push(structuredSkippedReviewers);
  } else if (skippedReviewerCount > 0) {
    notices.push({
      kind: 'skipped_reviewers',
      severity: 'info',
      count: skippedReviewerCount,
      source: 'manifest',
    });
  }

  const tokenBudgetLimitedReviewerCount = countTokenBudgetLimitedReviewers(runManifest);
  const structuredTokenBudgetLimited = structuredNotices.get('token_budget_limited');
  if (structuredTokenBudgetLimited) {
    notices.push(structuredTokenBudgetLimited);
  } else if (tokenBudgetLimitedReviewerCount > 0) {
    notices.push({
      kind: 'token_budget_limited',
      severity: 'warning',
      count: tokenBudgetLimitedReviewerCount,
      source: 'manifest',
    });
  }

  const decisionItemCount = countDecisionItems(report);
  const structuredUserDecision = structuredNotices.get('user_decision');
  if (structuredUserDecision) {
    notices.push(structuredUserDecision);
  } else if (decisionItemCount > 0) {
    notices.push({
      kind: 'user_decision',
      severity: 'action',
      count: decisionItemCount,
      source: 'report',
    });
  }

  return RELIABILITY_NOTICE_ORDER
    .map((kind) => notices.find((notice) => notice.kind === kind))
    .filter((notice): notice is ReviewReliabilityNotice => Boolean(notice));
}

export function buildCodeReviewReportSections(report: CodeReviewReportData): ReviewReportSections {
  const structuredSections = report.report_sections;

  // Normalize remediation groups: DecisionContext entries become their plan text for display
  const rawRemediationGroups = structuredSections?.remediation_groups;
  const normalizedRemediationGroups: Partial<Record<RemediationGroupId, string[]>> = {};
  if (rawRemediationGroups) {
    for (const [key, entries] of Object.entries(rawRemediationGroups) as [RemediationGroupId, (string | DecisionContext)[] | undefined][]) {
      if (!entries) continue;
      normalizedRemediationGroups[key] = entries.map((entry) => {
        if (typeof entry === 'string') return entry;
        return entry.plan;
      });
    }
  }

  const remediationGroups = buildGroups(REMEDIATION_GROUP_ORDER, normalizedRemediationGroups);
  const strengthGroups = buildGroups(STRENGTH_GROUP_ORDER, structuredSections?.strength_groups);
  const executiveSummary = nonEmpty(structuredSections?.executive_summary);
  const coverageNotes = nonEmpty(structuredSections?.coverage_notes);
  const partialReviewerCoverageNotes = buildPartialReviewerCoverageNotes(report.reviewers);
  const confidenceNote = report.summary?.confidence_note?.trim();

  return {
    executiveSummary: executiveSummary.length > 0
      ? executiveSummary
      : nonEmpty([report.summary?.overall_assessment]),
    remediationGroups: remediationGroups.length > 0
      ? remediationGroups
      : buildLegacyRemediationGroups(report),
    strengthGroups: strengthGroups.length > 0
      ? strengthGroups
      : buildLegacyStrengthGroups(report),
    coverageNotes: coverageNotes.length > 0
      ? nonEmpty([...coverageNotes, ...partialReviewerCoverageNotes])
      : nonEmpty([confidenceNote, ...partialReviewerCoverageNotes]),
    issueStats: buildIssueStats(report.issues),
    reviewerStats: buildReviewerStats(report.reviewers),
  };
}

export function getDefaultExpandedCodeReviewSectionIds(report: CodeReviewReportData): ReviewSectionId[] {
  const sections = buildCodeReviewReportSections(report);
  const expanded: ReviewSectionId[] = ['summary'];

  if (sections.remediationGroups.length > 0) {
    expanded.push('remediation');
  }

  return expanded;
}

function mergeLabels(labels?: Partial<CodeReviewReportMarkdownLabels>): CodeReviewReportMarkdownLabels {
  return {
    ...DEFAULT_CODE_REVIEW_MARKDOWN_LABELS,
    ...labels,
    groupTitles: {
      ...DEFAULT_CODE_REVIEW_MARKDOWN_LABELS.groupTitles,
      ...labels?.groupTitles,
    },
    reliabilityNoticeLabels: {
      ...DEFAULT_CODE_REVIEW_MARKDOWN_LABELS.reliabilityNoticeLabels,
      ...labels?.reliabilityNoticeLabels,
    },
  };
}

function pushList(lines: string[], items: string[], emptyLabel: string): void {
  if (items.length === 0) {
    lines.push(`- ${emptyLabel}`);
    return;
  }

  for (const item of items) {
    lines.push(`- ${item}`);
  }
}

function issueLocation(issue: CodeReviewIssue): string {
  if (!issue.file) {
    return '';
  }

  return issue.line ? `${issue.file}:${issue.line}` : issue.file;
}

function manifestTarget(manifest: ReviewTeamRunManifest): string {
  return manifest.target.tags.length > 0
    ? manifest.target.tags.join(', ')
    : manifest.target.source;
}

function manifestMemberLabel(member: ReviewTeamManifestMember): string {
  return member.displayName || member.subagentId;
}

function manifestMemberLine(member: ReviewTeamManifestMember): string {
  return `${manifestMemberLabel(member)} (${member.subagentId})`;
}

function pluralize(count: number, singular: string): string {
  return `${count} ${singular}${count === 1 ? '' : 's'}`;
}

function pushPreReviewSummarySection(
  lines: string[],
  manifest: ReviewTeamRunManifest,
): void {
  const summary = manifest.preReviewSummary;
  if (!summary) {
    return;
  }

  lines.push(`### Pre-review summary`);
  lines.push(`- ${summary.summary}`);
  lines.push(`- Files: ${summary.fileCount}`);
  if (summary.lineCount !== undefined) {
    lines.push(`- Lines changed: ${summary.lineCount} (${summary.lineCountSource})`);
  } else {
    lines.push(`- Lines changed: unknown (${summary.lineCountSource})`);
  }
  if (summary.workspaceAreas.length > 0) {
    for (const area of summary.workspaceAreas) {
      const sampleFiles = area.sampleFiles.length > 0
        ? ` (${area.sampleFiles.join(', ')})`
        : '';
      lines.push(`- ${area.key}: ${pluralize(area.fileCount, 'file')}${sampleFiles}`);
    }
  }
  lines.push('');
}

function pushSharedContextCacheSection(
  lines: string[],
  manifest: ReviewTeamRunManifest,
): void {
  const cachePlan = manifest.sharedContextCache;
  if (!cachePlan) {
    return;
  }

  lines.push(`### Shared context cache`);
  if (cachePlan.entries.length === 0) {
    lines.push('- None.');
  } else {
    for (const entry of cachePlan.entries) {
      lines.push(
        `- ${entry.cacheKey}: ${entry.path} -> ${entry.consumerPacketIds.join(', ')}`,
      );
    }
  }
  if (cachePlan.omittedEntryCount > 0) {
    lines.push(`- Omitted entries: ${cachePlan.omittedEntryCount}`);
  }
  lines.push('');
}

function pushIncrementalReviewCacheSection(
  lines: string[],
  manifest: ReviewTeamRunManifest,
): void {
  const cachePlan = manifest.incrementalReviewCache;
  if (!cachePlan) {
    return;
  }

  lines.push(`### Incremental review cache`);
  lines.push(`- Cache key: ${cachePlan.cacheKey}`);
  lines.push(`- Fingerprint: ${cachePlan.fingerprint}`);
  lines.push(`- Strategy: ${cachePlan.strategy}`);
  lines.push(`- Reviewer packets: ${cachePlan.reviewerPacketIds.join(', ') || 'none'}`);
  lines.push(`- Invalidates on: ${cachePlan.invalidatesOn.join(', ') || 'none'}`);
  lines.push('');
}

function pushRunManifestSection(
  lines: string[],
  manifest: ReviewTeamRunManifest,
  labels: CodeReviewReportMarkdownLabels,
): void {
  const activeReviewers = getActiveReviewTeamManifestMembers(manifest);

  lines.push(`## ${labels.runManifest}`);
  lines.push(`- ${labels.target}: ${manifestTarget(manifest)}`);
  lines.push(`- ${labels.budget}: ${manifest.tokenBudget.mode}`);
  lines.push(`- ${labels.estimatedCalls}: ${manifest.tokenBudget.estimatedReviewerCalls}`);
  if (manifest.strategyRecommendation) {
    lines.push(`- Recommended strategy: ${manifest.strategyRecommendation.strategyLevel}`);
    lines.push(`- Recommendation score: ${manifest.strategyRecommendation.score}`);
    lines.push(`- Recommendation rationale: ${manifest.strategyRecommendation.rationale}`);
  }
  lines.push('');
  lines.push(`### ${labels.activeReviewers}`);
  pushList(
    lines,
    activeReviewers.map((member) => manifestMemberLine(member)),
    labels.noItems,
  );
  lines.push('');
  lines.push(`### ${labels.skippedReviewers}`);
  pushList(
    lines,
    manifest.skippedReviewers.map((member) =>
      `${manifestMemberLine(member)}: ${member.reason ?? 'skipped'}`,
    ),
    labels.noItems,
  );
  lines.push('');
  pushPreReviewSummarySection(lines, manifest);
  pushSharedContextCacheSection(lines, manifest);
  pushIncrementalReviewCacheSection(lines, manifest);
}

export function formatCodeReviewReportMarkdown(
  report: CodeReviewReportData,
  labels?: Partial<CodeReviewReportMarkdownLabels>,
  options?: CodeReviewReportMarkdownOptions,
): string {
  const mergedLabels = mergeLabels(labels);
  const sections = buildCodeReviewReportSections(report);
  const issues = report.issues ?? [];
  const reviewers = report.reviewers ?? [];
  const lines: string[] = [];

  lines.push(`# ${report.review_mode === 'deep' ? mergedLabels.titleDeep : mergedLabels.titleStandard}`);
  lines.push('');
  lines.push(`## ${mergedLabels.executiveSummary}`);
  pushList(lines, sections.executiveSummary, mergedLabels.noItems);
  lines.push('');
  lines.push(`## ${mergedLabels.reviewDecision}`);
  lines.push(`- ${mergedLabels.riskLevel}: ${report.summary?.risk_level ?? 'unknown'}`);
  lines.push(`- ${mergedLabels.recommendedAction}: ${report.summary?.recommended_action ?? 'unknown'}`);
  if (report.review_scope?.trim()) {
    lines.push(`- ${mergedLabels.scope}: ${report.review_scope.trim()}`);
  }
  lines.push('');
  if (report.review_mode === 'deep' && options?.runManifest) {
    pushRunManifestSection(lines, options.runManifest, mergedLabels);
  }
  const reliabilityNotices = buildCodeReviewReliabilityNotices(report, options?.runManifest);
  if (reliabilityNotices.length > 0) {
    lines.push(`## ${mergedLabels.reliabilitySignals}`);
    reliabilityNotices.forEach((notice) => {
      lines.push(reliabilityNoticeMarkdownLine(notice, mergedLabels));
    });
    lines.push('');
  }
  lines.push(`## ${mergedLabels.issues}`);
  if (issues.length === 0) {
    lines.push(`- ${mergedLabels.noIssues}`);
  } else {
    issues.forEach((issue, index) => {
      const location = issueLocation(issue);
      const heading = [
        `${index + 1}.`,
        `[${issue.severity ?? 'info'}/${issue.certainty ?? 'possible'}]`,
        issue.title ?? 'Untitled issue',
        location ? `(${location})` : '',
      ].filter(Boolean).join(' ');

      lines.push(heading);
      if (issue.category) {
        lines.push(`   - ${issue.category}`);
      }
      if (issue.source_reviewer) {
        lines.push(`   - ${mergedLabels.source}: ${issue.source_reviewer}`);
      }
      if (issue.description) {
        lines.push(`   - ${issue.description}`);
      }
      if (issue.suggestion) {
        lines.push(`   - ${mergedLabels.suggestion}: ${issue.suggestion}`);
      }
      if (issue.validation_note) {
        lines.push(`   - ${mergedLabels.validation}: ${issue.validation_note}`);
      }
    });
  }
  lines.push('');
  lines.push(`## ${mergedLabels.remediationPlan}`);
  for (const group of sections.remediationGroups) {
    lines.push(`### ${mergedLabels.groupTitles[group.id]}`);
    pushList(lines, group.items, mergedLabels.noItems);
    lines.push('');
  }
  if (sections.remediationGroups.length === 0) {
    lines.push(`- ${mergedLabels.noItems}`);
    lines.push('');
  }
  lines.push(`## ${mergedLabels.strengths}`);
  for (const group of sections.strengthGroups) {
    lines.push(`### ${mergedLabels.groupTitles[group.id]}`);
    pushList(lines, group.items, mergedLabels.noItems);
    lines.push('');
  }
  if (sections.strengthGroups.length === 0) {
    lines.push(`- ${mergedLabels.noItems}`);
    lines.push('');
  }
  lines.push(`## ${mergedLabels.reviewTeam}`);
  if (reviewers.length === 0) {
    lines.push(`- ${mergedLabels.noItems}`);
  } else {
    for (const reviewer of reviewers) {
      const issueCount = typeof reviewer.issue_count === 'number'
        ? `; ${mergedLabels.findings}: ${reviewer.issue_count}`
        : '';
      lines.push(`- ${reviewer.name} (${reviewer.specialty}; ${mergedLabels.status}: ${reviewer.status}${issueCount})`);
      if (reviewer.summary) {
        lines.push(`  - ${reviewer.summary}`);
      }
      const packetId = reviewer.packet_id?.trim();
      if (packetId || reviewer.packet_status_source) {
        const packetLabel = packetId || 'missing';
        const sourceLabel = reviewer.packet_status_source
          ? ` (${reviewer.packet_status_source})`
          : '';
        lines.push(`  - ${mergedLabels.packet}: ${packetLabel}${sourceLabel}`);
      }
      if (reviewer.partial_output?.trim()) {
        lines.push(`  - ${mergedLabels.partialOutput}: ${reviewer.partial_output.trim()}`);
      }
    }
  }
  lines.push('');
  lines.push(`## ${mergedLabels.coverageNotes}`);
  pushList(lines, sections.coverageNotes, mergedLabels.noItems);

  return lines.join('\n').trimEnd();
}
