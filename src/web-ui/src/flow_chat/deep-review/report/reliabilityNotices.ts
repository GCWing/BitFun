import type { ReviewTeamRunManifest } from '@/shared/services/reviewTeamService';
import type {
  CodeReviewReliabilitySignal,
  CodeReviewReportData,
  CodeReviewReportMarkdownLabels,
  CodeReviewReviewer,
  ReviewReliabilityNotice,
  ReviewReliabilityNoticeKind,
  ReviewReliabilityNoticeSeverity,
  ReviewReliabilitySignalSource,
} from './codeReviewReport';

export const PARTIAL_TIMEOUT_REVIEWER_STATUSES = new Set([
  'partial_timeout',
  'timed_out',
  'cancelled_by_user',
]);

const RELIABILITY_NOTICE_ORDER: ReviewReliabilityNoticeKind[] = [
  'context_pressure',
  'reduced_scope',
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

export const RELIABILITY_NOTICE_FALLBACK_LABELS: Record<ReviewReliabilityNoticeKind, string> = {
  context_pressure: 'Context pressure rising',
  compression_preserved: 'Compression preserved key facts',
  cache_hit: 'Incremental cache reused reviewer output',
  cache_miss: 'Incremental cache missed or refreshed',
  concurrency_limited: 'Reviewer launch was concurrency-limited',
  partial_reviewer: 'Reviewer timed out with partial result',
  reduced_scope: 'Reduced-depth coverage',
  retry_guidance: 'Retry guidance emitted',
  skipped_reviewers: 'Skipped reviewers',
  token_budget_limited: 'Token budget limited reviewer coverage',
  user_decision: 'User decision needed',
};

const RELIABILITY_NOTICE_SEVERITY_BY_KIND: Record<
  ReviewReliabilityNoticeKind,
  ReviewReliabilityNoticeSeverity
> = {
  context_pressure: 'info',
  compression_preserved: 'info',
  cache_hit: 'info',
  cache_miss: 'info',
  concurrency_limited: 'warning',
  partial_reviewer: 'warning',
  reduced_scope: 'info',
  retry_guidance: 'warning',
  skipped_reviewers: 'info',
  token_budget_limited: 'warning',
  user_decision: 'action',
};

function nonEmptyStrings(values?: Array<string | undefined | null>): string[] {
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

function isReducedScopeProfile(runManifest?: ReviewTeamRunManifest): boolean {
  const reviewDepth = runManifest?.scopeProfile?.reviewDepth;
  return reviewDepth === 'high_risk_only' || reviewDepth === 'risk_expanded';
}

function countDecisionItems(report: CodeReviewReportData): number {
  const structuredDecisionItems = report.report_sections?.remediation_groups?.needs_decision ?? [];
  if (structuredDecisionItems.length > 0) {
    const stringItems = structuredDecisionItems.filter(
      (item): item is string => typeof item === 'string',
    );
    return nonEmptyStrings(stringItems).length;
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

export function reliabilityNoticeMarkdownLine(
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

  const structuredReducedScope = structuredNotices.get('reduced_scope');
  if (structuredReducedScope) {
    notices.push(structuredReducedScope);
  } else if (isReducedScopeProfile(runManifest)) {
    notices.push({
      kind: 'reduced_scope',
      severity: 'info',
      source: 'manifest',
      ...(runManifest?.scopeProfile?.coverageExpectation
        ? { detail: runManifest.scopeProfile.coverageExpectation }
        : {}),
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
