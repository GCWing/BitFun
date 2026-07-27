import { api } from './ApiClient';

export type SessionExecutionTargetRequest =
  | { kind: 'local' }
  | { kind: 'newManagedWorktree'; baseRef?: string; copyLocalChanges?: boolean }
  | { kind: 'existingWorktree'; worktreeId: string };

export type WorktreeLifecycle = 'managed' | 'permanent' | 'external';

export interface WorktreeSettings {
  rootPath: string;
  branchPrefix: string;
  copyLocalChanges: boolean;
}

export interface SessionExecutionTarget {
  kind: 'local' | 'managedWorktree' | 'existingWorktree';
  worktreeId?: string;
  rootPath: string;
  baseRef?: string;
  baseCommit?: string;
  branch?: string;
  lifecycle?: WorktreeLifecycle;
}

export interface WorktreeSessionSummary {
  sessionId: string;
  sessionName: string;
  status: string;
  archived: boolean;
}

export interface WorktreeSummary {
  worktreeId: string;
  projectWorkspacePath: string;
  path: string;
  head: string;
  branch?: string;
  lifecycle: WorktreeLifecycle;
  isMain: boolean;
  dirty: boolean;
  locked: boolean;
  missing: boolean;
  hasUnpublishedCommits: boolean;
  associatedSessionCount: number;
  runningSessionCount: number;
  sessions: WorktreeSessionSummary[];
}

export type WorktreeErrorCode =
  | 'remote_unsupported'
  | 'not_git_repository'
  | 'unborn_repo'
  | 'invalid_base_ref'
  | 'worktree_not_found'
  | 'worktree_busy'
  | 'worktree_locked'
  | 'dirty_worktree'
  | 'unpublished_commits'
  | 'copy_conflict'
  | 'invalid_path'
  | 'branch_exists'
  | 'request_conflict'
  | 'rollback_incomplete'
  | 'git_failed'
  | 'io_failed';

interface WorktreeErrorPayload {
  code: WorktreeErrorCode;
  message: string;
  recoveryPath?: string;
}

export class WorktreeCommandError extends Error {
  constructor(
    public readonly code: WorktreeErrorCode,
    message: string,
    public readonly recoveryPath?: string,
  ) {
    super(message);
    this.name = 'WorktreeCommandError';
  }
}

export interface WorktreeCreateRequest {
  requestId: string;
  projectWorkspacePath: string;
  sourceWorkspacePath?: string;
  baseRef?: string;
  copyLocalChanges?: boolean;
}

export interface WorktreeCreateResult {
  worktree: WorktreeSummary;
  executionTarget: SessionExecutionTarget;
  created: boolean;
}

export interface WorktreeMutationResult {
  worktree: WorktreeSummary;
}

export interface WorktreeChangedEvent {
  projectWorkspacePath: string;
}

export interface WorktreeSessionBindingResult {
  sessionId: string;
  workspacePath: string;
  projectWorkspacePath: string;
  workspaceId?: string;
  executionTarget: SessionExecutionTarget;
  /** Set when a released worktree was kept because it still held local work. */
  retainedWorktreePath?: string;
}

export function toWorktreeCommandError(error: unknown): WorktreeCommandError {
  const candidates: unknown[] = [error];
  if (error instanceof Error) {
    const enriched = error as Error & { data?: unknown; cause?: unknown };
    candidates.push(enriched.data, enriched.cause, error.message);
  }
  for (const candidate of candidates) {
    let value = candidate;
    if (typeof value === 'string') {
      try {
        value = JSON.parse(value);
      } catch {
        continue;
      }
    }
    if (value && typeof value === 'object') {
      const payload = value as Partial<WorktreeErrorPayload>;
      if (typeof payload.code === 'string' && typeof payload.message === 'string') {
        return new WorktreeCommandError(
          payload.code as WorktreeErrorCode,
          payload.message,
          payload.recoveryPath,
        );
      }
    }
  }
  return new WorktreeCommandError('git_failed', error instanceof Error ? error.message : String(error));
}

async function invokeWorktree<T>(command: string, request: unknown): Promise<T> {
  try {
    return await api.invoke<T>(command, { request });
  } catch (error) {
    throw toWorktreeCommandError(error);
  }
}

export class WorktreeAPI {
  list(projectWorkspacePath: string): Promise<WorktreeSummary[]> {
    return invokeWorktree('worktree_list', { projectWorkspacePath });
  }

  create(request: WorktreeCreateRequest): Promise<WorktreeCreateResult> {
    return invokeWorktree('worktree_create', request);
  }

  createBranch(
    projectWorkspacePath: string,
    worktreeId: string,
    branch: string,
    requestId: string,
  ): Promise<WorktreeMutationResult> {
    return invokeWorktree('worktree_create_branch', {
      projectWorkspacePath,
      worktreeId,
      branch,
      requestId,
    });
  }

  promote(
    projectWorkspacePath: string,
    worktreeId: string,
    requestId: string,
  ): Promise<WorktreeMutationResult> {
    return invokeWorktree('worktree_promote', {
      projectWorkspacePath,
      worktreeId,
      requestId,
    });
  }

  remove(
    projectWorkspacePath: string,
    worktreeId: string,
    requestId: string,
    force = false,
  ): Promise<{ worktreeId: string; removed: boolean }> {
    return invokeWorktree('worktree_remove', {
      projectWorkspacePath,
      worktreeId,
      requestId,
      force,
    });
  }

  recreate(
    projectWorkspacePath: string,
    worktreeId: string,
    requestId: string,
  ): Promise<WorktreeMutationResult> {
    return invokeWorktree('worktree_recreate', {
      projectWorkspacePath,
      worktreeId,
      requestId,
    });
  }

  /**
   * Move a session into a managed worktree, or back to the project checkout.
   * Only allowed while the session has no messages yet.
   */
  bindSession(
    sessionId: string,
    enabled: boolean,
    requestId: string,
  ): Promise<WorktreeSessionBindingResult> {
    return invokeWorktree('worktree_bind_session', { sessionId, enabled, requestId });
  }

  onChanged(callback: (event: WorktreeChangedEvent) => void): () => void {
    return api.listen<WorktreeChangedEvent>('worktree://changed', callback);
  }
}

export const worktreeAPI = new WorktreeAPI();
