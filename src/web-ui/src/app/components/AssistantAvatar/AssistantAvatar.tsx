import React from 'react';
import {
  getAssistantAvatarPreset,
  resolveAssistantAvatarPreset,
  type AssistantAvatarFamily,
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

const PresetArtwork: React.FC<{
  family: AssistantAvatarFamily;
  variant: number;
}> = ({ family, variant }) => {
  if (family === 'signal') {
    return (
      <svg viewBox="0 0 32 32" focusable="false" aria-hidden="true">
        <circle cx="16" cy="16" r={variant === 1 ? 3.4 : 2.8} />
        <path d="M10.5 11.2a7 7 0 0 0 0 9.6" fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" />
        <path className="assistant-avatar__secondary-stroke" d="M7.2 8.2a11.2 11.2 0 0 0 0 15.6" fill="none" strokeWidth="2.1" strokeLinecap="round" />
        <path d="M21.5 11.2a7 7 0 0 1 0 9.6" fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" />
        <path className="assistant-avatar__secondary-stroke" d="M24.8 8.2a11.2 11.2 0 0 1 0 15.6" fill="none" strokeWidth="2.1" strokeLinecap="round" />
      </svg>
    );
  }

  if (family === 'orbit') {
    return (
      <svg viewBox="0 0 32 32" focusable="false" aria-hidden="true">
        <circle className="assistant-avatar__secondary-fill" cx="16" cy="16" r="5.2" />
        <ellipse cx="16" cy="16" rx="12" ry="6.2" fill="none" stroke="currentColor" strokeWidth="2" transform={variant === 1 ? 'rotate(-24 16 16)' : 'rotate(28 16 16)'} />
        <circle cx={variant === 1 ? 26 : 7} cy={variant === 1 ? 12 : 11} r="2.2" />
        <circle className="assistant-avatar__secondary-fill" cx={variant === 1 ? 9 : 23} cy={variant === 1 ? 23 : 24} r="1.4" />
      </svg>
    );
  }

  if (family === 'mosaic') {
    return (
      <svg viewBox="0 0 32 32" focusable="false" aria-hidden="true">
        <rect x="6" y="6" width="9" height="9" rx="2.4" />
        <rect className="assistant-avatar__secondary-fill" x="17" y="6" width="9" height={variant === 1 ? 14 : 8} rx="2.4" />
        <rect className="assistant-avatar__secondary-fill" x="6" y="17" width={variant === 1 ? 14 : 8} height="9" rx="2.4" />
        <rect x={variant === 1 ? 22 : 16} y={variant === 1 ? 22 : 16} width={variant === 1 ? 4 : 10} height={variant === 1 ? 4 : 10} rx="2" />
      </svg>
    );
  }

  return (
    <svg viewBox="0 0 32 32" focusable="false" aria-hidden="true">
      <path
        d={variant === 1
          ? 'M8 13.2C8 8.7 11.5 6 16 6s8 2.7 8 7.2v5.6C24 23.3 20.5 26 16 26s-8-2.7-8-7.2v-5.6Z'
          : 'M7 14c0-5 4-8 9-8s9 3 9 8v4c0 5-4 8-9 8s-9-3-9-8v-4Z'}
        className="assistant-avatar__secondary-fill"
      />
      <circle cx="13" cy="16" r="1.8" />
      <circle cx="19" cy="16" r="1.8" />
      <path d={variant === 1 ? 'M13 21c1.8 1 4.2 1 6 0' : 'M13.2 21h5.6'} fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
      {variant === 1 ? <path d="m21.5 8.5 2-2 2 2-2 2-2-2Z" /> : null}
    </svg>
  );
};

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
  const preset = explicitPreset ?? resolveAssistantAvatarPreset(undefined, stableKey || name);
  const usesPreset = Boolean(explicitPreset) || !displayedEmoji;
  const classes = [
    'assistant-avatar',
    usesPreset ? 'is-preset' : 'is-emoji',
    active && 'is-active',
    status !== 'idle' && `is-${status}`,
    className,
  ].filter(Boolean).join(' ');
  const accessibleName = name?.trim() ? `${name.trim()} avatar` : 'Assistant avatar';

  return (
    <span
      className={classes}
      data-bf-component="assistant-avatar"
      data-bf-part="root"
      data-bf-family={usesPreset ? preset.family : 'emoji'}
      data-bf-preset={usesPreset ? preset.id : undefined}
      data-bf-palette={usesPreset ? preset.palette : undefined}
      data-bf-state={[active && 'active', status !== 'idle' && status].filter(Boolean).join(' ') || undefined}
      style={{ '--assistant-avatar-size': `${size}px` } as React.CSSProperties}
      role={decorative ? undefined : 'img'}
      aria-hidden={decorative ? 'true' : undefined}
      aria-label={decorative ? undefined : accessibleName}
    >
      <span className="assistant-avatar__art" aria-hidden="true">
        {usesPreset ? (
          <PresetArtwork family={preset.family} variant={preset.variant} />
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
