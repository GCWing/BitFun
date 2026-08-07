/* eslint-disable @typescript-eslint/no-use-before-define */
/**
 * Explore group renderer.
 * Renders merged explore-only rounds as a collapsible region.
 */

import React, { useRef, useMemo, useCallback, useEffect, useLayoutEffect, useState } from 'react';
import { ChevronRight } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { FlowItem, FlowToolItem, FlowTextItem, FlowThinkingItem, ToolRejectOptions } from '../../types/flow-chat';
import type { ExploreGroupData } from '../../store/modernFlowChatStore';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ExploreGroupRenderer');
import { FlowTextBlock } from '../FlowTextBlock';
import { FlowToolCard } from '../FlowToolCard';
import { ModelThinkingDisplay } from '../../tool-cards/ModelThinkingDisplay';
import { useToolCardHeightContract } from '../../tool-cards/useToolCardHeightContract';
import { useFlowChatContext, useFlowChatVolatileContext } from './FlowChatContext';
import { SmoothHeightCollapse } from './SmoothHeightCollapse';
import { FLOWCHAT_COLLAPSE_DURATION_MS } from './flowChatCollapseMotion';
import './ExploreRegion.scss';

export interface ExploreGroupRendererProps {
  data: ExploreGroupData;
  turnId: string;
}

function getExploreGroupKind(
  stats: ExploreGroupData['stats'],
  itemCount: number
): 'read' | 'search' | 'command' | 'mixed' | 'other' {
  const activeKinds = [
    stats.readCount > 0 ? 'read' : null,
    stats.searchCount > 0 ? 'search' : null,
    stats.commandCount > 0 ? 'command' : null,
  ].filter(Boolean) as Array<'read' | 'search' | 'command'>;

  if (activeKinds.length === 1) {
    return activeKinds[0];
  }

  if (activeKinds.length > 1) {
    return 'mixed';
  }

  return itemCount > 0 ? 'other' : 'mixed';
}

export const ExploreGroupRenderer: React.FC<ExploreGroupRendererProps> = React.memo(({
  data,
  turnId,
}) => {
  const { t } = useTranslation('flow-chat');
  const containerRef = useRef<HTMLDivElement>(null);
  const [scrollState, setScrollState] = useState({ hasScroll: false, atTop: true, atBottom: true });
  
  const {
    onExploreGroupToggle,
    onCollapseGroup,
  } = useFlowChatContext();
  const { exploreGroupStates } = useFlowChatVolatileContext();
  
  const { 
    groupId, 
    allItems, 
    stats, 
    isGroupStreaming,
    isLastGroupInTurn,
    wasCutByCritical,
  } = data;
  const prevWasCutRef = useRef(wasCutByCritical);
  const {
    cardRootRef,
    applyExpandedState,
  } = useToolCardHeightContract({
    toolId: groupId,
    toolName: 'explore-group',
  });
  
  const hasExplicitState = exploreGroupStates?.has(groupId) ?? false;
  const explicitExpanded = exploreGroupStates?.get(groupId) ?? false;
  // Default: expanded while the group is still the tail; collapsed once cut.
  const defaultExpanded = !wasCutByCritical;
  const isExpanded = hasExplicitState ? explicitExpanded : defaultExpanded;
  const isCollapsed = !isExpanded;
  const groupKind = getExploreGroupKind(stats, allItems.length);
  // Header is always interactive so the user can collapse/expand at any time.
  const allowManualToggle = true;

  const checkScrollState = useCallback(() => {
    const el = containerRef.current;
    if (!el) {
      return;
    }

    const next = {
      hasScroll: el.scrollHeight > el.clientHeight + 1,
      atTop: el.scrollTop <= 5,
      atBottom: el.scrollTop + el.clientHeight >= el.scrollHeight - 5,
    };
    // Shallow-compare so unchanged scroll edges do not trigger a re-render.
    setScrollState(prev => (
      prev.hasScroll === next.hasScroll &&
      prev.atTop === next.atTop &&
      prev.atBottom === next.atBottom
        ? prev
        : next
    ));
  }, []);

  // rAF-merge the raw scroll events; only one state check per frame.
  const scrollFrameRef = useRef<number | null>(null);
  const handleContentScroll = useCallback(() => {
    if (scrollFrameRef.current !== null) {
      return;
    }
    scrollFrameRef.current = requestAnimationFrame(() => {
      scrollFrameRef.current = null;
      checkScrollState();
    });
  }, [checkScrollState]);
  useEffect(() => () => {
    if (scrollFrameRef.current !== null) {
      cancelAnimationFrame(scrollFrameRef.current);
    }
  }, []);

  // One-shot auto-collapse: fires exactly once when the group transitions from
  // tail (wasCutByCritical=false) to cut (wasCutByCritical=true).
  //
  // Do not use `isExpanded` to guard this effect. The render that flips
  // `wasCutByCritical` also recomputes the default expanded state. No explicit
  // state means the live-tail default may collapse naturally; an explicit
  // state is user intent and must not be overwritten.
  useLayoutEffect(() => {
    const justGotCut = wasCutByCritical && !prevWasCutRef.current;
    prevWasCutRef.current = wasCutByCritical;

    if (!justGotCut || hasExplicitState) return;

    log.debug('explore group cut by critical', { groupId });

    applyExpandedState(true, false, () => {
      onCollapseGroup?.(groupId);
    });
  }, [
    applyExpandedState,
    groupId,
    hasExplicitState,
    wasCutByCritical,
    onCollapseGroup,
  ]);
  
  // Auto-scroll to bottom while the group is still the tail and new items arrive.
  // Use double requestAnimationFrame to ensure the browser has completed
  // layout of newly added content before we measure scrollHeight.
  useEffect(() => {
    if (!isCollapsed && isLastGroupInTurn && !wasCutByCritical && containerRef.current) {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          if (containerRef.current) {
            containerRef.current.scrollTop = containerRef.current.scrollHeight;
            checkScrollState();
          }
        });
      });
    }
  }, [allItems, checkScrollState, isCollapsed, isLastGroupInTurn, wasCutByCritical]);

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
    'explore-region--collapsible',
    isCollapsed ? 'explore-region--collapsed' : 'explore-region--expanded',
    isGroupStreaming ? 'explore-region--streaming' : null,
    // --bounded: group is still growing (tail, not yet cut). Controls fixed
    // max-height and gradient masks regardless of streaming state.
    !wasCutByCritical ? 'explore-region--bounded' : null,
    scrollState.hasScroll ? 'explore-region--has-scroll' : null,
    scrollState.atTop ? 'explore-region--at-top' : null,
    scrollState.atBottom ? 'explore-region--at-bottom' : null,
  ].filter(Boolean).join(' ');
  return (
    <div data-bf-component="explore-group" data-bf-part="root" data-bf-state={isExpanded ? 'expanded' : undefined}
      ref={cardRootRef}
      data-testid="chat-explore-group"
      data-tool-card-id={groupId}
      data-group-kind={groupKind}
      data-expanded={isExpanded ? 'true' : 'false'}
      data-read-count={String(stats.readCount)}
      data-search-count={String(stats.searchCount)}
      data-command-count={String(stats.commandCount)}
      className={className}
    >
      {allowManualToggle && (
        <div
          data-bf-component="explore-group"
          data-bf-part="header"
          className="explore-region__header"
          onClick={handleToggle}
          data-testid="chat-explore-group-toggle"
          data-group-kind={groupKind}
          data-expanded={isExpanded ? 'true' : 'false'}
        >
          <ChevronRight size={14} className="explore-region__icon" />
          <span data-bf-component="explore-group" data-bf-part="summary" className="explore-region__summary">{displaySummary}</span>
        </div>
      )}
      <SmoothHeightCollapse
        isOpen={isExpanded}
        data-bf-component="explore-group"
        data-bf-part="contentWrapper"
        className="explore-region__content-wrapper"
        innerClassName="explore-region__content-inner"
        durationMs={FLOWCHAT_COLLAPSE_DURATION_MS}
      >
        <div
          ref={containerRef}
          data-bf-component="explore-group"
          data-bf-part="content"
          className="explore-region__content"
          onScroll={handleContentScroll}
          data-testid="chat-explore-group-content"
          data-group-kind={groupKind}
          data-expanded={isExpanded ? 'true' : 'false'}
        >
          {allItems.map((item, idx) => (
            <ExploreItemRenderer
              key={item.id}
              item={item}
              turnId={turnId}
              isLastItem={isLastGroupInTurn && idx === allItems.length - 1}
            />
          ))}
        </div>
      </SmoothHeightCollapse>
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
  
  const handleConfirm = useCallback(async (toolId: string, permissionOptionId?: string, approve?: boolean) => {
    if (onToolConfirm) {
      await onToolConfirm(toolId, permissionOptionId, approve);
    }
  }, [onToolConfirm]);
  
  const handleReject = useCallback(async (toolId: string, options?: ToolRejectOptions) => {
    if (onToolReject) {
      await onToolReject(toolId, options);
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
        <div data-bf-component="explore-group" data-bf-part="item" className="flowchat-flow-item" data-flow-item-id={item.id} data-flow-item-type="tool">
          <FlowToolCard
            toolItem={item as FlowToolItem}
            isLastItem={isLastItem}
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
