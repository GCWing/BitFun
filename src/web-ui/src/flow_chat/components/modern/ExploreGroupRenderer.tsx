/**
 * Explore group renderer.
 * Renders merged explore-only rounds as a collapsible region.
 */

import React, { useRef, useMemo, useCallback, useEffect } from 'react';
import { ChevronRight } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { FlowItem, FlowToolItem, FlowTextItem, FlowThinkingItem } from '../../types/flow-chat';
import type { ExploreGroupData } from '../../store/modernFlowChatStore';
import { FlowTextBlock } from '../FlowTextBlock';
import { FlowToolCard } from '../FlowToolCard';
import { ModelThinkingDisplay } from '../../tool-cards/ModelThinkingDisplay';
import { useFlowChatContext } from './FlowChatContext';
import './ExploreRegion.scss';

export interface ExploreGroupRendererProps {
  data: ExploreGroupData;
  turnId: string;
}

export const ExploreGroupRenderer: React.FC<ExploreGroupRendererProps> = ({
  data,
  turnId,
}) => {
  const { t } = useTranslation('flow-chat');
  const containerRef = useRef<HTMLDivElement>(null);
  
  const { 
    exploreGroupStates, 
    onExploreGroupToggle, 
    onCollapseGroup 
  } = useFlowChatContext();
  
  const { 
    groupId, 
    allItems, 
    stats, 
    isGroupStreaming,
    isFollowedByCritical 
  } = data;
  
  // Track auto-collapse once to prevent flicker.
  const hasAutoCollapsed = useRef(false);
  // Reset collapse state when the merged group changes.
  const prevGroupId = useRef(groupId);
  
  if (prevGroupId.current !== groupId) {
    prevGroupId.current = groupId;
    hasAutoCollapsed.current = false;
  }
  
  // Auto-collapse once critical content follows, without waiting for streaming to end.
  if (isFollowedByCritical && !hasAutoCollapsed.current) {
    hasAutoCollapsed.current = true;
  }
  
  const shouldAutoCollapse = hasAutoCollapsed.current;
  
  const userExpanded = exploreGroupStates?.get(groupId) ?? false;
  
  const isCollapsed = shouldAutoCollapse && !userExpanded;
  
  // Auto-scroll to bottom during streaming.
  useEffect(() => {
    if (!isCollapsed && isGroupStreaming && containerRef.current) {
      requestAnimationFrame(() => {
        if (containerRef.current) {
          containerRef.current.scrollTop = containerRef.current.scrollHeight;
        }
      });
    }
  }, [allItems, isCollapsed, isGroupStreaming]);
  
  // Build summary text with i18n.
  const displaySummary = useMemo(() => {
    const { readCount, searchCount } = stats;
    
    const parts: string[] = [];
    if (readCount > 0) {
      parts.push(t('exploreRegion.readFiles', { count: readCount }));
    }
    if (searchCount > 0) {
      parts.push(t('exploreRegion.searchCount', { count: searchCount }));
    }
    
    if (parts.length === 0) {
      return t('exploreRegion.exploreCount', { count: allItems.length });
    }
    
    return parts.join(t('exploreRegion.separator'));
  }, [stats, allItems.length, t]);
  
  const handleToggle = useCallback(() => {
    // Notify VirtualMessageList to avoid auto-scrolling on user action.
    window.dispatchEvent(new CustomEvent('tool-card-toggle'));
    
    if (isCollapsed) {
      // Expand only the clicked group.
      onExploreGroupToggle?.(groupId);
    } else {
      // Collapse only the current group.
      onCollapseGroup?.(groupId);
    }
  }, [isCollapsed, groupId, onExploreGroupToggle, onCollapseGroup]);

  // Build class list.
  const className = [
    'explore-region',
    shouldAutoCollapse ? 'explore-region--collapsible' : null,
    isCollapsed ? 'explore-region--collapsed' : 'explore-region--expanded',
    isGroupStreaming ? 'explore-region--streaming' : null,
  ].filter(Boolean).join(' ');

  // Non-collapsible: just render content without header (streaming, no auto-collapse yet).
  if (!shouldAutoCollapse) {
    return (
      <div className={className}>
        <div ref={containerRef} className="explore-region__content">
          {allItems.map(item => (
            <ExploreItemRenderer
              key={item.id}
              item={item}
              turnId={turnId}
            />
          ))}
        </div>
      </div>
    );
  }

  // Collapsible: unified header + animated content wrapper.
  return (
    <div className={className}>
      <div className="explore-region__header" onClick={handleToggle}>
        <ChevronRight size={14} className="explore-region__icon" />
        <span className="explore-region__summary">{displaySummary}</span>
      </div>
      <div className="explore-region__content-wrapper">
        <div className="explore-region__content-inner">
          <div ref={containerRef} className="explore-region__content">
            {allItems.map(item => (
              <ExploreItemRenderer
                key={item.id}
                item={item}
                turnId={turnId}
              />
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};

/**
 * Explore item renderer inside the explore region.
 * Uses React.memo to avoid unnecessary re-renders.
 */
interface ExploreItemRendererProps {
  item: FlowItem;
  turnId: string;
}

const ExploreItemRenderer = React.memo<ExploreItemRendererProps>(({ item }) => {
  const {
    onToolConfirm,
    onToolReject,
    onFileViewRequest,
    onTabOpen,
    sessionId,
  } = useFlowChatContext();
  
  const handleConfirm = useCallback(async (toolId: string, updatedInput?: any) => {
    if (onToolConfirm) {
      await onToolConfirm(toolId, updatedInput);
    }
  }, [onToolConfirm]);
  
  const handleReject = useCallback(async () => {
    if (onToolReject) {
      await onToolReject(item.id);
    }
  }, [onToolReject, item.id]);
  
  const handleOpenInEditor = useCallback((filePath: string) => {
    if (onFileViewRequest) {
      onFileViewRequest(filePath);
    }
  }, [onFileViewRequest]);
  
  const handleOpenInPanel = useCallback((_panelType: string, data: any) => {
    if (onTabOpen) {
      onTabOpen(data);
    }
  }, [onTabOpen]);
  
  switch (item.type) {
    case 'text':
      return (
        <FlowTextBlock
          textItem={item as FlowTextItem}
        />
      );
    
    case 'thinking':
      return (
        <ModelThinkingDisplay thinkingItem={item as FlowThinkingItem} />
      );
    
    case 'tool':
      return (
        <FlowToolCard
          toolItem={item as FlowToolItem}
          onConfirm={handleConfirm}
          onReject={handleReject}
          onOpenInEditor={handleOpenInEditor}
          onOpenInPanel={handleOpenInPanel}
          sessionId={sessionId}
        />
      );
    
    default:
      return null;
  }
});

ExploreGroupRenderer.displayName = 'ExploreGroupRenderer';
