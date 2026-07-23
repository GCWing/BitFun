import React from 'react';
import { Bot, MessageSquarePlus, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge, Tooltip } from '@/component-library';
import type { WorkspaceInfo } from '@/shared/types';
import { getCardGradient } from '@/shared/utils/cardGradients';

interface AssistantCardProps {
  workspace: WorkspaceInfo;
  onClick: () => void;
  onNewSession?: () => void;
  onDelete?: () => void;
  isPrimary?: boolean;
  style?: React.CSSProperties;
}

const AssistantCard: React.FC<AssistantCardProps> = ({ workspace, onClick, onNewSession, onDelete, isPrimary, style }) => {
  const { t } = useTranslation('scenes/profile');
  const identity = workspace.identity;

  const name = identity?.name?.trim() || workspace.name || t('nursery.card.unnamed');
  const emoji = identity?.emoji?.trim() ?? '';
  const creature = identity?.creature?.trim() || '';
  const vibe = identity?.vibe?.trim() || '';

  const gradient = getCardGradient(workspace.id || name);

  const handleCardKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onClick();
    }
  };

  return (
    <div
      className="assistant-card"
      role="button"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={handleCardKeyDown}
      aria-label={name}
      style={{
        ...style,
        '--assistant-card-gradient': gradient,
      } as React.CSSProperties}
    >
      {/* Header: avatar + name + badges */}
      <div className="assistant-card__header">
        <div className="assistant-card__avatar">
          {emoji ? (
            <span className="assistant-card__emoji">{emoji}</span>
          ) : (
            <Bot className="assistant-card__avatar-icon" size={20} strokeWidth={1.6} aria-hidden />
          )}
        </div>
        <div className="assistant-card__header-info">
          <div className="assistant-card__title-row">
            <span className="assistant-card__name">{name}</span>
            {isPrimary && (
              <span className="assistant-card__primary-badge">
                {t('nursery.card.primaryBadge')}
              </span>
            )}
          </div>
          <div className="assistant-card__badges">
            {creature && <Badge variant="neutral">{creature}</Badge>}
          </div>
        </div>
      </div>

      {/* Body: vibe / description */}
      <div className="assistant-card__body">
        {vibe ? (
          <p className="assistant-card__vibe">{vibe}</p>
        ) : (
          <p className="assistant-card__vibe assistant-card__vibe--empty">
            {t('nursery.card.noVibe')}
          </p>
        )}
      </div>

      {/* Footer */}
      <div className="assistant-card__footer">
        <div className="assistant-card__footer-inner">
          <span className="assistant-card__footer-hint">
            {t('nursery.card.configure')}
          </span>
          {(onNewSession || onDelete) ? (
            <div className="assistant-card__footer-actions">
              {onNewSession && (
                <Tooltip content={t('nursery.card.newSession')} placement="top">
                  <button
                    type="button"
                    className="assistant-card__new-session-btn"
                    onClick={(e) => {
                      e.stopPropagation();
                      onNewSession();
                    }}
                    aria-label={t('nursery.card.newSession')}
                  >
                    <MessageSquarePlus size={15} strokeWidth={2} aria-hidden />
                  </button>
                </Tooltip>
              )}
              {onDelete && (
                <Tooltip content={t('nursery.card.delete')} placement="top">
                  <button
                    type="button"
                    className="assistant-card__delete-btn"
                    onClick={(e) => {
                      e.stopPropagation();
                      onDelete();
                    }}
                    aria-label={t('nursery.card.delete')}
                  >
                    <Trash2 size={14} strokeWidth={2} aria-hidden />
                  </button>
                </Tooltip>
              )}
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
};

export default AssistantCard;
