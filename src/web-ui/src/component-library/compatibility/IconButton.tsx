import React from 'react';
import { IconButton as DesignSystemIconButton } from '@bitfun/ui';
import { Tooltip } from '../components/Tooltip/Tooltip';

export interface IconButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'default' | 'primary' | 'ghost' | 'danger' | 'success' | 'warning' | 'ai';
  size?: 'xs' | 'small' | 'medium' | 'large';
  shape?: 'square' | 'circle';
  isLoading?: boolean;
  tooltip?: React.ReactNode;
  tooltipPlacement?: 'top' | 'bottom' | 'left' | 'right';
  tooltipFollowCursor?: boolean;
}

const sizeMap = {
  xs: 'xs',
  small: 'sm',
  medium: 'md',
  large: 'lg',
} as const;

/**
 * Compatibility adapter for the two remaining FlowChat call sites.
 * New product code must import IconButton directly from `@bitfun/ui`.
 */
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
  const tone = variant === 'danger' ? 'danger' : 'neutral';
  const designSystemVariant = variant === 'primary' || variant === 'success'
    ? 'primary'
    : variant === 'warning' || variant === 'ai'
      ? 'fill'
      : 'quiet';
  const button = (
    <DesignSystemIconButton
      {...props}
      ref={ref}
      aria-label={props['aria-label'] ?? ''}
      className={[
        'icon-btn',
        `icon-btn--${size}`,
        `icon-btn--${variant}`,
        shape === 'circle' && 'icon-btn--circle',
        className,
      ].filter(Boolean).join(' ')}
      disabled={disabled}
      icon={children}
      loading={isLoading}
      shape={shape}
      size={sizeMap[size]}
      tone={tone}
      variant={designSystemVariant}
    />
  );

  if (tooltip && !disabled && !isLoading) {
    return (
      <Tooltip content={tooltip} placement={tooltipPlacement} followCursor={tooltipFollowCursor}>
        {button}
      </Tooltip>
    );
  }

  return button;
});

IconButton.displayName = 'IconButton';
