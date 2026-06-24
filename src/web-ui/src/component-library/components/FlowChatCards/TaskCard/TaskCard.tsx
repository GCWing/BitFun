/**
 * TaskCard - AI task tool card component
 * Used to display AI task execution process and results
 */

import React, { useState } from 'react';
import { Bot, ChevronDown, ChevronUp } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { BaseToolCard, BaseToolCardProps } from '../BaseToolCard';
import './TaskCard.scss';

export interface TaskCardProps extends Omit<BaseToolCardProps, 'toolName' | 'displayName'> {
  taskType?: string;
  taskDescription?: string;
  taskResult?: string;
}

export const TaskCard: React.FC<TaskCardProps> = ({
  taskType,
  taskDescription,
  taskResult,
  input,
  result,
  status = 'pending',
  displayMode = 'compact',
  ...baseProps
}) => {
  const { t } = useI18n('components');
  const [isExpanded, setIsExpanded] = useState(false);

  const resolvedTaskType = taskType || input?.task_type || input?.type || t('flowChatCards.taskCard.defaultType');
  const resolvedDescription = taskDescription || input?.description || input?.task || input?.prompt || t('flowChatCards.taskCard.unspecifiedTask');
  const resolvedResult = taskResult || result?.result || result?.output || '';

  if (displayMode === 'compact') {
    return (
      <div className={`task-card task-card--compact task-card--${status}`}>
        <Bot className="task-card__icon" size={14} />
        <span className="task-card__action">{resolvedTaskType}:</span>
        <span className="task-card__description" title={resolvedDescription}>
          {resolvedDescription}
        </span>
      </div>
    );
  }

  return (
    <BaseToolCard
      toolName="Task"
      displayName={t('flowChatCards.taskCard.title')}
      icon={<Bot size={18} />}
      description={t('flowChatCards.taskCard.description')}
      status={status}
      displayMode={displayMode}
      input={input}
      result={result}
      primaryColor="var(--color-purple-600)"
      className="task-card"
      {...baseProps}
    >
      <div className="task-card__info">
        <div className="task-card__info-row">
          <span className="task-card__label">{t('flowChatCards.taskCard.taskType')}:</span>
          <span className="task-card__value">{resolvedTaskType}</span>
        </div>
        <div className="task-card__info-row">
          <span className="task-card__label">{t('flowChatCards.taskCard.taskDesc')}:</span>
          <span className="task-card__value">{resolvedDescription}</span>
        </div>
      </div>

      {status === 'completed' && resolvedResult && (
        <div className="task-card__result-section">
          <button
            className="task-card__result-header"
            onClick={() => setIsExpanded(!isExpanded)}
          >
            <Bot size={14} />
            <span>{t('flowChatCards.taskCard.taskResult')}</span>
            {isExpanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
          </button>
          
          {isExpanded && (
            <div className="task-card__result-content">
              <pre className="task-card__result-text">{resolvedResult}</pre>
            </div>
          )}
        </div>
      )}

      {(status === 'running' || status === 'streaming') && (
        <div className="task-card__executing">
          <Bot className="task-card__executing-icon" size={14} />
          <span>{t('flowChatCards.taskCard.processing')}</span>
        </div>
      )}
    </BaseToolCard>
  );
};
