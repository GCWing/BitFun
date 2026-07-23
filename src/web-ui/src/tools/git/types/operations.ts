

export type GitOperationType = 
  | 'init'
  | 'clone'
  | 'add'
  | 'commit'
  | 'push'
  | 'pull'
  | 'fetch'
  | 'merge'
  | 'rebase'
  | 'checkout'
  | 'branch'
  | 'tag'
  | 'reset'
  | 'revert'
  | 'stash'
  | 'clean'
  | 'diff'
  | 'log'
  | 'status'
  | 'config';

export type GitOperationStatus = 
  | 'pending'
  | 'running'
  | 'success'
  | 'failed'
  | 'cancelled';

export interface GitOperationResult {
  success: boolean;
  data?: any;
  error?: string;
  output?: string;
  duration?: number;
}

export interface GitOperationParams {
  type: GitOperationType;
  repositoryPath: string;
  args?: string[];
  options?: Record<string, any>;
}

export interface GitAddParams {
  files: string[];
  all?: boolean;
  force?: boolean;
}

export interface GitCommitParams {
  message: string;
  amend?: boolean;
  noVerify?: boolean;
  author?: {
    name: string;
    email: string;
  };
}

export interface GitPushParams {
  remote?: string;
  branch?: string;
  force?: boolean;
  setUpstream?: boolean;
  tags?: boolean;
}

export interface GitPullParams {
  remote?: string;
  branch?: string;
  rebase?: boolean;
  fastForward?: boolean;
}

export interface GitBranchParams {
  action: 'create' | 'delete' | 'rename' | 'list';
  name?: string;
  newName?: string;
  startPoint?: string;
  force?: boolean;
  includeRemote?: boolean;
}

export interface GitMergeParams {
  branch: string;
  strategy?: 'merge' | 'squash' | 'no-ff';
  message?: string;
  abort?: boolean;
}

export interface GitResetParams {
  mode: 'soft' | 'mixed' | 'hard';
  target?: string;
  files?: string[];
}

export interface GitDiffParams {
  source?: string;
  target?: string;
  files?: string[];
  staged?: boolean;
  stat?: boolean;
}

export interface GitLogParams {
  maxCount?: number;
  skip?: number;
  since?: string;
  until?: string;
  author?: string;
  grep?: string;
  path?: string;
  stat?: boolean;
  oneline?: boolean;
}

export interface GitConflictResolution {
  filePath: string;
  resolution: 'ours' | 'theirs' | 'manual';
  content?: string;
}

export interface GitStashParams {
  action: 'save' | 'pop' | 'apply' | 'drop' | 'list' | 'show';
  message?: string;
  index?: number;
  includeUntracked?: boolean;
}

export interface GitOperationHistory {
  id: string;
  type: GitOperationType;
  params: GitOperationParams;
  status: GitOperationStatus;
  result?: GitOperationResult;
  startTime: Date;
  endTime?: Date;
  description: string;
}