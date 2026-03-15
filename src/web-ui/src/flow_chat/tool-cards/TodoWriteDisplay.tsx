/**
 * Tool card for TodoWrite with a dot-track progress view.
 */

import React, { useState, useMemo } from 'react';
import { Loader2, ListTodo, CheckCircle2, Circle, XCircle, PlayCircle, ChevronDown, ChevronUp } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import './TodoWriteDisplay.scss';

export const TodoWriteDisplay: React.FC<ToolCardProps> = ({
  toolItem,
  config,
}) => {
  const { t } = useTranslation('flow-chat');
  const { status, toolResult, partialParams, isParamsStreaming } = toolItem;
  
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null);
  const [expandedState, setExpandedState] = useState<boolean | null>(null);

  const todosToDisplay = useMemo(() => {
    if (isParamsStreaming && partialParams?.todos && Array.isArray(partialParams.todos)) {
      return partialParams.todos;
    }
    if (toolResult?.result?.todos && Array.isArray(toolResult.result.todos)) {
      return toolResult.result.todos;
    }
    return [];
  }, [partialParams, toolResult, isParamsStreaming]);

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

  const hoveredTask = useMemo(() => {
    if (hoveredIndex !== null && todosToDisplay[hoveredIndex]) {
      return todosToDisplay[hoveredIndex];
    }
    return null;
  }, [hoveredIndex, todosToDisplay]);

  const isAllCompleted = useMemo(() => {
    return todosToDisplay.length > 0 && taskStats.completed === taskStats.total;
  }, [todosToDisplay.length, taskStats]);

  const isExpanded = useMemo(() => {
    if (expandedState !== null) return expandedState;
    return inProgressTasks.length === 0 && todosToDisplay.length > 0 && !isAllCompleted;
  }, [expandedState, inProgressTasks.length, todosToDisplay.length, isAllCompleted]);

  const isLoading = status === 'preparing' || status === 'streaming' || status === 'running';
  
  const displayMode = config?.displayMode || 'compact';

  if (displayMode === 'compact') {
    return (
      <div className={`tool-display-compact todo-write-compact status-${status}`}>
        <span className="tool-icon">
          {isLoading ? (
            <Loader2 className="animate-spin" size={14} />
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

  const renderTrackDot = (todo: any, index: number) => {
    const statusClass = `track-dot--${todo.status}`;
    const isHovered = hoveredIndex === index;
    return (
      <div 
        key={todo.id || index} 
        className={`track-dot ${statusClass} ${isHovered ? 'track-dot--hovered' : ''}`}
        onMouseEnter={() => setHoveredIndex(index)}
        onMouseLeave={() => setHoveredIndex(null)}
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
          <PlayCircle size={12} className="todo-status-icon todo-status-icon--in-progress" />
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
    if (hoveredTask) {
      return hoveredTask;
    }
    if (inProgressTasks.length > 0) {
      return inProgressTasks[0];
    }
    return null;
  }, [hoveredTask, inProgressTasks]);

  return (
    <div className={`flow-tool-card todo-write-card mode-${displayMode} status-${status} ${isAllCompleted ? 'all-completed' : ''}`}>
      <div
        className={`tool-card-header ${todosToDisplay.length > 0 ? 'clickable' : ''}`}
        onClick={todosToDisplay.length > 0 ? () => setExpandedState(!isExpanded) : undefined}
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
                  {inProgressTasks.length > 1 && !hoveredTask && (
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
