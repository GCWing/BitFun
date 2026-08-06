import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('IssueFixAPI');

export interface IssueFixAvailability {
  available: boolean;
  program?: string | null;
  /** "override" (LOOPX_BIN / bundled sidecar) or "path"; null when missing. */
  source?: string | null;
  ghInstalled?: boolean;
  ghAuthenticated?: boolean;
}

export interface IssueFixKernelTodo {
  issueRef: string;
  issueUrl: string;
  todoId: string;
  status: string;
  selected: boolean;
}

export interface IssueFixHostLoopState {
  enabled: boolean;
  jobId?: string | null;
  sessionId?: string | null;
  activeTurnId?: string | null;
  nextRunAtMs?: number | null;
  lastRunStatus?: string | null;
  lastError?: string | null;
  consecutiveFailures?: number;
}

export interface IssueFixUserQuestion {
  todoId: string;
  prompt: string;
}

/** Open user-lane todo shown read-only in the "pending your action" block. */
export interface IssueFixUserTodo {
  todoId: string;
  taskClass: string;
  text: string;
  link?: string | null;
}

export type IssueFixUserDecision = 'approve' | 'reject' | 'cancel';

export interface IssueFixAutonomousStatusResponse {
  goalId: string;
  agentId: string;
  kernelState: string;
  shouldRun: boolean;
  actionRequired: boolean;
  recommendedAction?: string | null;
  gatePrompt?: string | null;
  selectedTodoId?: string | null;
  issues: IssueFixKernelTodo[];
  userQuestion?: IssueFixUserQuestion | null;
  userTodos?: IssueFixUserTodo[];
  hostLoop: IssueFixHostLoopState;
}

/** Cheap poll projection: LoopX todo list + host loop, no `quota should-run`. */
export interface IssueFixAutonomousPollResponse {
  goalId: string;
  agentId: string;
  actionRequired: boolean;
  issues: IssueFixKernelTodo[];
  userQuestion?: IssueFixUserQuestion | null;
  userTodos?: IssueFixUserTodo[];
  hostLoop: IssueFixHostLoopState;
}

export interface IssueFixAnswerUserQuestionRequest {
  repositoryPath: string;
  todoId: string;
  decision: IssueFixUserDecision;
  reason?: string | null;
}

export interface IssueFixStartAutonomousRequest {
  sessionId: string;
  repo: string;
  repositoryPath: string;
  issues: Array<{
    issueRef: string;
    issueUrl: string;
  }>;
}

export interface IssueFixStartAutonomousResponse extends IssueFixAutonomousStatusResponse {
  addedIssueRefs: string[];
  immediateTurnId?: string | null;
}

class IssueFixAPI {
  async probe(): Promise<IssueFixAvailability> {
    try {
      return await api.invoke('issue_fix_probe', {});
    } catch (error) {
      log.warn('Issue-fix probe failed; treating the feature as unavailable', { error });
      return { available: false };
    }
  }

  async getAutonomousStatus(repositoryPath: string): Promise<IssueFixAutonomousStatusResponse> {
    const request = { repositoryPath };
    try {
      return await api.invoke('issue_fix_autonomous_status', { request });
    } catch (error) {
      log.error('Failed to read continuous Issue-Fix state', { repositoryPath, error });
      throw createTauriCommandError('issue_fix_autonomous_status', error, request);
    }
  }

  /**
   * Background-poll variant of `getAutonomousStatus`: projects issue todos and
   * open gates from LoopX's todo list only, so an interval poll never invokes
   * `quota should-run` (which appends a LoopX rollout event per call).
   */
  async pollAutonomous(repositoryPath: string): Promise<IssueFixAutonomousPollResponse> {
    const request = { repositoryPath };
    try {
      return await api.invoke('issue_fix_autonomous_poll', { request });
    } catch (error) {
      log.error('Failed to poll continuous Issue-Fix state', { repositoryPath, error });
      throw createTauriCommandError('issue_fix_autonomous_poll', error, request);
    }
  }

  /**
   * Disable the host wake loop. LoopX Kernel state is left untouched; a later
   * start resumes from wherever the Kernel says the work stands.
   */
  async stopAutonomous(repositoryPath: string): Promise<IssueFixHostLoopState> {
    const request = { repositoryPath };
    try {
      return await api.invoke('issue_fix_stop_autonomous', { request });
    } catch (error) {
      log.error('Failed to stop continuous Issue-Fix', { repositoryPath, error });
      throw createTauriCommandError('issue_fix_stop_autonomous', error, request);
    }
  }

  async startAutonomous(
    request: IssueFixStartAutonomousRequest,
  ): Promise<IssueFixStartAutonomousResponse> {
    try {
      return await api.invoke('issue_fix_start_autonomous', { request });
    } catch (error) {
      log.error('Failed to start continuous Issue-Fix', {
        repo: request.repo,
        issueRefs: request.issues.map((issue) => issue.issueRef),
        error,
      });
      throw createTauriCommandError('issue_fix_start_autonomous', error, request);
    }
  }

  async answerUserQuestion(
    request: IssueFixAnswerUserQuestionRequest,
  ): Promise<IssueFixAutonomousStatusResponse> {
    try {
      return await api.invoke('issue_fix_answer_user_question', { request });
    } catch (error) {
      log.error('Failed to answer continuous Issue-Fix user question', {
        todoId: request.todoId,
        decision: request.decision,
        error,
      });
      throw createTauriCommandError('issue_fix_answer_user_question', error, request);
    }
  }
}

export const issueFixAPI = new IssueFixAPI();
