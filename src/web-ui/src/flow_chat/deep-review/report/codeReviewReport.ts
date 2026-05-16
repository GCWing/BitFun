import type { ReviewTeamRunManifest } from '@/shared/services/reviewTeamService';

export { buildCodeReviewReliabilityNotices } from './reliabilityNotices';
export {
  buildCodeReviewReportSections,
  getDefaultExpandedCodeReviewSectionIds,
} from './reportSections';
export {
  DEFAULT_CODE_REVIEW_MARKDOWN_LABELS,
  formatCodeReviewReportMarkdown,
} from './markdown';

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
  | 'reduced_scope'
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

const RETRYABLE_CAPACITY_REASONS = new Set([
  'provider_rate_limit',
  'provider_concurrency_limit',
  'retry_after',
  'launch_batch_blocked',
  'temporary_overload',
]);

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
