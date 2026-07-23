/**
 * Git Agent Hook - AI-powered commit message generation
 */

import { useState, useCallback, useRef } from 'react';
import { gitAgentAPI } from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';
import type {
  CommitMessage,
  CommitMessageOptions,
  PreviewCommitMessageResponse,
} from '../types/git-agent.types';

const log = createLogger('useGitAgent');

export interface UseGitAgentOptions {
  repoPath: string;
}

export interface UseGitAgentReturn {
  commitMessage: CommitMessage | null;
  commitPreview: PreviewCommitMessageResponse | null;
  isGeneratingCommit: boolean;
  generateCommitMessage: (options?: CommitMessageOptions) => Promise<void>;
  quickGenerateCommit: () => Promise<void>;
  previewCommitMessage: () => Promise<void>;
  cancelCommitGeneration: () => void;
  
  error: string | null;
  clearError: () => void;
  reset: () => void;
}

export const useGitAgent = ({ repoPath }: UseGitAgentOptions): UseGitAgentReturn => {
  const [commitMessage, setCommitMessage] = useState<CommitMessage | null>(null);
  const [commitPreview, setCommitPreview] = useState<PreviewCommitMessageResponse | null>(null);
  const [isGeneratingCommit, setIsGeneratingCommit] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const cancelledCommitRef = useRef(false);
  
  const generateCommitMessage = useCallback(async (options?: CommitMessageOptions) => {
    if (!repoPath) {
      setError('Repository path not set');
      return;
    }
    
    setIsGeneratingCommit(true);
    setError(null);
    
    try {
      const result = await gitAgentAPI.generateCommitMessage({
        repoPath,
        options
      });
      
      setCommitMessage(result);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to generate commit message';
      setError(errorMessage);
      log.error('Failed to generate commit message', { repoPath, error: err });
    } finally {
      setIsGeneratingCommit(false);
    }
  }, [repoPath]);
  
  const quickGenerateCommit = useCallback(async () => {
    if (!repoPath) {
      setError('Repository path not set');
      return;
    }
    
    cancelledCommitRef.current = false;
    setIsGeneratingCommit(true);
    setError(null);
    
    try {
      const result = await gitAgentAPI.quickCommit(repoPath);
      
      if (cancelledCommitRef.current) {
        return;
      }
      
      log.debug('Commit message generated', { commitType: result.commitType });
      setCommitMessage(result);
    } catch (err) {
      if (cancelledCommitRef.current) {
        return;
      }
      const errorMessage = err instanceof Error ? err.message : 'Failed to quick generate commit message';
      log.error('Failed to quick generate commit message', { repoPath, error: err });
      setError(errorMessage);
    } finally {
      setIsGeneratingCommit(false);
    }
  }, [repoPath]);
  
  const cancelCommitGeneration = useCallback(() => {
    cancelledCommitRef.current = true;
    setIsGeneratingCommit(false);
    setError(null);
  }, []);
  
  const previewCommitMessage = useCallback(async () => {
    if (!repoPath) {
      setError('Repository path not set');
      return;
    }
    
    setIsGeneratingCommit(true);
    setError(null);
    
    try {
      const result = await gitAgentAPI.previewCommit(repoPath);
      setCommitPreview(result);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to preview commit message';
      setError(errorMessage);
      log.error('Failed to preview commit message', { repoPath, error: err });
    } finally {
      setIsGeneratingCommit(false);
    }
  }, [repoPath]);

  /**
   * Clear the last error.
   */
  const clearError = useCallback(() => {
    setError(null);
  }, []);
  
  /**
   * Reset all state to initial values.
   */
  const reset = useCallback(() => {
    setCommitMessage(null);
    setCommitPreview(null);
    setError(null);
  }, []);
  
  return {

    commitMessage,
    commitPreview,
    isGeneratingCommit,
    generateCommitMessage,
    quickGenerateCommit,
    previewCommitMessage,
    cancelCommitGeneration,
    

    error,
    clearError,
    reset
  };
};

export default useGitAgent;
