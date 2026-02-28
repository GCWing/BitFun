/**
 * IconButton component
 * Optimized component for icon buttons
 */

import React from 'react';
import { Tooltip } from '../Tooltip/Tooltip';
import './IconButton.scss';

export interface IconButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'default' | 'primary' | 'ghost' | 'danger' | 'success' | 'warning' | 'ai';
  size?: 'xs' | 'small' | 'medium' | 'large';
  shape?: 'square' | 'circle';
  isLoading?: boolean;
  tooltip?: React.ReactNode;
  tooltipPlacement?: 'top' | 'bottom' | 'left' | 'right';
  /** When true, tooltip follows the mouse cursor (default: true for better UX on small targets). */
  tooltipFollowCursor?: boolean;
}

export const IconButton = React.forwardRef<HTMLButtonElement, IconButtonProps>(({
  children,
  variant = 'default',
  size = 'medium',
  shape = 'square',
  isLoading = false,
  tooltip,
  tooltipPlacement = 'top',
  tooltipFollowCursor = true,
  className = '',
  disabled,
  ...props
}, ref) => {
  const classNames = [
    'icon-btn',
    `icon-btn--${size}`,
    `icon-btn--${variant}`,
    shape === 'circle' && 'icon-btn--circle',
    isLoading && 'icon-btn--loading',
    className
  ].filter(Boolean).join(' ');

  const button = (
    <button
      ref={ref}
      className={classNames}
      disabled={disabled || isLoading}
      {...props}
    >
      {children}
    </button>
  );

  if (tooltip && !disabled) {
    return (
      <Tooltip content={tooltip} placement={tooltipPlacement} followCursor={tooltipFollowCursor}>
        {button}
      </Tooltip>
    );
  }

  return button;
});

IconButton.displayName = 'IconButton';
