import React from 'react';
import './TaskRunningIndicator.scss';

export type TaskRunningIndicatorSize = 'xs' | 'sm' | 'md' | 'lg';

export interface TaskRunningIndicatorProps {
  /** Size relative to parent font-size */
  size?: TaskRunningIndicatorSize;
  /** Custom class name */
  className?: string;
}

export const TaskRunningIndicator: React.FC<TaskRunningIndicatorProps> = ({
  size = 'md',
  className = '',
}) => {
  return (
    <span
      className={`task-running-indicator task-running-indicator--${size} ${className}`}
      aria-hidden="true"
      role="presentation"
    >
      <span className="task-running-indicator__bar" />
      <span className="task-running-indicator__bar" />
      <span className="task-running-indicator__bar" />
    </span>
  );
};

TaskRunningIndicator.displayName = 'TaskRunningIndicator';

export default TaskRunningIndicator;
