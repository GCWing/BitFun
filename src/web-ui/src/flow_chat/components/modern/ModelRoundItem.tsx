/* eslint-disable @typescript-eslint/no-use-before-define */
/**
 * Model round item component.
 * Renders mixed FlowItems (text + tools).
 *
 * Note: explore-only rounds are handled by ExploreGroupRenderer,
 * and this component only renders rounds with critical output.
 */

import React, { useMemo, useState, useCallback, useEffect, useLayoutEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Copy, Check } from 'lucide-react';
import type { ModelRound, ModelRoundAttempt, FlowItem, FlowTextItem, FlowToolItem, FlowThinkingItem, TokenUsage, ToolRejectOptions } from '../../types/flow-chat';
import { useI18n } from '@/infrastructure/i18n';
import { FlowTextBlock } from '../FlowTextBlock';
import { FlowToolCard } from '../FlowToolCard';
import { ModelThinkingDisplay } from '../../tool-cards/ModelThinkingDisplay';
import { isCollapsibleTool } from '../../tool-cards/toolCardMetadata';
import { useFlowChatContext } from './FlowChatContext';
import { FlowChatStore } from '../../store/FlowChatStore';
import { taskCollapseStateManager } from '../../store/TaskCollapseStateManager';
import { ExportImageButton } from './ExportImageButton';
import { ForkSessionButton } from './ForkSessionButton';
import {
  buildModelRoundItemGroups,
  COMPLETED_TOOL_TRANSIENT_MS,
  isCompletedToolInTransientWindow,
  type ModelRoundItemGroup,
} from './modelRoundItemGrouping';
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
import {
  isStartupRenderTraceEnabled,
  recordReactRenderProfile,
  startupTrace,
} from '@/shared/utils/startupTrace';
import { SubagentProjectionView } from '../subagent/SubagentProjectionView';
import { formatSessionViewPreviewText } from '../../utils/sessionViewPreview';
import { buildModelRoundUsageMeta } from '../../utils/tokenUsageDisplay';
import './ModelRoundItem.scss';
import './SubagentItems.scss';

const log = createLogger('ModelRoundItem');

interface ModelRoundGroupSummary {
  textItemCount: number;
  toolItemCount: number;
  criticalGroupCount: number;
  exploreGroupCount: number;
}

function summarizeModelRoundItemGroups(groups: ModelRoundItemGroup[]): ModelRoundGroupSummary {
  return groups.reduce<ModelRoundGroupSummary>((summary, group) => {
    if (group.type === 'explore') {
      summary.exploreGroupCount += 1;
      for (const item of group.items) {
        if (item.type === 'text') {
          summary.textItemCount += 1;
        } else if (item.type === 'tool') {
          summary.toolItemCount += 1;
        }
      }
      return summary;
    }

    summary.criticalGroupCount += 1;
    if (group.item.type === 'text') {
      summary.textItemCount += 1;
    } else if (group.item.type === 'tool') {
      summary.toolItemCount += 1;
    }
    return summary;
  }, {
    textItemCount: 0,
    toolItemCount: 0,
    criticalGroupCount: 0,
    exploreGroupCount: 0,
  });
}

interface ModelRoundRenderTraceProps {
  startedAtMs: number;
  turnId: string;
  round: ModelRound;
  itemCount: number;
  groupCount: number;
  renderedCount: number;
  visibleGroupStartIndex: number;
  visibleGroupEndIndex: number;
  allGroupSummary: ModelRoundGroupSummary;
  visibleGroupSummary: ModelRoundGroupSummary;
}

const ModelRoundRenderTrace: React.FC<ModelRoundRenderTraceProps> = ({
  startedAtMs,
  turnId,
  round,
  itemCount,
  groupCount,
  renderedCount,
  visibleGroupStartIndex,
  visibleGroupEndIndex,
  allGroupSummary,
  visibleGroupSummary,
}) => {
  useLayoutEffect(() => {
    recordReactRenderProfile(startupTrace, {
      component: 'ModelRoundItem',
      phase: 'commit',
      actualDurationMs: performance.now() - startedAtMs,
      turnId,
      roundId: round.id,
      itemCount,
      groupCount,
      renderedCount,
      visibleGroupStartIndex,
      visibleGroupEndIndex,
      textItemCount: allGroupSummary.textItemCount,
      toolItemCount: allGroupSummary.toolItemCount,
      visibleTextItemCount: visibleGroupSummary.textItemCount,
      visibleToolItemCount: visibleGroupSummary.toolItemCount,
      criticalGroupCount: allGroupSummary.criticalGroupCount,
      exploreGroupCount: allGroupSummary.exploreGroupCount,
      isStreaming: round.isStreaming,
    });
  });

  return null;
};

interface ModelRoundItemProps {
  round: ModelRound;
  turnId: string;
  isLastRound?: boolean;
  isTurnComplete?: boolean;
  turnStartedAt?: number;
  turnEndedAt?: number;
  turnDurationMs?: number;
  turnTokenUsage?: TokenUsage;
}

function sortRoundAttempts(attempts: ModelRoundAttempt[]): ModelRoundAttempt[] {
  return [...attempts].sort((left, right) => left.index - right.index);
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
  directSubagentDialogTurnId?: string;
  turnId: string;
  roundId?: string;
  completedToolExitNowMs: number;
  allowCompletedToolExit?: boolean;
}

const TaskWithSubagentWrapper: React.FC<TaskWithSubagentWrapperProps> = React.memo(({
  taskItem,
  parentTaskToolId,
  parentSessionId,
  directSubagentSessionId,
  directSubagentDialogTurnId,
  turnId,
  roundId,
  completedToolExitNowMs,
  allowCompletedToolExit = false,
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
        roundId={roundId}
        isLastItem={false}
        completedToolExitNowMs={completedToolExitNowMs}
        allowCompletedToolExit={allowCompletedToolExit}
      />
      <SubagentProjectionView
        parentTaskToolId={parentTaskToolId}
        parentSessionId={parentSessionId}
        directSubagentSessionId={directSubagentSessionId}
        directSubagentDialogTurnId={directSubagentDialogTurnId}
        parentToolIds={new Set<string>([parentTaskToolId, (taskItem as FlowToolItem).toolCall?.id].filter(Boolean) as string[])}
        liveItemsMode={isTaskRunning ? 'full-turn' : 'last-round'}
        turnId={turnId}
      />
    </div>
  );
});

export const ModelRoundItem = React.memo<ModelRoundItemProps>(
  ({
    round,
    turnId,
    isLastRound = false,
    isTurnComplete = false,
    turnStartedAt,
    turnEndedAt,
    turnDurationMs,
    turnTokenUsage,
  }) => {
    const { t } = useTranslation('flow-chat');
    const { formatDate, formatNumber } = useI18n('flow-chat');
    const { sessionId } = useFlowChatContext();
    const [copied, setCopied] = useState(false);
    const [showRetryHistory, setShowRetryHistory] = useState(false);
    const [showRoundHistory, setShowRoundHistory] = useState(false);
    const [openHistoryRoundAttemptIds, setOpenHistoryRoundAttemptIds] = useState<Record<string, boolean>>({});
    const copyButtonRef = useRef<HTMLButtonElement>(null);
    const renderTraceEnabled = isStartupRenderTraceEnabled();
    const renderTraceStartedAtMs = renderTraceEnabled ? performance.now() : null;
    
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

    const attempts = useMemo(
      () => sortRoundAttempts(round.attempts ?? []),
      [round.attempts]
    );
    const olderAttempts = attempts.length > 1 ? attempts.slice(0, -1) : [];
    const latestAttempt = attempts.length > 0 ? attempts[attempts.length - 1] : undefined;
    const historyRounds = round.historyRounds ?? [];

    useEffect(() => {
      if (olderAttempts.length === 0 && showRetryHistory) {
        setShowRetryHistory(false);
      }
    }, [olderAttempts.length, showRetryHistory]);

    useEffect(() => {
      if (historyRounds.length === 0 && showRoundHistory) {
        setShowRoundHistory(false);
      }
    }, [historyRounds.length, showRoundHistory]);

    const toggleHistoryRoundAttempts = useCallback((historyRoundId: string) => {
      setOpenHistoryRoundAttemptIds((current) => ({
        ...current,
        [historyRoundId]: !current[historyRoundId],
      }));
    }, []);

    // Keep the recorded round order; FlowChatStore already applies immutable updates.
    const sortedItems = useMemo(
      () => latestAttempt?.items ?? round.items,
      [latestAttempt?.items, round.items]
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
    const allGroupSummary = useMemo(
      () => renderTraceEnabled ? summarizeModelRoundItemGroups(groupedItems) : null,
      [groupedItems, renderTraceEnabled],
    );
    const visibleGroupSummary = useMemo(
      () => renderTraceEnabled ? summarizeModelRoundItemGroups(visibleGroupedItems) : null,
      [renderTraceEnabled, visibleGroupedItems],
    );
    const hasDeferredEarlierGroups = visibleGroupStartIndex > 0;
    const hasDeferredLaterGroups = visibleGroupEndIndex < groupedItems.length;

    const renderGroupList = useCallback((
      groups: ModelRoundItemGroup[],
      options: {
        roundId: string;
        keyPrefix: string;
        isFinalSection: boolean;
      },
    ) => (
      groups.map((group, groupIndex) => {
        const isLastGroup = groupIndex === groups.length - 1;
        const isLast = options.isFinalSection && isLastGroup;
        switch (group.type) {
          case 'explore':
            return group.items.map((item, itemIdx) => (
              <FlowItemRenderer
                key={`${options.keyPrefix}:${item.id}`}
                item={item}
                turnId={turnId}
                roundId={options.roundId}
                isLastItem={isLast && itemIdx === group.items.length - 1}
                completedToolExitNowMs={transientNowMs}
                allowCompletedToolExit
              />
            ));

          case 'critical': {
            const projectedSubagent = group.item.type === 'tool' && (group.item as FlowToolItem).toolName === 'Task'
              ? group.item as FlowToolItem
              : undefined;
            if (projectedSubagent) {
              return (
                <TaskWithSubagentWrapper
                  key={`${options.keyPrefix}:task-with-subagent-${projectedSubagent.id}`}
                  taskItem={projectedSubagent}
                  parentTaskToolId={projectedSubagent.id}
                  parentSessionId={sessionId}
                  directSubagentSessionId={projectedSubagent.subagentSessionId}
                  directSubagentDialogTurnId={projectedSubagent.subagentDialogTurnId}
                  turnId={turnId}
                  roundId={options.roundId}
                  completedToolExitNowMs={transientNowMs}
                  allowCompletedToolExit={false}
                />
              );
            }
            return (
              <FlowItemRenderer
                key={`${options.keyPrefix}:${group.item.id}`}
                item={group.item}
                turnId={turnId}
                roundId={options.roundId}
                isLastItem={isLast}
                completedToolExitNowMs={transientNowMs}
                allowCompletedToolExit={false}
              />
            );
          }

          default:
            return null;
        }
      })
    ), [sessionId, transientNowMs, turnId]);

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

    const completedAt = turnEndedAt ?? round.endTime;
    const effectiveDurationMs = turnDurationMs ??
      (typeof turnStartedAt === 'number' && typeof completedAt === 'number'
        ? Math.max(0, completedAt - turnStartedAt)
        : round.durationMs);
    const usageMetaItems = useMemo(() => buildModelRoundUsageMeta({
      completedAt,
      durationMs: effectiveDurationMs,
      tokenUsage: turnTokenUsage,
      status: round.status,
      formatTime: timestamp => formatDate(new Date(timestamp), {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
      }),
      formatNumber,
      t,
    }), [completedAt, effectiveDurationMs, formatDate, formatNumber, round.status, t, turnTokenUsage]);
    const shouldRenderFooter = isTurnComplete &&
      isLastRound &&
      !round.isStreaming &&
      (hasContent || usageMetaItems.length > 0);
    
    return (
      <div 
        className={`model-round-item model-round-item--${round.isStreaming ? 'streaming' : 'complete'}`}
        data-testid="chat-assistant-message"
        data-turn-id={turnId}
        data-round-id={round.id}
        data-status={round.status}
        data-model-id={round.modelId || ''}
        data-model-alias={round.modelAlias || ''}
        data-streaming={round.isStreaming ? 'true' : 'false'}
      >
        {renderTraceEnabled && renderTraceStartedAtMs !== null && allGroupSummary && visibleGroupSummary && (
          <ModelRoundRenderTrace
            startedAtMs={renderTraceStartedAtMs}
            turnId={turnId}
            round={round}
            itemCount={sortedItems.length}
            groupCount={groupedItems.length}
            renderedCount={renderedGroupCount}
            visibleGroupStartIndex={visibleGroupStartIndex}
            visibleGroupEndIndex={visibleGroupEndIndex}
            allGroupSummary={allGroupSummary}
            visibleGroupSummary={visibleGroupSummary}
          />
        )}
        {hasDeferredEarlierGroups && (
          <div className="model-round-item__history-loader">
            {t('modelRound.loadingMoreHistory')}
          </div>
        )}

        {historyRounds.length > 0 && (
          <div className="model-round-item__retry-history">
            <button
              type="button"
              className="model-round-item__retry-toggle"
              onClick={() => setShowRoundHistory(current => !current)}
            >
              {showRoundHistory
                ? t('modelRound.roundHistoryHide')
                : t('modelRound.roundHistoryShow', { count: historyRounds.length })}
            </button>

            {showRoundHistory && historyRounds.map((historyRound, historyIndex) => {
              const historyAttempts = sortRoundAttempts(historyRound.attempts ?? []);
              const historyOlderAttempts = historyAttempts.length > 1
                ? historyAttempts.slice(0, -1)
                : [];
              const historyLatestAttempt = historyAttempts.length > 0
                ? historyAttempts[historyAttempts.length - 1]
                : undefined;
              const showHistoryRoundAttempts = openHistoryRoundAttemptIds[historyRound.id] === true;
              const historyGroups = buildModelRoundItemGroups({
                items: historyLatestAttempt?.items ?? historyRound.items,
                isStreaming: false,
                disableExploreGrouping: true,
                isCollapsibleTool,
                nowMs: transientNowMs,
              });

              return (
                <div key={historyRound.id} className="model-round-item__retry-attempt">
                  <div className="model-round-item__retry-attempt-label">
                    {t('modelRound.roundRetryLabel', { index: historyIndex + 1 })}
                  </div>
                  {historyOlderAttempts.length > 0 && (
                    <div className="model-round-item__retry-history">
                      <button
                        type="button"
                        className="model-round-item__retry-toggle"
                        onClick={() => toggleHistoryRoundAttempts(historyRound.id)}
                      >
                        {showHistoryRoundAttempts
                          ? t('modelRound.retryHistoryHide')
                          : t('modelRound.retryHistoryShow', { count: historyOlderAttempts.length })}
                      </button>

                      {showHistoryRoundAttempts && historyOlderAttempts.map((attempt) => {
                        const attemptGroups = buildModelRoundItemGroups({
                          items: attempt.items,
                          isStreaming: false,
                          disableExploreGrouping: true,
                          isCollapsibleTool,
                          nowMs: transientNowMs,
                        });

                        return (
                          <div key={attempt.id} className="model-round-item__retry-attempt">
                            <div className="model-round-item__retry-attempt-label">
                              {t('modelRound.attemptLabel', { index: attempt.index })}
                            </div>
                            {renderGroupList(attemptGroups, {
                              roundId: historyRound.id,
                              keyPrefix: `history-round:${historyRound.id}:attempt:${attempt.id}`,
                              isFinalSection: false,
                            })}
                          </div>
                        );
                      })}
                    </div>
                  )}
                  {renderGroupList(historyGroups, {
                    roundId: historyRound.id,
                    keyPrefix: `history-round:${historyRound.id}`,
                    isFinalSection: false,
                  })}
                </div>
              );
            })}
          </div>
        )}

        {olderAttempts.length > 0 && (
          <div className="model-round-item__retry-history">
            <button
              type="button"
              className="model-round-item__retry-toggle"
              onClick={() => setShowRetryHistory(current => !current)}
            >
              {showRetryHistory
                ? t('modelRound.retryHistoryHide')
                : t('modelRound.retryHistoryShow', { count: olderAttempts.length })}
            </button>

            {showRetryHistory && olderAttempts.map((attempt) => {
              const attemptGroups = buildModelRoundItemGroups({
                items: attempt.items,
                isStreaming: false,
                disableExploreGrouping: true,
                isCollapsibleTool,
                nowMs: transientNowMs,
              });

              return (
                <div key={attempt.id} className="model-round-item__retry-attempt">
                  <div className="model-round-item__retry-attempt-label">
                    {t('modelRound.attemptLabel', { index: attempt.index })}
                  </div>
                  {renderGroupList(attemptGroups, {
                    roundId: round.id,
                    keyPrefix: `attempt:${attempt.id}`,
                    isFinalSection: false,
                  })}
                </div>
              );
            })}
          </div>
        )}

        {renderGroupList(visibleGroupedItems, {
          roundId: round.id,
          keyPrefix: latestAttempt ? `attempt:${latestAttempt.id}` : 'round',
          isFinalSection: isLastRound,
        })}

        {hasDeferredLaterGroups && (
          <div className="model-round-item__history-loader">
            {t('modelRound.loadingMoreHistory')}
          </div>
        )}

        {shouldRenderFooter && (
          <div className="model-round-item__footer">
            {usageMetaItems.length > 0 && (
              <div
                className="model-round-item__meta"
                aria-label={t('modelRound.meta.label')}
              >
                {usageMetaItems.map(item => (
                  <span key={item.key} className="model-round-item__meta-item">
                    <span className="model-round-item__meta-label">{item.label}</span>
                    <span className="model-round-item__meta-value">{item.value}</span>
                  </span>
                ))}
              </div>
            )}

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
      prev.round.historyRounds === next.round.historyRounds &&
      prev.isLastRound === next.isLastRound &&
      prev.isTurnComplete === next.isTurnComplete &&
      prev.turnStartedAt === next.turnStartedAt &&
      prev.turnEndedAt === next.turnEndedAt &&
      prev.turnDurationMs === next.turnDurationMs &&
      prev.turnTokenUsage === next.turnTokenUsage
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
  roundId?: string;
  isLastItem?: boolean;
  completedToolExitNowMs: number;
  allowCompletedToolExit?: boolean;
}

// Do not memoize: streaming content updates frequently.
const FlowItemRenderer: React.FC<FlowItemRendererProps> = ({
  item,
  turnId,
  roundId,
  isLastItem,
  completedToolExitNowMs,
  allowCompletedToolExit = false,
}) => {
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
          traceContext={{
            turnId,
            roundId,
            itemId: item.id,
          }}
          testId="chat-assistant-message-content"
          testAttributes={{
            'data-turn-id': turnId,
            'data-flow-item-id': item.id,
            'data-status': item.status,
          }}
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
      const shouldAnimateCompletedExit =
        allowCompletedToolExit &&
        isCollapsible &&
        isCompletedTool &&
        isCompletedToolInTransientWindow(toolItem, completedToolExitNowMs);
      const isSettledCompletedTool =
        allowCompletedToolExit && isCollapsible && isCompletedTool && !shouldAnimateCompletedExit;
      const toolClassName = [
        'flowchat-flow-item',
        isCollapsible && isCompletedTool ? 'flowchat-flow-item--tool-transition' : null,
        shouldAnimateCompletedExit ? 'flowchat-flow-item--tool-completed' : null,
        isSettledCompletedTool ? 'flowchat-flow-item--tool-settled' : null,
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
            onReject={async (_toolId: string, options?: ToolRejectOptions) => {
              if (onToolReject) {
                await onToolReject(item.id, options);
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
