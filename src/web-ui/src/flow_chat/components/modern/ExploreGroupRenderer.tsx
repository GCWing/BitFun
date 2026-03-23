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
import { useToolCardHeightContract } from '../../tool-cards/useToolCardHeightContract';
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
    isLastGroupInTurn
  } = data;
  const {
    cardRootRef,
    applyExpandedState,
  } = useToolCardHeightContract({
    toolId: groupId,
    toolName: 'explore-group',
    getCardHeight: () => (
      containerRef.current?.scrollHeight
      ?? containerRef.current?.getBoundingClientRect().height
      ?? null
    ),
  });
  
  const isExpanded = exploreGroupStates?.get(groupId) ?? false;
  const isCollapsed = !isExpanded;
  
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
    const { readCount, searchCount, thinkingCount } = stats;
    
    const parts: string[] = [];
    if (thinkingCount > 0) {
      parts.push(t('exploreRegion.thinkingCount', { count: thinkingCount }));
    }
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
    if (isCollapsed) {
      applyExpandedState(false, true, () => {
        onExploreGroupToggle?.(groupId);
      });
      return;
    }

    applyExpandedState(true, false, () => {
      onCollapseGroup?.(groupId);
    });
  }, [applyExpandedState, groupId, isCollapsed, onCollapseGroup, onExploreGroupToggle]);

  // Build class list.
  const className = [
    'explore-region',
    'explore-region--collapsible',
    isCollapsed ? 'explore-region--collapsed' : 'explore-region--expanded',
    isGroupStreaming ? 'explore-region--streaming' : null,
  ].filter(Boolean).join(' ');
  return (
    <div
      ref={cardRootRef}
      data-tool-card-id={groupId}
      className={className}
    >
      <div className="explore-region__header" onClick={handleToggle}>
        <ChevronRight size={14} className="explore-region__icon" />
        <span className="explore-region__summary">{displaySummary}</span>
      </div>
      <div className="explore-region__content-wrapper">
        <div className="explore-region__content-inner">
          <div ref={containerRef} className="explore-region__content">
            {allItems.map((item, idx) => (
              <ExploreItemRenderer
                key={item.id}
                item={item}
                turnId={turnId}
                isLastItem={isLastGroupInTurn && idx === allItems.length - 1}
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
  isLastItem?: boolean;
}

const ExploreItemRenderer = React.memo<ExploreItemRendererProps>(({ item, isLastItem }) => {
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
      onFileViewRequest(filePath, filePath.split(/[/\\]/).pop() || filePath);
    }
  }, [onFileViewRequest]);
  
  const handleOpenInPanel = useCallback((_panelType: string, data: any) => {
    if (onTabOpen) {
      onTabOpen(data, sessionId);
    }
  }, [onTabOpen, sessionId]);
  
  switch (item.type) {
    case 'text':
      return (
        <FlowTextBlock
          textItem={item as FlowTextItem}
        />
      );
    
    case 'thinking': {
      const thinkingItem = item as FlowThinkingItem;
      return (
        <ModelThinkingDisplay thinkingItem={thinkingItem} isLastItem={isLastItem} />
      );
    }
    
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
