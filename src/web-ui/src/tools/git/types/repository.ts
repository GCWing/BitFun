/**
 * Git repository types
 */

export type GitFileStatus = 
  | 'untracked'
  | 'modified'
  | 'added'
  | 'deleted'
  | 'renamed'
  | 'copied'
  | 'staged'
  | 'conflicted';

export interface GitFileChange {
  path: string;
  oldPath?: string;
  status: GitFileStatus;
  staged: boolean;
  additions?: number;
  deletions?: number;
}

export type GitBranchStatus = 
  | 'current'
  | 'local'
  | 'remote'
  | 'tracking'
  | 'diverged'
  | 'stale'
  | 'conflicted';

export type GitBranchRelationType = 
  | 'parent'
  | 'child'
  | 'sibling'
  | 'merged'
  | 'diverged';

export interface GitBranchRelation {
  type: GitBranchRelationType;
  branch: string;
  commonAncestor?: string;
  establishedAt?: Date;
}

export interface GitBranchStats {
  commitCount: number;
  contributorCount: number;
  fileChanges: number;
  linesChanged: {
    additions: number;
    deletions: number;
  };
  activityScore: number;
}

export interface GitBranch {
  name: string;
  current: boolean;
  remote: boolean;
  remoteName?: string;
  upstream?: string;
  ahead: number;
  behind: number;
  lastCommit?: string;
  lastCommitDate?: Date;
  
  baseBranch?: string;
  relations?: GitBranchRelation[];
  childBranches?: string[];
  mergedBranches?: string[];
  
  status?: GitBranchStatus;
  hasConflicts?: boolean;
  canMerge?: boolean;
  isStale?: boolean;
  mergeStatus?: string;
  
  stats?: GitBranchStats;
  createdAt?: string;
  lastActivityAt?: string;
  
  tags?: string[];
  description?: string;
  type?: string;
  branch_type?: string;
  linkedIssues?: string[];
}

export interface GitRemote {
  name: string;
  fetchUrl: string;
  pushUrl: string;
}

export interface GitCommit {
  hash: string;
  shortHash: string;
  message: string;
  author: string;
  authorEmail: string;
  date: Date;
  parents: string[];
  files?: GitFileChange[];
  additions?: number;
  deletions?: number;
}

export interface GitStatus {
  current_branch: string;
  staged: GitFileStatusDetail[];
  unstaged: GitFileStatusDetail[];
  untracked: string[];
  ahead: number;
  behind: number;
}

export interface GitFileStatusDetail {
  path: string;
  status: string;
  index_status?: string;
  workdir_status?: string;
}

export interface GitRepository {
  rootPath: string;
  name: string;
  status: GitStatus;
  branches: GitBranch[];
  remotes: GitRemote[];
  recentCommits: GitCommit[];
  isRepository: boolean;
  gitVersion?: string;
}

export interface GitConfig {
  userName?: string;
  userEmail?: string;
  defaultBranch?: string;
  autocrlf?: 'true' | 'false' | 'input';
  fileMode?: boolean;
}

export interface GitTag {
  name: string;
  type: 'lightweight' | 'annotated';
  target: string;
  message?: string;
  tagger?: string;
  date?: Date;
}