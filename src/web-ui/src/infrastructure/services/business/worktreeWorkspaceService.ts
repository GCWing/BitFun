import { gitAPI, type GitWorktreeInfo } from '@/infrastructure/api/service-api/GitAPI';
import type { WorkspaceInfo } from '@/shared/types';
import { isLinkedWorktreeWorkspace } from '@/shared/types';

export interface CreateWorktreeWorkspaceOptions {
  repositoryPath: string;
  branch: string;
  isNew: boolean;
  openAfterCreate: boolean;
  openWorkspace: (path: string) => Promise<WorkspaceInfo>;
}

export interface CreateWorktreeWorkspaceResult {
  worktree: GitWorktreeInfo;
  openedWorkspace?: WorkspaceInfo;
}

export class WorktreeWorkspaceCreationError extends Error {
  constructor(
    public readonly stage: 'create' | 'open',
    error: unknown,
  ) {
    super(error instanceof Error ? error.message : String(error));
    this.name = 'WorktreeWorkspaceCreationError';
  }
}

export interface DeleteWorktreeWorkspaceOptions {
  workspace: WorkspaceInfo;
  closeWorkspaceById: (workspaceId: string) => Promise<void>;
}

export async function createWorktreeWorkspace(
  options: CreateWorktreeWorkspaceOptions,
): Promise<CreateWorktreeWorkspaceResult> {
  let worktree: GitWorktreeInfo;
  try {
    worktree = await gitAPI.addWorktree(
      options.repositoryPath,
      options.branch,
      options.isNew,
    );
  } catch (error) {
    throw new WorktreeWorkspaceCreationError('create', error);
  }

  if (!options.openAfterCreate) {
    return { worktree };
  }

  let openedWorkspace: WorkspaceInfo;
  try {
    openedWorkspace = await options.openWorkspace(worktree.path);
  } catch (error) {
    throw new WorktreeWorkspaceCreationError('open', error);
  }
  return {
    worktree,
    openedWorkspace,
  };
}

export async function deleteWorktreeWorkspace(
  options: DeleteWorktreeWorkspaceOptions,
): Promise<void> {
  const { workspace, closeWorkspaceById } = options;

  if (!isLinkedWorktreeWorkspace(workspace) || !workspace.worktree) {
    throw new Error('Current workspace is not a removable linked worktree');
  }

  await closeWorkspaceById(workspace.id);
  await gitAPI.removeWorktree(
    workspace.worktree.mainRepoPath,
    workspace.rootPath,
  );
}
