 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';


export interface GitRepoHistory {
  url: string;
  lastUsed: string;
  localPath?: string;
}

 
export class GitRepoHistoryAPI {
   
  async saveGitRepoHistory(repos: GitRepoHistory[]): Promise<void> {
    try {
      await api.invoke('save_git_repo_history', {
        request: { repos }
      });
    } catch (error) {
      throw createTauriCommandError('save_git_repo_history', error, { repos });
    }
  }

   
  async loadGitRepoHistory(): Promise<GitRepoHistory[]> {
    try {
      return await api.invoke('load_git_repo_history', {
        request: {}
      });
    } catch (error) {
      throw createTauriCommandError('load_git_repo_history', error);
    }
  }
}


export const gitRepoHistoryAPI = new GitRepoHistoryAPI();


