import { PARTIAL_TIMEOUT_REVIEWER_STATUSES } from './reliabilityNotices';
import type {
  CodeReviewIssue,
  CodeReviewReportData,
  CodeReviewReviewer,
  DecisionContext,
  RemediationGroupId,
  ReviewIssueStats,
  ReviewReportGroup,
  ReviewReportSections,
  ReviewReviewerStats,
  ReviewSectionId,
  StrengthGroupId,
} from './codeReviewReport';

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

const DEGRADED_REVIEWER_STATUSES = new Set([
  'timed_out',
  'cancelled_by_user',
  'failed',
  'skipped',
]);

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

function buildGroups<TId extends string>(
  order: TId[],
  data?: Partial<Record<TId, string[]>>,
): Array<ReviewReportGroup<TId>> {
  return order
    .map((id) => ({ id, items: nonEmpty(data?.[id]) }))
    .filter((group) => group.items.length > 0);
}

function buildLegacyRemediationGroups(
  report: CodeReviewReportData,
): Array<ReviewReportGroup<RemediationGroupId>> {
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

function buildLegacyStrengthGroups(
  report: CodeReviewReportData,
): Array<ReviewReportGroup<StrengthGroupId>> {
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

export function buildCodeReviewReportSections(report: CodeReviewReportData): ReviewReportSections {
  const structuredSections = report.report_sections;

  const rawRemediationGroups = structuredSections?.remediation_groups;
  const normalizedRemediationGroups: Partial<Record<RemediationGroupId, string[]>> = {};
  if (rawRemediationGroups) {
    for (const [key, entries] of Object.entries(rawRemediationGroups) as [
      RemediationGroupId,
      (string | DecisionContext)[] | undefined,
    ][]) {
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

export function getDefaultExpandedCodeReviewSectionIds(
  report: CodeReviewReportData,
): ReviewSectionId[] {
  const sections = buildCodeReviewReportSections(report);
  const expanded: ReviewSectionId[] = ['summary'];

  if (sections.remediationGroups.length > 0) {
    expanded.push('remediation');
  }

  return expanded;
}
