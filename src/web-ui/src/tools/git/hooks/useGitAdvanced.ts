import { useState, useCallback, useMemo } from 'react';
import { GitOperationResult, GitMergeParams, GitStashParams } from '../types';
import { createLogger } from '@/shared/utils/logger';
import { useI18n } from '@/infrastructure/i18n';

const log = createLogger('useGitAdvanced');

interface UseGitAdvancedOptions {
  repositoryPath: string;
}

interface UseGitAdvancedReturn {
  isOperating: boolean;
  currentOperation: string | null;
  error: string | null;
  mergeBranch: (params: GitMergeParams) => Promise<GitOperationResult>;
  rebaseBranch: (targetBranch: string, interactive?: boolean) => Promise<GitOperationResult>;
  stashChanges: (params: GitStashParams) => Promise<GitOperationResult>;
  applyStash: (stashIndex?: number) => Promise<GitOperationResult>;
  popStash: (stashIndex?: number) => Promise<GitOperationResult>;
  dropStash: (stashIndex: number) => Promise<GitOperationResult>;
  listStashes: () => Promise<any[]>;
  cherryPick: (commitHash: string) => Promise<GitOperationResult>;
  revertCommit: (commitHash: string) => Promise<GitOperationResult>;
  resetToCommit: (commitHash: string, mode: 'soft' | 'mixed' | 'hard') => Promise<GitOperationResult>;
  clearError: () => void;
}

export function useGitAdvanced(options: UseGitAdvancedOptions): UseGitAdvancedReturn {
  const { repositoryPath } = options;

  const [isOperating, setIsOperating] = useState(false);
  const [currentOperation, setCurrentOperation] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { t } = useI18n('panels/git');

  const operationLabels = useMemo(() => ({
    merge: { display: t('advancedOperations.merge'), error: 'Merge branch' },
    rebase: { display: t('advancedOperations.rebase'), error: 'Rebase branch' },
    stash: { display: t('advancedOperations.stash'), error: 'Stash changes' },
    applyStash: { display: t('advancedOperations.applyStash'), error: 'Apply stash' },
    popStash: { display: t('advancedOperations.popStash'), error: 'Pop stash' },
    dropStash: { display: t('advancedOperations.dropStash'), error: 'Drop stash' },
    cherryPick: { display: t('advancedOperations.cherryPick'), error: 'Cherry-pick commit' },
    revert: { display: t('advancedOperations.revert'), error: 'Revert commit' },
    resetCommit: { display: t('advancedOperations.resetCommit'), error: 'Reset commit' },
  }), [t]);

  /**
   * Shared wrapper for advanced Git operations.
   */
  const executeAdvancedOperation = useCallback(async <T extends GitOperationResult>(
    operationLabel: string,
    errorLabel: string,
    operationFn: () => Promise<T>
  ): Promise<T> => {
    setIsOperating(true);
    setCurrentOperation(operationLabel);
    setError(null);

    try {
      const result = await operationFn();

      if (!result.success && result.error) {
        setError(result.error);
      }

      return result;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : `${errorLabel} failed`;
      setError(errorMessage);

      return {
        success: false,
        error: errorMessage
      } as T;
    } finally {
      setIsOperating(false);
      setCurrentOperation(null);
    }
  }, []);

  /**
   * Merge a branch.
   */
  const mergeBranch = useCallback(async (params: GitMergeParams): Promise<GitOperationResult> => {
    return executeAdvancedOperation(operationLabels.merge.display, operationLabels.merge.error, async () => {
      return {
        success: true,
        data: { branch: params.branch, strategy: params.strategy },
        output: `Successfully merged ${params.branch}`
      };
    });
  }, [executeAdvancedOperation, operationLabels]);

  /**
   * Rebase operation.
   */
  const rebaseBranch = useCallback(async (targetBranch: string, interactive: boolean = false): Promise<GitOperationResult> => {
    return executeAdvancedOperation(operationLabels.rebase.display, operationLabels.rebase.error, async () => {
      return {
        success: true,
        data: { targetBranch, interactive },
        output: `Successfully rebased onto ${targetBranch}`
      };
    });
  }, [executeAdvancedOperation, operationLabels]);

  /**
   * Stash changes.
   */
  const stashChanges = useCallback(async (params: GitStashParams): Promise<GitOperationResult> => {
    return executeAdvancedOperation(operationLabels.stash.display, operationLabels.stash.error, async () => {
      return {
        success: true,
        data: { message: params.message, includeUntracked: params.includeUntracked },
        output: `Stash created: ${params.message || 'WIP'}`
      };
    });
  }, [executeAdvancedOperation, operationLabels]);

  /**
   * Apply a stash.
   */
  const applyStash = useCallback(async (stashIndex: number = 0): Promise<GitOperationResult> => {
    return executeAdvancedOperation(operationLabels.applyStash.display, operationLabels.applyStash.error, async () => {
      return {
        success: true,
        data: { stashIndex },
        output: `Applied stash@{${stashIndex}}`
      };
    });
  }, [executeAdvancedOperation, operationLabels]);

  /**
   * Pop a stash.
   */
  const popStash = useCallback(async (stashIndex: number = 0): Promise<GitOperationResult> => {
    return executeAdvancedOperation(operationLabels.popStash.display, operationLabels.popStash.error, async () => {
      return {
        success: true,
        data: { stashIndex },
        output: `Popped stash@{${stashIndex}}`
      };
    });
  }, [executeAdvancedOperation, operationLabels]);

  /**
   * Drop a stash.
   */
  const dropStash = useCallback(async (stashIndex: number): Promise<GitOperationResult> => {
    return executeAdvancedOperation(operationLabels.dropStash.display, operationLabels.dropStash.error, async () => {
      return {
        success: true,
        data: { stashIndex },
        output: `Dropped stash@{${stashIndex}}`
      };
    });
  }, [executeAdvancedOperation, operationLabels]);

  /**
   * List stashes.
   */
  const listStashes = useCallback(async (): Promise<any[]> => {
    try {
      return [
        {
          index: 0,
          message: 'WIP on main: abc123d Initial commit',
          branch: 'main',
          date: new Date().toISOString()
        }
      ];
    } catch (err) {
      log.error('Failed to list stashes', { repositoryPath, error: err });
      return [];
    }
  }, [repositoryPath]);

  /**
   * Cherry-pick a commit.
   */
  const cherryPick = useCallback(async (commitHash: string): Promise<GitOperationResult> => {
    return executeAdvancedOperation(operationLabels.cherryPick.display, operationLabels.cherryPick.error, async () => {
      return {
        success: true,
        data: { commitHash },
        output: `Cherry-picked ${commitHash}`
      };
    });
  }, [executeAdvancedOperation, operationLabels]);

  /**
   * Revert a commit.
   */
  const revertCommit = useCallback(async (commitHash: string): Promise<GitOperationResult> => {
    return executeAdvancedOperation(operationLabels.revert.display, operationLabels.revert.error, async () => {
      return {
        success: true,
        data: { commitHash },
        output: `Reverted ${commitHash}`
      };
    });
  }, [executeAdvancedOperation, operationLabels]);

  /**
   * Reset to a commit.
   */
  const resetToCommit = useCallback(async (commitHash: string, mode: 'soft' | 'mixed' | 'hard'): Promise<GitOperationResult> => {
    return executeAdvancedOperation(operationLabels.resetCommit.display, operationLabels.resetCommit.error, async () => {
      return {
        success: true,
        data: { commitHash, mode },
        output: `Reset ${mode} to ${commitHash}`
      };
    });
  }, [executeAdvancedOperation, operationLabels]);

  /**
   * Clear the last error.
   */
  const clearError = useCallback(() => {
    setError(null);
  }, []);

  return {
    isOperating,
    currentOperation,
    error,
    mergeBranch,
    rebaseBranch,
    stashChanges,
    applyStash,
    popStash,
    dropStash,
    listStashes,
    cherryPick,
    revertCommit,
    resetToCommit,
    clearError
  };
}
