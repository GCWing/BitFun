import React from 'react';
import type { SessionLineageLifecycle } from '../utils/sessionLineage';
import { getSubagentAvatarDefinition } from './catalog';
import type { SubagentIdentityAssignment } from './allocator';
import './SubagentAvatar.scss';

export interface SubagentAvatarProps {
  identity: SubagentIdentityAssignment;
  name?: string;
  size?: number;
  status?: SessionLineageLifecycle;
  decorative?: boolean;
  className?: string;
}

export const SubagentAvatar: React.FC<SubagentAvatarProps> = ({
  identity,
  name,
  size = 28,
  status = 'idle',
  decorative = true,
  className = '',
}) => {
  const avatar = getSubagentAvatarDefinition(identity.avatarId);
  const classes = [
    'subagent-avatar',
    `subagent-avatar--${status}`,
    className,
  ].filter(Boolean).join(' ');
  const accessibleName = name?.trim() ? `${name.trim()} avatar` : 'Subagent avatar';

  return (
    <span
      className={classes}
      data-bf-component="subagent-avatar"
      data-bf-part="root"
      data-bf-avatar-id={identity.avatarId}
      data-bf-name-id={identity.nameId}
      data-bf-state={status}
      style={{ '--subagent-avatar-size': `${size}px` } as React.CSSProperties}
      role={decorative ? undefined : 'img'}
      aria-hidden={decorative ? 'true' : undefined}
      aria-label={decorative ? undefined : accessibleName}
    >
      <span className="subagent-avatar__art" aria-hidden="true">
        <img src={avatar.src} alt="" draggable={false} />
      </span>
      <span className="subagent-avatar__status" aria-hidden="true" />
    </span>
  );
};

SubagentAvatar.displayName = 'SubagentAvatar';
