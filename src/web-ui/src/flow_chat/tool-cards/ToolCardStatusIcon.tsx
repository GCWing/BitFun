/**
 * Shared right status-icon component for tool card headers.
 * Provides consistent icon sizing (14px SVG) and optional left border divider.
 */
import React, { ReactNode } from 'react';

export interface ToolCardStatusIconProps {
  /** Status icon content (should be 14px lucide icon or CubeLoading) */
  icon: ReactNode;
  /** Show left border divider (default false) */
  withDivider?: boolean;
  /** Additional class name */
  className?: string;
}

export const ToolCardStatusIcon: React.FC<ToolCardStatusIconProps> = ({
  icon,
  withDivider = false,
  className,
}) => {
  return (
    <div
      className={`tool-card-status-icon${withDivider ? ' tool-card-status-icon--with-divider' : ''}${className ? ` ${className}` : ''}`}
    >
      {icon}
    </div>
  );
};
