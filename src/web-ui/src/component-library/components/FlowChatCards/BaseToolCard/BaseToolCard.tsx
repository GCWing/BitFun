/**
 * Base Tool Card Component
 * Provides common layout and state management for all tool cards
 */

import React from 'react';
import { Loader2, CheckCircle, XCircle } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { ToolProcessingDots } from '../ToolProcessingDots';
import './BaseToolCard.scss';

export interface BaseToolCardProps {
  toolName: string;
  displayName: string;
  icon?: React.ReactNode | string;
  description?: string;
  primaryColor?: string;
  status?: 'pending' | 'preparing' | 'running' | 'streaming' | 'completed' | 'error';
  displayMode?: 'compact' | 'standard' | 'detailed';
  requiresConfirmation?: boolean;
  userConfirmed?: boolean;
  input?: Record<string, any>;
  result?: any;
  error?: string;
  children?: React.ReactNode;
  onConfirm?: (input?: any) => void;
  onReject?: () => void;
  onExpand?: () => void;
  className?: string;
}

export const BaseToolCard: React.FC<BaseToolCardProps> = ({
  displayName,
  icon,
  description,
  primaryColor = 'var(--bf-appearance-token-color-accent-600)',
  status = 'pending',
  displayMode = 'standard',
  requiresConfirmation = false,
  userConfirmed = false,
  input,
  result,
  error,
  children,
  onConfirm,
  onReject,
  onExpand,
  className = ''
}) => {
  const { t } = useI18n('components');
  const currentStatus = status as 'pending' | 'preparing' | 'running' | 'streaming' | 'completed' | 'cancelled' | 'error';
  
  const getStatusIcon = () => {
    switch (currentStatus) {
      case 'preparing':
        return <Loader2 className="base-tool-card__status-spinner base-tool-card__status-preparing" size={12} />;
      case 'running':
      case 'streaming':
        return <Loader2 className="base-tool-card__status-spinner" size={12} />;
      case 'completed':
        return <CheckCircle className="base-tool-card__status-success" size={12} />;
      case 'cancelled':
        return <XCircle className="base-tool-card__status-cancelled" size={12} />;
      case 'error':
        return <XCircle className="base-tool-card__status-error" size={12} />;
      default:
        return <ToolProcessingDots className="base-tool-card__status-pending" size={12} />;
    }
  };

  const getStatusText = () => {
    if (requiresConfirmation && !userConfirmed) {
      return t('flowChatCards.baseToolCard.awaitingConfirmation');
    }
    switch (currentStatus) {
      case 'preparing':
        return t('flowChatCards.baseToolCard.analyzing');
      case 'running':
      case 'streaming':
        return t('flowChatCards.baseToolCard.running');
      case 'completed':
        return t('flowChatCards.baseToolCard.completed');
      case 'cancelled':
        return t('flowChatCards.baseToolCard.cancelled');
      case 'error':
        return t('flowChatCards.baseToolCard.failed');
      default:
        return t('flowChatCards.baseToolCard.preparing');
    }
  };

  const handleConfirm = () => {
    onConfirm?.(input);
  };

  const handleReject = () => {
    onReject?.();
  };

  if (displayMode === 'compact') {
    return (
      <div 
        className={`base-tool-card base-tool-card--compact base-tool-card--${currentStatus} ${className}`}
        data-bf-component="flow-chat-card"
        data-bf-part="root"
        data-bf-display="compact"
        data-bf-status={currentStatus}
        style={{ '--base-tool-card-accent-color': primaryColor } as React.CSSProperties}
      >
        <div className="base-tool-card__compact-content" data-bf-component="flow-chat-card" data-bf-part="compact">
          {typeof icon === 'string' ? (
            <span className="base-tool-card__icon-emoji">{icon}</span>
          ) : (
            <span className="base-tool-card__icon">{icon}</span>
          )}
          <span className="base-tool-card__action">{displayName}</span>
          {children}
          <span className="base-tool-card__status" data-bf-component="flow-chat-card" data-bf-part="status">{getStatusIcon()}</span>
        </div>
      </div>
    );
  }

  return (
    <div 
      className={`base-tool-card base-tool-card--${displayMode} base-tool-card--${currentStatus} ${className}`}
      data-bf-component="flow-chat-card"
      data-bf-part="root"
      data-bf-display={displayMode}
      data-bf-status={currentStatus}
      style={{ '--base-tool-card-accent-color': primaryColor } as React.CSSProperties}
    >
      <div className="base-tool-card__header" data-bf-component="flow-chat-card" data-bf-part="header">
        <div className="base-tool-card__info">
          {typeof icon === 'string' ? (
            <span className="base-tool-card__icon-emoji">{icon}</span>
          ) : (
            <span className="base-tool-card__icon">{icon}</span>
          )}
          <div className="base-tool-card__details">
            <div className="base-tool-card__name">{displayName}</div>
            {description && (
              <div className="base-tool-card__description">{description}</div>
            )}
          </div>
        </div>
        <div className="base-tool-card__status-container" data-bf-component="flow-chat-card" data-bf-part="status">
          <span className="base-tool-card__status-icon">{getStatusIcon()}</span>
          <span className="base-tool-card__status-text">{getStatusText()}</span>
        </div>
      </div>

      {children && (
        <div className="base-tool-card__content" data-bf-component="flow-chat-card" data-bf-part="content">
          {children}
        </div>
      )}

      {displayMode === 'detailed' && input && (
        <div className="base-tool-card__input">
          <div className="base-tool-card__input-label">{t('flowChatCards.baseToolCard.inputParams')}</div>
          <div className="base-tool-card__input-content">
            <pre>{JSON.stringify(input, null, 2)}</pre>
          </div>
        </div>
      )}

      {requiresConfirmation && !userConfirmed && currentStatus !== 'completed' && (
        <div className="base-tool-card__actions" data-bf-component="flow-chat-card" data-bf-part="actions">
          <button 
            className="base-tool-card__button base-tool-card__button--confirm"
            onClick={handleConfirm}
            disabled={status === 'streaming'}
          >
            {t('flowChatCards.baseToolCard.confirm')}
          </button>
          <button 
            className="base-tool-card__button base-tool-card__button--reject"
            onClick={handleReject}
            disabled={status === 'streaming'}
          >
            {t('flowChatCards.baseToolCard.cancel')}
          </button>
        </div>
      )}

      {result && (
        <div className="base-tool-card__result" data-bf-component="flow-chat-card" data-bf-part="result">
          <div className="base-tool-card__result-label">{t('flowChatCards.baseToolCard.result')}</div>
          <div className="base-tool-card__result-content">
            <pre>{JSON.stringify(result, null, 2)}</pre>
          </div>
          {displayMode === 'standard' && onExpand && (
            <button 
              className="base-tool-card__button base-tool-card__button--expand"
              onClick={onExpand}
            >
              {t('flowChatCards.baseToolCard.viewDetails')}
            </button>
          )}
        </div>
      )}

      {error && (
        <div className="base-tool-card__error" data-bf-component="flow-chat-card" data-bf-part="error">
          <XCircle className="base-tool-card__error-icon" size={16} />
          <span className="base-tool-card__error-message">{error}</span>
        </div>
      )}
    </div>
  );
};
