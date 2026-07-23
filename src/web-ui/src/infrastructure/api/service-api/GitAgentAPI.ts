

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';




export type CommitType = 
  | 'Feat' 
  | 'Fix' 
  | 'Docs' 
  | 'Style' 
  | 'Refactor' 
  | 'Perf' 
  | 'Test' 
  | 'Chore' 
  | 'CI' 
  | 'Revert';


export type CommitFormat = 'Conventional' | 'Angular' | 'Simple' | 'Custom';


export type Language = 'Chinese' | 'English';


export type FileChangeType = 'Added' | 'Modified' | 'Deleted' | 'Renamed';


export type ChangePattern = 
  | 'FeatureAddition'
  | 'BugFix'
  | 'Refactoring'
  | 'ConfigChange'
  | 'DependencyUpdate'
  | 'DocumentationUpdate'
  | 'TestUpdate'
  | 'StyleChange';



export interface GenerateCommitMessageRequest {
  repoPath: string;
  options?: CommitMessageOptions;
}

export interface QuickCommitMessageRequest {
  repoPath: string;
}

export interface PreviewCommitMessageRequest {
  repoPath: string;
}



export interface CommitMessageOptions {
  format?: CommitFormat;
  includeFiles?: boolean;
  maxTitleLength?: number;
  includeBody?: boolean;
  language?: Language;
}



export interface CommitMessage {
  title: string;
  body?: string;
  footer?: string;
  fullMessage: string;
  commitType: CommitType;
  scope?: string;
  confidence: number;
  changesSummary: ChangesSummary;
}

export interface ChangesSummary {
  totalAdditions: number;
  totalDeletions: number;
  filesChanged: number;
  fileChanges: FileChange[];
  affectedModules: string[];
  changePatterns: ChangePattern[];
}

export interface FileChange {
  path: string;
  changeType: FileChangeType;
  additions: number;
  deletions: number;
  fileType: string;
}

export interface PreviewCommitMessageResponse {
  title: string;
  commitType: string;
  scope?: string;
  confidence: number;
  filesChanged: number;
  additions: number;
  deletions: number;
}



export class GitAgentAPI {
   
  async generateCommitMessage(request: GenerateCommitMessageRequest): Promise<CommitMessage> {
    try {
      return await api.invoke<CommitMessage>('generate_commit_message', { request });
    } catch (error) {
      throw createTauriCommandError('generate_commit_message', error, request);
    }
  }

   
  async quickCommitMessage(request: QuickCommitMessageRequest): Promise<CommitMessage> {
    try {
      return await api.invoke<CommitMessage>('quick_commit_message', { request });
    } catch (error) {
      throw createTauriCommandError('quick_commit_message', error, request);
    }
  }

   
  async quickCommit(repoPath: string): Promise<CommitMessage> {
    return this.quickCommitMessage({ repoPath });
  }

   
  async previewCommitMessage(request: PreviewCommitMessageRequest): Promise<PreviewCommitMessageResponse> {
    try {
      return await api.invoke<PreviewCommitMessageResponse>('preview_commit_message', { request });
    } catch (error) {
      throw createTauriCommandError('preview_commit_message', error, request);
    }
  }

   
  async previewCommit(repoPath: string): Promise<PreviewCommitMessageResponse> {
    return this.previewCommitMessage({ repoPath });
  }
}



export const gitAgentAPI = new GitAgentAPI();



export default gitAgentAPI;
