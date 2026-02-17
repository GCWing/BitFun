/**
 * Button component
 */

import React, { forwardRef } from 'react';
import './Button.scss';

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'dashed' | 'danger' | 'success' | 'accent' | 'ai';
  size?: 'small' | 'medium' | 'large';
  isLoading?: boolean;
  iconOnly?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(({
  children,
  variant = 'primary',
  size = 'medium',
  isLoading = false,
  iconOnly = false,
  className = '',
  disabled,
  ...props
}, ref) => {
  const sizeClassMap = {
    small: 'sm',
    medium: 'base',
    large: 'lg'
  };

  const getVariantClass = (variant: string) => {
    switch (variant) {
      case 'primary':
      case 'accent':
        return 'btn-primary';
      case 'secondary':
        return 'btn-secondary';
      case 'ai':
        return 'btn-action btn-action-ai';
      case 'danger':
        return 'btn-action btn-action-danger';
      case 'success':
        return 'btn-action btn-action-success';
      case 'ghost':
        return 'btn-ghost';
      case 'dashed':
        return 'btn-dashed';
      default:
        return 'btn-secondary';
    }
  };

  const classNames = [
    'btn',
    getVariantClass(variant),
    `btn-${sizeClassMap[size] || 'base'}`,
    iconOnly && 'btn-icon-only',
    isLoading && 'btn-loading',
    disabled && 'btn-disabled',
    className
  ].filter(Boolean).join(' ');

  return (
    <button
      ref={ref}
      className={classNames}
      disabled={disabled || isLoading}
      {...props}
    >
      {isLoading ? (
        <>
          <span className="btn-loading-icon"></span>
          <span className="btn-loading-text">Loading...</span>
        </>
      ) : (
        children
      )}
    </button>
  );
});

Button.displayName = 'Button';