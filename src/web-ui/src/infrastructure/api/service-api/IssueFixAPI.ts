import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('IssueFixAPI');

export interface IssueFixAvailability {
  available: boolean;
  /** Present only when available, for diagnostics. */
  program?: string | null;
}

export interface IssueFixPlanRequest {
  /** Public-safe `owner/repo`. */
  repo: string;
  issueRef: string;
  issueUrl: string;
  /** Local checkout. Only read from — planning never writes. */
  repositoryPath: string;
  baseBranch?: string;
}

export interface IssueFixPlanResponse {
  issueRef: string;
  route: 'fix_pr' | 'comment_only' | 'triage_only';
  nextStep: 'runnable_successor' | 'monitor_continuation' | 'user_gate' | 'no_followup';
  contextGrounding: 'grounded' | 'partial' | 'ungrounded' | 'not_provided';
  /** LoopX's reason codes, verbatim. */
  reasonCodes: string[];
  /** Which of change_scope / reproduction / validation are still unresolved. */
  unresolvedAspects: string[];
  /** The branch LoopX would use. Never created while planning. */
  issueBranch?: string | null;
  branchReady: boolean;
}

export interface IssueFixExecuteRequest {
  /** The session whose agent loop should do the fixing. */
  sessionId: string;
  /** Public-safe `owner/repo`. */
  repo: string;
  issueRef: string;
  issueUrl: string;
  /** Local checkout the agent works in. */
  repositoryPath: string;
  baseBranch?: string;
  /** Issue title, included in the task message. */
  issueTitle?: string;
}

export interface IssueFixExecuteResponse {
  issueRef: string;
  route: 'fix_pr' | 'comment_only' | 'triage_only';
  /** Whether the fix task was actually submitted to the agent loop. */
  submitted: boolean;
  /** Why nothing was submitted, when `submitted` is false. */
  notSubmittedReason?: string | null;
  /** The dialog turn id, when submitted. */
  turnId?: string | null;
}

/**
 * Planning-only access to the issue-fix chain.
 *
 * There is no execute path here by design: nothing this class can reach will
 * create a branch, run a command, or open a pull request.
 */
class IssueFixAPI {
  async probe(): Promise<IssueFixAvailability> {
    try {
      return await api.invoke('issue_fix_probe', {});
    } catch (error) {
      // Treat a probe failure as "unavailable" rather than surfacing an error:
      // the feature simply stays hidden, which is the same outcome as a host
      // without loopx installed.
      log.warn('Issue-fix probe failed; treating the feature as unavailable', { error });
      return { available: false };
    }
  }

  async planIssue(request: IssueFixPlanRequest): Promise<IssueFixPlanResponse> {
    try {
      return await api.invoke('issue_fix_plan_issue', { request });
    } catch (error) {
      log.error('Failed to plan an issue fix', {
        repo: request.repo,
        issueRef: request.issueRef,
        error,
      });
      throw createTauriCommandError('issue_fix_plan_issue', error, request);
    }
  }

  async executeIssue(request: IssueFixExecuteRequest): Promise<IssueFixExecuteResponse> {
    try {
      return await api.invoke('issue_fix_execute', { request });
    } catch (error) {
      log.error('Failed to execute an issue fix', {
        repo: request.repo,
        issueRef: request.issueRef,
        error,
      });
      throw createTauriCommandError('issue_fix_execute', error, request);
    }
  }
}

export const issueFixAPI = new IssueFixAPI();
