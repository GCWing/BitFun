/**
 * Model round item component.
 * Renders mixed FlowItems (text + tools).
 *
 * Note: explore-only rounds are handled by ExploreGroupRenderer,
 * and this component only renders rounds with critical output.
 */

import React, { useMemo, useState, useCallback, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Copy, Check, ThumbsUp, ThumbsDown, AlertTriangle } from 'lucide-react';
import type { ModelRound, FlowItem, FlowTextItem, FlowToolItem, FlowThinkingItem } from '../../types/flow-chat';
import { FlowTextBlock } from '../FlowTextBlock';
import { FlowToolCard } from '../FlowToolCard';
import { ModelThinkingDisplay } from '../../tool-cards/ModelThinkingDisplay';
import { isCollapsibleTool } from '../../tool-cards';
import { useFlowChatContext } from './FlowChatContext';
import { FlowChatStore } from '../../store/FlowChatStore';
import { taskCollapseStateManager } from '../../store/TaskCollapseStateManager';
import { notificationService } from '../../../shared/notification-system/services/NotificationService';
import { ExportImageButton } from './ExportImageButton';
import { Tooltip } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import './ModelRoundItem.scss';
import './SubagentItems.scss';

const log = createLogger('ModelRoundItem');

interface ModelRoundItemProps {
  round: ModelRound;
  turnId: string;
  isLastRound?: boolean;
}

export const ModelRoundItem = React.memo<ModelRoundItemProps>(
  ({ round, turnId, isLastRound = false }) => {
    const { t } = useTranslation('flow-chat');
    const [copied, setCopied] = useState(false);
    const copyButtonRef = useRef<HTMLButtonElement>(null);
    
    useEffect(() => {
      if (!copied) return;
      
      const handleClickOutside = (event: MouseEvent) => {
        if (copyButtonRef.current && !copyButtonRef.current.contains(event.target as Node)) {
          setCopied(false);
        }
      };
      
      document.addEventListener('mousedown', handleClickOutside);
      return () => {
        document.removeEventListener('mousedown', handleClickOutside);
      };
    }, [copied]);
    
    const handleFeedback = useCallback((_type: 'like' | 'dislike' | 'report') => {
      notificationService.info(t('modelRound.feedbackThanks'), {
        title: t('modelRound.feedbackDevVersion')
      });
    }, [t]);
    
    // Keep insertion order; do not sort by timestamp.
    // Subagent ordering is controlled by insertModelRoundItemAfterTool.
    // FlowChatStore uses immutable updates, so rely on round.items reference.
    const sortedItems = useMemo(
      () => round.items,
      [round.items]
    );
    
    type ItemGroup = 
      | { type: 'explore'; items: FlowItem[]; isLast: boolean }
      | { type: 'critical'; item: FlowItem }
      | { type: 'subagent'; parentTaskToolId: string; items: FlowItem[] };
    
    // Group items in two passes:
    // 1) group subagent items
    // 2) group normal items into explore/critical via anchor tool
    const groupedItems = useMemo(() => {
      const intermediateGroups: Array<{ type: 'normal', item: FlowItem } | { type: 'subagent', parentTaskToolId: string, items: FlowItem[] }> = [];
      let currentSubagentGroup: { parentTaskToolId: string, items: FlowItem[] } | null = null;
      
      for (const item of sortedItems) {
        const isSubagentItemFlag = (item as any).isSubagentItem === true;
        const parentTaskToolId = (item as any).parentTaskToolId;
        
        if (isSubagentItemFlag && parentTaskToolId) {
          if (currentSubagentGroup && currentSubagentGroup.parentTaskToolId === parentTaskToolId) {
            currentSubagentGroup.items.push(item);
          } else {
            if (currentSubagentGroup) {
              intermediateGroups.push({ type: 'subagent', ...currentSubagentGroup });
            }
            currentSubagentGroup = { parentTaskToolId, items: [item] };
          }
        } else {
          if (currentSubagentGroup) {
            intermediateGroups.push({ type: 'subagent', ...currentSubagentGroup });
            currentSubagentGroup = null;
          }
          intermediateGroups.push({ type: 'normal', item });
        }
      }
      
      if (currentSubagentGroup) {
        intermediateGroups.push({ type: 'subagent', ...currentSubagentGroup });
      }
      
      // Core idea: text/thinking collapse is decided by the next "anchor tool".
      // - Anchor tool = look forward, skip text/thinking, take the first tool.
      // - If anchor tool is collapsible -> collapse text/thinking with the tool.
      // - If anchor tool is critical -> show text/thinking with the tool.
      
      const finalGroups: ItemGroup[] = [];
      let exploreBuffer: FlowItem[] = [];
      let pendingBuffer: FlowItem[] = [];
      
      const normalItems: FlowItem[] = [];
      for (let i = 0; i < intermediateGroups.length; i++) {
        if (intermediateGroups[i].type === 'normal') {
          normalItems.push((intermediateGroups[i] as any).item);
        }
      }
      
      const flushExploreBuffer = (isLast: boolean) => {
        if (exploreBuffer.length > 0) {
          finalGroups.push({ type: 'explore', items: [...exploreBuffer], isLast });
          exploreBuffer = [];
        }
      };
      
      const flushPendingAsCritical = () => {
        for (const item of pendingBuffer) {
          finalGroups.push({ type: 'critical', item });
        }
        pendingBuffer = [];
      };
      
      let normalItemIndex = 0;
      
      for (let i = 0; i < intermediateGroups.length; i++) {
        const group = intermediateGroups[i];
        const isLastGroup = i === intermediateGroups.length - 1;
        
        if (group.type === 'subagent') {
          flushExploreBuffer(false);
          flushPendingAsCritical();
          finalGroups.push(group);
        } else {
          const item = group.item;
          const isLastNormalItem = normalItemIndex === normalItems.length - 1;
          
          if (item.type === 'text' || item.type === 'thinking') {
            pendingBuffer.push(item);
            
            if (isLastNormalItem) {
              flushExploreBuffer(false);
              flushPendingAsCritical();
            }
          } else if (item.type === 'tool') {
            const toolName = (item as FlowToolItem).toolName;
            const isExploreTool = isCollapsibleTool(toolName);
            
            if (isExploreTool) {
              exploreBuffer.push(...pendingBuffer, item);
              pendingBuffer = [];
              
              if (isLastNormalItem || isLastGroup) {
                flushExploreBuffer(true);
              }
            } else {
              flushExploreBuffer(false);
              flushPendingAsCritical();
              finalGroups.push({ type: 'critical', item });
            }
          }
          
          normalItemIndex++;
        }
      }
      
      flushExploreBuffer(true);
      flushPendingAsCritical();
      
      return finalGroups;
    }, [sortedItems]);
    
    const extractDialogTurnContent = useCallback(() => {
      const flowChatStore = FlowChatStore.getInstance();
      const state = flowChatStore.getState();
      
      let targetSession = null;
      for (const [, session] of state.sessions) {
        if (session.dialogTurns.some((turn: any) => turn.id === turnId)) {
          targetSession = session;
          break;
        }
      }
      
      if (!targetSession) return '';
      
      const dialogTurn = targetSession.dialogTurns.find((turn: any) => turn.id === turnId);
      if (!dialogTurn) return '';
      
      const contentParts: string[] = [];
      
      if (dialogTurn.userMessage?.content) {
        contentParts.push(`${t('modelRound.userLabel')}\n${dialogTurn.userMessage.content}`);
      }
      
      dialogTurn.modelRounds.forEach((modelRound: any) => {
        const roundContent: string[] = [];
        
        modelRound.items.forEach((item: any) => {
          if (item.type === 'text' && item.content?.trim()) {
            roundContent.push(item.content.trim());
          } else if (item.type === 'tool' && item.toolCall) {
            const toolName = item.toolName || t('copyOutput.unknownTool');
            roundContent.push(t('modelRound.toolCallLabel', { name: toolName }));
          }
        });
        
        if (roundContent.length > 0) {
          contentParts.push(roundContent.join('\n\n'));
        }
      });
      
      return contentParts.join('\n\n---\n\n');
    }, [turnId]);
    
    const handleCopy = useCallback(async () => {
      try {
        const content = extractDialogTurnContent();
        
        if (!content.trim()) {
          log.warn('No content to copy');
          return;
        }
        
        await navigator.clipboard.writeText(content);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      } catch (error) {
        log.error('Failed to copy', error);
      }
    }, [extractDialogTurnContent]);
    
    const hasContent = sortedItems.some(item => 
      (item.type === 'text' && (item as FlowTextItem).content.trim()) ||
      (item.type === 'tool' && (item as FlowToolItem).toolCall)
    );
    
    return (
      <div 
        className={`model-round-item model-round-item--${round.isStreaming ? 'streaming' : 'complete'}`}
      >
        {groupedItems.map((group, groupIndex) => {
          const isLastGroup = groupIndex === groupedItems.length - 1;
          const isLast = isLastRound && isLastGroup;
          switch (group.type) {
            case 'explore':
              return group.items.map((item, itemIdx) => (
                <FlowItemRenderer 
                  key={item.id}
                  item={item}
                  turnId={turnId}
                  roundId={round.id}
                  isLastItem={isLast && itemIdx === group.items.length - 1}
                />
              ));
            
            case 'critical':
              return (
                <FlowItemRenderer 
                  key={group.item.id}
                  item={group.item}
                  turnId={turnId}
                  roundId={round.id}
                  isLastItem={isLast}
                />
              );
            
            case 'subagent':
              return (
                <SubagentItemsContainer 
                  key={`subagent-group-${group.parentTaskToolId}-${groupIndex}`}
                  parentTaskToolId={group.parentTaskToolId}
                  items={group.items}
                  turnId={turnId}
                  roundId={round.id}
                />
              );
            
            default:
              return null;
          }
        })}
        
        {isLastRound && hasContent && !round.isStreaming && (
          <div className="model-round-item__footer">
            <Tooltip content={copied ? t('modelRound.copiedDialog') : t('modelRound.copyDialog')} placement="top">
              <button
                ref={copyButtonRef}
                className={`model-round-item__action-btn model-round-item__copy-btn ${copied ? 'copied' : ''}`}
                onClick={handleCopy}
              >
                {copied ? <Check size={14} /> : <Copy size={14} />}
              </button>
            </Tooltip>
            
            <ExportImageButton turnId={turnId} />
            
            <div className="model-round-item__feedback-group">
              <Tooltip content={t('modelRound.like')}>
                <button
                  className="model-round-item__action-btn model-round-item__feedback-btn"
                  onClick={() => handleFeedback('like')}
                >
                  <ThumbsUp size={14} />
                </button>
              </Tooltip>
              
              <Tooltip content={t('modelRound.dislike')}>
                <button
                  className="model-round-item__action-btn model-round-item__feedback-btn"
                  onClick={() => handleFeedback('dislike')}
                >
                  <ThumbsDown size={14} />
                </button>
              </Tooltip>
              
              <Tooltip content={t('modelRound.report')}>
                <button
                  className="model-round-item__action-btn model-round-item__feedback-btn"
                  onClick={() => handleFeedback('report')}
                >
                  <AlertTriangle size={14} />
                </button>
              </Tooltip>
            </div>
          </div>
        )}
      </div>
    );
  },
  (prev, next) => {
    // Streaming content accumulates, so always re-render.
    if (next.round.isStreaming || prev.round.isStreaming) {
      return false;
    }
    
    // In complete state, compare items array reference to detect tool state changes.
    return (
      prev.round.id === next.round.id &&
      prev.round.items === next.round.items
    );
  }
);

ModelRoundItem.displayName = 'ModelRoundItem';

/**
 * Subagent items container.
 * Wraps all subagent items for a parent task in a scrollable container.
 */
interface SubagentItemsContainerProps {
  parentTaskToolId: string;
  items: FlowItem[];
  turnId: string;
  roundId: string;
}

const SubagentItemsContainer = React.memo<SubagentItemsContainerProps>(({ 
  parentTaskToolId, 
  items, 
  turnId, 
  roundId 
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  // Track user scroll-up to pause auto-scroll.
  const userScrolledUpRef = useRef(false);
  const lastScrollTopRef = useRef(0);
  
  const [isCollapsed, setIsCollapsed] = React.useState(() => 
    taskCollapseStateManager.isCollapsed(parentTaskToolId)
  );
  
  React.useEffect(() => {
    setIsCollapsed(taskCollapseStateManager.isCollapsed(parentTaskToolId));
    
    const unsubscribe = taskCollapseStateManager.addListener((toolId: string, collapsed: boolean) => {
      if (toolId === parentTaskToolId) {
        setIsCollapsed(collapsed);
      }
    });
    
    return unsubscribe;
  }, [parentTaskToolId]);
  
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    
    const handleScroll = () => {
      const currentScrollTop = container.scrollTop;
      const maxScrollTop = container.scrollHeight - container.clientHeight;
      
      if (currentScrollTop < lastScrollTopRef.current && maxScrollTop > 0) {
        if (lastScrollTopRef.current - currentScrollTop > 20) {
          userScrolledUpRef.current = true;
        }
      }
      
      if (maxScrollTop > 0 && maxScrollTop - currentScrollTop < 30) {
        userScrolledUpRef.current = false;
      }
      
      lastScrollTopRef.current = currentScrollTop;
    };
    
    container.addEventListener('scroll', handleScroll, { passive: true });
    return () => container.removeEventListener('scroll', handleScroll);
  }, [isCollapsed]);
  
  // Use MutationObserver for streaming auto-scroll with throttling.
  useEffect(() => {
    const container = containerRef.current;
    if (!container || isCollapsed) return;
    
    let rafId: number | null = null;
    let throttleTimer: NodeJS.Timeout | null = null;
    const THROTTLE_MS = 50;
    
    const scrollToBottom = () => {
      if (container && !userScrolledUpRef.current) {
        container.scrollTop = container.scrollHeight;
        lastScrollTopRef.current = container.scrollTop;
      }
    };
    
    const throttledScroll = () => {
      if (throttleTimer) return;
      
      throttleTimer = setTimeout(() => {
        throttleTimer = null;
        if (rafId) cancelAnimationFrame(rafId);
        rafId = requestAnimationFrame(scrollToBottom);
      }, THROTTLE_MS);
    };
    
    const observer = new MutationObserver(throttledScroll);
    
    observer.observe(container, {
      childList: true,
      subtree: true,
      characterData: true,
    });
    
    scrollToBottom();
    
    return () => {
      observer.disconnect();
      if (rafId) cancelAnimationFrame(rafId);
      if (throttleTimer) clearTimeout(throttleTimer);
    };
  }, [isCollapsed]);
  
  if (isCollapsed) {
    return null;
  }
  
  return (
    <div className="subagent-items-wrapper">
      <div 
        ref={containerRef}
        className="subagent-items-container"
        data-parent-tool-id={parentTaskToolId}
      >
        {items.map((item, idx) => (
          <SubagentItemRenderer 
            key={item.id}
            item={item}
            turnId={turnId}
            roundId={roundId}
            isLastItem={idx === items.length - 1}
          />
        ))}
      </div>
    </div>
  );
});

/**
 * Subagent item renderer (used inside the container, no collapse logic).
 */
const SubagentItemRenderer = React.memo<{ item: FlowItem; turnId: string; roundId: string; isLastItem?: boolean }>(({ item, isLastItem }) => {
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
    
    case 'thinking':
      return (
        <ModelThinkingDisplay thinkingItem={item as FlowThinkingItem} isLastItem={isLastItem} />
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

/**
 * FlowItem renderer (text or tool).
 */
interface FlowItemRendererProps {
  item: FlowItem;
  turnId: string;
  roundId: string;
  isLastItem?: boolean;
}

// Do not memoize: streaming content updates frequently.
const FlowItemRenderer: React.FC<FlowItemRendererProps> = ({ item, isLastItem }) => {
  const {
    onToolConfirm,
    onToolReject,
    onFileViewRequest,
    onTabOpen,
    sessionId,
  } = useFlowChatContext();
  
  const isSubagentItem = (item as any).isSubagentItem === true;
  const parentTaskToolId = (item as any).parentTaskToolId;
  
  const [isParentCollapsed, setIsParentCollapsed] = React.useState(false);
  
  React.useEffect(() => {
    if (isSubagentItem && parentTaskToolId) {
      setIsParentCollapsed(taskCollapseStateManager.isCollapsed(parentTaskToolId));
      
      const unsubscribe = taskCollapseStateManager.addListener((toolId: string, collapsed: boolean) => {
        if (toolId === parentTaskToolId) {
          setIsParentCollapsed(collapsed);
        }
      });
      
      return unsubscribe;
    }
  }, [isSubagentItem, parentTaskToolId]);
  
  const itemClassName = isSubagentItem 
    ? `subagent-item ${isParentCollapsed ? 'subagent-item--collapsed' : 'subagent-item--expanded'}`
    : '';
  
  const wrapContent = (content: React.ReactNode) => {
    if (isSubagentItem && parentTaskToolId) {
      return (
        <div 
          className={itemClassName}
          data-parent-tool-id={parentTaskToolId}
        >
          {content}
        </div>
      );
    }
    return content;
  };
  
  switch (item.type) {
    case 'text':
      return wrapContent(
        <FlowTextBlock
          textItem={item as FlowTextItem}
        />
      );
    
    case 'thinking':
      return wrapContent(
        <ModelThinkingDisplay thinkingItem={item as FlowThinkingItem} isLastItem={isLastItem} />
      );
    
    case 'tool':
      return wrapContent(
        <div className="flowchat-flow-item" data-flow-item-id={item.id} data-flow-item-type="tool">
          <FlowToolCard
            toolItem={item as FlowToolItem}
            onConfirm={async (toolId: string, updatedInput?: any) => {
              if (onToolConfirm) {
                await onToolConfirm(toolId, updatedInput);
              }
            }}
            onReject={async () => {
              if (onToolReject) {
                await onToolReject(item.id);
              }
            }}
            onOpenInEditor={(filePath: string) => {
              if (onFileViewRequest) {
                onFileViewRequest(filePath, filePath.split(/[/\\]/).pop() || filePath);
              }
            }}
            onOpenInPanel={(_panelType: string, data: any) => {
              if (onTabOpen) {
                // data contains the full tabInfo payload.
                onTabOpen(data, sessionId);
              }
            }}
            sessionId={sessionId}
          />
        </div>
      );
    
    default:
      return null;
  }
};
