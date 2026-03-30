/**
 * TodoCard - todo management tool card component
 * Used to show task lists and manage status
 */

import React from 'react';
import { CheckSquare, Square, Circle } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { BaseToolCard, BaseToolCardProps } from '../BaseToolCard';
import './TodoCard.scss';

export interface TodoItem {
  id: string;
  content: string;
  status: 'pending' | 'in_progress' | 'completed';
}

export interface TodoCardProps extends Omit<BaseToolCardProps, 'toolName' | 'displayName'> {
  todos?: TodoItem[];
  action?: 'create' | 'update' | 'list';
}

export const TodoCard: React.FC<TodoCardProps> = ({
  todos,
  input,
  result,
  status = 'pending',
  displayMode = 'compact',
  ...baseProps
}) => {
  const { t } = useI18n('components');
  const resolvedTodos = todos || result?.todos || input?.todos || [];

  const stats = {
    total: resolvedTodos.length,
    completed: resolvedTodos.filter((t: TodoItem) => t.status === 'completed').length,
    inProgress: resolvedTodos.filter((t: TodoItem) => t.status === 'in_progress').length,
    pending: resolvedTodos.filter((t: TodoItem) => t.status === 'pending').length
  };

  const getStatusIcon = (todoStatus: string) => {
    switch (todoStatus) {
      case 'completed':
        return <CheckSquare size={12} className="todo-card__status-icon todo-card__status-icon--completed" />;
      case 'in_progress':
        return <Circle size={12} className="todo-card__status-icon todo-card__status-icon--in-progress" />;
      default:
        return <Square size={12} className="todo-card__status-icon todo-card__status-icon--pending" />;
    }
  };

  if (displayMode === 'compact') {
    return (
      <div className={`todo-card todo-card--compact todo-card--${status}`}>
        <CheckSquare className="todo-card__icon" size={14} />
        <span className="todo-card__action">{t('flowChatCards.todoCard.title')}:</span>
        <span className="todo-card__stats">
          {t('flowChatCards.todoCard.taskCount', { total: stats.total, completed: stats.completed })}
        </span>
      </div>
    );
  }

  return (
    <BaseToolCard
      toolName="TodoWrite"
      displayName={t('flowChatCards.todoCard.title')}
      icon={<CheckSquare size={16} />}
      description={t('flowChatCards.todoCard.description')}
      status={status}
      displayMode={displayMode}
      input={input}
      result={result}
      primaryColor="#0d9488"
      className="todo-card"
      {...baseProps}
    >
      {resolvedTodos.length > 0 && (
        <div className="todo-card__stats-box">
          <div className="todo-card__stat-item">
            <span className="todo-card__stat-value">{stats.total}</span>
            <span className="todo-card__stat-label">{t('flowChatCards.todoCard.total')}</span>
          </div>
          <div className="todo-card__stat-item">
            <span className="todo-card__stat-value todo-card__stat-value--in-progress">{stats.inProgress}</span>
            <span className="todo-card__stat-label">{t('flowChatCards.todoCard.inProgress')}</span>
          </div>
          <div className="todo-card__stat-item">
            <span className="todo-card__stat-value todo-card__stat-value--completed">{stats.completed}</span>
            <span className="todo-card__stat-label">{t('flowChatCards.todoCard.completed')}</span>
          </div>
        </div>
      )}

      {resolvedTodos.length > 0 && (
        <div className="todo-card__list">
          {resolvedTodos.map((todo: TodoItem) => (
            <div key={todo.id} className={`todo-card__item todo-card__item--${todo.status}`}>
              {getStatusIcon(todo.status)}
              <span className="todo-card__item-content">{todo.content}</span>
            </div>
          ))}
        </div>
      )}

      {resolvedTodos.length === 0 && status === 'completed' && (
        <div className="todo-card__empty">
          {t('flowChatCards.todoCard.noTasks')}
        </div>
      )}
    </BaseToolCard>
  );
};
