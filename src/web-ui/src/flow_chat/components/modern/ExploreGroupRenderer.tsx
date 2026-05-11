/* eslint-disable @typescript-eslint/no-use-before-define */
/**
 * Explore group renderer.
 * Renders merged explore-only rounds as a collapsible region.
 */

import React, { useRef, useMemo, useCallback, useEffect, useState } from 'react';
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

export const ExploreGroupRenderer: React.FC<ExploreGroupRendererProps> = React.memo(({
  data,
  turnId,
}) => {
  const { t } = useTranslation('flow-chat');
  const containerRef = useRef<HTMLDivElement>(null);
  const [scrollState, setScrollState] = useState({ hasScroll: false, atTop: true, atBottom: true });
  
  const { 
    exploreGroupStates, 
    onExploreGroupToggle, 
    onExpandGroup,
    onCollapseGroup 
  } = useFlowChatContext();
  
  const { 
    groupId, 
    allItems, 
    stats, 
    isGroupStreaming,
    isLastGroupInTurn
  } = data;
  const wasStreamingRef = useRef(isGroupStreaming);
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
  
  const hasExplicitState = exploreGroupStates?.has(groupId) ?? false;
  const explicitExpanded = exploreGroupStates?.get(groupId) ?? false;
  const isExpanded = hasExplicitState ? explicitExpanded : isGroupStreaming;
  const isCollapsed = !isExpanded;
  const allowManualToggle = !isGroupStreaming;

  const checkScrollState = useCallback(() => {
    const el = containerRef.current;
    if (!el) {
      return;
    }

    setScrollState({
      hasScroll: el.scrollHeight > el.clientHeight + 1,
      atTop: el.scrollTop <= 5,
      atBottom: el.scrollTop + el.clientHeight >= el.scrollHeight - 5,
    });
  }, []);

  useEffect(() => {
    if (isGroupStreaming && !hasExplicitState) {
      applyExpandedState(false, true, () => {
        onExpandGroup?.(groupId);
      });
      wasStreamingRef.current = true;
      return;
    }

    if (wasStreamingRef.current && !isGroupStreaming && isExpanded) {
      applyExpandedState(true, false, () => {
        onCollapseGroup?.(groupId);
      }, {
        reason: 'auto',
      });
    }

    wasStreamingRef.current = isGroupStreaming;
  }, [
    applyExpandedState,
    groupId,
    hasExplicitState,
    isExpanded,
    isGroupStreaming,
    onCollapseGroup,
    onExpandGroup,
  ]);
  
  // Auto-scroll to bottom during streaming.
  // Use double requestAnimationFrame to ensure the browser has completed
  // layout of newly added content before we measure scrollHeight.
  useEffect(() => {
    if (!isCollapsed && isGroupStreaming && containerRef.current) {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          if (containerRef.current) {
            containerRef.current.scrollTop = containerRef.current.scrollHeight;
            checkScrollState();
          }
        });
      });
    }
  }, [allItems, checkScrollState, isCollapsed, isGroupStreaming]);

  useEffect(() => {
    if (!isExpanded) {
      setScrollState({ hasScroll: false, atTop: true, atBottom: true });
      return;
    }

    const el = containerRef.current;
    if (!el) {
      return;
    }

    const frameId = requestAnimationFrame(checkScrollState);

    if (typeof ResizeObserver === 'undefined') {
      return () => cancelAnimationFrame(frameId);
    }

    const observer = new ResizeObserver(() => {
      checkScrollState();
    });
    observer.observe(el);

    return () => {
      cancelAnimationFrame(frameId);
      observer.disconnect();
    };
  }, [allItems, checkScrollState, isExpanded]);
  
  // Build summary text with i18n.
  const displaySummary = useMemo(() => {
    const { readCount, searchCount, commandCount } = stats;
    
    const parts: string[] = [];
    if (readCount > 0) {
      parts.push(t('exploreRegion.readFiles', { count: readCount }));
    }
    if (searchCount > 0) {
      parts.push(t('exploreRegion.searchCount', { count: searchCount }));
    }
    if (commandCount > 0) {
      parts.push(t('exploreRegion.commandCount', { count: commandCount }));
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
    allowManualToggle ? 'explore-region--collapsible' : null,
    isCollapsed ? 'explore-region--collapsed' : 'explore-region--expanded',
    isGroupStreaming ? 'explore-region--streaming' : null,
    scrollState.hasScroll ? 'explore-region--has-scroll' : null,
    scrollState.atTop ? 'explore-region--at-top' : null,
    scrollState.atBottom ? 'explore-region--at-bottom' : null,
  ].filter(Boolean).join(' ');
  return (
    <div
      ref={cardRootRef}
      data-tool-card-id={groupId}
      className={className}
    >
      {allowManualToggle && (
        <div className="explore-region__header" onClick={handleToggle}>
          <ChevronRight size={14} className="explore-region__icon" />
          <span className="explore-region__summary">{displaySummary}</span>
        </div>
      )}
      <div className="explore-region__content-wrapper">
        <div className="explore-region__content-inner">
          <div ref={containerRef} className="explore-region__content" onScroll={checkScrollState}>
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
});

/**
 * Explore item renderer inside the explore region.
 * Uses React.memo to avoid unnecessary re-renders.
 */
interface ExploreItemRendererProps {
  item: FlowItem;
  turnId: string;
  isLastItem?: boolean;
}

const ExploreItemRenderer = React.memo<ExploreItemRendererProps>(({ item, turnId, isLastItem }) => {
  const {
    onToolConfirm,
    onToolReject,
    onFileViewRequest,
    onTabOpen,
    sessionId,
  } = useFlowChatContext();
  
  const handleConfirm = useCallback(async (toolId: string, updatedInput?: any, permissionOptionId?: string, approve?: boolean) => {
    if (onToolConfirm) {
      await onToolConfirm(toolId, updatedInput, permissionOptionId, approve);
    }
  }, [onToolConfirm]);
  
  const handleReject = useCallback(async (toolId: string, permissionOptionId?: string) => {
    if (onToolReject) {
      await onToolReject(toolId, permissionOptionId);
    }
  }, [onToolReject]);
  
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
        <div className="flowchat-flow-item" data-flow-item-id={item.id} data-flow-item-type="tool">
          <FlowToolCard
            toolItem={item as FlowToolItem}
            onConfirm={handleConfirm}
            onReject={handleReject}
            onOpenInEditor={handleOpenInEditor}
            onOpenInPanel={handleOpenInPanel}
            sessionId={sessionId}
            turnId={turnId}
          />
        </div>
      );

    default:
      return null;
  }
});

ExploreGroupRenderer.displayName = 'ExploreGroupRenderer';
