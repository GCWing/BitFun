/**
 * TaskDetailPanel - Subtask detail panel.
 * Minimal layout to match the FlowChat background.
 */

import React, { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { 
  Split,
  AlertCircle,
  Square
} from 'lucide-react';
import type { FlowToolItem, FlowTextItem, FlowThinkingItem, FlowItem, FlowChatState } from '../../types/flow-chat';
import { FlowChatStore } from '../../store/FlowChatStore';
import { FlowTextBlock } from '../FlowTextBlock';
import { FlowToolCard } from '../FlowToolCard';
import { ModelThinkingDisplay } from '../../tool-cards/ModelThinkingDisplay';
import { ToolTimeoutIndicator } from '../../tool-cards/ToolTimeoutIndicator';
import { Button, DotMatrixLoader } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import type { ReviewerContext } from '@/shared/services/reviewTeamService';
import './TaskDetailPanel.scss';

const log = createLogger('TaskDetailPanel');
const TASK_DETAIL_INITIAL_RENDER_COUNT = 18;
const TASK_DETAIL_RENDER_BATCH_SIZE = 32;

type FlowChatSession = NonNullable<ReturnType<FlowChatStore['getState']>['sessions'] extends Map<string, infer S> ? S : never>;

interface TaskDetailSnapshot {
  toolItem: FlowToolItem | null;
  subagentItems: FlowItem[];
}

function isTerminalStatus(status: FlowItem['status'] | undefined): boolean {
  return status === 'completed' || status === 'cancelled' || status === 'error';
}

function isRunningStatus(status: FlowItem['status'] | undefined): boolean {
  return status === 'preparing' || status === 'streaming' || status === 'running';
}

function readTaskDurationMs(toolResult: FlowToolItem['toolResult'] | undefined): number | undefined {
  const resultDuration = toolResult?.result?.duration;
  if (typeof resultDuration === 'number') {
    return resultDuration;
  }
  if (typeof toolResult?.duration_ms === 'number') {
    return toolResult.duration_ms;
  }
  return undefined;
}

function areFlowItemsEqual(prev: FlowItem[], next: FlowItem[]): boolean {
  if (prev === next) return true;
  if (prev.length !== next.length) return false;

  for (let i = 0; i < prev.length; i += 1) {
    const a = prev[i] as any;
    const b = next[i] as any;

    if (
      a.id !== b.id ||
      a.type !== b.type ||
      a.content !== b.content ||
      a.status !== b.status ||
      a.isStreaming !== b.isStreaming ||
      a.isParamsStreaming !== b.isParamsStreaming ||
      a.toolResult !== b.toolResult ||
      a.partialParams !== b.partialParams ||
      a.interruptionReason !== b.interruptionReason
    ) {
      return false;
    }
  }

  return true;
}

function areSnapshotsEqual(prev: TaskDetailSnapshot, next: TaskDetailSnapshot): boolean {
  return (
    prev.toolItem === next.toolItem &&
    areFlowItemsEqual(prev.subagentItems, next.subagentItems)
  );
}

function collectTaskDetailSnapshot(
  state: FlowChatState,
  sessionId: string | undefined,
  parentTaskToolIds: Set<string>,
  directSubagentSessionId?: string,
): TaskDetailSnapshot {
  if (parentTaskToolIds.size === 0 && !directSubagentSessionId) {
    return { toolItem: null, subagentItems: [] };
  }

  const preferredSession = sessionId ? state.sessions.get(sessionId) : undefined;
  const sessionsToSearch = preferredSession
    ? [preferredSession]
    : Array.from(state.sessions.values());

  const collectFromSessions = (sessions: Iterable<FlowChatSession>): TaskDetailSnapshot => {
    const subagentItems: FlowItem[] = [];
    let toolItem: FlowToolItem | null = null;

    for (const session of sessions) {
      for (const turn of session.dialogTurns) {
        for (const round of turn.modelRounds) {
          for (const item of round.items) {
            const itemAny = item as any;

            if (!toolItem && item.type === 'tool' && parentTaskToolIds.has(item.id)) {
              toolItem = item as FlowToolItem;
            }

            if (
              itemAny.isSubagentItem &&
              (
                parentTaskToolIds.has(itemAny.parentTaskToolId) ||
                (directSubagentSessionId && itemAny.subagentSessionId === directSubagentSessionId)
              )
            ) {
              subagentItems.push(item);
            }
          }
        }
      }
    }

    return { toolItem, subagentItems };
  };

  const preferredSnapshot = collectFromSessions(sessionsToSearch);
  if (preferredSnapshot.toolItem || preferredSnapshot.subagentItems.length > 0 || !preferredSession) {
    return preferredSnapshot;
  }

  return collectFromSessions(state.sessions.values());
}

export interface TaskDetailData {
  toolItem: FlowToolItem;
  taskInput: {
    description: string;
    prompt: string;
    agentType: string;
    reviewerContext?: ReviewerContext | null;
  } | null;
  sessionId?: string;
}

export interface TaskDetailPanelProps {
  data: TaskDetailData;
}

export const TaskDetailPanel: React.FC<TaskDetailPanelProps> = ({ data }) => {
  const { t } = useTranslation('flow-chat');
  const { t: tAgents } = useTranslation('scenes/agents');
  const { toolItem: initialToolItem, taskInput, sessionId } = data || {};
  const parentTaskToolId = initialToolItem?.id;
  const parentTaskToolCallId = initialToolItem?.toolCall?.id;
  const directSubagentSessionId = initialToolItem?.subagentSessionId;
  const parentTaskToolIds = useMemo(
    () => new Set([parentTaskToolId, parentTaskToolCallId].filter(Boolean) as string[]),
    [parentTaskToolId, parentTaskToolCallId],
  );
  
  const [taskSnapshot, setTaskSnapshot] = useState<TaskDetailSnapshot>(() => ({
    toolItem: initialToolItem ?? null,
    subagentItems: [],
  }));
  const [isSnapshotHydrated, setIsSnapshotHydrated] = useState(false);
  const [visibleSubagentCount, setVisibleSubagentCount] = useState(0);
  const [stoppingSubagent, setStoppingSubagent] = useState(false);
  const [stopError, setStopError] = useState<string | null>(null);
  
  const contentRef = useRef<HTMLDivElement>(null);
  // Track auto-scroll; disable when the user scrolls up.
  const shouldAutoScrollRef = useRef(true);

  // Collect only the state this detail panel cares about, and avoid re-rendering
  // when unrelated flow-chat updates arrive.
  useEffect(() => {
    if (parentTaskToolIds.size === 0 && !directSubagentSessionId) {
      setIsSnapshotHydrated(true);
      return;
    }
    
    const flowChatStore = FlowChatStore.getInstance();
    let previousSnapshot: TaskDetailSnapshot = {
      toolItem: initialToolItem ?? null,
      subagentItems: [],
    };
    let hydrationFrameId: number | null = null;
    let frameId: number | null = null;
    let latestState: FlowChatState | null = null;
    let unsubscribe: (() => void) | null = null;
    let disposed = false;

    setIsSnapshotHydrated(false);
    setTaskSnapshot(current => areSnapshotsEqual(current, previousSnapshot) ? current : previousSnapshot);

    const updateTaskSnapshot = (state: FlowChatState) => {
      latestState = state;
      if (frameId !== null) {
        return;
      }

      frameId = requestAnimationFrame(() => {
        frameId = null;
        if (!latestState || disposed) {
          return;
        }

        const nextSnapshot = collectTaskDetailSnapshot(
          latestState,
          sessionId,
          parentTaskToolIds,
          directSubagentSessionId,
        );

        if (!areSnapshotsEqual(previousSnapshot, nextSnapshot)) {
          previousSnapshot = nextSnapshot;
          setTaskSnapshot(nextSnapshot);
        }

        if (!isRunningStatus(nextSnapshot.toolItem?.status ?? initialToolItem?.status)) {
          unsubscribe?.();
          unsubscribe = null;
        }
      });
    };

    const hydrateSnapshot = () => {
      if (disposed) {
        return;
      }

      previousSnapshot = collectTaskDetailSnapshot(
        flowChatStore.getState(),
        sessionId,
        parentTaskToolIds,
        directSubagentSessionId,
      );

      setTaskSnapshot(current => areSnapshotsEqual(current, previousSnapshot) ? current : previousSnapshot);
      setIsSnapshotHydrated(true);

      // Completed/cancelled/error task details are static. Avoid keeping a global
      // FlowChatStore subscription alive, because streaming elsewhere would still
      // force this panel to scan the conversation tree on every store update.
      if (isRunningStatus(previousSnapshot.toolItem?.status ?? initialToolItem?.status)) {
        unsubscribe = flowChatStore.subscribe(updateTaskSnapshot);
      }
    };

    // Let the panel chrome paint before scanning and rendering a potentially
    // large subagent transcript.
    hydrationFrameId = requestAnimationFrame(() => {
      hydrationFrameId = requestAnimationFrame(hydrateSnapshot);
    });

    return () => {
      disposed = true;
      if (hydrationFrameId !== null) {
        cancelAnimationFrame(hydrationFrameId);
      }
      if (frameId !== null) {
        cancelAnimationFrame(frameId);
      }
      unsubscribe?.();
    };
  }, [sessionId, parentTaskToolIds, directSubagentSessionId, initialToolItem, initialToolItem?.status]);

  const toolItem = taskSnapshot.toolItem || initialToolItem;
  const status = toolItem?.status;
  const toolResult = toolItem?.toolResult;
  const isRunning = status === 'preparing' || status === 'streaming' || status === 'running';
  const isFailed = status === 'error' || toolResult?.success === false;
  const taskDurationMs = readTaskDurationMs(toolResult);
  const isCompleted = status === 'completed' && !isFailed;
  const subagentItems = useMemo(() => {
    if (isRunning) {
      return taskSnapshot.subagentItems;
    }

    return taskSnapshot.subagentItems.map((item) => {
      if (isTerminalStatus(item.status)) {
        return item;
      }

      if (item.type === 'text') {
        return {
          ...item,
          status: 'completed',
          isStreaming: false,
        } as FlowTextItem;
      }

      if (item.type === 'thinking') {
        return {
          ...item,
          status: 'completed',
          isStreaming: false,
          isCollapsed: true,
        } as FlowThinkingItem;
      }

      return item;
    });
  }, [isRunning, taskSnapshot.subagentItems]);
  const subagentSessionId = toolItem?.subagentSessionId || directSubagentSessionId
    || subagentItems.find((item) => item.subagentSessionId)?.subagentSessionId;
  const canStopSubagent = Boolean(isRunning && subagentSessionId);
  const visibleSubagentItems = useMemo(
    () => subagentItems.slice(0, visibleSubagentCount),
    [subagentItems, visibleSubagentCount],
  );
  const hasPendingSubagentRender = visibleSubagentCount < subagentItems.length;

  useEffect(() => {
    const total = subagentItems.length;

    if (total === 0) {
      setVisibleSubagentCount(0);
      return;
    }

    setVisibleSubagentCount(current => {
      if (current === 0) {
        return Math.min(TASK_DETAIL_INITIAL_RENDER_COUNT, total);
      }

      if (current > total) {
        return total;
      }

      if (isRunning && current >= total - 2) {
        return total;
      }

      return current;
    });
  }, [isRunning, subagentItems.length]);

  useEffect(() => {
    if (visibleSubagentCount >= subagentItems.length) {
      return;
    }

    const frameId = requestAnimationFrame(() => {
      setVisibleSubagentCount(current =>
        Math.min(current + TASK_DETAIL_RENDER_BATCH_SIZE, subagentItems.length)
      );
    });

    return () => {
      cancelAnimationFrame(frameId);
    };
  }, [visibleSubagentCount, subagentItems.length]);

  const getErrorMessage = () => {
    if (toolResult && 'error' in toolResult) {
      return toolResult.error as string;
    }
    return t('toolCards.taskTool.subAgentFailed');
  };

  // Detect user-initiated scroll to pause auto-scroll.
  useEffect(() => {
    const container = contentRef.current;
    if (!container) return;
    
    const handleWheel = (e: WheelEvent) => {
      if (e.deltaY < 0) {
        // User scrolls up, pause auto-scroll.
        shouldAutoScrollRef.current = false;
      } else if (e.deltaY > 0) {
        const { scrollTop, scrollHeight, clientHeight } = container;
        const distanceFromBottom = scrollHeight - scrollTop - clientHeight;
        if (distanceFromBottom < 100) {
          // Re-enable auto-scroll near the bottom.
          shouldAutoScrollRef.current = true;
        }
      }
    };
    
    container.addEventListener('wheel', handleWheel, { passive: true });
    return () => container.removeEventListener('wheel', handleWheel);
  }, []);

  // Auto-scroll during streaming output.
  useEffect(() => {
    const container = contentRef.current;
    if (!container || !isRunning) return;
    
    if (shouldAutoScrollRef.current) {
      requestAnimationFrame(() => {
        container.scrollTop = container.scrollHeight - container.clientHeight;
      });
    }
  }, [isRunning, subagentItems]);
  
  // Reset auto-scroll when a run starts.
  useEffect(() => {
    if (isRunning) {
      shouldAutoScrollRef.current = true;
    }
  }, [isRunning]);

  useEffect(() => {
    if (!isRunning) {
      setStoppingSubagent(false);
    }
  }, [isRunning]);

  // Open files in a split editor layout.
  const handleOpenInEditor = useCallback(async (filePath: string) => {
    if (!filePath) return;
    
    try {
      const { useAgentCanvasStore } = await import('@/app/components/panels/content-canvas/stores/canvasStore');
      const store = useAgentCanvasStore.getState();
      
      if (store.layout.splitMode === 'none') {
        store.setSplitMode('horizontal');
      }
      
      const fileName = filePath.split(/[/\\]/).pop() || filePath;
      
      store.addTab({
        type: 'code-editor',
        title: fileName,
        data: { filePath },
        metadata: { filePath }
      }, 'pinned', 'secondary');
      
    } catch (error) {
      log.error('Failed to open file', { filePath, error });
    }
  }, []);

  const handleStopSubagent = useCallback(async () => {
    if (!subagentSessionId || stoppingSubagent) {
      return;
    }

    setStoppingSubagent(true);
    setStopError(null);

    try {
      await agentAPI.cancelSession(subagentSessionId);
    } catch (error) {
      const message = error instanceof Error
        ? error.message
        : t('toolCards.taskDetailPanel.stopSubagentFailed', {
          defaultValue: 'Failed to stop this subagent.',
        });
      setStopError(message);
      log.error('Failed to stop subagent session', {
        subagentSessionId,
        error,
      });
      setStoppingSubagent(false);
    }
  }, [stoppingSubagent, subagentSessionId, t]);

  const renderSubagentItem = useCallback((item: FlowItem) => {
    switch (item.type) {
      case 'text':
        return (
          <FlowTextBlock
            key={item.id}
            textItem={item as FlowTextItem}
            replayStreamingOnMount={false}
          />
        );
      
      case 'thinking':
        return (
          <ModelThinkingDisplay 
            key={item.id}
            thinkingItem={item as FlowThinkingItem} 
          />
        );
      
      case 'tool':
        return (
          <FlowToolCard
            key={item.id}
            toolItem={item as FlowToolItem}
            sessionId={sessionId}
            onOpenInEditor={handleOpenInEditor}
          />
        );
      
      default:
        return null;
    }
  }, [sessionId, handleOpenInEditor]);

  if (!toolItem) {
    return (
      <div className="task-detail-panel task-detail-panel--empty">
        <div className="task-detail-panel__header">
          <span className="task-detail-panel__header-title">
            {t('toolCards.taskDetailPanel.untitled')}
          </span>
        </div>
        <div className="task-detail-panel__empty-content">
          {t('toolCards.taskDetailPanel.noData')}
        </div>
      </div>
    );
  }

  const rc = taskInput?.reviewerContext;

  return (
    <div className="task-detail-panel">
      <div className="task-detail-panel__header">
        <Split size={14} className="task-detail-panel__header-icon" />
        <span className="task-detail-panel__header-title">
          {taskInput?.description || t('toolCards.taskDetailPanel.untitled')}
        </span>
        {taskInput?.agentType && (
          <span className="task-detail-panel__header-badge">
            {rc
              ? tAgents(`reviewTeams.members.${rc.definitionKey}.funName`, {
                  defaultValue: rc.roleName,
                })
              : taskInput.agentType}
          </span>
        )}
        <ToolTimeoutIndicator
          startTime={toolItem?.startTime}
          isRunning={isRunning}
          timeoutMs={
            typeof toolItem?.toolCall?.input?.timeout_seconds === 'number' && toolItem.toolCall.input.timeout_seconds > 0
              ? toolItem.toolCall.input.timeout_seconds * 1000
              : undefined
          }
          showControls={true}
          subagentSessionId={subagentSessionId}
          completedDurationMs={taskDurationMs}
          completedStatus={isFailed ? 'error' : status === 'cancelled' ? 'cancelled' : isCompleted ? 'success' : undefined}
          completedFailureReason={isFailed ? getErrorMessage() : undefined}
        />
        {isRunning && (
          <span className="task-detail-panel__header-loading">
            <DotMatrixLoader size="small" />
          </span>
        )}
      </div>

      {isFailed && (
        <div className="task-detail-panel__error-banner">
          <AlertCircle size={14} className="task-detail-panel__error-banner-icon" />
          <span className="task-detail-panel__error-banner-text">{getErrorMessage()}</span>
        </div>
      )}

      <div
        ref={contentRef}
        className="task-detail-panel__content"
      >
        {rc ? (
          <details className="task-detail-panel__reviewer-section" open>
            <summary>{t('toolCards.taskDetailPanel.reviewerContextLabel')}</summary>
            <div className="task-detail-panel__reviewer-context">
              <div className="task-detail-panel__reviewer-role" style={{ color: rc.accentColor }}>
                {tAgents(`reviewTeams.members.${rc.definitionKey}.role`, {
                  defaultValue: rc.roleName,
                })}
              </div>
              <div className="task-detail-panel__reviewer-desc">
                {tAgents(`reviewTeams.members.${rc.definitionKey}.description`, {
                  defaultValue: rc.description,
                })}
              </div>
              <ul className="task-detail-panel__reviewer-responsibilities">
                {rc.responsibilities.map((resp, idx) => (
                  <li key={idx}>
                    {tAgents(`reviewTeams.members.${rc.definitionKey}.responsibilities.${idx}`, {
                      defaultValue: resp,
                    })}
                  </li>
                ))}
              </ul>
            </div>
          </details>
        ) : taskInput?.prompt && taskInput.prompt !== 'Not provided' && (
          <details className="task-detail-panel__prompt-section">
            <summary>{t('toolCards.taskDetailPanel.promptLabel')}</summary>
            <pre className="task-detail-panel__prompt-content">{taskInput.prompt}</pre>
          </details>
        )}

        {canStopSubagent && (
          <div className="task-detail-panel__actions">
            <Button
              variant="secondary"
              size="small"
              onClick={() => void handleStopSubagent()}
              disabled={stoppingSubagent}
            >
              <Square size={12} style={{ marginRight: 6 }} />
              {stoppingSubagent
                ? t('toolCards.taskDetailPanel.stoppingSubagent', {
                  defaultValue: 'Stopping subagent...',
                })
                : t('toolCards.taskDetailPanel.stopSubagent', {
                  defaultValue: 'Stop subagent',
                })}
            </Button>
            <span className="task-detail-panel__actions-hint">
              {t('toolCards.taskDetailPanel.stopSubagentHint', {
                defaultValue:
                  'Cancels only this reviewer/subagent. The parent review can keep going and still produce a summary.',
              })}
            </span>
          </div>
        )}

        {stopError && (
          <div className="task-detail-panel__error">
            <AlertCircle size={14} />
            <span>{stopError}</span>
          </div>
        )}

        {subagentItems.length > 0 && (
          <div className="task-detail-panel__execution">
            {visibleSubagentItems.map(item => renderSubagentItem(item))}
          </div>
        )}

        {hasPendingSubagentRender && (
          <div className="task-detail-panel__loading task-detail-panel__loading--inline">
            <DotMatrixLoader size="small" />
            <span>
              {t('toolCards.taskDetailPanel.loadingMore', {
                defaultValue: 'Loading more output...',
              })}
            </span>
          </div>
        )}

        {((isRunning || !isSnapshotHydrated) && subagentItems.length === 0) && (
          <div className="task-detail-panel__loading">
            <DotMatrixLoader size="medium" />
            <span>
              {isSnapshotHydrated
                ? t('toolCards.taskDetailPanel.status.running')
                : t('toolCards.taskDetailPanel.loading', {
                  defaultValue: 'Loading task details...',
                })}
            </span>
          </div>
        )}
      </div>
    </div>
  );
};

export default TaskDetailPanel;
