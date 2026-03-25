/**
 * Common tool card component
 * Provides unified card styles and interaction logic
 */

import React, { ReactNode } from 'react';
import './BaseToolCard.scss';

export interface BaseToolCardProps {
  /** Tool status */
  status: 'pending' | 'preparing' | 'streaming' | 'running' | 'completed' | 'error' | 'cancelled' | 'analyzing' | 'pending_confirmation' | 'confirmed';
  /** Whether expanded */
  isExpanded?: boolean;
  /** Card click callback */
  onClick?: (e: React.MouseEvent) => void;
  /** Custom class name */
  className?: string;
  /** Header content */
  header: ReactNode;
  /** Expanded content (optional) */
  expandedContent?: ReactNode;
  /** Error content (optional) */
  errorContent?: ReactNode;
  /** Whether to show error */
  isFailed?: boolean;
  /** Whether user confirmation is required (for highlighting border) */
  requiresConfirmation?: boolean;
}

/**
 * Base tool card component
 */
export const BaseToolCard: React.FC<BaseToolCardProps> = ({
  status,
  isExpanded = false,
  onClick,
  className = '',
  header,
  expandedContent,
  errorContent,
  isFailed = false,
  requiresConfirmation = false,
}) => {
  const hasExpandedContent = isExpanded && expandedContent && !isFailed;
  const showConfirmationHighlight = requiresConfirmation && 
    status !== 'completed' && 
    status !== 'confirmed' &&
    status !== 'cancelled' && 
    status !== 'error';
  
  return (
    <div className={`base-tool-card-wrapper ${showConfirmationHighlight ? 'requires-confirmation' : ''} ${className}`}>
      <div 
        className={`base-tool-card status-${status} ${isExpanded ? 'expanded' : ''}`}
        onClick={onClick}
      >
        <div className="base-tool-card-header">
          {header}
        </div>
      </div>
      
      {hasExpandedContent && (
        <div className="base-tool-card-expanded">
          {expandedContent}
        </div>
      )}
      
      {isFailed && errorContent && (
        <div className="base-tool-card-error">
          {errorContent}
        </div>
      )}
    </div>
  );
};

/**
 * Tool card header subcomponent Props
 */
export interface ToolCardHeaderProps {
  /** Left tool identifier icon (colored) */
  icon?: ReactNode;
  /** Custom class name for tool icon */
  iconClassName?: string;
  /** Action text */
  action?: string;
  /** Main content */
  content?: ReactNode;
  /** Right extra content (e.g., statistics, buttons, etc.) */
  extra?: ReactNode;
  /** Status icon at right border */
  statusIcon?: ReactNode;
}

/**
 * Tool card header component
 */
export const ToolCardHeader: React.FC<ToolCardHeaderProps> = ({
  icon,
  iconClassName,
  action,
  content,
  extra,
  statusIcon,
}) => {
  return (
    <>
      {icon && (
        <div className={`tool-card-icon tool-identifier-icon ${iconClassName || ''}`}>
          {icon}
        </div>
      )}
      {action && <span className="tool-card-action">{action}</span>}
      {content && <div className="tool-card-content">{content}</div>}
      {extra && <div className="tool-card-extra">{extra}</div>}
      {statusIcon && (
        <div className={`tool-card-status-icon ${extra ? 'tool-card-status-icon--with-divider' : ''}`}>
          {statusIcon}
        </div>
      )}
    </>
  );
};

