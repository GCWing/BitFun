/**
 * Sticky task indicator.
 * Shows at the top of the message list when the user has scrolled past a Task
 * tool card, indicating which Task they are currently reading.
 * Clicking the indicator scrolls the Task to the viewport top.
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
import { Split, ChevronUp } from 'lucide-react';
import { Tooltip } from '@/component-library';
import type { VisibleTaskInfo } from '../hooks/useVisibleTaskInfo';
import './StickyTaskIndicator.scss';

interface StickyTaskIndicatorProps {
  visible: boolean;
  taskInfo: VisibleTaskInfo | null;
  onClick: () => void;
}

export const StickyTaskIndicator: React.FC<StickyTaskIndicatorProps> = ({
  visible,
  taskInfo,
  onClick,
}) => {
  const { t } = useTranslation('flow-chat');

  const label = taskInfo?.label || t('toolCards.taskTool.defaultAgentKind');
  const tooltip = t('stickyTaskIndicator.tooltip');

  return (
    <div data-bf-component="sticky-task-indicator" data-bf-part="root" data-bf-state={visible ? 'visible' : ''}
      className={`sticky-task-indicator ${visible ? 'sticky-task-indicator--visible' : ''}`}
      aria-hidden={!visible}
    >
      <div data-bf-component="sticky-task-indicator" data-bf-part="gradient" className="sticky-task-indicator__gradient" />
      <div data-bf-component="sticky-task-indicator" data-bf-part="content" className="sticky-task-indicator__content">
        <Tooltip content={tooltip} placement="bottom">
          <button
            data-bf-component="sticky-task-indicator"
            data-bf-part="button"
            className="sticky-task-indicator__btn"
            onClick={onClick}
            aria-label={tooltip}
            tabIndex={visible ? 0 : -1}
          >
            <Split size={12} data-bf-component="sticky-task-indicator" data-bf-part="icon" className="sticky-task-indicator__icon" />
            <span data-bf-component="sticky-task-indicator" data-bf-part="label" className="sticky-task-indicator__label" title={label}>
              {label}
            </span>
            <ChevronUp size={12} data-bf-component="sticky-task-indicator" data-bf-part="arrow" className="sticky-task-indicator__arrow" />
          </button>
        </Tooltip>
      </div>
    </div>
  );
};

StickyTaskIndicator.displayName = 'StickyTaskIndicator';
