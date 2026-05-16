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
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ExploreGroupRenderer');
import { FlowTextBlock } from '../FlowTextBlock';
import { FlowToolCard } from '../FlowToolCard';
import { ModelThinkingDisplay } from '../../tool-cards/ModelThinkingDisplay';
import { useToolCardHeightContract } from '../../tool-cards/useToolCardHeightContract';
import { useFlowChatContext } from './FlowChatContext';
import { SmoothHeightCollapse } from './SmoothHeightCollapse';
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
    onCollapseGroup 
  } = useFlowChatContext();
  
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
    getCardHeight: () => (
      containerRef.current?.scrollHeight
      ?? containerRef.current?.getBoundingClientRect().height
      ?? null
    ),
  });
  
  const hasExplicitState = exploreGroupStates?.has(groupId) ?? false;
  const explicitExpanded = exploreGroupStates?.get(groupId) ?? false;
  // Default: expanded while the group is still the tail; collapsed once cut.
  const defaultExpanded = !wasCutByCritical;
  const isExpanded = hasExplicitState ? explicitExpanded : defaultExpanded;
  const isCollapsed = !isExpanded;
  // Header is always interactive so the user can collapse/expand at any time.
  const allowManualToggle = true;

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

  // One-shot auto-collapse: fires exactly once when the group transitions from
  // tail (wasCutByCritical=false) to cut (wasCutByCritical=true).
  //
  // IMPORTANT: do NOT use `isExpanded` to guard this effect. When wasCutByCritical
  // flips to true, the same render also recomputes isExpanded = false (because
  // defaultExpanded = !wasCutByCritical). So `justGotCut && isExpanded` would
  // always be false and the collapse-intent would never fire.
  //
  // Instead, reason about the state *before* the cut:
  //   - No explicit state → group was expanded by default (it was tail).
  //   - Explicit state = true → user had it open.
  // Both cases mean the group WAS visually expanded before this render; we need
  // to dispatch the height-contract event so Virtuoso can anchor-lock.
  useEffect(() => {
    const justGotCut = wasCutByCritical && !prevWasCutRef.current;
    prevWasCutRef.current = wasCutByCritical;

    if (!justGotCut) return;

    const wasExpanded = !hasExplicitState || explicitExpanded;
    log.debug('explore group cut by critical', { groupId, wasExpanded, hasExplicitState });

    if (wasExpanded) {
      applyExpandedState(true, false, () => {
        onCollapseGroup?.(groupId);
      }, {
        reason: 'auto',
      });
    }
  }, [
    applyExpandedState,
    explicitExpanded,
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
      <SmoothHeightCollapse
        isOpen={isExpanded}
        className="explore-region__content-wrapper"
        innerClassName="explore-region__content-inner"
        durationMs={320}
      >
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
