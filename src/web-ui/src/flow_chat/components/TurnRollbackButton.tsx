import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { snapshotAPI } from '@/infrastructure/api';
import { notificationService } from '@/shared/notification-system';
import { confirmDanger } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import './TurnRollbackButton.scss';

const log = createLogger('TurnRollbackButton');

interface TurnRollbackButtonProps {
  sessionId: string;
  turnIndex: number;
  isCurrent: boolean;
  onRollbackComplete?: () => void;
}

export const TurnRollbackButton: React.FC<TurnRollbackButtonProps> = ({
  sessionId,
  turnIndex,
  isCurrent,
  onRollbackComplete,
}) => {
  const { t } = useTranslation('flow-chat');
  const [loading, setLoading] = useState(false);
  
  const handleRollback = async () => {
    if (isCurrent || loading) return;

    const index = turnIndex + 1;
    const confirmed = await confirmDanger(
      t('message.rollbackPanelDialogTitle', { index }),
      (
        <>
          <p className="confirm-dialog__message-intro">{t('message.rollbackPanelDialogIntro')}</p>
          <ul className="confirm-dialog__bullet-list">
            <li>{t('message.rollbackPanelBulletRestore', { index })}</li>
            <li>{t('message.rollbackPanelBulletUndo', { index })}</li>
            <li>{t('message.rollbackPanelBulletHistory')}</li>
          </ul>
        </>
      )
    );

    if (!confirmed) return;
    
    setLoading(true);
    try {
      const restoredFiles = await snapshotAPI.rollbackToTurn(sessionId, turnIndex);
      
      log.debug('Rollback completed', { sessionId, turnIndex, restoredFilesCount: restoredFiles.length });
      
      // Notify related components to refresh.
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      
      // Refresh file tree.
      globalEventBus.emit('file-tree:refresh');
      
      // Refresh open files in the editor.
      restoredFiles.forEach(filePath => {
        globalEventBus.emit('editor:file-changed', { filePath });
      });
      
      // Refresh snapshot state.
      globalEventBus.emit('snapshot:rollback-completed', { 
        sessionId,
        turnIndex,
        restoredFiles
      });
      
      // Notify parent.
      if (onRollbackComplete) {
        onRollbackComplete();
      }
      
    } catch (error) {
      log.error('Rollback failed', { sessionId, turnIndex, error });
      notificationService.error(
        `${t('message.rollbackFailed')}: ${error instanceof Error ? error.message : String(error)}`
      );
    } finally {
      setLoading(false);
    }
  };
  
  if (isCurrent) {
    return <span className="turn-rollback-button-current">Current</span>;
  }
  
  return (
    <button
      className="turn-rollback-button"
      onClick={handleRollback}
      disabled={loading}
      title={`Rollback to turn ${turnIndex + 1}`}
    >
      {loading ? 'Rolling back...' : 'Rollback'}
    </button>
  );
};


