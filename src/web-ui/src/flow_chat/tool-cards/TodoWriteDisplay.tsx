/**
 * Tool card for TodoWrite.
 */

import React, { useState, useMemo, useCallback } from 'react';
import { ListTodo, CheckCircle2, Circle, XCircle } from 'lucide-react';
import { TaskRunningIndicator } from '../../component-library';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { useDialogTurnTodos } from '../hooks/useDialogTurnTodos';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { ToolCardStatusSlot } from './ToolCardStatusSlot';
import './TodoWriteDisplay.scss';

type TodoStatus = 'completed' | 'in_progress' | 'pending' | 'cancelled';

interface TodoLike {
  id?: string | number;
  content?: string;
  status?: TodoStatus | string;
}

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

  const todosToDisplay: TodoLike[] = useMemo(() => {
    if (isParamsStreaming && partialParams?.todos && Array.isArray(partialParams.todos)) {
      return partialParams.todos as TodoLike[];
    }
    if (turnTodos.length > 0) {
      return turnTodos as TodoLike[];
    }
    if (toolResult?.result?.todos && Array.isArray(toolResult.result.todos)) {
      return toolResult.result.todos as TodoLike[];
    }
    return [];
  }, [partialParams, toolResult, isParamsStreaming, turnTodos]);

  const taskStats = useMemo(() => {
    if (todosToDisplay.length === 0) return { completed: 0, total: 0 };
    const completed = todosToDisplay.filter((td) => td.status === 'completed').length;
    return { completed, total: todosToDisplay.length };
  }, [todosToDisplay]);

  const inProgressTasks = useMemo(
    () => todosToDisplay.filter((td) => td.status === 'in_progress'),
    [todosToDisplay],
  );

  const isAllCompleted = useMemo(
    () => todosToDisplay.length > 0 && taskStats.completed === taskStats.total,
    [todosToDisplay.length, taskStats],
  );

  const statusSlotProps =
    status === 'error' || status === 'cancelled'
      ? { status, defaultIcon: 'status' as const }
      : isAllCompleted
        ? { status: 'completed' as const, defaultIcon: 'status' as const }
        : { status, defaultIcon: 'tool' as const };

  const isExpanded = useMemo(() => {
    if (expandedState !== null) return expandedState;
    return inProgressTasks.length === 0 && todosToDisplay.length > 0 && !isAllCompleted;
  }, [expandedState, inProgressTasks.length, todosToDisplay.length, isAllCompleted]);

  const isLoading = status === 'preparing' || status === 'streaming' || status === 'running';

  const displayMode = config?.displayMode || 'compact';

  const currentDisplayTask = useMemo(() => {
    if (inProgressTasks.length > 0) return inProgressTasks[0];
    return null;
  }, [inProgressTasks]);

  const handleToggleExpanded = useCallback(() => {
    if (todosToDisplay.length === 0) return;
    applyExpandedState(isExpanded, !isExpanded, (nextExpanded) => {
      setExpandedState(nextExpanded);
    });
  }, [applyExpandedState, isExpanded, todosToDisplay.length]);

  const renderTodoItem = (todo: TodoLike, index: number) => (
    <div key={todo.id ?? index} className={`todo-item status-${todo.status}`}>
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

  /* ---------- Compact (single-line) display mode ---------- */

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
            <span className="todo-count">
              {t('toolCards.todoWrite.tasksCount', { count: todosToDisplay.length })}
            </span>
            <span className="todo-progress">
              {t('toolCards.todoWrite.progress', {
                completed: taskStats.completed,
                total: taskStats.total,
              })}
            </span>
          </>
        )}
      </div>
    );
  }

  /* ---------- Standard display mode ---------- */

  const hasTodos = todosToDisplay.length > 0;
  const headerExpanded = isExpanded && hasTodos;
  const tasksLabel = t('toolCards.todoWrite.tasks');

  const statsSuffix =
    hasTodos && taskStats.total > 0 ? (
      <span className="todo-stats todo-stats--suffix">
        {' '}
        ({taskStats.completed}/{taskStats.total})
      </span>
    ) : null;

  const headerActionCollapsed =
    hasTodos ? (
      <span className="todo-header-action-cluster">
        <span className="todo-header-tasks-label">{tasksLabel}</span>
        <span className="todo-stats">({taskStats.completed}/{taskStats.total})</span>
      </span>
    ) : (
      tasksLabel
    );

  const headerContent = (() => {
    if (!hasTodos && isLoading) {
      return (
        <span className="todo-header-content todo-header-content--muted">{tasksLabel}…</span>
      );
    }
    if (isAllCompleted) {
      return (
        <span className="todo-header-content todo-header-content--success">
          {t('toolCards.todoWrite.allCompleted')}
          {headerExpanded ? statsSuffix : null}
        </span>
      );
    }
    if (currentDisplayTask) {
      return (
        <span className="todo-header-content">
          <span className="todo-header-current">{currentDisplayTask.content}</span>
          {inProgressTasks.length > 1 && (
            <span className="todo-header-more">+{inProgressTasks.length - 1}</span>
          )}
          {headerExpanded ? statsSuffix : null}
        </span>
      );
    }
    if (hasTodos) {
      return (
        <span className="todo-header-content todo-header-content--muted">
          {t('toolCards.todoWrite.tasksCount', { count: todosToDisplay.length })}
          {headerExpanded ? statsSuffix : null}
        </span>
      );
    }
    return null;
  })();

  const headerAction = headerExpanded ? undefined : headerActionCollapsed;

  const expandedContent = hasTodos ? (
    <div className="todo-expanded-body">
      <div className="todo-full-list">
        {todosToDisplay.map((todo, idx) => renderTodoItem(todo, idx))}
      </div>
    </div>
  ) : undefined;

  return (
    <div
      ref={cardRootRef}
      data-tool-card-id={toolId ?? ''}
      className={`todo-write-host mode-${displayMode} status-${status}`}
    >
      <CompactToolCard
        status={status}
        isExpanded={isExpanded && hasTodos}
        onClick={hasTodos ? handleToggleExpanded : undefined}
        clickable={hasTodos}
        className="todo-write-card"
        header={
          <CompactToolCardHeader
            icon={
              <ToolCardStatusSlot
                status={statusSlotProps.status}
                toolIcon={<ListTodo size={16} className="todo-card-icon" />}
                defaultIcon={statusSlotProps.defaultIcon}
              />
            }
            action={headerAction}
            content={headerContent}
            expandable={hasTodos}
            isExpanded={isExpanded && hasTodos}
          />
        }
        expandedContent={expandedContent}
      />
    </div>
  );
};
