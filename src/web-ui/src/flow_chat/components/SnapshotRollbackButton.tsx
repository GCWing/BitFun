/**
 * Turn snapshot rollback button.
 * Allows rolling back to a specific dialog turn's code state.
 */

import React, { useState } from 'react';
import { RotateCcw, Check, AlertCircle, Loader2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { createLogger } from '@/shared/utils/logger';
import './SnapshotRollbackButton.scss';

const log = createLogger('SnapshotRollbackButton');

export interface SnapshotRollbackButtonProps {
  sessionId: string;
  turnIndex: number;
  turnId: string;
  isCurrentTurn?: boolean;
  onRollbackSuccess?: () => void;
  onRollbackError?: (error: string) => void;
}

export const SnapshotRollbackButton: React.FC<SnapshotRollbackButtonProps> = ({
  sessionId,
  turnIndex,
  turnId,
  isCurrentTurn = false,
  onRollbackSuccess,
  onRollbackError
}) => {
  const [isRollingBack, setIsRollingBack] = useState(false);
  const [rollbackStatus, setRollbackStatus] = useState<'idle' | 'success' | 'error'>('idle');
  const [errorMessage, setErrorMessage] = useState<string>('');

  const handleRollback = async () => {
    if (isRollingBack || isCurrentTurn) return;

    try {
      setIsRollingBack(true);
      setRollbackStatus('idle');
      setErrorMessage('');

      log.debug('Starting rollback', { sessionId, turnIndex, turnId });

      // Call backend API to roll back.
      await invoke('rollback_to_turn', {
        sessionId,
        turnIndex
      });

      setRollbackStatus('success');
      onRollbackSuccess?.();

      // Reset status after 3 seconds.
      setTimeout(() => {
        setRollbackStatus('idle');
      }, 3000);

    } catch (error) {
      log.error('Rollback failed', { sessionId, turnIndex, turnId, error });
      const errorMsg = error instanceof Error ? error.message : String(error);
      setErrorMessage(errorMsg);
      setRollbackStatus('error');
      onRollbackError?.(errorMsg);

      // Reset error state after 5 seconds.
      setTimeout(() => {
        setRollbackStatus('idle');
        setErrorMessage('');
      }, 5000);
    } finally {
      setIsRollingBack(false);
    }
  };

  if (isCurrentTurn) {
    return (
      <div data-bf-component="snapshot-rollback-button" data-bf-part="root" data-bf-outcome="current" className="snapshot-rollback-button snapshot-rollback-button--current">
        <Check size={14} />
        <span data-bf-component="snapshot-rollback-button" data-bf-part="label">Current code state</span>
      </div>
    );
  }

  if (rollbackStatus === 'success') {
    return (
      <div data-bf-component="snapshot-rollback-button" data-bf-part="root" data-bf-outcome="success" className="snapshot-rollback-button snapshot-rollback-button--success">
        <Check size={14} />
        <span data-bf-component="snapshot-rollback-button" data-bf-part="label">Rolled back to this turn</span>
      </div>
    );
  }

  if (rollbackStatus === 'error') {
    return (
      <div data-bf-component="snapshot-rollback-button" data-bf-part="root" data-bf-outcome="error" className="snapshot-rollback-button snapshot-rollback-button--error" title={errorMessage}>
        <AlertCircle size={14} />
        <span data-bf-component="snapshot-rollback-button" data-bf-part="label">Rollback failed</span>
      </div>
    );
  }

  return (
    <button data-bf-component="snapshot-rollback-button" data-bf-part="root" data-bf-outcome="idle" data-bf-state={isRollingBack ? 'loading' : ''}
      className={`snapshot-rollback-button ${isRollingBack ? 'snapshot-rollback-button--loading' : ''}`}
      onClick={handleRollback}
      disabled={isRollingBack}
      title={`Rollback to code state at turn ${turnIndex + 1}`}
    >
      {isRollingBack ? (
        <>
          <Loader2 size={14} className="snapshot-rollback-button__spinner" />
          <span data-bf-component="snapshot-rollback-button" data-bf-part="label">Rolling back...</span>
        </>
      ) : (
        <>
          <RotateCcw size={14} />
          <span data-bf-component="snapshot-rollback-button" data-bf-part="label">Rollback to this turn</span>
        </>
      )}
    </button>
  );
};

