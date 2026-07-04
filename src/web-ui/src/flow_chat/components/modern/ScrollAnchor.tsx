/**
 * Scroll anchor component.
 * Shows user message markers with hover preview and jump navigation.
 */

import React, { useState, useMemo, useCallback, useRef, useEffect } from 'react';
import { useVirtualItems } from '../../store/modernFlowChatStore';
import { i18nService } from '@/infrastructure/i18n';
import './ScrollAnchor.scss';

interface ScrollAnchorProps {
  onAnchorNavigate: (turnId: string) => void;
  scrollerRef?: React.RefObject<HTMLElement | null>;
}

interface AnchorPoint {
  id: string;
  turnId: string;
  index: number;
  position: number;
  content: string;
  timestamp: number;
  turnNumber: number;
}


export const ScrollAnchor: React.FC<ScrollAnchorProps> = ({
  onAnchorNavigate,
  scrollerRef,
}) => {
  const virtualItems = useVirtualItems();
  const [hoveredAnchor, setHoveredAnchor] = useState<AnchorPoint | null>(null);
  const [previewPosition, setPreviewPosition] = useState({ x: 0, y: 0 });
  const [isScrolling, setIsScrolling] = useState(false);
  
  const scrollTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const [isHovering, setIsHovering] = useState(false);

  useEffect(() => {
    const scroller = scrollerRef?.current;
    if (!scroller) return;

    const handleScroll = () => {
      setIsScrolling(true);

      if (scrollTimeoutRef.current) {
        clearTimeout(scrollTimeoutRef.current);
      }

      scrollTimeoutRef.current = setTimeout(() => {
        setIsScrolling(false);
      }, 800);
    };

    scroller.addEventListener('scroll', handleScroll, { passive: true });

    return () => {
      scroller.removeEventListener('scroll', handleScroll);
      if (scrollTimeoutRef.current) {
        clearTimeout(scrollTimeoutRef.current);
      }
    };
  }, [scrollerRef]);

  useEffect(() => {
    const scroller = scrollerRef?.current;
    if (!scroller) return;

    if (isHovering) {
      scroller.classList.add('anchor-hovering');
    } else {
      scroller.classList.remove('anchor-hovering');
    }

    return () => {
      scroller.classList.remove('anchor-hovering');
    };
  }, [scrollerRef, isHovering]);

  const anchorPoints = useMemo<AnchorPoint[]>(() => {
    if (virtualItems.length === 0) return [];

    const userMessageItems = virtualItems
      .map((item, index) => ({ item, index }))
      .filter(({ item }) => item.type === 'user-message');

    if (userMessageItems.length === 0) return [];

    return userMessageItems.map(({ item, index }, turnIndex) => {
      const userMessage = (item as any).data;
      const position = 2 + (index / virtualItems.length) * 96;

      return {
        id: userMessage.id,
        turnId: item.turnId,
        index,
        position,
        content: userMessage.content || '',
        timestamp: userMessage.timestamp || Date.now(),
        turnNumber: turnIndex + 1,
      };
    });
  }, [virtualItems]);

  const handleAnchorClick = useCallback((anchor: AnchorPoint) => {
    onAnchorNavigate(anchor.turnId);

    setHoveredAnchor(null);
  }, [onAnchorNavigate]);

  const handleAnchorMouseEnter = useCallback((anchor: AnchorPoint, event: React.MouseEvent) => {
    setHoveredAnchor(anchor);
    setPreviewPosition({ x: event.clientX, y: event.clientY });
  }, []);

  const handleAnchorMouseLeave = useCallback(() => {
    setHoveredAnchor(null);
  }, []);

  const formatTimestamp = (timestamp: number) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const minutes = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);

    if (minutes < 1) return 'Just now';
    if (minutes < 60) return `${minutes} min ago`;
    if (hours < 24) return `${hours} hr ago`;
    if (days < 7) return `${days} days ago`;
    
    return i18nService.formatDate(date, { 
      month: 'short', 
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    });
  };

  const truncateContent = (content: string, maxLength: number = 100) => {
    if (content.length <= maxLength) return content;
    return content.substring(0, maxLength) + '...';
  };

  const handleContainerMouseEnter = useCallback(() => {
    setIsHovering(true);
  }, []);

  const handleContainerMouseLeave = useCallback(() => {
    setIsHovering(false);
  }, []);

  if (anchorPoints.length === 0) return null;

  return (
    <>
      <div 
        className={`scroll-anchor ${isScrolling ? 'scrolling' : ''} ${isHovering ? 'hovering' : ''}`}
        onMouseEnter={handleContainerMouseEnter}
        onMouseLeave={handleContainerMouseLeave}
      >
        <div className="scroll-anchor__track">
          {anchorPoints.map((anchor, idx) => (
            <div
              key={anchor.id}
              className={`scroll-anchor__point ${hoveredAnchor?.id === anchor.id ? 'active' : ''}`}
              style={{ 
                top: `${anchor.position}%`,
                '--flowchat-scroll-anchor-delay': `${idx * 0.03}s`
              } as React.CSSProperties}
              onClick={(e) => {
                e.stopPropagation();
                handleAnchorClick(anchor);
              }}
              onMouseEnter={(e) => handleAnchorMouseEnter(anchor, e)}
              onMouseLeave={handleAnchorMouseLeave}
            />
          ))}
        </div>
      </div>

      {hoveredAnchor && (
        <div
          className="scroll-anchor__preview"
          style={{
            left: `${previewPosition.x - 20}px`,
            top: `${previewPosition.y}px`,
          }}
        >
          <div className="scroll-anchor__preview-indicator">
            <span className="scroll-anchor__preview-turn">
              {hoveredAnchor.turnNumber}/{anchorPoints.length}
            </span>
          </div>
          <div className="scroll-anchor__preview-header">
            <span className="scroll-anchor__preview-label">User message</span>
            <span className="scroll-anchor__preview-time">
              {formatTimestamp(hoveredAnchor.timestamp)}
            </span>
          </div>
          <div className="scroll-anchor__preview-content">
            {truncateContent(hoveredAnchor.content, 150)}
          </div>
        </div>
      )}
    </>
  );
};

ScrollAnchor.displayName = 'ScrollAnchor';
