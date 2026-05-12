import type { FlowToolItem, Session } from '../types/flow-chat';

export interface CodeReviewSummaryData {
  overall_assessment?: string;
  risk_level?: 'low' | 'medium' | 'high' | 'critical';
  recommended_action?: 'approve' | 'approve_with_suggestions' | 'request_changes' | 'block';
}

export interface CodeReviewIssueData {
  file?: string;
  severity?: string;
  title?: string;
}

export interface CodeReviewResultData {
  schemaVersion?: number;
  summary?: CodeReviewSummaryData;
  issues?: CodeReviewIssueData[];
  positive_points?: string[];
  review_mode?: 'standard' | 'deep';
  remediation_plan?: string[];
}

export type CodeReviewResultLookup =
  | { status: 'valid'; result: CodeReviewResultData }
  | { status: 'missing'; reason: 'no_submit_code_review' }
  | { status: 'invalid'; reason: 'unreadable_submit_code_review' };

export interface ReviewResultSummary {
  issueCount: number;
  riskLevel: CodeReviewSummaryData['risk_level'];
  recommendedAction: CodeReviewSummaryData['recommended_action'];
  summaryText: string;
}

function parseReviewResult(result: unknown): CodeReviewResultData | null {
  if (!result) {
    return null;
  }

  if (typeof result === 'string') {
    try {
      return parseReviewResult(JSON.parse(result));
    } catch {
      return null;
    }
  }

  if (typeof result === 'object' && 'summary' in result) {
    return result as CodeReviewResultData;
  }

  return null;
}

export function findLatestCodeReviewResultState(session?: Session | null): CodeReviewResultLookup {
  if (!session) {
    return {
      status: 'missing',
      reason: 'no_submit_code_review',
    };
  }

  // Scan dialog turns from newest to oldest
  const turns = session.dialogTurns;
  for (let t = turns.length - 1; t >= 0; t -= 1) {
    const rounds = turns[t].modelRounds;
    for (let r = rounds.length - 1; r >= 0; r -= 1) {
      const items = rounds[r].items;
      for (let i = items.length - 1; i >= 0; i -= 1) {
        const item = items[i];
        if (item.type === 'tool') {
          const toolItem = item as FlowToolItem;
          if (toolItem.toolName === 'submit_code_review') {
            const parsed = parseReviewResult(toolItem.toolResult?.result);
            if (parsed) {
              return {
                status: 'valid',
                result: parsed,
              };
            }
            return {
              status: 'invalid',
              reason: 'unreadable_submit_code_review',
            };
          }
        }
      }
    }
  }

  return {
    status: 'missing',
    reason: 'no_submit_code_review',
  };
}

export function findLatestCodeReviewResult(session?: Session | null): CodeReviewResultData | null {
  const state = findLatestCodeReviewResultState(session);
  return state.status === 'valid' ? state.result : null;
}

export function summarizeCodeReviewResult(result?: CodeReviewResultData | null): ReviewResultSummary {
  const issueCount = result?.issues?.length ?? 0;
  return {
    issueCount,
    riskLevel: result?.summary?.risk_level,
    recommendedAction: result?.summary?.recommended_action,
    summaryText: result?.summary?.overall_assessment?.trim() || '',
  };
}

function uniqueNonEmpty(values: Array<string | undefined | null>): string[] {
  const seen = new Set<string>();
  const next: string[] = [];

  for (const value of values) {
    const trimmed = value?.trim();
    if (!trimmed || seen.has(trimmed)) {
      continue;
    }
    seen.add(trimmed);
    next.push(trimmed);
  }

  return next;
}

export function collectReviewChangedFiles(params: {
  snapshotFiles?: string[];
  reviewResult?: CodeReviewResultData | null;
  requestedFiles?: string[];
}): string[] {
  const snapshotFiles = uniqueNonEmpty(params.snapshotFiles ?? []);
  if (snapshotFiles.length > 0) {
    return snapshotFiles;
  }

  const issueFiles = uniqueNonEmpty((params.reviewResult?.issues ?? []).map(issue => issue.file));
  if (issueFiles.length > 0) {
    return issueFiles;
  }

  return uniqueNonEmpty(params.requestedFiles ?? []);
}
