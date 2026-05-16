/**
 * Compact three-dot pulse for tool "pending / parsing" states (replaces clock icon).
 */

import React from 'react';
import './ToolProcessingDots.scss';

export type ToolProcessingDotsSize = 10 | 12 | 14 | 16;

export interface ToolProcessingDotsProps {
  /** Visual scale aligned with common lucide-react icon sizes in tool headers */
  size?: ToolProcessingDotsSize;
  className?: string;
}

export const ToolProcessingDots: React.FC<ToolProcessingDotsProps> = ({
  size = 14,
  className = '',
}) => (
  <span
    className={`bitfun-tool-processing-dots bitfun-tool-processing-dots--s${size} ${className}`.trim()}
    aria-hidden
    role="presentation"
  >
    <span className="bitfun-tool-processing-dots__dot" />
    <span className="bitfun-tool-processing-dots__dot" />
    <span className="bitfun-tool-processing-dots__dot" />
  </span>
);
