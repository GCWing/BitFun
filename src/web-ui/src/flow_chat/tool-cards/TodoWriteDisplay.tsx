/**
 * Tool card for TodoWrite.
 */

import React, { useState, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { useDialogTurnTodos } from '../hooks/useDialogTurnTodos';
import {
  TodoToolCard as TodoToolCardView,
  type TodoToolCardItemStatus,
} from '@openbitfun/ui/flow-chat';
import { createTodoRenderItems, type TodoLike } from './todoRenderItems';

function normalizeTodoStatus(status: TodoLike['status']): TodoToolCardItemStatus {
  if (status === 'completed' || status === 'in_progress' || status === 'cancelled') {
    return status;
  }
  return 'pending';
}

export const TodoWriteDisplay: React.FC<ToolCardProps> = ({
  toolItem,
  config,
  turnId,
  sessionId,
}) => {
  const { t } = useTranslation('flow-chat');
  const { status, toolResult, partialParams, isParamsStreaming } = toolItem;

  const [isExpanded, setIsExpanded] = useState(false);
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

  const todoRenderItems = useMemo(
    () => createTodoRenderItems(todosToDisplay),
    [todosToDisplay],
  );

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

  const isLoading = status === 'preparing' || status === 'streaming' || status === 'running';

  const displayMode = config?.displayMode || 'compact';

  const currentDisplayTask = useMemo(() => {
    if (inProgressTasks.length > 0) return inProgressTasks[0];
    if (
      todosToDisplay.length > 0 &&
      todosToDisplay.every((todo) => todo.status === 'pending')
    ) {
      return todosToDisplay[0];
    }
    return null;
  }, [inProgressTasks, todosToDisplay]);

  const handleToggleExpanded = useCallback(() => {
    if (todosToDisplay.length === 0) return;
    applyExpandedState(isExpanded, !isExpanded, setIsExpanded);
  }, [applyExpandedState, isExpanded, todosToDisplay.length]);

  /* ---------- Compact (single-line) display mode ---------- */

  if (displayMode === 'compact') {
    return (
      <TodoToolCardView
        allCompleted={isAllCompleted}
        compactCountLabel={todosToDisplay.length > 0
          ? t('toolCards.todoWrite.tasksCount', { count: todosToDisplay.length })
          : undefined}
        compactProgressLabel={todosToDisplay.length > 0 ? t('toolCards.todoWrite.progress', {
          completed: taskStats.completed,
          total: taskStats.total,
        }) : undefined}
        completedCount={taskStats.completed}
        items={[]}
        loading={isLoading}
        mode="compact"
        status={status}
        title={t('toolCards.todoWrite.tasks')}
        totalCount={taskStats.total}
      />
    );
  }

  /* ---------- Standard display mode ---------- */

  const hasTodos = todosToDisplay.length > 0;
  const tasksLabel = t('toolCards.todoWrite.tasks');
  const headerContent = (() => {
    if (!hasTodos && isLoading) {
      return `${tasksLabel}…`;
    }
    if (isAllCompleted) {
      return t('toolCards.todoWrite.allCompleted');
    }
    if (currentDisplayTask) {
      return `${currentDisplayTask.content ?? ''}${inProgressTasks.length > 1 ? ` +${inProgressTasks.length - 1}` : ''}`;
    }
    if (hasTodos) {
      return t('toolCards.todoWrite.tasksCount', { count: todosToDisplay.length });
    }
    return null;
  })();

  return (
    <div data-openbitfun-adapter="todo-write"
      ref={cardRootRef}
      data-tool-card-id={toolId ?? ''}
    >
      <TodoToolCardView
        status={status}
        isExpanded={isExpanded && hasTodos}
        onToggle={hasTodos ? handleToggleExpanded : undefined}
        allCompleted={isAllCompleted}
        completedCount={taskStats.completed}
        items={todoRenderItems.map(({ todo, key }) => ({
          content: todo.content,
          key,
          status: normalizeTodoStatus(todo.status),
        }))}
        loading={isLoading}
        mode="standard"
        summary={headerContent}
        title={tasksLabel}
        totalCount={taskStats.total}
      />
    </div>
  );
};
