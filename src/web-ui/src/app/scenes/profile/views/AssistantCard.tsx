import React from 'react';
import {
  Bot,
  ChevronRight,
  LoaderCircle,
  MessageSquarePlus,
  Settings2,
  Trash2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge, Tooltip } from '@/component-library';
import type { WorkspaceInfo } from '@/shared/types';

interface AssistantCardProps {
  workspace: WorkspaceInfo;
  onClick: () => void;
  onNewSession?: () => void;
  onDelete?: () => void;
  isPrimary?: boolean;
  isDeleting?: boolean;
  isStartingSession?: boolean;
  style?: React.CSSProperties;
}

const AssistantCard: React.FC<AssistantCardProps> = ({
  workspace,
  onClick,
  onNewSession,
  onDelete,
  isPrimary,
  isDeleting = false,
  isStartingSession = false,
  style,
}) => {
  const { t } = useTranslation('scenes/profile');
  const identity = workspace.identity;

  const name = identity?.name?.trim() || workspace.name || t('nursery.card.unnamed');
  const emoji = identity?.emoji?.trim() ?? '';
  const creature = identity?.creature?.trim() || '';
  const vibe = identity?.vibe?.trim() || '';

  return (
    <article
      data-bf-component="assistant-card"
      data-bf-part="root"
      data-bf-primary={isPrimary ? 'true' : 'false'}
      data-bf-state={isDeleting || isStartingSession ? 'busy' : undefined}
      className={['assistant-card', isDeleting && 'assistant-card--busy'].filter(Boolean).join(' ')}
      role="listitem"
      style={style}
    >
      <button
        data-bf-component="assistant-card"
        data-bf-part="main"
        type="button"
        className="assistant-card__main"
        onClick={onClick}
        aria-label={`${t('nursery.card.configure')}: ${name}`}
        disabled={isDeleting}
      >
        <span className="assistant-card__header" data-bf-component="assistant-card" data-bf-part="header">
          <span className="assistant-card__avatar" data-bf-component="assistant-card" data-bf-part="avatar">
            {emoji ? (
              <span className="assistant-card__emoji">{emoji}</span>
            ) : (
              <Bot className="assistant-card__avatar-icon" size={20} strokeWidth={1.6} aria-hidden="true" />
            )}
          </span>
          <span className="assistant-card__header-info" data-bf-component="assistant-card" data-bf-part="headerInfo">
            <span className="assistant-card__title-row">
              <span className="assistant-card__name" data-bf-component="assistant-card" data-bf-part="name">{name}</span>
              {isPrimary && (
                <span className="assistant-card__primary-badge" data-bf-component="assistant-card" data-bf-part="primaryBadge">
                  {t('nursery.card.primaryBadge')}
                </span>
              )}
            </span>
            {creature ? (
              <span className="assistant-card__badges" data-bf-component="assistant-card" data-bf-part="badges">
                <Badge variant="neutral">{creature}</Badge>
              </span>
            ) : null}
          </span>
          <ChevronRight
            data-bf-component="assistant-card"
            data-bf-part="chevron"
            className="assistant-card__chevron"
            size={16}
            strokeWidth={1.7}
            aria-hidden="true"
          />
        </span>

        <span className="assistant-card__body" data-bf-component="assistant-card" data-bf-part="body">
          {vibe ? (
            <span className="assistant-card__vibe" data-bf-component="assistant-card" data-bf-part="vibe">{vibe}</span>
          ) : (
            <span className="assistant-card__vibe assistant-card__vibe--empty" data-bf-component="assistant-card" data-bf-part="vibe">
              {t('nursery.card.noVibe')}
            </span>
          )}
        </span>

        <span className="assistant-card__configure" data-bf-component="assistant-card" data-bf-part="configure">
          <Settings2 size={13} strokeWidth={1.8} aria-hidden="true" />
          <span>{t('nursery.card.configure')}</span>
        </span>
      </button>

      <footer className="assistant-card__footer" data-bf-component="assistant-card" data-bf-part="footer">
        {onNewSession ? (
          <button
            data-bf-component="assistant-card"
            data-bf-part="newSession"
            type="button"
            className="assistant-card__new-session-btn"
            onClick={onNewSession}
            disabled={isStartingSession || isDeleting}
            aria-busy={isStartingSession}
          >
            {isStartingSession ? (
              <LoaderCircle className="nursery-spinning" size={14} strokeWidth={1.8} aria-hidden="true" />
            ) : (
              <MessageSquarePlus size={14} strokeWidth={1.8} aria-hidden="true" />
            )}
            <span>
              {t(isStartingSession ? 'nursery.card.startingSession' : 'nursery.card.newSession')}
            </span>
          </button>
        ) : (
          <span />
        )}

        {onDelete ? (
          <Tooltip content={t('nursery.card.delete')} placement="top">
            <button
              data-bf-component="assistant-card"
              data-bf-part="delete"
              type="button"
              className="assistant-card__delete-btn"
              onClick={onDelete}
              aria-label={t('nursery.card.delete')}
              disabled={isDeleting || isStartingSession}
            >
              {isDeleting ? (
                <LoaderCircle className="nursery-spinning" size={14} strokeWidth={1.8} aria-hidden="true" />
              ) : (
                <Trash2 size={14} strokeWidth={1.8} aria-hidden="true" />
              )}
            </button>
          </Tooltip>
        ) : null}
      </footer>
    </article>
  );
};

export default AssistantCard;
