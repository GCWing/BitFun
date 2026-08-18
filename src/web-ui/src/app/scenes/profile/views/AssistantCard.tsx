import React from 'react';
import {
  ChevronRight,
  LoaderCircle,
  MessageSquarePlus,
  MoreHorizontal,
  Pin,
  Settings2,
  Trash2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge, Tooltip } from '@/component-library';
import { AssistantAvatar } from '@/app/components/AssistantAvatar';
import type { WorkspaceInfo } from '@/shared/types';

interface AssistantCardProps {
  workspace: WorkspaceInfo;
  onClick: () => void;
  onNewSession?: () => void;
  onDelete?: () => void;
  onSetPrimary?: () => void;
  isPrimary?: boolean;
  isDeleting?: boolean;
  isStartingSession?: boolean;
  isSettingPrimary?: boolean;
  style?: React.CSSProperties;
}

const AssistantCard: React.FC<AssistantCardProps> = ({
  workspace,
  onClick,
  onNewSession,
  onDelete,
  onSetPrimary,
  isPrimary,
  isDeleting = false,
  isStartingSession = false,
  isSettingPrimary = false,
  style,
}) => {
  const { t } = useTranslation('scenes/profile');
  const identity = workspace.identity;

  const name = identity?.name?.trim() || workspace.name || t('nursery.card.unnamed');
  const avatar = identity?.avatar?.trim() ?? '';
  const emoji = identity?.emoji?.trim() ?? '';
  const creature = identity?.creature?.trim() || '';
  const vibe = identity?.vibe?.trim() || '';

  return (
    <article
      data-bf-component="assistant-card"
      data-bf-part="root"
      data-bf-primary={isPrimary ? 'true' : 'false'}
      data-bf-state={isDeleting || isStartingSession || isSettingPrimary ? 'busy' : undefined}
      className={['assistant-card', (isDeleting || isSettingPrimary) && 'assistant-card--busy'].filter(Boolean).join(' ')}
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
        disabled={isDeleting || isSettingPrimary}
      >
        <span className="assistant-card__header" data-bf-component="assistant-card" data-bf-part="header">
          <span className="assistant-card__avatar" data-bf-component="assistant-card" data-bf-part="avatar">
            <AssistantAvatar
              presetId={avatar}
              emoji={emoji}
              stableKey={workspace.assistantId || workspace.id}
              name={name}
              size={44}
            />
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
            {vibe ? (
              <span className="assistant-card__vibe" data-bf-component="assistant-card" data-bf-part="vibe">{vibe}</span>
            ) : (
              <span className="assistant-card__vibe assistant-card__vibe--empty" data-bf-component="assistant-card" data-bf-part="vibe">
                {t('nursery.card.noVibe')}
              </span>
            )}
            {creature ? (
              <span className="assistant-card__badges" data-bf-component="assistant-card" data-bf-part="badges">
                <Badge variant="neutral">{creature}</Badge>
              </span>
            ) : null}
          </span>
          <MoreHorizontal
            data-bf-component="assistant-card"
            data-bf-part="chevron"
            className="assistant-card__chevron"
            size={16}
            strokeWidth={1.7}
            aria-hidden="true"
          />
        </span>
      </button>

      <footer className="assistant-card__footer" data-bf-component="assistant-card" data-bf-part="footer">
        <button
          data-bf-component="assistant-card"
          data-bf-part="configure"
          type="button"
          className="assistant-card__configure"
          onClick={onClick}
          disabled={isDeleting || isSettingPrimary}
          aria-label={`${t('nursery.card.configure')}: ${name}`}
        >
          <span className="assistant-card__configure-icon" aria-hidden="true">
            <Settings2 size={14} strokeWidth={1.8} />
          </span>
          <span>{t('nursery.card.configure')}</span>
          <ChevronRight className="assistant-card__configure-chevron" size={15} strokeWidth={1.7} aria-hidden="true" />
        </button>

        <span className="assistant-card__session-actions">
          {onNewSession ? (
            <button
              data-bf-component="assistant-card"
              data-bf-part="newSession"
              type="button"
              className="assistant-card__new-session-btn"
              onClick={onNewSession}
              disabled={isStartingSession || isDeleting || isSettingPrimary}
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
          ) : null}

          <span className="assistant-card__footer-actions">
            {onSetPrimary ? (
              <Tooltip content={t('nursery.card.setPrimary')} placement="top">
                <button
                  data-bf-component="assistant-card"
                  data-bf-part="setPrimary"
                  type="button"
                  className="assistant-card__set-primary-btn"
                  onClick={onSetPrimary}
                  aria-label={t('nursery.card.setPrimary')}
                  aria-busy={isSettingPrimary}
                  disabled={isDeleting || isStartingSession || isSettingPrimary}
                >
                  {isSettingPrimary ? (
                    <LoaderCircle className="nursery-spinning" size={14} strokeWidth={1.8} aria-hidden="true" />
                  ) : (
                    <Pin size={14} strokeWidth={1.8} aria-hidden="true" />
                  )}
                </button>
              </Tooltip>
            ) : null}

            {onDelete ? (
              <Tooltip content={t('nursery.card.delete')} placement="top">
                <button
                  data-bf-component="assistant-card"
                  data-bf-part="delete"
                  type="button"
                  className="assistant-card__delete-btn"
                  onClick={onDelete}
                  aria-label={t('nursery.card.delete')}
                  disabled={isDeleting || isStartingSession || isSettingPrimary}
                >
                  {isDeleting ? (
                    <LoaderCircle className="nursery-spinning" size={14} strokeWidth={1.8} aria-hidden="true" />
                  ) : (
                    <Trash2 size={14} strokeWidth={1.8} aria-hidden="true" />
                  )}
                </button>
              </Tooltip>
            ) : null}
          </span>
        </span>
      </footer>
    </article>
  );
};

export default AssistantCard;
