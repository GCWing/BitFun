/**
 * Git service layer - interacts with backend Tauri commands
 */

import { gitAPI } from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';
import { measureAsync } from '@/shared/utils/timing';
import { i18nService } from '@/infrastructure/i18n';

const log = createLogger('GitService');
export type { 
  GitRepository, 
  GitStatus, 
  GitCommit, 
  GitBranch, 
  GitOperationResult,
  GitAddParams,
  GitCommitParams,
  GitPushParams,
  GitPullParams,
  GitDiffParams,
  GitLogParams,
  GitGraph,
  GitGraphNode,
  GitGraphRef
} from '@/infrastructure/api/service-api/GitAPI';

import { 
  GitRepository, 
  GitStatus, 
  GitCommit, 
  GitBranch, 
  GitOperationResult,
  GitAddParams,
  GitCommitParams,
  GitPushParams,
  GitPullParams,
  GitDiffParams,
  GitLogParams
} from '../types';

export class GitService {
  private static instance: GitService;
  
  private nonGitRepositoryCache: Set<string> = new Set();
  private readonly CACHE_EXPIRE_TIME = 5 * 60 * 1000;
  private cacheTimestamps: Map<string, number> = new Map();

  private constructor() {}

  public static getInstance(): GitService {
    if (!GitService.instance) {
      GitService.instance = new GitService();
    }
    return GitService.instance;
  }

  private clearExpiredCache(): void {
    const now = Date.now();
    const expiredPaths: string[] = [];
    
    this.cacheTimestamps.forEach((timestamp, path) => {
      if (now - timestamp > this.CACHE_EXPIRE_TIME) {
        expiredPaths.push(path);
      }
    });
    
    expiredPaths.forEach(path => {
      this.nonGitRepositoryCache.delete(path);
      this.cacheTimestamps.delete(path);
    });
  }

  private isInNonGitCache(path: string): boolean {
    this.clearExpiredCache();
    return this.nonGitRepositoryCache.has(path);
  }

  private addToNonGitCache(path: string): void {
    this.nonGitRepositoryCache.add(path);
    this.cacheTimestamps.set(path, Date.now());
  }

  private removeFromNonGitCache(path: string): void {
    this.nonGitRepositoryCache.delete(path);
    this.cacheTimestamps.delete(path);
  }

  private adaptRepository(apiRepo: import('@/infrastructure/api/service-api/GitAPI').GitRepository): GitRepository {
    const status: GitStatus = {
      current_branch: apiRepo.branch || 'main',
      staged: [],
      unstaged: [],
      untracked: [],
      ahead: 0,
      behind: 0
    };

    return {
      rootPath: apiRepo.path,
      name: apiRepo.name,
      status: status,
      branches: [],
      remotes: [],
      recentCommits: [],
      isRepository: true
    };
  }

  private adaptStatus(apiStatus: import('@/infrastructure/api/service-api/GitAPI').GitStatus): GitStatus {
    return {
      current_branch: apiStatus.current_branch || 'main',
      staged: apiStatus.staged || [],
      unstaged: apiStatus.unstaged || [],
      untracked: apiStatus.untracked || [],
      ahead: apiStatus.ahead || 0,
      behind: apiStatus.behind || 0
    };
  }

  private adaptBranches(apiBranches: import('@/infrastructure/api/service-api/GitAPI').GitBranch[]): GitBranch[] {
    return apiBranches.map(branch => ({
      name: branch.name,
      current: branch.current,
      remote: branch.remote,
      lastCommit: branch.lastCommit,
      ahead: branch.ahead || 0,
      behind: branch.behind || 0,
      hasChanges: false,
      isProtected: false,
      upstreamBranch: branch.remote ? `origin/${branch.name}` : undefined
    }));
  }

  private adaptCommits(apiCommits: import('@/infrastructure/api/service-api/GitAPI').GitCommit[]): GitCommit[] {
    return apiCommits.map(commit => {
      const adaptedFiles = (commit.files || []).map(filePath => ({
        path: filePath,
        status: 'modified' as const,
        staged: false
      }));

      return {
        hash: commit.hash,
        shortHash: commit.hash.substring(0, 7),
        message: commit.message,
        author: commit.author,
        authorEmail: '',
        date: new Date(commit.date),
        parents: [],
        files: adaptedFiles
      };
    });
  }

  async isGitRepository(path: string): Promise<boolean> {
    try {
      if (this.isInNonGitCache(path)) {
        return false;
      }

      const timeoutPromise = new Promise<never>((_, reject) => 
        setTimeout(() => reject(new Error('Check repository timeout')), 5000)
      );
      
      const result = await Promise.race([
        gitAPI.isGitRepository(path),
        timeoutPromise
      ]);
      
      if (!result) {
        this.addToNonGitCache(path);
      } else {
        this.removeFromNonGitCache(path);
      }
      
      return result;
    } catch (error) {
      log.error('Failed to check git repository', error);
      this.addToNonGitCache(path);
      return false;
    }
  }

  async getRepository(path: string): Promise<GitRepository | null> {
    try {
      if (this.isInNonGitCache(path)) {
        return null;
      }

      const timeoutPromise = new Promise<never>((_, reject) => 
        setTimeout(() => reject(new Error('Get repository timeout')), 8000)
      );
      
      const result = await measureAsync(() => Promise.race([
        gitAPI.getRepository(path),
        timeoutPromise
      ]));
      
      log.debug('Repository info retrieved', { path, durationMs: result.durationMs });
      
      this.removeFromNonGitCache(path);
      
      return this.adaptRepository(result.value);
    } catch (error) {
      log.error('Failed to get repository info', error);
      this.addToNonGitCache(path);
      return null;
    }
  }

  async getStatus(repositoryPath: string): Promise<GitStatus | null> {
    try {
      if (this.isInNonGitCache(repositoryPath)) {
        return null;
      }

      const timeoutPromise = new Promise<never>((_, reject) => 
        setTimeout(() => reject(new Error('Get git status timeout')), 6000)
      );
      
      const result = await Promise.race([
        gitAPI.getStatus(repositoryPath),
        timeoutPromise
      ]);
      
      this.removeFromNonGitCache(repositoryPath);
      
      return this.adaptStatus(result);
    } catch (error) {
      log.error('Failed to get git status', error);
      this.addToNonGitCache(repositoryPath);
      return null;
    }
  }

  async getBranches(repositoryPath: string, includeRemote: boolean = false): Promise<GitBranch[]> {
    try {
      const result = await gitAPI.getBranches(repositoryPath, includeRemote);
      return this.adaptBranches(result);
    } catch (error) {
      log.error('Failed to get branches', error);
      return [];
    }
  }

  async getCommits(repositoryPath: string, params: GitLogParams = {}): Promise<GitCommit[]> {
    try {
      const result = await gitAPI.getCommits(repositoryPath, params);
      return this.adaptCommits(result);
    } catch (error) {
      log.error('Failed to get commits', error);
      return [];
    }
  }

  /**
   * Stage files.
   */
  async addFiles(repositoryPath: string, params: GitAddParams): Promise<GitOperationResult> {
    try {
      const result = await gitAPI.addFiles(repositoryPath, params);
      return result;
    } catch (error) {
      log.error('Failed to add files', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : i18nService.t('panels/git:errors.addFailed')
      };
    }
  }

  /**
   * Commit changes.
   */
  async commit(repositoryPath: string, params: GitCommitParams): Promise<GitOperationResult> {
    try {
      const result = await gitAPI.commit(repositoryPath, params);
      return result;
    } catch (error) {
      log.error('Failed to commit', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : i18nService.t('panels/git:errors.commitFailed')
      };
    }
  }

  /**
   * Push to remote.
   */
  async push(repositoryPath: string, params: GitPushParams = {}): Promise<GitOperationResult> {
    try {
      const result = await gitAPI.push(repositoryPath, params);
      return result;
    } catch (error) {
      log.error('Failed to push', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : i18nService.t('panels/git:errors.pushFailed')
      };
    }
  }

  /**
   * Pull from remote.
   */
  async pull(repositoryPath: string, params: GitPullParams = {}): Promise<GitOperationResult> {
    try {
      const result = await gitAPI.pull(repositoryPath, params);
      return result;
    } catch (error) {
      log.error('Failed to pull', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : i18nService.t('panels/git:errors.pullFailed')
      };
    }
  }

  /**
   * Checkout a branch.
   */
  async checkoutBranch(repositoryPath: string, branchName: string): Promise<GitOperationResult> {
    try {
      const result = await gitAPI.checkoutBranch(repositoryPath, branchName);
      return result;
    } catch (error) {
      log.error('Failed to checkout branch', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : i18nService.t('panels/git:errors.checkoutFailed')
      };
    }
  }

  /**
   * Create a branch.
   */
  async createBranch(repositoryPath: string, branchName: string, startPoint?: string): Promise<GitOperationResult> {
    try {
      const result = await gitAPI.createBranch(repositoryPath, branchName, startPoint);
      return result;
    } catch (error) {
      log.error('Failed to create branch', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : i18nService.t('panels/git:errors.createBranchFailed')
      };
    }
  }

  /**
   * Delete a branch.
   */
  async deleteBranch(repositoryPath: string, branchName: string, force: boolean = false): Promise<GitOperationResult> {
    try {
      const result = await gitAPI.deleteBranch(repositoryPath, branchName, force);
      return result;
    } catch (error) {
      log.error('Failed to delete branch', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : i18nService.t('panels/git:errors.deleteBranchFailed')
      };
    }
  }

  /**
   * Get diff output.
   */
  async getDiff(repositoryPath: string, params: GitDiffParams): Promise<string> {
    try {
      const result = await gitAPI.getDiff(repositoryPath, params);
      return result;
    } catch (error) {
      log.error('Failed to get diff', error);
      return '';
    }
  }

  /**
   * Reset changes for one or more files.
   */
  async resetFiles(repositoryPath: string, files: string[], staged: boolean = false): Promise<GitOperationResult> {
    try {
      const result = await gitAPI.resetFiles(repositoryPath, files, staged);
      return result;
    } catch (error) {
      log.error('Failed to reset files', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : i18nService.t('panels/git:errors.resetFilesFailed')
      };
    }
  }

  /**
   * Reset to a commit.
   */
  async resetToCommit(repositoryPath: string, commitHash: string, mode: 'soft' | 'mixed' | 'hard' = 'mixed'): Promise<GitOperationResult> {
    try {
      const result = await gitAPI.resetToCommit(repositoryPath, commitHash, mode);
      return result;
    } catch (error) {
      log.error('Failed to reset to commit', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : i18nService.t('panels/git:errors.resetCommitFailed')
      };
    }
  }

  /**
   * Get file content at a commit (defaults to current HEAD if provided by backend).
   */
  async getFileContent(repositoryPath: string, filePath: string, commit?: string): Promise<string> {
    try {
      const result = await gitAPI.getFileContent(repositoryPath, filePath, commit);
      return result;
    } catch (error) {
      log.error('Failed to get file content', error);
      return '';
    }
  }

  /**
   * Get enhanced branch list (falls back to basic list on failure).
   */
  async getEnhancedBranches(repositoryPath: string, includeRemote: boolean = false): Promise<GitBranch[]> {
    try {
      const result = await gitAPI.getEnhancedBranches(repositoryPath, includeRemote);
      return this.adaptBranches(result);
    } catch (error) {
      log.error('Failed to get enhanced branches', error);

      return this.getBranches(repositoryPath, includeRemote);
    }
  }

  /**
   * Cherry-pick a commit onto the current branch.
   */
  async cherryPick(repositoryPath: string, commitHash: string, noCommit: boolean = false): Promise<GitOperationResult> {
    try {
      const result = await gitAPI.cherryPick(repositoryPath, commitHash, noCommit);
      return result;
    } catch (error) {
      log.error('Failed to cherry-pick', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : i18nService.t('panels/git:errors.cherryPickFailed')
      };
    }
  }

  /**
   * Abort an in-progress cherry-pick.
   */
  async cherryPickAbort(repositoryPath: string): Promise<GitOperationResult> {
    try {
      const result = await gitAPI.cherryPickAbort(repositoryPath);
      return result;
    } catch (error) {
      log.error('Failed to abort cherry-pick', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : i18nService.t('panels/git:errors.cherryPickAbortFailed')
      };
    }
  }

  /**
   * Continue an in-progress cherry-pick.
   */
  async cherryPickContinue(repositoryPath: string): Promise<GitOperationResult> {
    try {
      const result = await gitAPI.cherryPickContinue(repositoryPath);
      return result;
    } catch (error) {
      log.error('Failed to continue cherry-pick', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : i18nService.t('panels/git:errors.cherryPickContinueFailed')
      };
    }
  }

}


export const gitService = GitService.getInstance();
