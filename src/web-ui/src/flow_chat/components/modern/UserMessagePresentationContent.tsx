import React from 'react';
import {
  AtSign,
  Code2,
  File,
  Folder,
  GitBranch,
  Link,
  MessageCircle,
  Puzzle,
  Terminal,
  type LucideIcon,
} from 'lucide-react';
import type { ContextItem } from '@/shared/types/context';
import type { ComposerPresentation } from '../../utils/composerPresentation';

function contextIcon(type: ContextItem['type']): LucideIcon {
  switch (type) {
    case 'session-reference':
      return MessageCircle;
    case 'file':
    case 'image':
      return File;
    case 'directory':
      return Folder;
    case 'code-snippet':
    case 'mermaid-node':
    case 'mermaid-diagram':
      return Code2;
    case 'pull-request':
    case 'git-ref':
      return GitBranch;
    case 'terminal-command':
      return Terminal;
    case 'url':
      return Link;
    case 'web-element':
      return AtSign;
    default: {
      const exhaustive: never = type;
      return exhaustive;
    }
  }
}

export const UserMessagePresentationContent: React.FC<{
  presentation: ComposerPresentation;
}> = ({ presentation }) => (
  <>
    {presentation.segments.map((segment, index) => {
      if (segment.kind === 'text') {
        return <React.Fragment key={`text-${index}`}>{segment.text}</React.Fragment>;
      }

      if (segment.kind === 'inline-token') {
        const Icon = segment.tokenType === 'skill' ? Puzzle : AtSign;
        return (
          <span
            key={`token-${index}`}
            className={`user-message-item__reference user-message-item__reference--${segment.tokenType}`}
            title={segment.label}
          >
            <Icon size={13} aria-hidden />
            <span className="user-message-item__reference-label">{segment.label}</span>
          </span>
        );
      }

      const Icon = contextIcon(segment.context.type);
      return (
        <span
          key={`context-${index}`}
          className={`user-message-item__reference user-message-item__reference--${segment.context.type}`}
          title={segment.title}
        >
          <Icon size={13} aria-hidden />
          <span className="user-message-item__reference-label">{segment.label}</span>
        </span>
      );
    })}
  </>
);
