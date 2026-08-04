import React, { useState, useEffect, useCallback } from 'react';
import { snapshotAPI } from '@/infrastructure/api';
import { useI18n } from '@/infrastructure/i18n';
import type { TurnSnapshot } from '@/infrastructure/api/service-api/SnapshotAPI';
import { TurnRollbackButton } from './TurnRollbackButton';
import { createLogger } from '@/shared/utils/logger';
import './TurnHistoryPanel.scss';

const log = createLogger('TurnHistoryPanel');

interface TurnHistoryPanelProps {
  sessionId: string;
}

/**
 * Turn history panel.
 * Shows all turns in the current session and allows rollback.
 */
export const TurnHistoryPanel: React.FC<TurnHistoryPanelProps> = ({ sessionId }) => {
  const { formatDate } = useI18n('flow-chat');
  const [turns, setTurns] = useState<TurnSnapshot[]>([]);
  const [loading, setLoading] = useState(false);
  const [currentTurnIndex, setCurrentTurnIndex] = useState<number>(-1);

  const loadTurns = useCallback(async () => {
    if (!sessionId) return;
    
    setLoading(true);
    try {
      const turnList = await snapshotAPI.getSessionTurnSnapshots(sessionId);
      setTurns(turnList);
      setCurrentTurnIndex(turnList.length > 0 ? turnList.length - 1 : -1);
    } catch (error) {
      log.error('Failed to load turn snapshots', { sessionId, error });
    } finally {
      setLoading(false);
    }
  }, [sessionId]);

  useEffect(() => {
    void loadTurns();
  }, [loadTurns]);

  const handleRollbackComplete = () => {
    void loadTurns();
  };

  if (loading) {
    return <div data-bf-component="turn-history-panel" data-bf-part="loading" className="turn-history-panel-loading">Loading...</div>;
  }

  if (turns.length === 0) {
    return (
      <div data-bf-component="turn-history-panel" data-bf-part="empty" className="turn-history-panel-empty">
        <p>No turn history available.</p>
        <p className="hint">A snapshot is created after each AI response.</p>
      </div>
    );
  }

  return (
    <div data-bf-component="turn-history-panel" data-bf-part="root" className="turn-history-panel">
      <div data-bf-component="turn-history-panel" data-bf-part="header" className="turn-history-header">
        <h3>Session history</h3>
        <span data-bf-component="turn-history-panel" data-bf-part="count" className="turn-count">{turns.length} turns</span>
      </div>

      <div data-bf-component="turn-history-panel" data-bf-part="list" className="turn-history-list">
        {turns.map((turn, index) => (
          <div 
            key={`${turn.sessionId}-${turn.turnIndex}`} 
            data-bf-component="turn-history-panel"
            data-bf-part="item"
            data-bf-state={index === currentTurnIndex ? 'current' : ''}
            className={`turn-history-item ${index === currentTurnIndex ? 'current' : ''}`}
          >
            <div data-bf-component="turn-history-panel" data-bf-part="itemHeader" className="turn-item-header">
              <span className="turn-index">Turn {index + 1}</span>
              <TurnRollbackButton
                sessionId={turn.sessionId}
                turnIndex={turn.turnIndex}
                isCurrent={index === currentTurnIndex}
                onRollbackComplete={handleRollbackComplete}
              />
            </div>
            
            {turn.modifiedFiles.length > 0 && (
              <div data-bf-component="turn-history-panel" data-bf-part="files" className="turn-item-files">
                <span className="files-label">Modified files:</span>
                <ul data-bf-component="turn-history-panel" data-bf-part="filesList" className="files-list">
                  {turn.modifiedFiles.slice(0, 3).map((file: string, fileIndex: number) => (
                    <li key={fileIndex} className="file-item">{file}</li>
                  ))}
                  {turn.modifiedFiles.length > 3 && (
                    <li className="file-item-more">
                      {turn.modifiedFiles.length - 3} more files...
                    </li>
                  )}
                </ul>
              </div>
            )}

            <div data-bf-component="turn-history-panel" data-bf-part="time" className="turn-item-time">
              {formatDate(new Date(turn.timestamp * 1000), {
                dateStyle: 'medium',
                timeStyle: 'short',
              })}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};
