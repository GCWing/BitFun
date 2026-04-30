 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('GitAPI');

export interface GitRepository {
  path: string;
  name: string;
  current_branch: string;
  is_bare: boolean;
  has_changes: boolean;
  remotes: string[];
  
  branch?: string;
  remote?: string;
  lastCommit?: string;
}


export interface GitFileStatusDetail {
   
  path: string;
   
  status: string;
   
  index_status?: string;
   
  workdir_status?: string;
}

export interface GitStatus {
  staged: GitFileStatusDetail[];
  unstaged: GitFileStatusDetail[];
  untracked: string[];
  conflicts: string[];
  current_branch: string;
  ahead: number;
  behind: number;
}

export interface GitCommit {
  hash: string;
  message: string;
  author: string;
  date: string;
  files?: string[];
}

export interface GitBranch {
  name: string;
  current: boolean;
  remote: boolean;
  lastCommit?: string;
  ahead?: number;
  behind?: number;
}

export interface GitOperationResult {
  success: boolean;
  message?: string;
  error?: string;
}

export interface GitAddParams {
  files: string[];
  all?: boolean;
}

export interface GitCommitParams {
  message: string;
  amend?: boolean;
  signoff?: boolean;
}

export interface GitPushParams {
  remote?: string;
  branch?: string;
  force?: boolean;
  setUpstream?: boolean;
}

export interface GitPullParams {
  remote?: string;
  branch?: string;
  rebase?: boolean;
}

export interface GitDiffParams {
  source?: string;
  target?: string;
  files?: string[];
  stat?: boolean;
  filePath?: string;
  staged?: boolean;
  commit?: string;
}

export interface GitChangedFilesParams {
  source?: string;
  target?: string;
  staged?: boolean;
}

export type GitChangedFileStatus =
  | 'added'
  | 'modified'
  | 'deleted'
  | 'renamed'
  | 'copied'
  | 'unknown';

export interface GitChangedFile {
  path: string;
  old_path?: string;
  status: GitChangedFileStatus;
}

export interface GitLogParams {
  maxCount?: number;
  since?: string;
  until?: string;
  author?: string;
}

export interface GitOperationParams {
  repositoryPath: string;
  command: string;
  args?: string[];
}


export interface GitGraphRef {
  name: string;
  refType: 'branch' | 'remote' | 'tag';
  isCurrent: boolean;
  isHead: boolean;
}

export interface GitGraphNode {
  hash: string;
  message: string;
  fullMessage: string;
  authorName: string;
  authorEmail: string;
  timestamp: number;
  parents: string[];
  children: string[];
  refs: GitGraphRef[];
  lane: number;
  forkingLanes: number[];
  mergingLanes: number[];
  passingLanes: number[];
}

export interface GitGraph {
  nodes: GitGraphNode[];
  maxLane: number;
  currentBranch?: string;
}


export interface GitWorktreeInfo {
   
  path: string;
   
  branch: string | null;
  /** HEAD commit hash */
  head: string;
   
  isMain: boolean;
   
  isLocked: boolean;
   
  isPrunable: boolean;
}

export class GitAPI {
   
  async isGitRepository(repositoryPath: string): Promise<boolean> {
    try {
      return await api.invoke('git_is_repository', { 
        request: { repositoryPath } 
      });
    } catch (error) {
      throw createTauriCommandError('git_is_repository', error, { repositoryPath });
    }
  }

   
  async getRepository(repositoryPath: string): Promise<GitRepository> {
    try {
      return await api.invoke('git_get_repository', { 
        request: { repositoryPath } 
      });
    } catch (error) {
      throw createTauriCommandError('git_get_repository', error, { repositoryPath });
    }
  }

   
  async getStatus(repositoryPath: string): Promise<GitStatus> {
    try {
      return await api.invoke('git_get_status', { 
        request: { repositoryPath } 
      });
    } catch (error) {
      throw createTauriCommandError('git_get_status', error, { repositoryPath });
    }
  }

   
  async getBranches(repositoryPath: string, includeRemote: boolean = false): Promise<GitBranch[]> {
    try {
      return await api.invoke('git_get_branches', { 
        request: { repositoryPath, includeRemote } 
      });
    } catch (error) {
      throw createTauriCommandError('git_get_branches', error, { repositoryPath, includeRemote });
    }
  }

   
  async getEnhancedBranches(repositoryPath: string, includeRemote: boolean = false): Promise<GitBranch[]> {
    try {
      return await api.invoke('git_get_enhanced_branches', { 
        request: { repositoryPath, includeRemote } 
      });
    } catch (error) {
      throw createTauriCommandError('git_get_enhanced_branches', error, { repositoryPath, includeRemote });
    }
  }

   
  async getCommits(repositoryPath: string, params: GitLogParams = {}): Promise<GitCommit[]> {
    try {
      return await api.invoke('git_get_commits', { 
        request: { repositoryPath, params } 
      });
    } catch (error) {
      throw createTauriCommandError('git_get_commits', error, { repositoryPath, params });
    }
  }

   
  async addFiles(repositoryPath: string, params: GitAddParams): Promise<GitOperationResult> {
    try {
      return await api.invoke('git_add_files', { 
        request: { repositoryPath, params } 
      });
    } catch (error) {
      throw createTauriCommandError('git_add_files', error, { repositoryPath, params });
    }
  }

   
  async commit(repositoryPath: string, params: GitCommitParams): Promise<GitOperationResult> {
    try {
      return await api.invoke('git_commit', { 
        request: { repositoryPath, params } 
      });
    } catch (error) {
      throw createTauriCommandError('git_commit', error, { repositoryPath, params });
    }
  }

   
  async push(repositoryPath: string, params: GitPushParams = {}): Promise<GitOperationResult> {
    try {
      
      const backendParams = {
        remote: params.remote,
        branch: params.branch,
        force: params.force,
        set_upstream: params.setUpstream
      };
      
      return await api.invoke('git_push', { 
        request: { repositoryPath, params: backendParams } 
      });
    } catch (error) {
      throw createTauriCommandError('git_push', error, { repositoryPath, params });
    }
  }

   
  async pull(repositoryPath: string, params: GitPullParams = {}): Promise<GitOperationResult> {
    try {
      return await api.invoke('git_pull', { 
        request: { repositoryPath, params } 
      });
    } catch (error) {
      throw createTauriCommandError('git_pull', error, { repositoryPath, params });
    }
  }

   
  async checkoutBranch(repositoryPath: string, branchName: string): Promise<GitOperationResult> {
    try {
      return await api.invoke('git_checkout_branch', { 
        request: { repositoryPath, branchName } 
      });
    } catch (error) {
      throw createTauriCommandError('git_checkout_branch', error, { repositoryPath, branchName });
    }
  }

   
  async createBranch(repositoryPath: string, branchName: string, startPoint?: string): Promise<GitOperationResult> {
    try {
      
      const effectiveStartPoint = startPoint && startPoint.trim() ? startPoint : undefined;
      return await api.invoke('git_create_branch', { 
        request: { repositoryPath, branchName, startPoint: effectiveStartPoint } 
      });
    } catch (error) {
      throw createTauriCommandError('git_create_branch', error, { repositoryPath, branchName, startPoint });
    }
  }

   
  async deleteBranch(repositoryPath: string, branchName: string, force: boolean = false): Promise<GitOperationResult> {
    try {
      return await api.invoke('git_delete_branch', { 
        request: { repositoryPath, branchName, force } 
      });
    } catch (error) {
      throw createTauriCommandError('git_delete_branch', error, { repositoryPath, branchName, force });
    }
  }

   
  async resetToCommit(repositoryPath: string, commitHash: string, mode: 'soft' | 'mixed' | 'hard' = 'mixed'): Promise<GitOperationResult> {
    try {
      return await api.invoke('git_reset_to_commit', { 
        request: { repositoryPath, commitHash, mode } 
      });
    } catch (error) {
      throw createTauriCommandError('git_reset_to_commit', error, { repositoryPath, commitHash, mode });
    }
  }

   
  async getDiff(repositoryPath: string, params: GitDiffParams): Promise<string> {
    try {
      return await api.invoke('git_get_diff', { 
        request: { repositoryPath, params } 
      });
    } catch (error) {
      throw createTauriCommandError('git_get_diff', error, { repositoryPath, params });
    }
  }

   
  async getChangedFiles(repositoryPath: string, params: GitChangedFilesParams): Promise<GitChangedFile[]> {
    try {
      return await api.invoke('git_get_changed_files', {
        request: { repositoryPath, params }
      });
    } catch (error) {
      throw createTauriCommandError('git_get_changed_files', error, { repositoryPath, params });
    }
  }


  async resetFiles(repositoryPath: string, files: string[], staged: boolean = false): Promise<GitOperationResult> {
    try {
      return await api.invoke('git_reset_files', { 
        request: { repositoryPath, files, staged } 
      });
    } catch (error) {
      throw createTauriCommandError('git_reset_files', error, { repositoryPath, files, staged });
    }
  }

   
  async getFileContent(repositoryPath: string, filePath: string, commit?: string): Promise<string> {
    try {
      return await api.invoke('git_get_file_content', { 
        request: { repositoryPath, filePath, commit } 
      });
    } catch (error) {
      throw createTauriCommandError('git_get_file_content', error, { repositoryPath, filePath, commit });
    }
  }
   
  async getGraph(repositoryPath: string, maxCount?: number, branchName?: string): Promise<GitGraph> {
    try {
      const result = await api.invoke<GitGraph>('git_get_graph', { 
        repositoryPath,
        maxCount: maxCount || null,
        branchName: branchName || null
      });
      return result;
    } catch (error) {
      log.error('Failed to get git graph', { repositoryPath, maxCount, branchName, error });
      throw createTauriCommandError('git_get_graph', error, { repositoryPath, maxCount, branchName });
    }
  }

   
  async cherryPick(repositoryPath: string, commitHash: string, noCommit: boolean = false): Promise<GitOperationResult> {
    try {
      return await api.invoke('git_cherry_pick', { 
        request: { repositoryPath, commitHash, noCommit } 
      });
    } catch (error) {
      throw createTauriCommandError('git_cherry_pick', error, { repositoryPath, commitHash, noCommit });
    }
  }

   
  async cherryPickAbort(repositoryPath: string): Promise<GitOperationResult> {
    try {
      return await api.invoke('git_cherry_pick_abort', { 
        request: { repositoryPath } 
      });
    } catch (error) {
      throw createTauriCommandError('git_cherry_pick_abort', error, { repositoryPath });
    }
  }

   
  async cherryPickContinue(repositoryPath: string): Promise<GitOperationResult> {
    try {
      return await api.invoke('git_cherry_pick_continue', { 
        request: { repositoryPath } 
      });
    } catch (error) {
      throw createTauriCommandError('git_cherry_pick_continue', error, { repositoryPath });
    }
  }

  

   
  async listWorktrees(repositoryPath: string): Promise<GitWorktreeInfo[]> {
    try {
      return await api.invoke('git_list_worktrees', { 
        request: { repositoryPath } 
      });
    } catch (error) {
      throw createTauriCommandError('git_list_worktrees', error, { repositoryPath });
    }
  }

   
  async addWorktree(repositoryPath: string, branch: string, createBranch: boolean = false): Promise<GitWorktreeInfo> {
    try {
      return await api.invoke('git_add_worktree', { 
        request: { repositoryPath, branch, createBranch } 
      });
    } catch (error) {
      throw createTauriCommandError('git_add_worktree', error, { repositoryPath, branch, createBranch });
    }
  }

   
  async removeWorktree(repositoryPath: string, worktreePath: string, force: boolean = false): Promise<GitOperationResult> {
    try {
      return await api.invoke('git_remove_worktree', { 
        request: { repositoryPath, worktreePath, force } 
      });
    } catch (error) {
      throw createTauriCommandError('git_remove_worktree', error, { repositoryPath, worktreePath, force });
    }
  }
}


export const gitAPI = new GitAPI();
