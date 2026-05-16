/**
 * SnapshotCard - snapshot file operation card component
 * Used to show file operations (Write/Edit/Delete) changes and provide confirmation actions
 */

import React from 'react';
import { CheckCircle, XCircle, Maximize2, FileText, Loader2 } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { BaseToolCard, BaseToolCardProps } from '../BaseToolCard';
import { ToolProcessingDots } from '../ToolProcessingDots';
import './SnapshotCard.scss';

export interface SnapshotCardProps extends Omit<BaseToolCardProps, 'toolName' | 'displayName'> {
  operationType?: 'write' | 'edit' | 'delete' | 'multi-edit';
  filePath?: string;
  diffStats?: {
    additions: number;
    deletions: number;
    filesCount?: number;
  };
  onAccept?: () => void;
  onReject?: () => void;
  onViewDetails?: () => void;
  loading?: boolean;
  snapshotStatus?: 'pending' | 'accepted' | 'rejected' | 'conflict';
}

export const SnapshotCard: React.FC<SnapshotCardProps> = ({
  operationType = 'edit',
  filePath,
  diffStats,
  input,
  result,
  status = 'pending',
  displayMode = 'compact',
  onAccept,
  onReject,
  onViewDetails,
  loading = false,
  snapshotStatus = 'pending',
  ...baseProps
}) => {
  const { t } = useI18n('components');
  const resolvedFilePath = filePath || input?.file_path || input?.target_file || input?.path || t('flowChatCards.snapshotCard.unspecifiedFile');
  const fileName = resolvedFilePath.split(/[/\\]/).pop() || t('flowChatCards.snapshotCard.file');

  const getOperationInfo = () => {
    const operationMap = {
      'write': { name: t('flowChatCards.snapshotCard.writeFile'), color: '#22c55e' },
      'edit': { name: t('flowChatCards.snapshotCard.editFile'), color: '#f59e0b' },
      'delete': { name: t('flowChatCards.snapshotCard.deleteFile'), color: '#ef4444' },
      'multi-edit': { name: t('flowChatCards.snapshotCard.multiEdit'), color: '#f59e0b' }
    };
    return operationMap[operationType] || operationMap['edit'];
  };

  const operationInfo = getOperationInfo();

  const stats = diffStats || {
    additions: 0,
    deletions: 0,
    filesCount: result?.filesCount || 1
  };

  const getStatusIcon = () => {
    switch (status) {
      case 'running':
      case 'streaming':
        return <Loader2 className="snapshot-card__status-spinner" size={12} />;
      case 'completed':
        return <CheckCircle className="snapshot-card__status-success" size={12} />;
      case 'error':
        return <XCircle className="snapshot-card__status-error" size={12} />;
      default:
        return <ToolProcessingDots className="snapshot-card__status-pending" size={12} />;
    }
  };

  if (displayMode === 'compact') {
    return (
      <div 
        className={`snapshot-card snapshot-card--compact snapshot-card--${snapshotStatus} status-${status}`}
        style={{ '--operation-color': operationInfo.color } as React.CSSProperties}
        onClick={onViewDetails}
      >
        <FileText className="snapshot-card__icon" size={14} />
        <span className="snapshot-card__action">{operationInfo.name}:</span>
        <span className="snapshot-card__filename" title={resolvedFilePath}>
          {fileName}
        </span>
        
        {stats.filesCount && stats.filesCount > 1 ? (
          <span className="snapshot-card__stats">
            <span className="file-count">{t('flowChatCards.snapshotCard.filesCount', { count: stats.filesCount })}</span>
            {stats.additions > 0 && <span className="additions">+{stats.additions}</span>}
            {stats.deletions > 0 && <span className="deletions">-{stats.deletions}</span>}
          </span>
        ) : (
          <span className="snapshot-card__stats">
            {stats.additions > 0 && <span className="additions">+{stats.additions}</span>}
            {stats.deletions > 0 && <span className="deletions">-{stats.deletions}</span>}
          </span>
        )}

        <span className="snapshot-card__status">{getStatusIcon()}</span>
        
        <div className="snapshot-card__actions" onClick={(e) => e.stopPropagation()}>
          <button
            className="snapshot-card__action-btn snapshot-card__action-btn--accept"
            onClick={onAccept}
            title={t('flowChatCards.snapshotCard.accept')}
            disabled={loading || status !== 'completed'}
          >
            <CheckCircle size={12} />
          </button>
          <button
            className="snapshot-card__action-btn snapshot-card__action-btn--reject"
            onClick={onReject}
            title={t('flowChatCards.snapshotCard.reject')}
            disabled={loading || status !== 'completed'}
          >
            <XCircle size={12} />
          </button>
          <button
            className="snapshot-card__action-btn snapshot-card__action-btn--fullscreen"
            onClick={onViewDetails}
            title={t('flowChatCards.snapshotCard.viewDetails')}
          >
            <Maximize2 size={12} />
          </button>
        </div>
      </div>
    );
  }

  return (
    <BaseToolCard
      toolName={operationType}
      displayName={operationInfo.name}
      icon={<FileText size={18} />}
      description={`${operationInfo.name}: ${fileName}`}
      status={status}
      displayMode={displayMode}
      input={input}
      result={result}
      primaryColor={operationInfo.color}
      className={`snapshot-card snapshot-card--${snapshotStatus}`}
      {...baseProps}
    >
      <div className="snapshot-card__file-info">
        <div className="snapshot-card__info-row">
          <span className="snapshot-card__label">{t('flowChatCards.snapshotCard.filePath')}:</span>
          <span className="snapshot-card__value" title={resolvedFilePath}>
            {resolvedFilePath}
          </span>
        </div>
        
        {(stats.additions > 0 || stats.deletions > 0) && (
          <div className="snapshot-card__diff-stats">
            {stats.additions > 0 && (
              <div className="snapshot-card__stat-item snapshot-card__stat-item--add">
                <span className="snapshot-card__stat-value">+{stats.additions}</span>
                <span className="snapshot-card__stat-label">{t('flowChatCards.snapshotCard.linesAdded')}</span>
              </div>
            )}
            {stats.deletions > 0 && (
              <div className="snapshot-card__stat-item snapshot-card__stat-item--del">
                <span className="snapshot-card__stat-value">-{stats.deletions}</span>
                <span className="snapshot-card__stat-label">{t('flowChatCards.snapshotCard.linesDeleted')}</span>
              </div>
            )}
          </div>
        )}
      </div>

      {status === 'completed' && (
        <div className="snapshot-card__button-group">
          <button
            className="snapshot-card__button snapshot-card__button--accept"
            onClick={onAccept}
            disabled={loading}
          >
            <CheckCircle size={14} />
            <span>{t('flowChatCards.snapshotCard.accept')}</span>
          </button>
          <button
            className="snapshot-card__button snapshot-card__button--reject"
            onClick={onReject}
            disabled={loading}
          >
            <XCircle size={14} />
            <span>{t('flowChatCards.snapshotCard.reject')}</span>
          </button>
          {onViewDetails && (
            <button
              className="snapshot-card__button snapshot-card__button--view"
              onClick={onViewDetails}
            >
              <Maximize2 size={14} />
              <span>{t('flowChatCards.snapshotCard.viewFullDiff')}</span>
            </button>
          )}
        </div>
      )}

      {(status === 'running' || status === 'streaming') && (
        <div className="snapshot-card__processing">
          <Loader2 className="snapshot-card__processing-icon" size={14} />
          <span>{t('flowChatCards.snapshotCard.processing')}</span>
        </div>
      )}

      {status === 'error' && result?.error && (
        <div className="snapshot-card__error">
          <XCircle size={14} />
          <span>{result.error}</span>
        </div>
      )}
    </BaseToolCard>
  );
};
