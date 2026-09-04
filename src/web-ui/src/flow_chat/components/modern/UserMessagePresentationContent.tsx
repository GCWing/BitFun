import React from 'react';
import type { ComposerPresentation } from '../../utils/composerPresentation';
import { MessageReferenceCapsule, messageInlineTokenIcon } from './MessageReferenceCapsule';

export const UserMessagePresentationContent: React.FC<{
  presentation: ComposerPresentation;
}> = ({ presentation }) => (
  <>
    {presentation.segments.map((segment, index) => {
      if (segment.kind === 'text') {
        return <React.Fragment key={`text-${index}`}>{segment.text}</React.Fragment>;
      }

      if (segment.kind === 'inline-token') {
        return (
          <MessageReferenceCapsule
            key={`token-${index}`}
            type={segment.tokenType}
            title={segment.label}
            label={segment.label}
          >{messageInlineTokenIcon(segment.tokenType)}</MessageReferenceCapsule>
        );
      }

      return (
        <MessageReferenceCapsule
          key={`context-${index}`}
          type={segment.context.type}
          title={segment.title}
          label={segment.label}
        />
      );
    })}
  </>
);
