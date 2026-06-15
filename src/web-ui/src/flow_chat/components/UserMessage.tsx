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

// Tag metadata
const TAG_CONFIG = {
  file: { icon: File, color: '#60a5fa', label: 'File' },
  dir: { icon: Folder, color: '#a78bfa', label: 'Directory' },
  code: { icon: Code, color: '#4ade80', label: 'Code' },
  img: { icon: Image, color: '#fb923c', label: 'Image' },
  cmd: { icon: Terminal, color: '#94a3b8', label: 'Command' },
  chart: { icon: FileText, color: '#22d3ee', label: 'Chart' },
  git: { icon: GitBranch, color: '#f87171', label: 'Git' },
  link: { icon: Link, color: '#60a5fa', label: 'Link' },
  pr: { icon: GitPullRequest, color: '#a78bfa', label: 'Pull Request' }
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
  
  // Map hex color to Tag color tokens.
  const getTagColor = (color: string): 'blue' | 'green' | 'red' | 'yellow' | 'purple' | 'gray' => {
    if (color.includes('60a5fa') || color.includes('3b82f6')) return 'blue';
    if (color.includes('4ade80') || color.includes('34c197')) return 'green';
    if (color.includes('f87171') || color.includes('ef4444')) return 'red';
    if (color.includes('fb923c') || color.includes('f59e0b')) return 'yellow';
    if (color.includes('a78bfa') || color.includes('8b5cf6')) return 'purple';
    return 'gray';
  };

  const tagColor = getTagColor(config.color);
  
  return (
    <Tag 
      color={tagColor}
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
