import React from 'react';
import { Button, IconButton } from '@bitfun/ui';
import {
  ChevronRight,
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
        <Button
          variant="outline"
          size="sm"
          leadingIcon={<Settings2 />}
          trailingIcon={<ChevronRight />}
          className="assistant-card__configure"
          onClick={onClick}
          disabled={isDeleting || isSettingPrimary}
          aria-label={`${t('nursery.card.configure')}: ${name}`}
        >
          {t('nursery.card.configure')}
        </Button>

        <span className="assistant-card__session-actions">
          {onNewSession ? (
            <Button
              variant="fill"
              size="sm"
              leadingIcon={<MessageSquarePlus />}
              loading={isStartingSession}
              onClick={onNewSession}
              disabled={isStartingSession || isDeleting || isSettingPrimary}
            >
              {t(isStartingSession ? 'nursery.card.startingSession' : 'nursery.card.newSession')}
            </Button>
          ) : null}

          <span className="assistant-card__footer-actions">
            {onSetPrimary ? (
              <Tooltip content={t('nursery.card.setPrimary')}>
                <IconButton
                  data-bf-component="assistant-card"
                  data-bf-part="setPrimary"
                  size="sm"
                  onClick={onSetPrimary}
                  aria-label={t('nursery.card.setPrimary')}
                  loading={isSettingPrimary}
                  disabled={isDeleting || isStartingSession || isSettingPrimary}
                  icon={<Pin size={14} strokeWidth={1.8} aria-hidden="true" />}
                />
              </Tooltip>
            ) : null}

            {onDelete ? (
              <Tooltip content={t('nursery.card.delete')}>
                <IconButton
                  data-bf-component="assistant-card"
                  data-bf-part="delete"
                  tone="danger"
                  size="sm"
                  onClick={onDelete}
                  aria-label={t('nursery.card.delete')}
                  loading={isDeleting}
                  disabled={isDeleting || isStartingSession || isSettingPrimary}
                  icon={<Trash2 size={14} strokeWidth={1.8} aria-hidden="true" />}
                />
              </Tooltip>
            ) : null}
          </span>
        </span>
      </footer>
    </article>
  );
};

export default AssistantCard;
