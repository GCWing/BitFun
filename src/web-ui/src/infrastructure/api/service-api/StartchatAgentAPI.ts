 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

 
export interface WorkStateOptions {
   
  analyzeGit?: boolean;
   
  predictNextActions?: boolean;
   
  includeQuickActions?: boolean;
   
  language?: 'Chinese' | 'English';
}

 
export interface GreetingMessage {
   
  title: string;
   
  subtitle: string;
   
  tagline?: string;
}

 
export interface GitWorkState {
   
  currentBranch: string;
   
  unstagedFiles: number;
   
  stagedFiles: number;
   
  unpushedCommits: number;
   
  aheadBehind?: {
    ahead: number;
    behind: number;
  };
   
  modifiedFiles: FileModification[];
}

 
export interface FileModification {
   
  path: string;
   
  changeType: 'Added' | 'Modified' | 'Deleted' | 'Renamed' | 'Untracked';
   
  module?: string;
}

 
export interface WorkItem {
   
  title: string;
   
  description: string;
   
  relatedFiles: string[];
   
  category: 'Backend' | 'Frontend' | 'API' | 'Database' | 'Infrastructure' | 'Testing' | 'Documentation' | 'Other';
   
  icon: string;
}

 
export interface TimeInfo {
   
  minutesSinceLastCommit?: number;
   
  lastCommitTimeDesc?: string;
   
  timeOfDay: 'Morning' | 'Afternoon' | 'Evening' | 'Night';
}

 
export interface CurrentWorkState {
   
  summary: string;
   
  gitState?: GitWorkState;
   
  ongoingWork: WorkItem[];
   
  timeInfo: TimeInfo;
}

 
export interface PredictedAction {
   
  description: string;
   
  priority: 'High' | 'Medium' | 'Low';
   
  icon: string;
   
  isReminder: boolean;
}

 
export interface QuickAction {
   
  title: string;
   
  command: string;
   
  icon: string;
   
  actionType: 'Continue' | 'ViewStatus' | 'Commit' | 'Visualize' | 'Custom';
}

 
export interface WorkStateAnalysis {
   
  greeting: GreetingMessage;
   
  currentState: CurrentWorkState;
   
  predictedActions: PredictedAction[];
   
  quickActions: QuickAction[];
   
  analyzedAt: string;
}

 
export interface WorkStateSummaryResponse {
   
  greetingTitle: string;
   
  currentStateSummary: string;
   
  hasGitChanges: boolean;
   
  unstagedFiles: number;
   
  unpushedCommits: number;
   
  predictedActionsCount: number;
}

 
export class StartchatAgentAPI {
   
  async analyzeWorkState(
    repoPath: string,
    options?: WorkStateOptions
  ): Promise<WorkStateAnalysis> {
    try {
      return await api.invoke('analyze_work_state', {
        request: {
          repoPath,
          options: options || null
        }
      });
    } catch (error) {
      throw createTauriCommandError('analyze_work_state', error, { repoPath, options });
    }
  }

   
  async quickAnalyzeWorkState(repoPath: string, language?: 'Chinese' | 'English'): Promise<WorkStateAnalysis> {
    try {
      
      return await api.invoke('quick_analyze_work_state', {
        request: {
          repoPath,
          language: language || null
        }
      }, {
        retries: 0
      });
    } catch (error) {
      throw createTauriCommandError('quick_analyze_work_state', error, { repoPath });
    }
  }

   
  async generateGreetingOnly(repoPath: string): Promise<WorkStateAnalysis> {
    try {
      return await api.invoke('generate_greeting_only', {
        request: {
          repoPath
        }
      });
    } catch (error) {
      throw createTauriCommandError('generate_greeting_only', error, { repoPath });
    }
  }

   
  async getWorkStateSummary(repoPath: string): Promise<WorkStateSummaryResponse> {
    try {
      return await api.invoke('get_work_state_summary', {
        request: {
          repoPath
        }
      });
    } catch (error) {
      throw createTauriCommandError('get_work_state_summary', error, { repoPath });
    }
  }
}


export const startchatAgentAPI = new StartchatAgentAPI();

