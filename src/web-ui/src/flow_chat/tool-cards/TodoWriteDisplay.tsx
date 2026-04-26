/**
 * Tool card for TodoWrite with a dot-track progress view.
 */

import React, { useState, useMemo, useCallback } from 'react';
import { ListTodo, CheckCircle2, Circle, XCircle, ChevronDown, ChevronUp } from 'lucide-react';
import { TaskRunningIndicator } from '../../component-library';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { useDialogTurnTodos } from '../hooks/useDialogTurnTodos';
import './TodoWriteDisplay.scss';

export const TodoWriteDisplay: React.FC<ToolCardProps> = ({
  toolItem,
  config,
  turnId,
  sessionId,
}) => {
  const { t } = useTranslation('flow-chat');
  const { status, toolResult, partialParams, isParamsStreaming } = toolItem;

  const [expandedState, setExpandedState] = useState<boolean | null>(null);
  const toolId = toolItem.id;
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });

  const turnTodos = useDialogTurnTodos(sessionId, turnId);

  const todosToDisplay = useMemo(() => {
    if (isParamsStreaming && partialParams?.todos && Array.isArray(partialParams.todos)) {
      return partialParams.todos;
    }
    if (turnTodos.length > 0) {
      return turnTodos;
    }
    if (toolResult?.result?.todos && Array.isArray(toolResult.result.todos)) {
      return toolResult.result.todos;
    }
    return [];
  }, [partialParams, toolResult, isParamsStreaming, turnTodos]);

  const taskStats = useMemo(() => {
    if (todosToDisplay.length === 0) {
      return { completed: 0, total: 0 };
    }
    const completed = todosToDisplay.filter((t: any) => t.status === 'completed').length;
    return { completed, total: todosToDisplay.length };
  }, [todosToDisplay]);

  const inProgressTasks = useMemo(() => {
    return todosToDisplay.filter((t: any) => t.status === 'in_progress');
  }, [todosToDisplay]);

  const isAllCompleted = useMemo(() => {
    return todosToDisplay.length > 0 && taskStats.completed === taskStats.total;
  }, [todosToDisplay.length, taskStats]);

  const isExpanded = useMemo(() => {
    if (expandedState !== null) return expandedState;
    return inProgressTasks.length === 0 && todosToDisplay.length > 0 && !isAllCompleted;
  }, [expandedState, inProgressTasks.length, todosToDisplay.length, isAllCompleted]);

  const isLoading = status === 'preparing' || status === 'streaming' || status === 'running';
  
  const displayMode = config?.displayMode || 'compact';

  const renderTrackDot = (todo: any, index: number) => {
    const statusClass = `track-dot--${todo.status}`;
    return (
      <div
        key={todo.id || index}
        className={`track-dot ${statusClass}`}
      />
    );
  };

  const renderTodoItem = (todo: any, index: number) => (
    <div key={todo.id || index} className={`todo-item status-${todo.status}`}>
      <div className="todo-item-left">
        {todo.status === 'completed' && (
          <CheckCircle2 size={12} className="todo-status-icon todo-status-icon--completed" />
        )}
        {todo.status === 'in_progress' && (
          <TaskRunningIndicator size="xs" className="todo-status-icon todo-status-icon--in-progress" />
        )}
        {todo.status === 'pending' && (
          <Circle size={12} className="todo-status-icon todo-status-icon--pending" />
        )}
        {todo.status === 'cancelled' && (
          <XCircle size={12} className="todo-status-icon todo-status-icon--cancelled" />
        )}
        <span className="todo-content">{todo.content}</span>
      </div>
    </div>
  );

  const currentDisplayTask = useMemo(() => {
    if (inProgressTasks.length > 0) {
      return inProgressTasks[0];
    }
    return null;
  }, [inProgressTasks]);

  const handleToggleExpanded = useCallback(() => {
    if (todosToDisplay.length === 0) {
      return;
    }

    applyExpandedState(isExpanded, !isExpanded, (nextExpanded) => {
      setExpandedState(nextExpanded);
    });
  }, [applyExpandedState, isExpanded, todosToDisplay.length]);

  if (displayMode === 'compact') {
    return (
      <div className={`tool-display-compact todo-write-compact status-${status}`}>
        <span className="tool-icon">
          {isLoading ? (
            <TaskRunningIndicator size="sm" className="todo-compact-loading-icon" />
          ) : (
            <ListTodo size={14} />
          )}
        </span>
        {todosToDisplay.length > 0 && (
          <>
            <span className="todo-count">{t('toolCards.todoWrite.tasksCount', { count: todosToDisplay.length })}</span>
            <span className="todo-progress">{t('toolCards.todoWrite.progress', { completed: taskStats.completed, total: taskStats.total })}</span>
          </>
        )}
      </div>
    );
  }

  return (
    <div
      ref={cardRootRef}
      data-tool-card-id={toolId ?? ''}
      className={`flow-tool-card todo-write-card mode-${displayMode} status-${status} ${isAllCompleted ? 'all-completed' : ''}`}
    >
      <div
        className={`tool-card-header ${todosToDisplay.length > 0 ? 'clickable' : ''}`}
        onClick={todosToDisplay.length > 0 ? handleToggleExpanded : undefined}
      >
        <div className="todo-header-center">
          {isAllCompleted ? (
            <>
              <CheckCircle2 size={11} className="all-completed-icon" />
              <span className="all-completed-label">{t('toolCards.todoWrite.allCompleted')}</span>
            </>
          ) : (
            <>
              {todosToDisplay.length > 0 && (
                <div className="track-dots">
                  {todosToDisplay.map((todo: any, idx: number) => renderTrackDot(todo, idx))}
                </div>
              )}

              {!isExpanded && todosToDisplay.length > 0 && currentDisplayTask && (
                <div className={`current-task-inline current-task-inline--${currentDisplayTask.status}`}>
                  <span className="inline-task-text">{currentDisplayTask.content}</span>
                  {inProgressTasks.length > 1 && (
                    <span className="inline-task-more">+{inProgressTasks.length - 1}</span>
                  )}
                </div>
              )}

              {todosToDisplay.length > 0 && (
                <div className="todo-track">
                  <span className="track-stats">{taskStats.completed}/{taskStats.total}</span>
                  {isExpanded ? <ChevronUp size={12} className="expand-icon" /> : <ChevronDown size={12} className="expand-icon" />}
                </div>
              )}
            </>
          )}
        </div>
      </div>

      {isExpanded && todosToDisplay.length > 0 && (
        <div className="todo-full-list">
          {todosToDisplay.map((todo: any, idx: number) => renderTodoItem(todo, idx))}
        </div>
      )}
    </div>
  );
};
