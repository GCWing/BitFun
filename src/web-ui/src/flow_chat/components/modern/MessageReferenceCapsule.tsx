import React from 'react';
import { OverflowText } from '@openbitfun/ui';
import { messageContextIcon, messageInlineTokenIcon } from './messageReferenceIcons';
import './MessageReferenceCapsule.scss';

export const MessageReferenceCapsule: React.FC<{
  className?: string;
  type: string;
  label: string;
  title?: string;
  children?: React.ReactNode;
}> = ({ className = '', type, label, title, children }) => {

  return (
    <span
      className={`message-reference-capsule message-reference-capsule--${type} user-message-item__reference user-message-item__reference--${type} ${className}`.trim()}
      data-openbitfun-component="user-message-item"
      data-openbitfun-part="content"
      data-openbitfun-state={type}
      title={title ?? label}
    >
      {children ?? (type === 'skill' || type === 'widget'
        ? messageInlineTokenIcon(type)
        : messageContextIcon(type))}
      <OverflowText className="message-reference-capsule__label user-message-item__reference-label">{label}</OverflowText>
    </span>
  );
};
