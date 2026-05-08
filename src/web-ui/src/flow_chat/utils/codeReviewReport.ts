export type ReviewRiskLevel = 'low' | 'medium' | 'high' | 'critical';
export type ReviewAction = 'approve' | 'approve_with_suggestions' | 'request_changes' | 'block';
export type ReviewMode = 'standard' | 'deep';
export type ReviewIssueSeverity = 'critical' | 'high' | 'medium' | 'low' | 'info';
export type ReviewIssueCertainty = 'confirmed' | 'likely' | 'possible';
export type ReviewSectionId = 'summary' | 'issues' | 'remediation' | 'strengths' | 'team' | 'coverage';
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
  issue_count?: number;
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

export interface CodeReviewReportMarkdownLabels {
  titleStandard: string;
  titleDeep: string;
  executiveSummary: string;
  reviewDecision: string;
  riskLevel: string;
  recommendedAction: string;
  scope: string;
  issues: string;
  noIssues: string;
  remediationPlan: string;
  strengths: string;
  reviewTeam: string;
  coverageNotes: string;
  status: string;
  findings: string;
  validation: string;
  suggestion: string;
  source: string;
  noItems: string;
  groupTitles: Record<RemediationGroupId | StrengthGroupId, string>;
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

export const DEFAULT_CODE_REVIEW_MARKDOWN_LABELS: CodeReviewReportMarkdownLabels = {
  titleStandard: 'Code Review Report',
  titleDeep: 'Deep Review Report',
  executiveSummary: 'Executive Summary',
  reviewDecision: 'Review Decision',
  riskLevel: 'Risk Level',
  recommendedAction: 'Recommended Action',
  scope: 'Scope',
  issues: 'Issues',
  noIssues: 'No validated issues.',
  remediationPlan: 'Remediation Plan',
  strengths: 'Strengths',
  reviewTeam: 'Code Review Team',
  coverageNotes: 'Coverage Notes',
  status: 'Status',
  findings: 'Findings',
  validation: 'Validation',
  suggestion: 'Suggestion',
  source: 'Source',
  noItems: 'None.',
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
    } else if (DEGRADED_REVIEWER_STATUSES.has(reviewer.status)) {
      degraded += 1;
    }
  }

  return {
    total: reviewers.length,
    completed,
    degraded,
  };
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
      ? coverageNotes
      : nonEmpty([confidenceNote]),
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

export function formatCodeReviewReportMarkdown(
  report: CodeReviewReportData,
  labels?: Partial<CodeReviewReportMarkdownLabels>,
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
    }
  }
  lines.push('');
  lines.push(`## ${mergedLabels.coverageNotes}`);
  pushList(lines, sections.coverageNotes, mergedLabels.noItems);

  return lines.join('\n').trimEnd();
}
