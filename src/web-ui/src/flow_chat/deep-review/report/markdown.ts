import type {
  CodeReviewIssue,
  CodeReviewReportData,
  CodeReviewReportMarkdownLabels,
  CodeReviewReportMarkdownOptions,
  RemediationGroupId,
  StrengthGroupId,
} from './codeReviewReport';
import {
  buildCodeReviewReliabilityNotices,
  RELIABILITY_NOTICE_FALLBACK_LABELS,
  reliabilityNoticeMarkdownLine,
} from './reliabilityNotices';
import { buildCodeReviewReportSections } from './reportSections';
import {
  DEFAULT_REVIEW_COVERAGE_SOURCE_LABELS,
  formatReviewCoverageSource,
} from './reviewCoverageSource';

export const DEFAULT_CODE_REVIEW_MARKDOWN_LABELS: CodeReviewReportMarkdownLabels = {
  titleStandard: 'Review Report',
  titleDeep: 'Strict Review Report',
  executiveSummary: 'Executive Summary',
  reviewDecision: 'Review Decision',
  riskLevel: 'Risk Level',
  recommendedAction: 'Recommended Action',
  evidenceStatus: 'Evidence Status',
  scope: 'Scope',
  issues: 'Issues',
  noIssues: 'No validated issues.',
  remediationPlan: 'Remediation Plan',
  strengths: 'Strengths',
  reliabilitySignals: 'Review Reliability',
  coverageNotes: 'Coverage Notes',
  validation: 'Validation',
  suggestion: 'Suggestion',
  source: 'Source',
  noItems: 'None.',
  coverageSourceLabels: DEFAULT_REVIEW_COVERAGE_SOURCE_LABELS,
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
    coverageSourceLabels: {
      ...DEFAULT_CODE_REVIEW_MARKDOWN_LABELS.coverageSourceLabels,
      ...labels?.coverageSourceLabels,
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
  options?: CodeReviewReportMarkdownOptions,
): string {
  const mergedLabels = mergeLabels(labels);
  const sections = buildCodeReviewReportSections(report);
  const issues = report.issues ?? [];
  const lines: string[] = [];

  lines.push(`# ${report.review_mode === 'deep' ? mergedLabels.titleDeep : mergedLabels.titleStandard}`);
  lines.push('');
  lines.push(`## ${mergedLabels.executiveSummary}`);
  pushList(lines, sections.executiveSummary, mergedLabels.noItems);
  lines.push('');
  lines.push(`## ${mergedLabels.reviewDecision}`);
  lines.push(`- ${mergedLabels.riskLevel}: ${report.summary?.risk_level ?? 'unknown'}`);
  lines.push(`- ${mergedLabels.recommendedAction}: ${report.summary?.recommended_action ?? 'unknown'}`);
  lines.push(`- ${mergedLabels.evidenceStatus}: ${report.evidence_status ?? 'unknown'}`);
  if (report.review_scope?.trim()) {
    lines.push(`- ${mergedLabels.scope}: ${report.review_scope.trim()}`);
  }
  lines.push('');
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
      const coverageSource = formatReviewCoverageSource(
        issue.source_reviewer,
        mergedLabels.coverageSourceLabels,
      );
      if (coverageSource) {
        lines.push(`   - ${mergedLabels.source}: ${coverageSource}`);
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
    lines.push(`### ${mergedLabels.groupTitles[group.id as RemediationGroupId]}`);
    pushList(lines, group.items, mergedLabels.noItems);
    lines.push('');
  }
  if (sections.remediationGroups.length === 0) {
    lines.push(`- ${mergedLabels.noItems}`);
    lines.push('');
  }
  lines.push(`## ${mergedLabels.strengths}`);
  for (const group of sections.strengthGroups) {
    lines.push(`### ${mergedLabels.groupTitles[group.id as StrengthGroupId]}`);
    pushList(lines, group.items, mergedLabels.noItems);
    lines.push('');
  }
  if (sections.strengthGroups.length === 0) {
    lines.push(`- ${mergedLabels.noItems}`);
    lines.push('');
  }
  lines.push(`## ${mergedLabels.coverageNotes}`);
  pushList(lines, sections.coverageNotes, mergedLabels.noItems);

  return lines.join('\n').trimEnd();
}
