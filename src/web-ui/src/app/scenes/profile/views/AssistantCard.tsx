import React from 'react';
import { Button, Icon, IconButton, StatusPill, Tooltip } from '@openbitfun/ui';

import { useTranslation } from 'react-i18next';

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
      data-openbitfun-component="assistant-card"
      data-openbitfun-part="root"
      data-openbitfun-primary={isPrimary ? 'true' : 'false'}
      data-openbitfun-state={isDeleting || isStartingSession || isSettingPrimary ? 'busy' : undefined}
      className={['assistant-card', (isDeleting || isSettingPrimary) && 'assistant-card--busy'].filter(Boolean).join(' ')}
      role="listitem"
      style={style}
    >
      <button
        data-openbitfun-component="assistant-card"
        data-openbitfun-part="main"
        type="button"
        className="assistant-card__main"
        onClick={onClick}
        aria-label={`${t('nursery.card.configure')}: ${name}`}
        disabled={isDeleting || isSettingPrimary}
      >
        <span className="assistant-card__header" data-openbitfun-component="assistant-card" data-openbitfun-part="header">
          <span className="assistant-card__avatar" data-openbitfun-component="assistant-card" data-openbitfun-part="avatar">
            <AssistantAvatar
              presetId={avatar}
              emoji={emoji}
              stableKey={workspace.assistantId || workspace.id}
              name={name}
              size={44}
            />
          </span>
          <span className="assistant-card__header-info" data-openbitfun-component="assistant-card" data-openbitfun-part="headerInfo">
            <span className="assistant-card__title-row">
              <span className="assistant-card__name" data-openbitfun-component="assistant-card" data-openbitfun-part="name">{name}</span>
              {isPrimary && (
                <span className="assistant-card__primary-badge" data-openbitfun-component="assistant-card" data-openbitfun-part="primaryBadge">
                  {t('nursery.card.primaryBadge')}
                </span>
              )}
            </span>
            {vibe ? (
              <span className="assistant-card__vibe" data-openbitfun-component="assistant-card" data-openbitfun-part="vibe">{vibe}</span>
            ) : (
              <span className="assistant-card__vibe assistant-card__vibe--empty" data-openbitfun-component="assistant-card" data-openbitfun-part="vibe">
                {t('nursery.card.noVibe')}
              </span>
            )}
            {creature ? (
              <span className="assistant-card__badges" data-openbitfun-component="assistant-card" data-openbitfun-part="badges">
                <StatusPill tone="neutral">{creature}</StatusPill>
              </span>
            ) : null}
          </span>
          <Icon name="chevron-right" size="sm" data-openbitfun-component="assistant-card" data-openbitfun-part="chevron" className="assistant-card__chevron" aria-hidden="true" />
        </span>
      </button>

      <footer className="assistant-card__footer" data-openbitfun-component="assistant-card" data-openbitfun-part="footer">
        <Button
          variant="outline"
          size="sm"
          leadingIcon={<Icon name="settings" size="sm" />}
          trailingIcon={<Icon name="chevron-right" size="sm" />}
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
              leadingIcon={<Icon name="side-chat" size="sm" />}
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
                  data-openbitfun-component="assistant-card"
                  data-openbitfun-part="setPrimary"
                  size="sm"
                  onClick={onSetPrimary}
                  aria-label={t('nursery.card.setPrimary')}
                  loading={isSettingPrimary}
                  disabled={isDeleting || isStartingSession || isSettingPrimary}
                  icon={<Icon name="pin" size="sm" aria-hidden="true" />}
                />
              </Tooltip>
            ) : null}

            {onDelete ? (
              <Tooltip content={t('nursery.card.delete')}>
                <IconButton
                  data-openbitfun-component="assistant-card"
                  data-openbitfun-part="delete"
                  tone="danger"
                  size="sm"
                  onClick={onDelete}
                  aria-label={t('nursery.card.delete')}
                  loading={isDeleting}
                  disabled={isDeleting || isStartingSession || isSettingPrimary}
                  icon={<Icon name="delete" size="sm" aria-hidden="true" />}
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
