/**
 * User message component.
 * Parses and renders inline context tags inside user text.
 */

import React, { useMemo, useState, useRef, useEffect } from 'react';
import { File, Folder, Code, Image, Terminal, GitBranch, Link, FileText, GitPullRequest } from 'lucide-react';
import { Tag } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { shouldIgnoreCardToggleClick } from '@/shared/utils/textSelection';
import { SnapshotRollbackButton } from './SnapshotRollbackButton';
import './UserMessage.scss';

export interface UserMessageProps {
  message?: string; // New API
  content?: string; // Legacy API
  timestamp?: number;
  showTimestamp?: boolean;
  className?: string;
  // Turn snapshot support
  sessionId?: string;
  turnIndex?: number;
  turnId?: string;
  showSnapshotButton?: boolean;
  isCurrentTurn?: boolean;
}

// Content segment type: text or tag.
type ContentPart = 
  | { type: 'text'; content: string }
  | { type: 'tag'; tagType: string; label: string };

type InlineTagColor = 'blue' | 'green' | 'red' | 'yellow' | 'purple' | 'gray';

// Tag metadata
const TAG_CONFIG = {
  file: { icon: File, tagColor: 'blue' as InlineTagColor, label: 'File' },
  dir: { icon: Folder, tagColor: 'purple' as InlineTagColor, label: 'Directory' },
  code: { icon: Code, tagColor: 'green' as InlineTagColor, label: 'Code' },
  img: { icon: Image, tagColor: 'yellow' as InlineTagColor, label: 'Image' },
  cmd: { icon: Terminal, tagColor: 'gray' as InlineTagColor, label: 'Command' },
  chart: { icon: FileText, tagColor: 'gray' as InlineTagColor, label: 'Chart' },
  git: { icon: GitBranch, tagColor: 'red' as InlineTagColor, label: 'Git' },
  link: { icon: Link, tagColor: 'blue' as InlineTagColor, label: 'Link' },
  pr: { icon: GitPullRequest, tagColor: 'purple' as InlineTagColor, label: 'Pull Request' }
};

/**
 * Parse message content into inline segments.
 * Supported format: #type:value
 *
 * Tag formats:
 * - #file:filename - File reference
 * - #dir:dirname - Directory reference
 * - #code:file:10-20 - Code snippet
 * - #img:image - Image reference
 * - #cmd:command - Command reference
 * - #chart:chart - Chart reference
 * - #git:branch - Git reference
 * - #link:URL - Link reference
 */
function parseMessageContent(content: string): ContentPart[] {
  const parts: ContentPart[] = [];
  
  // Match #type:value until whitespace or line break.
  const tagPattern = /#(file|dir|code|img|cmd|chart|git|link|pr):([^\s\n]+)/g;
  
  let lastIndex = 0;
  let match;
  
  while ((match = tagPattern.exec(content)) !== null) {
    if (match.index > lastIndex) {
      const textBefore = content.slice(lastIndex, match.index);
      if (textBefore) {
        parts.push({ type: 'text', content: textBefore });
      }
    }
    
    const tagType = match[1];
    const label = match[2];
    
    parts.push({
      type: 'tag',
      tagType,
      label
    });
    
    lastIndex = match.index + match[0].length;
  }
  
  if (lastIndex < content.length) {
    const textAfter = content.slice(lastIndex);
    if (textAfter) {
      parts.push({ type: 'text', content: textAfter });
    }
  }
  
  if (parts.length === 0) {
    parts.push({ type: 'text', content });
  }
  
  return parts;
}

/**
 * Inline context tag component.
 */
const InlineContextTag: React.FC<{ tagType: string; label: string }> = ({ tagType, label }) => {
  const config = TAG_CONFIG[tagType as keyof typeof TAG_CONFIG] || TAG_CONFIG.file;
  const IconComponent = config.icon;
  
  return (
    <Tag 
      color={config.tagColor}
      size="small"
      className="inline-context-tag"
      title={`${config.label}: ${label}`}
    >
      <IconComponent size={12} style={{ marginRight: '4px', display: 'inline-flex', verticalAlign: 'middle' }} />
      <span>{label}</span>
    </Tag>
  );
};

export const UserMessage: React.FC<UserMessageProps> = React.memo(({
  message,
  content,
  timestamp,
  showTimestamp = false,
  className = '',
  sessionId,
  turnIndex,
  turnId,
  showSnapshotButton = false,
  isCurrentTurn = false
}) => {
  const { formatDate } = useI18n('flow-chat');
  const messageContent = message || content || '';
  const parts = useMemo(() => parseMessageContent(messageContent), [messageContent]);
  const [isExpanded, setIsExpanded] = useState(false);
  const [hasOverflow, setHasOverflow] = useState(false);
  const messageRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  
  useEffect(() => {
    const checkOverflow = () => {
      if (contentRef.current && !isExpanded) {
        const element = contentRef.current;
        const isOverflowing = element.scrollHeight > element.clientHeight || 
                              element.scrollWidth > element.clientWidth;
        setHasOverflow(isOverflowing);
      } else {
        setHasOverflow(false);
      }
    };
    
    checkOverflow();
    
    window.addEventListener('resize', checkOverflow);
    
    return () => {
      window.removeEventListener('resize', checkOverflow);
    };
  }, [messageContent, isExpanded]);
  
  const toggleExpand = (e: React.MouseEvent) => {
    if (shouldIgnoreCardToggleClick(e, contentRef.current)) {
      return;
    }

    if (!hasOverflow && !isExpanded) {
      return;
    }
    e.stopPropagation();
    setIsExpanded(prev => !prev);
  };
  
  useEffect(() => {
    if (!isExpanded) {
      return;
    }
    
    const handleClickOutside = (event: MouseEvent) => {
      if (messageRef.current && !messageRef.current.contains(event.target as Node)) {
        setIsExpanded(false);
      }
    };
    
    const timeoutId = setTimeout(() => {
      document.addEventListener('click', handleClickOutside, true);
    }, 100);
    
    return () => {
      clearTimeout(timeoutId);
      document.removeEventListener('click', handleClickOutside, true);
    };
  }, [isExpanded]);
  
  const currentClassName = `user-message ${className} ${isExpanded ? 'user-message--expanded' : 'user-message--collapsed'}`;
  
  return (
    <div 
      ref={messageRef}
      className={currentClassName}
    >
      <div 
        className="message-content" 
        onClick={toggleExpand}
        style={{ cursor: (hasOverflow || isExpanded) ? 'pointer' : 'text' }}
      >
        <div className="message-inline-content" ref={contentRef}>
          {parts.map((part, index) => {
            if (part.type === 'text') {
              return part.content.split('\n').map((line, lineIndex) => (
                <React.Fragment key={`text-${index}-${lineIndex}`}>
                  {lineIndex > 0 && <br />}
                  {line}
                </React.Fragment>
              ));
            } else {
              return (
                <InlineContextTag
                  key={`tag-${index}`}
                  tagType={part.tagType}
                  label={part.label}
                />
              );
            }
          })}
        </div>
      </div>
      
      <div className="message-footer">
        {showTimestamp && timestamp && (
          <div className="message-timestamp">
            {formatDate(new Date(timestamp), {
              hour: '2-digit',
              minute: '2-digit',
            })}
          </div>
        )}
        
        {showSnapshotButton && sessionId && turnId !== undefined && turnIndex !== undefined && (
          <div className="message-snapshot-action">
            <SnapshotRollbackButton
              sessionId={sessionId}
              turnIndex={turnIndex}
              turnId={turnId}
              isCurrentTurn={isCurrentTurn}
            />
          </div>
        )}
      </div>
    </div>
  );
});

UserMessage.displayName = 'UserMessage';
