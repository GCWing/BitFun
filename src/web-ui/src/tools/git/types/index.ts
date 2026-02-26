export * from './repository';
export * from './operations';
export * from './events';
export * from './git-agent.types';
export * from './graph';

export interface GitFeatureItem {
  id: string;
  title: string;
  description: string;
  icon: string;
  enabled: boolean;
  onClick: () => void;
  shortcut?: string;
}

export interface GitStatusDisplayConfig {
  showBranch: boolean;
  showChangeCount: boolean;
  showSyncStatus: boolean;
  showConflicts: boolean;
  refreshInterval: number;
}

export interface GitAIConfig {
  enableCommitMessageGeneration: boolean;
  enableConflictResolution: boolean;
  enableCodeReview: boolean;
  model?: string;
  apiConfig?: Record<string, any>;
}

export interface GitPreferences {
  defaultEditor?: string;
  autoStage?: boolean;
  autoPush?: boolean;
  confirmDangerousOperations: boolean;
  display: GitStatusDisplayConfig;
  ai: GitAIConfig;
}

export interface GitGlobalState {
  currentRepository: GitRepository | null;
  operationHistory: GitOperationHistory[];
  preferences: GitPreferences;
  isOperating: boolean;
  currentOperation?: string;
  error: string | null;
}

import { GitOperationHistory } from './operations';
import { GitRepository } from './repository';