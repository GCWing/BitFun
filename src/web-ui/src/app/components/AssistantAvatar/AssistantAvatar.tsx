import React from 'react';
import {
  getAssistantAvatarPreset,
  resolveAssistantAvatarPreset,
} from './assistantAvatarPresets';
import { firstAvatarGrapheme } from './assistantAvatarValue';
import './AssistantAvatar.scss';

export type AssistantAvatarStatus = 'idle' | 'running' | 'attention' | 'unread' | 'error';

export interface AssistantAvatarProps {
  presetId?: string | null;
  emoji?: string | null;
  stableKey?: string | null;
  name?: string | null;
  size?: number;
  status?: AssistantAvatarStatus;
  active?: boolean;
  decorative?: boolean;
  className?: string;
}

const AssistantAvatar: React.FC<AssistantAvatarProps> = ({
  presetId,
  emoji,
  stableKey,
  name,
  size = 32,
  status = 'idle',
  active = false,
  decorative = true,
  className = '',
}) => {
  const displayedEmoji = firstAvatarGrapheme(emoji ?? '');
  const explicitPreset = getAssistantAvatarPreset(presetId);
  const preset = explicitPreset ?? resolveAssistantAvatarPreset(undefined, stableKey);
  const usesPreset = Boolean(explicitPreset) || !displayedEmoji;
  const classes = [
    'assistant-avatar',
    usesPreset ? 'is-image' : 'is-emoji',
    active && 'is-active',
    status !== 'idle' && `is-${status}`,
    className,
  ].filter(Boolean).join(' ');
  const accessibleName = name?.trim() ? `${name.trim()} avatar` : 'Assistant avatar';

  return (
    <span
      className={classes}
      data-openbitfun-component="assistant-avatar"
      data-openbitfun-part="root"
      data-openbitfun-family={usesPreset ? preset.family : 'emoji'}
      data-openbitfun-preset={usesPreset ? preset.id : undefined}
      data-openbitfun-state={[active && 'active', status !== 'idle' && status].filter(Boolean).join(' ') || undefined}
      style={{ '--assistant-avatar-size': `${size}px` } as React.CSSProperties}
      role={decorative ? undefined : 'img'}
      aria-hidden={decorative ? 'true' : undefined}
      aria-label={decorative ? undefined : accessibleName}
    >
      <span className="assistant-avatar__art" aria-hidden="true">
        {usesPreset ? (
          <img
            className="assistant-avatar__image"
            src={preset.imageSrc}
            alt=""
            decoding="async"
            draggable={false}
          />
        ) : (
          <span className="assistant-avatar__emoji">{displayedEmoji}</span>
        )}
      </span>
      {status === 'attention' || status === 'unread' || status === 'error' ? (
        <span className="assistant-avatar__status-dot" aria-hidden="true" />
      ) : null}
    </span>
  );
};

export default AssistantAvatar;
