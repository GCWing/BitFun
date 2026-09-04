import React from 'react';
import { Icon } from '@openbitfun/ui';
import {
  AtSign,
  Code2,
  File,
  MessageCircle,
} from 'lucide-react';
import type { ContextItem } from '@/shared/types/context';
import type { ComposerPresentation } from '../../utils/composerPresentation';

const catalogIcon = (name: 'extension' | 'folder' | 'git' | 'link' | 'terminal') => (
  <Icon name={name} size="lg" style={{ width: 13, height: 13 }} aria-hidden />
);

function contextIcon(type: ContextItem['type']): React.ReactNode {
  switch (type) {
    case 'session-reference':
      return <MessageCircle size={13} aria-hidden />;
    case 'file':
    case 'image':
      return <File size={13} aria-hidden />;
    case 'directory':
      return catalogIcon('folder');
    case 'code-snippet':
    case 'mermaid-node':
    case 'mermaid-diagram':
      return <Code2 size={13} aria-hidden />;
    case 'pull-request':
    case 'git-ref':
      return catalogIcon('git');
    case 'terminal-command':
      return catalogIcon('terminal');
    case 'url':
      return catalogIcon('link');
    case 'web-element':
      return <AtSign size={13} aria-hidden />;
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
        const icon = segment.tokenType === 'skill'
          ? catalogIcon('extension')
          : <AtSign size={13} aria-hidden />;
        return (
          <span
            key={`token-${index}`}
            className={`user-message-item__reference user-message-item__reference--${segment.tokenType}`}
            title={segment.label}
          >
            {icon}
            <span className="user-message-item__reference-label">{segment.label}</span>
          </span>
        );
      }

      return (
        <span
          key={`context-${index}`}
          className={`user-message-item__reference user-message-item__reference--${segment.context.type}`}
          title={segment.title}
        >
          {contextIcon(segment.context.type)}
          <span className="user-message-item__reference-label">{segment.label}</span>
        </span>
      );
    })}
  </>
);
