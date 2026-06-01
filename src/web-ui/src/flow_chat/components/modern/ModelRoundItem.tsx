/* eslint-disable @typescript-eslint/no-use-before-define */
/**
 * Model round item component.
 * Renders mixed FlowItems (text + tools).
 *
 * Note: explore-only rounds are handled by ExploreGroupRenderer,
 * and this component only renders rounds with critical output.
 */

import React, { useMemo, useState, useCallback, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Copy, Check } from 'lucide-react';
import type { ModelRound, FlowItem, FlowTextItem, FlowToolItem, FlowThinkingItem } from '../../types/flow-chat';
import { FlowTextBlock } from '../FlowTextBlock';
import { FlowToolCard } from '../FlowToolCard';
import { ModelThinkingDisplay } from '../../tool-cards/ModelThinkingDisplay';
import { isCollapsibleTool } from '../../tool-cards';
import { useFlowChatContext } from './FlowChatContext';
import { FlowChatStore } from '../../store/FlowChatStore';
import { taskCollapseStateManager } from '../../store/TaskCollapseStateManager';
import { ExportImageButton } from './ExportImageButton';
import { ForkSessionButton } from './ForkSessionButton';
import { buildModelRoundItemGroups, COMPLETED_TOOL_TRANSIENT_MS } from './modelRoundItemGrouping';
import {
  MODEL_ROUND_GROUP_RENDER_CHUNK_DELAY_MS,
  getInitialModelRoundGroupRenderCount,
  getNextModelRoundGroupRenderCount,
  getSynchronizedModelRoundGroupRenderCount,
  getVisibleModelRoundGroupEndIndex,
  getVisibleModelRoundGroupStartIndex,
} from './modelRoundProgressiveRender';
import { Tooltip } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import { SubagentProjectionView } from '../subagent/SubagentProjectionView';
import { formatSessionViewPreviewText } from '../../utils/sessionViewPreview';
import './ModelRoundItem.scss';
import './SubagentItems.scss';

const log = createLogger('ModelRoundItem');

interface ModelRoundItemProps {
  round: ModelRound;
  turnId: string;
  isLastRound?: boolean;
  isTurnComplete?: boolean;
}

function useTaskCollapsed(toolId: string): boolean {
  const [isCollapsed, setIsCollapsed] = useState(() =>
    taskCollapseStateManager.isCollapsed(toolId)
  );

  useEffect(() => {
    setIsCollapsed(taskCollapseStateManager.isCollapsed(toolId));

    const unsubscribe = taskCollapseStateManager.addListener((changedToolId, collapsed) => {
      if (changedToolId === toolId) {
        setIsCollapsed(collapsed);
      }
    });

    return unsubscribe;
  }, [toolId]);

  return isCollapsed;
}

interface TaskWithSubagentWrapperProps {
  taskItem: FlowItem;
  parentTaskToolId: string;
  parentSessionId?: string;
  directSubagentSessionId?: string;
  turnId: string;
}

const TaskWithSubagentWrapper: React.FC<TaskWithSubagentWrapperProps> = React.memo(({
  taskItem,
  parentTaskToolId,
  parentSessionId,
  directSubagentSessionId,
  turnId,
}) => {
  const isCollapsed = useTaskCollapsed(parentTaskToolId);
  const isTaskRunning =
    taskItem.status === 'preparing' || taskItem.status === 'streaming' || taskItem.status === 'running';
  const hasPrompt = Boolean(
    taskItem.type === 'tool' &&
    (taskItem as FlowToolItem).toolCall?.input?.prompt
  );
  const className = [
    'task-with-subagent-wrapper',
    !isCollapsed && 'task-with-subagent-wrapper--expanded',
    hasPrompt && 'task-with-subagent-wrapper--has-prompt',
  ].filter(Boolean).join(' ');

  return (
    <div className={className}>
      <FlowItemRenderer
        item={taskItem}
        turnId={turnId}
        isLastItem={false}
      />
      <SubagentProjectionView
        parentTaskToolId={parentTaskToolId}
        parentSessionId={parentSessionId}
        directSubagentSessionId={directSubagentSessionId}
        parentToolIds={new Set<string>([parentTaskToolId, (taskItem as FlowToolItem).toolCall?.id].filter(Boolean) as string[])}
        liveItemsMode={isTaskRunning ? 'full-turn' : 'last-round'}
        turnId={turnId}
      />
    </div>
  );
});

export const ModelRoundItem = React.memo<ModelRoundItemProps>(
  ({ round, turnId, isLastRound = false, isTurnComplete = false }) => {
    const { t } = useTranslation('flow-chat');
    const { sessionId } = useFlowChatContext();
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
    
    // Keep the recorded round order; FlowChatStore already applies immutable updates.
    const sortedItems = useMemo(
      () => round.items,
      [round.items]
    );
    
    const latestCompletedToolEndTime = useMemo(() => {
      return sortedItems.reduce((latest, item) => {
        if (item.type !== 'tool' || item.status !== 'completed') return latest;
        const endTime = (item as FlowToolItem).endTime;
        return typeof endTime === 'number' ? Math.max(latest, endTime) : latest;
      }, 0);
    }, [sortedItems]);
    const [transientNowMs, setTransientNowMs] = useState(() => Date.now());

    useEffect(() => {
      if (latestCompletedToolEndTime <= 0) return;

      const remainingMs = latestCompletedToolEndTime + COMPLETED_TOOL_TRANSIENT_MS - Date.now();
      if (remainingMs <= 0) {
        setTransientNowMs(Date.now());
        return;
      }

      setTransientNowMs(Date.now());
      const timeoutId = window.setTimeout(() => {
        setTransientNowMs(Date.now());
      }, remainingMs);

      return () => window.clearTimeout(timeoutId);
    }, [latestCompletedToolEndTime]);

    // Group items in two passes:
    // 1) group subagent items
    // 2) group normal items into explore/critical via anchor tool
    const groupedItems = useMemo(() => {
      return buildModelRoundItemGroups({
        items: sortedItems,
        isStreaming: round.isStreaming,
        disableExploreGrouping: round.renderHints?.disableExploreGrouping === true,
        isCollapsibleTool,
        nowMs: transientNowMs,
      });
    }, [round.isStreaming, round.renderHints?.disableExploreGrouping, sortedItems, transientNowMs]);

    const initialGroupRenderCount = useMemo(() => (
      getInitialModelRoundGroupRenderCount({
        groupCount: groupedItems.length,
        isStreaming: round.isStreaming,
      })
    ), [groupedItems.length, round.isStreaming]);

    const [renderedGroupState, setRenderedGroupState] = useState(() => ({
      roundId: round.id,
      count: initialGroupRenderCount,
    }));

    useEffect(() => {
      setRenderedGroupState((current) => {
        if (current.roundId !== round.id) {
          return { roundId: round.id, count: initialGroupRenderCount };
        }

        const nextCount = getSynchronizedModelRoundGroupRenderCount({
          currentCount: current.count,
          groupCount: groupedItems.length,
          initialCount: initialGroupRenderCount,
          isStreaming: round.isStreaming,
        });

        return current.count === nextCount
          ? current
          : { roundId: round.id, count: nextCount };
      });
    }, [groupedItems.length, initialGroupRenderCount, round.id, round.isStreaming]);

    const renderedGroupCount = renderedGroupState.roundId === round.id
      ? renderedGroupState.count
      : initialGroupRenderCount;

    useEffect(() => {
      if (round.isStreaming || renderedGroupCount >= groupedItems.length) {
        return;
      }

      const timeoutId = window.setTimeout(() => {
        setRenderedGroupState((current) => {
          if (current.roundId !== round.id) {
            return current;
          }

          return {
            roundId: round.id,
            count: getNextModelRoundGroupRenderCount({
              currentCount: current.count,
              groupCount: groupedItems.length,
            }),
          };
        });
      }, MODEL_ROUND_GROUP_RENDER_CHUNK_DELAY_MS);

      return () => window.clearTimeout(timeoutId);
    }, [groupedItems.length, renderedGroupCount, round.id, round.isStreaming]);

    const visibleGroupStartIndex = getVisibleModelRoundGroupStartIndex({
      renderedCount: renderedGroupCount,
      groupCount: groupedItems.length,
      isStreaming: round.isStreaming,
    });
    const visibleGroupEndIndex = getVisibleModelRoundGroupEndIndex({
      renderedCount: renderedGroupCount,
      groupCount: groupedItems.length,
      startIndex: visibleGroupStartIndex,
    });
    const visibleGroupedItems = useMemo(
      () => groupedItems.slice(visibleGroupStartIndex, visibleGroupEndIndex),
      [groupedItems, visibleGroupEndIndex, visibleGroupStartIndex],
    );
    const hasDeferredEarlierGroups = visibleGroupStartIndex > 0;
    const hasDeferredLaterGroups = visibleGroupEndIndex < groupedItems.length;

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
          } else if (item.type === 'thinking' && item.content?.trim()) {
            roundContent.push(`[Thinking]\n${item.content.trim()}`);
          } else if (item.type === 'tool' && item.toolCall) {
            const toolName = item.toolName || t('copyOutput.unknownTool');
            let toolContent = t('modelRound.toolCallLabel', { name: toolName }) + '\n';
            
            if (item.toolCall.input) {
              const inputStr = typeof item.toolCall.input === 'string'
                ? item.toolCall.input
                : JSON.stringify(item.toolCall.input, null, 2);
              toolContent += `\n[Input]\n\`\`\`json\n${inputStr}\n\`\`\`\n`;
            }
            
            if (item.toolResult) {
              if (item.toolResult.error) {
                toolContent += `\n[Error]\n${item.toolResult.error}\n`;
              } else if (item.toolResult.result !== undefined) {
                const resultStr = typeof item.toolResult.result === 'string'
                  ? item.toolResult.result
                  : JSON.stringify(item.toolResult.result, null, 2);
                toolContent += `\n[Result]\n\`\`\`\n${formatSessionViewPreviewText(resultStr)}\n\`\`\`\n`;
              }
            }
            
            roundContent.push(toolContent.trim());
          }
        });
        
        if (roundContent.length > 0) {
          contentParts.push(roundContent.join('\n\n'));
        }
      });
      
      return contentParts.join('\n\n---\n\n');
    }, [t, turnId]);
    
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
        {hasDeferredEarlierGroups && (
          <div className="model-round-item__history-loader">
            {t('modelRound.loadingMoreHistory')}
          </div>
        )}

        {visibleGroupedItems.map((group, groupIndex) => {
          const isLastGroup = visibleGroupStartIndex + groupIndex === groupedItems.length - 1;
          const isLast = isLastRound && isLastGroup;
          switch (group.type) {
            case 'explore':
              return group.items.map((item, itemIdx) => (
                <FlowItemRenderer
                  key={item.id}
                  item={item}
                  turnId={turnId}
                  isLastItem={isLast && itemIdx === group.items.length - 1}
                />
              ));

            case 'critical': {
              const projectedSubagent = group.item.type === 'tool' && (group.item as FlowToolItem).toolName === 'Task'
                ? group.item as FlowToolItem
                : undefined;
              if (projectedSubagent) {
                return (
                  <TaskWithSubagentWrapper
                    key={`task-with-subagent-${projectedSubagent.id}`}
                    taskItem={projectedSubagent}
                    parentTaskToolId={projectedSubagent.id}
                    parentSessionId={sessionId}
                    directSubagentSessionId={projectedSubagent.subagentSessionId}
                    turnId={turnId}
                  />
                );
              }
              return (
                <FlowItemRenderer 
                  key={group.item.id}
                  item={group.item}
                  turnId={turnId}
                  isLastItem={isLast}
                />
              );
            }
            
            default:
              return null;
          }
        })}

        {hasDeferredLaterGroups && (
          <div className="model-round-item__history-loader">
            {t('modelRound.loadingMoreHistory')}
          </div>
        )}

        {isTurnComplete && isLastRound && hasContent && !round.isStreaming && (
          <div className="model-round-item__footer">
            <ForkSessionButton sessionId={sessionId} turnId={turnId} />

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
      prev.round.items === next.round.items &&
      prev.isLastRound === next.isLastRound &&
      prev.isTurnComplete === next.isTurnComplete
    );
  }
);

ModelRoundItem.displayName = 'ModelRoundItem';

/**
 * FlowItem renderer (text or tool).
 */
interface FlowItemRendererProps {
  item: FlowItem;
  turnId: string;
  isLastItem?: boolean;
}

// Do not memoize: streaming content updates frequently.
const FlowItemRenderer: React.FC<FlowItemRendererProps> = ({ item, turnId, isLastItem }) => {
  const {
    onToolConfirm,
    onToolReject,
    onFileViewRequest,
    onTabOpen,
    sessionId,
  } = useFlowChatContext();
  
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
    
    case 'tool': {
      const toolItem = item as FlowToolItem;
      const isCompletedTool = toolItem.status === 'completed';
      const isCollapsible = isCollapsibleTool(toolItem.toolName);
      const toolClassName = [
        'flowchat-flow-item',
        isCollapsible && isCompletedTool ? 'flowchat-flow-item--tool-transition' : null,
        isCollapsible && isCompletedTool ? 'flowchat-flow-item--tool-completed' : null,
        isCollapsible && !isCompletedTool ? 'flowchat-flow-item--tool-active' : null,
      ].filter(Boolean).join(' ');

      return (
        <div className={toolClassName} data-flow-item-id={item.id} data-flow-item-type="tool">
          <FlowToolCard
            toolItem={toolItem}
            onConfirm={async (toolId: string, updatedInput?: any, permissionOptionId?: string, approve?: boolean) => {
              if (onToolConfirm) {
                await onToolConfirm(toolId, updatedInput, permissionOptionId, approve);
              }
            }}
            onReject={async (_toolId: string, permissionOptionId?: string) => {
              if (onToolReject) {
                await onToolReject(item.id, permissionOptionId);
              }
            }}
            onOpenInEditor={(filePath: string) => {
              if (onFileViewRequest) {
                onFileViewRequest(filePath, filePath.split(/[/\\]/).pop() || filePath);
              }
            }}
            onOpenInPanel={(_panelType: string, data: any) => {
              if (onTabOpen) {
                onTabOpen(data, sessionId);
              }
            }}
            sessionId={sessionId}
            turnId={turnId}
          />
        </div>
      );
    }

    default:
      return null;
  }
};
