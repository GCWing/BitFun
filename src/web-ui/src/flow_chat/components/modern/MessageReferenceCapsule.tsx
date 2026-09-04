import React from 'react';
import { Icon } from '@openbitfun/ui';
import { AtSign, Code2, File, MessageCircle } from 'lucide-react';
import type { ContextType } from '@/shared/types/context';
import './MessageReferenceCapsule.scss';

export function messageContextIcon(type: ContextType | string): React.ReactNode {
  switch (type) {
    case 'session-reference': return <MessageCircle size={13} aria-hidden />;
    case 'file':
    case 'image': return <File size={13} aria-hidden />;
    case 'directory': return <Icon name="folder" size="lg" style={{ width: 13, height: 13 }} aria-hidden />;
    case 'code-snippet':
    case 'mermaid-node':
    case 'mermaid-diagram': return <Code2 size={13} aria-hidden />;
    case 'pull-request':
    case 'git-ref': return <Icon name="git" size="lg" style={{ width: 13, height: 13 }} aria-hidden />;
    case 'terminal-command': return <Icon name="terminal" size="lg" style={{ width: 13, height: 13 }} aria-hidden />;
    case 'url': return <Icon name="link" size="lg" style={{ width: 13, height: 13 }} aria-hidden />;
    default: return <AtSign size={13} aria-hidden />;
  }
}

/**
 * Returns the icon used for composer inline tokens. Keeping this projection
 * shared means persisted Turn Rail previews render the same visual language as
 * the already-sent user message.
 */
export function messageInlineTokenIcon(tokenType: string): React.ReactNode {
  switch (tokenType) {
    case 'skill':
      return <Icon name="extension" size="lg" style={{ width: 13, height: 13 }} aria-hidden />;
    case 'widget':
    default:
      return <AtSign size={13} aria-hidden />;
  }
}

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
      <span className="message-reference-capsule__label user-message-item__reference-label">{label}</span>
    </span>
  );
};
