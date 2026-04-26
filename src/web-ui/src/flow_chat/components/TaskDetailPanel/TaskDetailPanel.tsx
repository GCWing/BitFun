/**
 * TaskDetailPanel - Subtask detail panel.
 * Minimal layout to match the FlowChat background.
 */

import React, { useEffect, useState, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { 
  Split,
  AlertCircle,
  Square
} from 'lucide-react';
import type { FlowToolItem, FlowTextItem, FlowThinkingItem, FlowItem } from '../../types/flow-chat';
import { FlowChatStore } from '../../store/FlowChatStore';
import { FlowTextBlock } from '../FlowTextBlock';
import { FlowToolCard } from '../FlowToolCard';
import { ModelThinkingDisplay } from '../../tool-cards/ModelThinkingDisplay';
import { ToolTimeoutIndicator } from '../../tool-cards/ToolTimeoutIndicator';
import { Button, Tooltip, DotMatrixLoader } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import './TaskDetailPanel.scss';

const log = createLogger('TaskDetailPanel');

export interface TaskDetailData {
  toolItem: FlowToolItem;
  taskInput: {
    description: string;
    prompt: string;
    agentType: string;
  } | null;
  sessionId?: string;
}

export interface TaskDetailPanelProps {
  data: TaskDetailData;
}

export const TaskDetailPanel: React.FC<TaskDetailPanelProps> = ({ data }) => {
  const { t } = useTranslation('flow-chat');
  const { toolItem, taskInput, sessionId } = data || {};
  const status = toolItem?.status;
  const toolResult = toolItem?.toolResult;
  const parentTaskToolId = toolItem?.id;
  
  const [subagentItems, setSubagentItems] = useState<FlowItem[]>([]);
  const [stoppingSubagent, setStoppingSubagent] = useState(false);
  const [stopError, setStopError] = useState<string | null>(null);
  
  const contentRef = useRef<HTMLDivElement>(null);
  // Track auto-scroll; disable when the user scrolls up.
  const shouldAutoScrollRef = useRef(true);

  // Collect subagent items associated with this task.
  useEffect(() => {
    if (!sessionId || !parentTaskToolId) return;
    
    const flowChatStore = FlowChatStore.getInstance();
    
    const updateSubagentItems = () => {
      const state = flowChatStore.getState();
      const session = state.sessions.get(sessionId);
      
      if (!session) return;
      
      // Scan dialog turns and rounds to find matching task items.
      const items: FlowItem[] = [];
      
      for (const turn of session.dialogTurns) {
        for (const round of turn.modelRounds) {
          for (const item of round.items) {
            const itemAny = item as any;
            if (itemAny.isSubagentItem && itemAny.parentTaskToolId === parentTaskToolId) {
              items.push(item);
            }
          }
        }
      }
      
      setSubagentItems(items);
    };
    
    updateSubagentItems();
    
    const unsubscribe = flowChatStore.subscribe(updateSubagentItems);
    
    return () => {
      unsubscribe();
    };
  }, [sessionId, parentTaskToolId]);

  const isRunning = status === 'preparing' || status === 'streaming' || status === 'running';
  const isFailed = status === 'error';
  const isCompleted = status === 'completed' && !isFailed;
  const subagentSessionId = toolItem?.subagentSessionId
    || subagentItems.find((item) => item.subagentSessionId)?.subagentSessionId;
  const canStopSubagent = Boolean(isRunning && subagentSessionId);

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

  return (
    <div className="task-detail-panel">
      <div className="task-detail-panel__header">
        <Split size={14} className="task-detail-panel__header-icon" />
        <span className="task-detail-panel__header-title">
          {taskInput?.description || t('toolCards.taskDetailPanel.untitled')}
        </span>
        {taskInput?.agentType && (
          <span className="task-detail-panel__header-badge">
            {taskInput.agentType}
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
          completedDurationMs={
            isCompleted && toolResult?.result?.duration
              ? toolResult.result.duration
              : undefined
          }
        />
        {isRunning && (
          <span className="task-detail-panel__header-loading">
            <DotMatrixLoader size="small" />
          </span>
        )}
        {isFailed && (
          <Tooltip content={getErrorMessage()} placement="bottom">
            <AlertCircle size={14} className="task-detail-panel__header-failed" />
          </Tooltip>
        )}
      </div>

      <div 
        ref={contentRef}
        className="task-detail-panel__content"
      >
        {taskInput?.prompt && taskInput.prompt !== 'Not provided' && (
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
            {subagentItems.map(item => renderSubagentItem(item))}
          </div>
        )}

        {isRunning && subagentItems.length === 0 && (
          <div className="task-detail-panel__loading">
            <DotMatrixLoader size="medium" />
            <span>{t('toolCards.taskDetailPanel.status.running')}</span>
          </div>
        )}
      </div>
    </div>
  );
};

export default TaskDetailPanel;
