import type {
  CodeReviewReportSectionsData,
  RemediationGroupId,
  ReviewMode,
} from './codeReviewReport';

export interface CodeReviewRemediationSummary {
  overall_assessment?: string;
  risk_level?: 'low' | 'medium' | 'high' | 'critical';
  recommended_action?: 'approve' | 'approve_with_suggestions' | 'request_changes' | 'block';
}

export interface CodeReviewRemediationIssue {
  severity?: 'critical' | 'high' | 'medium' | 'low' | 'info';
  certainty?: 'confirmed' | 'likely' | 'possible';
  category?: string;
  file?: string;
  line?: number | null;
  title?: string;
  description?: string;
  suggestion?: string | null;
  source_reviewer?: string;
  validation_note?: string;
}

export interface CodeReviewRemediationData {
  summary?: CodeReviewRemediationSummary;
  issues?: CodeReviewRemediationIssue[];
  remediation_plan?: string[];
  review_mode?: ReviewMode;
  report_sections?: CodeReviewReportSectionsData;
}

export interface ReviewRemediationItem {
  id: string;
  index: number;
  plan: string;
  issue?: CodeReviewRemediationIssue;
  groupId?: RemediationGroupId;
  requiresDecision?: boolean;
  defaultSelected: boolean;
}

const DEFAULT_SELECTED_SEVERITIES = new Set(['critical', 'high', 'medium']);
const REMEDIATION_GROUP_ORDER: RemediationGroupId[] = [
  'must_fix',
  'should_improve',
  'needs_decision',
  'verification',
];

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

function hasConcreteFixSignal(issue?: CodeReviewRemediationIssue): boolean {
  return Boolean(issue?.suggestion?.trim()) && issue?.certainty === 'confirmed';
}

function shouldSelectByDefault(
  reviewData: CodeReviewRemediationData,
  issue?: CodeReviewRemediationIssue,
): boolean {
  if (issue?.severity && DEFAULT_SELECTED_SEVERITIES.has(issue.severity)) {
    return true;
  }

  if (hasConcreteFixSignal(issue)) {
    return true;
  }

  return !issue && (
    reviewData.summary?.recommended_action === 'request_changes' ||
    reviewData.summary?.recommended_action === 'block'
  );
}

function buildStructuredRemediationItems(
  reviewData: CodeReviewRemediationData,
): ReviewRemediationItem[] {
  const remediationGroups = reviewData.report_sections?.remediation_groups;
  if (!remediationGroups) {
    return [];
  }

  const items: ReviewRemediationItem[] = [];

  for (const groupId of REMEDIATION_GROUP_ORDER) {
    for (const plan of nonEmpty(remediationGroups[groupId])) {
      const index = items.length;
      const requiresDecision = groupId === 'needs_decision';
      items.push({
        id: `remediation-${groupId}-${index}`,
        index,
        plan,
        groupId,
        requiresDecision,
        defaultSelected: groupId === 'must_fix',
      });
    }
  }

  return items;
}

export function buildReviewRemediationItems(
  reviewData: CodeReviewRemediationData,
): ReviewRemediationItem[] {
  const structuredItems = buildStructuredRemediationItems(reviewData);
  if (structuredItems.length > 0) {
    return structuredItems;
  }

  const items: ReviewRemediationItem[] = [];

  (reviewData.remediation_plan ?? []).forEach((plan, index) => {
    const trimmedPlan = plan.trim();
    if (!trimmedPlan) {
      return;
    }

    const issue = reviewData.issues?.[index];
    items.push({
      id: `remediation-${index}`,
      index,
      plan: trimmedPlan,
      ...(issue ? { issue } : {}),
      defaultSelected: shouldSelectByDefault(reviewData, issue),
    });
  });

  return items;
}

export function getDefaultSelectedRemediationIds(items: ReviewRemediationItem[]): string[] {
  return items
    .filter((item) => item.defaultSelected)
    .map((item) => item.id);
}

function formatIssueLocation(issue: CodeReviewRemediationIssue): string {
  if (!issue.file) {
    return 'Unknown location';
  }

  return issue.line ? `${issue.file}:${issue.line}` : issue.file;
}

function formatIssueForPrompt(item: ReviewRemediationItem): string {
  const issue = item.issue;
  if (!issue) {
    const groupLabel = item.groupId ? ` [${item.groupId}]` : '';
    return `${item.index + 1}.${groupLabel} No directly-linked issue. Plan: ${item.plan}`;
  }

  return [
    `${item.index + 1}. [${issue.severity ?? 'unknown'}/${issue.certainty ?? 'unknown'}] ${issue.title ?? 'Untitled issue'} (${formatIssueLocation(issue)})`,
    `   Description: ${issue.description ?? 'N/A'}`,
    `   Suggestion: ${issue.suggestion ?? item.plan}`,
    issue.validation_note ? `   Validation: ${issue.validation_note}` : undefined,
  ].filter(Boolean).join('\n');
}

export function buildSelectedRemediationPrompt(params: {
  reviewData: CodeReviewRemediationData;
  selectedIds: Set<string>;
  rerunReview: boolean;
}): string {
  return buildSelectedReviewRemediationPrompt({
    ...params,
    reviewMode: 'deep',
  });
}

export function buildSelectedReviewRemediationPrompt(params: {
  reviewData: CodeReviewRemediationData;
  selectedIds: Set<string>;
  rerunReview: boolean;
  reviewMode: ReviewMode;
}): string {
  if (params.selectedIds.size === 0) {
    return '';
  }

  const selectedItems = buildReviewRemediationItems(params.reviewData)
    .filter((item) => params.selectedIds.has(item.id));

  if (selectedItems.length === 0) {
    return '';
  }

  const planBlock = selectedItems
    .map((item, index) => `${index + 1}. ${item.plan}`)
    .join('\n');
  const issuesBlock = selectedItems
    .map(formatIssueForPrompt)
    .join('\n\n');
  const isDeepReview = params.reviewMode === 'deep';
  const reviewLabel = isDeepReview ? 'Deep Review' : 'Code Review';
  const rerunInstruction = isDeepReview
    ? 'After implementing fixes, run the most relevant verification. Then launch a full follow-up deep review of the fix diff by dispatching the review team (Business Logic, Performance, Security reviewers in parallel, followed by ReviewJudge). Submit the follow-up review result via submit_code_review.'
    : 'After implementing fixes, run the most relevant verification. Then submit a follow-up standard code review of the fix diff via submit_code_review.';

  return [
    `The user approved remediation for selected ${reviewLabel} findings only.`,
    '',
    'Please implement only the selected remediation items below. Do not broaden scope beyond these selected findings unless required for correctness.',
    params.rerunReview ? rerunInstruction : 'After implementing fixes, summarize what changed and what verification was run.',
    '',
    '## Selected Remediation Plan',
    planBlock,
    '',
    '## Selected Review Findings',
    issuesBlock,
  ].join('\n');
}
