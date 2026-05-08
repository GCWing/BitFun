import React from 'react';
import { Tooltip } from '../../component-library';
import './ToolCommandPreview.scss';

interface ToolCommandPreviewProps {
  command?: string | null;
  emptyText: React.ReactNode;
  as?: 'span' | 'code';
  className?: string;
  tooltipContent?: React.ReactNode;
  tooltipPlacement?: 'top' | 'bottom' | 'left' | 'right';
}

export const ToolCommandPreview = React.forwardRef<HTMLElement, ToolCommandPreviewProps>(({
  command,
  emptyText,
  as = 'span',
  className,
  tooltipContent,
  tooltipPlacement = 'bottom',
}, ref) => {
  const content = command?.trim()
    ? command
    : <span className="tool-command-preview__empty">{emptyText}</span>;
  const resolvedClassName = `tool-command-preview${className ? ` ${className}` : ''}`;
  const node = as === 'code' ? (
    <code ref={ref} className={resolvedClassName}>
      {content}
    </code>
  ) : (
    <span ref={ref} className={resolvedClassName}>
      {content}
    </span>
  );

  if (!tooltipContent) {
    return node;
  }

  return (
    <Tooltip
      content={<div className="tool-command-preview-tooltip-content">{tooltipContent}</div>}
      placement={tooltipPlacement}
      className="tool-command-preview-tooltip"
      interactive
    >
      {node}
    </Tooltip>
  );
});

ToolCommandPreview.displayName = 'ToolCommandPreview';
