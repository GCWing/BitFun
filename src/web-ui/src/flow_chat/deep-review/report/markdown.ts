import type {
  CodeReviewIssue,
  CodeReviewReportData,
  CodeReviewReportMarkdownLabels,
  CodeReviewReportMarkdownOptions,
  RemediationGroupId,
  StrengthGroupId,
} from './codeReviewReport';
import { formatRunManifestMarkdownSection } from './manifestSections';
import {
  buildCodeReviewReliabilityNotices,
  RELIABILITY_NOTICE_FALLBACK_LABELS,
  reliabilityNoticeMarkdownLine,
} from './reliabilityNotices';
import { buildCodeReviewReportSections } from './reportSections';

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
    lines.push(formatRunManifestMarkdownSection(options.runManifest, mergedLabels));
    lines.push('');
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
